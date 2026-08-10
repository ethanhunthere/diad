#!/usr/bin/env bash
#
# DIAD — Session supervisor  (MASTER FIX S4 — resilient boot chain)
# =============================================================================
# Entry point launched by Sway on login (exactly once, `exec diad-supervisor.sh`
# in /etc/sway/config — sway itself no longer starts waybar; this script owns
# it). Ordered, non-blocking boot:
#
#   (a) idempotent storage/display/network/power setup (marker-gated inside)
#   (b) DIAD daemon — wait for 127.0.0.1:7860/health up to 10 s (20 x 0.5 s);
#       on timeout log the daemon tail and set FALLBACK=1
#   (c) workspace 1: daemon healthy → DIAD AGENT GUI (brave --app); otherwise
#       foot diad-agent --tui (guaranteed surface either way)
#   (d) workspace 2: browser respawn loop (5 s, quiet) — the brave shim
#       resolves flatpak com.brave.Browser → chromium → notify-send
#   (e) workspace 3: DIAD Launchpad respawn loop (5 s, quiet), only while the
#       daemon serves it
#   (f) first-run wizard (independent window — never blocks the boot)
#   (g) steady state: waybar/mako/cliphist watchdogs (10 s) + daemon health
#       (restart max 2, then TUI fallback) + periodic log rotation
#
# Every step degrades without blocking the desktop (`set -uo pipefail`).
# =============================================================================

set -uo pipefail

HOME_DIR="${HOME:-/home/diad}"
DIAD_HOME="${HOME_DIR}/.diad"
LOG_DIR="${DIAD_HOME}/logs"
LOG_FILE="${LOG_DIR}/boot.log"

# ---- Environment ----
export WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}"
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"

mkdir -p "${LOG_DIR}" "${DIAD_HOME}/sessions"

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" >> "${LOG_FILE}"
}

# ---- Log rotation (F-13) ----
# Size-capped: any ~/.diad/logs/*.log (and the daemon log at ~/.diad/
# daemon.log, which sits outside the logs dir) over 1 MiB is copied to .1
# (one backup kept) and the live file truncated. copytruncate (not mv) so
# the daemon's open fd keeps writing to the live file. The live ISO runs
# from RAM — without this, logs grow forever and OOM the session.
rotate_logs() {
    local max_bytes log size
    max_bytes=1048576  # 1 MiB
    for log in "${LOG_DIR}"/*.log "${DIAD_HOME}/daemon.log"; do
        [[ -f "${log}" ]] || continue
        size=$(stat -c %s "${log}" 2>/dev/null || echo 0)
        if [[ ${size} -gt ${max_bytes} ]]; then
            cp -f "${log}" "${log}.1" 2>/dev/null || continue
            : > "${log}" 2>/dev/null || true
            log "supervisor: rotated ${log##*/} (${size} bytes -> .1)"
        fi
    done
}

# ---- Health / window-count helpers ----
daemon_healthy() {
    curl -sf http://127.0.0.1:7860/health >/dev/null 2>&1
}

# Number of real browser windows on any workspace, EXCLUDING the DIAD AGENT
# GUI and the DIAD Launchpad (both are also browser app-windows). Uses sway's
# window tree when available; falls back to a process count pre-sway.
browser_window_count() {
    if command -v swaymsg >/dev/null 2>&1 && swaymsg -t get_tree >/dev/null 2>&1; then
        swaymsg -t get_tree 2>/dev/null \
            | jq -r '.. | objects | select(.type? == "con" and (.app_id? // "" | test("brave|chromium"))) | (.name? // "")' 2>/dev/null \
            | grep -cvE '^(DIAD AGENT|DIAD Launchpad)$' || true
    else
        pgrep -fc 'brave|chromium' 2>/dev/null || echo 0
    fi
}

launchpad_window_count() {
    if command -v swaymsg >/dev/null 2>&1 && swaymsg -t get_tree >/dev/null 2>&1; then
        swaymsg -t get_tree 2>/dev/null \
            | jq -r '.. | objects | select(.type? == "con" and (.name? // "" == "DIAD Launchpad")) | .name' 2>/dev/null \
            | wc -l
    else
        pgrep -fc -- '--app=http://127.0.0.1:7860/launch' 2>/dev/null || echo 0
    fi
}

# The brave shim always exists on the ISO; it can only actually open a
# browser if flatpak com.brave.Browser OR chromium is present (S1 fallback).
browser_possible() {
    if command -v flatpak >/dev/null 2>&1 \
        && flatpak list --app --columns=application 2>/dev/null | grep -qx 'com.brave.Browser'; then
        return 0
    fi
    command -v chromium >/dev/null 2>&1
}

# Number of DIAD AGENT windows (GUI app-window OR TUI foot) anywhere. The GUI
# is a browser app-window whose title is "DIAD AGENT"; the TUI foot window
# carries the same --title. The 7860 alternative covers any future window
# whose name exposes the daemon URL.
agent_window_count() {
    if command -v swaymsg >/dev/null 2>&1 && swaymsg -t get_tree >/dev/null 2>&1; then
        swaymsg -t get_tree 2>/dev/null \
            | jq -r '.. | objects | select(.type? == "con" and (.name? // "" | test("7860|DIAD AGENT"))) | .name' 2>/dev/null \
            | wc -l
    else
        # Pre-sway fallback: only the TUI process can exist yet.
        pgrep -fc 'diad-agent --tui' 2>/dev/null || echo 0
    fi
}

# =============================================================================
log "supervisor: session start"
rotate_logs

# ---- (a) idempotent setup — run BEFORE the wizard so NetworkManager /
#      power-profiles-daemon / displays are ready ----
for script in diad-hardware-setup.sh diad-network-setup.sh diad-power-setup.sh \
    diad-display-setup.sh diad-storage-setup.sh; do
    if command -v "${script}" >/dev/null 2>&1; then
        "${script}" >> "${LOG_FILE}" 2>&1 || log "supervisor: ${script} failed (non-fatal)"
    else
        log "supervisor: ${script} not found"
    fi
done

# ---- (b) DIAD daemon ----
FALLBACK=0
DAEMON_RESTARTS=0
DAEMON_GRACE_UNTIL=0
if command -v diad-agent >/dev/null 2>&1; then
    nohup diad-agent --daemon >> "${DIAD_HOME}/daemon.log" 2>&1 &
    log "supervisor: DIAD daemon starting (pid $!)"
    healthy=0
    for _ in $(seq 1 20); do            # max 10 s (20 x 0.5 s)
        if daemon_healthy; then
            healthy=1
            break
        fi
        sleep 0.5
    done
    if [[ ${healthy} -eq 1 ]]; then
        log "supervisor: DIAD daemon healthy on 127.0.0.1:7860"
    else
        log "supervisor: DIAD daemon not healthy after 10 s — TUI fallback"
        tail -n 5 "${DIAD_HOME}/daemon.log" >> "${LOG_FILE}" 2>&1 || true
        FALLBACK=1
    fi
else
    log "supervisor: diad-agent not found — TUI fallback"
    FALLBACK=1
fi

# ---- (c) workspace 1: EXACTLY ONE DIAD AGENT window, focused ----
# PROMPT-2 S1.1 — never launch GUI and TUI together; WS1 agent windows == 1
# always. If an agent window already exists, do nothing (no duplicates).
# Daemon healthy → GUI via the shim (brave --app=…:7860), verify the window
# maps within 3 s, retry ONCE, then TUI fallback with the reason logged.
# Daemon down → foot diad-agent --tui directly.
launch_agent_tui() {
    foot --app-id=foot --title "DIAD AGENT" -e diad-agent --tui >/dev/null 2>&1 &
    log "supervisor: TUI fallback opened on workspace 1"
}
launch_agent_gui() {
    brave --app=http://127.0.0.1:7860 >/dev/null 2>&1 &
}

if [[ $(agent_window_count) -gt 0 ]]; then
    log "supervisor: agent window already present — skipping WS1 launch (no duplicates)"
elif [[ ${FALLBACK} -eq 1 ]]; then
    launch_agent_tui
else
    launch_agent_gui
    sleep 3
    if [[ $(agent_window_count) -gt 0 ]]; then
        log "supervisor: DIAD AGENT GUI verified on workspace 1"
    else
        log "supervisor: GUI window not mapped after 3 s — retrying once"
        launch_agent_gui
        sleep 3
        if [[ $(agent_window_count) -gt 0 ]]; then
            log "supervisor: DIAD AGENT GUI verified on workspace 1 (retry)"
        else
            log "supervisor: GUI failed twice — engaging TUI fallback (reason in daemon.log)"
            tail -n 10 "${DIAD_HOME}/daemon.log" >> "${LOG_FILE}" 2>&1 || true
            launch_agent_tui
        fi
    fi
fi

# ---- (f) first-run wizard — GUI mode: onboarding is a GUI modal (existing
#      browser wizard, localStorage-driven); TUI mode only: foot wizard.
#      Never both (PROMPT-2 S1.2). ----
if [[ ! -f "${DIAD_HOME}/.first-run-complete" ]]; then
    if [[ ${FALLBACK} -eq 1 ]]; then
        log "supervisor: first-run wizard via foot (TUI mode)"
        foot --app-id=foot --title "DIAD Setup" -e diad-first-run.sh >/dev/null 2>&1 &
    else
        log "supervisor: first-run onboarding runs in the GUI — no foot wizard"
    fi
fi

# ---- end-of-boot: focus workspace 1 on the primary output (PROMPT-2 S1.3) —-
#      the agent window is focused and visible WITHOUT any user click.
if command -v swaymsg >/dev/null 2>&1; then
    swaymsg workspace 1 >/dev/null 2>&1
    output_name="$(swaymsg -t get_outputs 2>/dev/null | jq -r '.[0].name // empty' 2>/dev/null)"
    if [[ -n "${output_name}" ]]; then
        swaymsg focus output "${output_name}" >/dev/null 2>&1
        log "supervisor: focus → workspace 1 on output ${output_name}"
    else
        log "supervisor: focus → workspace 1 (output query failed)"
    fi
fi

# ---- (d) workspace 2: browser respawn loop (5 s, quiet) ----
if browser_possible; then
    (
        brave --new-window >/dev/null 2>&1 &
        while true; do
            if [[ $(browser_window_count) -eq 0 ]]; then
                brave --new-window >/dev/null 2>&1 &
            fi
            sleep 5
        done
    ) >> "${LOG_FILE}" 2>&1 &
    log "supervisor: browser respawn loop started (workspace 2)"
else
    log "supervisor: no browser available (flatpak brave not installed, chromium missing)"
fi

# ---- (e) workspace 3: DIAD Launchpad respawn loop (5 s, quiet) ----
(
    while true; do
        if daemon_healthy && [[ $(launchpad_window_count) -eq 0 ]]; then
            brave --app=http://127.0.0.1:7860/launch >/dev/null 2>&1 &
        fi
        sleep 5
    done
) >> "${LOG_FILE}" 2>&1 &
log "supervisor: launchpad respawn loop started (workspace 3)"

# ---- (g) steady state: waybar/mako/cliphist watchdogs + daemon health
#          (restart max 2, then TUI fallback) + periodic log rotation ----
cycles=0
while true; do
    if ! pgrep -x waybar >/dev/null 2>&1; then
        log "supervisor: waybar not running — relaunching"
        waybar -c /etc/waybar/config -s /etc/waybar/style.css >/dev/null 2>&1 &
    fi
    if ! pgrep -x mako >/dev/null 2>&1; then
        log "supervisor: mako not running — relaunching"
        mako >/dev/null 2>&1 &
    fi
    # cliphist is populated by a persistent `wl-paste --watch` process.
    if ! pgrep -f "wl-paste --watch" >/dev/null 2>&1; then
        log "supervisor: cliphist watcher not running — relaunching"
        wl-paste --watch cliphist store >/dev/null 2>&1 &
    fi
    # Daemon health: restart max 2 times, then hand workspace 1 to the TUI.
    # A 30 s grace window after each restart stops the 10 s poll from
    # counting a slow start-up as another failure.
    if [[ ${FALLBACK} -eq 0 ]] && ! daemon_healthy; then
        if [[ ${DAEMON_RESTARTS} -lt 2 ]]; then
            DAEMON_RESTARTS=$((DAEMON_RESTARTS + 1))
            DAEMON_GRACE_UNTIL=$(( $(date +%s) + 30 ))
            log "supervisor: daemon unhealthy — restart ${DAEMON_RESTARTS}/2"
            nohup diad-agent --daemon >> "${DIAD_HOME}/daemon.log" 2>&1 &
        elif [[ $(date +%s) -ge ${DAEMON_GRACE_UNTIL} ]]; then
            log "supervisor: daemon restart limit reached — TUI fallback"
            foot --app-id=foot --title "DIAD AGENT" -e diad-agent --tui >/dev/null 2>&1 &
            FALLBACK=1
        fi
    fi
    cycles=$((cycles + 1))
    if [[ $((cycles % 10)) -eq 0 ]]; then
        rotate_logs
    fi
    sleep 10
done
