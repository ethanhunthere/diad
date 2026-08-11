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
#   (d) workspace 2: DIAD Apps icon page — spawn once, verify, one retry,
#       NO respawn loop (RBRO-004 WP-04): the browser opens only on click
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
# PROMPT-2 S4.2 — healthy means the API v1 status probe answers (the real
# agent surface), not just the bare /health ping.
daemon_healthy() {
    curl -sf http://127.0.0.1:7860/api/v1/status >/dev/null 2>&1
}

# Number of real browser windows on any workspace, EXCLUDING the DIAD app
# windows (agent GUI, DIAD Apps icon page, DIAD Launchpad — all are also
# browser app-windows). Uses sway's window tree when available; falls back
# to a process count pre-sway. (RBRO-004 WP-04: exclusion list updated.)
browser_window_count() {
    if command -v swaymsg >/dev/null 2>&1 && swaymsg -t get_tree >/dev/null 2>&1; then
        swaymsg -t get_tree 2>/dev/null \
            | jq -r '.. | objects | select(.type? == "con" and (.app_id? // "" | test("brave|chromium"))) | (.name? // "")' 2>/dev/null \
            | grep -cvE '^(DIAD AGENT|DIAD Apps|DIAD Launchpad)$' || true
    else
        pgrep -fc 'brave|chromium' 2>/dev/null || echo 0
    fi
}

# The DIAD Apps icon page (workspace 2). Matches the document title AND the
# map-time URL title, whichever sway currently reports (RBRO-004 WP-04).
apps_page_count() {
    if command -v swaymsg >/dev/null 2>&1 && swaymsg -t get_tree >/dev/null 2>&1; then
        swaymsg -t get_tree 2>/dev/null \
            | jq -r '.. | objects | select(.type? == "con" and ((.name? // "") | test("DIAD Apps|7860/apps"))) | .name' 2>/dev/null \
            | wc -l
    else
        pgrep -fc -- '--app=http://127.0.0.1:7860/apps' 2>/dev/null || echo 0
    fi
}

launchpad_window_count() {
    if command -v swaymsg >/dev/null 2>&1 && swaymsg -t get_tree >/dev/null 2>&1; then
        swaymsg -t get_tree 2>/dev/null \
            | jq -r '.. | objects | select(.type? == "con" and ((.name? // "") | test("DIAD Launchpad|7860/launch"))) | .name' 2>/dev/null \
            | wc -l
    else
        pgrep -fc -- '--app=http://127.0.0.1:7860/launch' 2>/dev/null || echo 0
    fi
}

# RBRO-004 WP-05 — both chat apps open? The launchpad should then vanish.
# Uses the daemon's own sway-tree parse (open[]), so this works even before
# the local sway IPC answers. 0/1 exit for if/then.
launchpad_should_close() {
    curl -sf http://127.0.0.1:7860/api/v1/launch/status 2>/dev/null \
        | jq -e '(.open | index("telegram")) and (.open | index("slack"))' >/dev/null 2>&1
}

# Enforcement backstop for the launchpad's own window.close(): when both
# chats are open and the page is somehow still mapped, close it from sway.
enforce_launchpad_close() {
    if launchpad_should_close \
        && [[ $(count_or_unknown launchpad_window_count) != "0" ]]; then
        swaymsg '[title="DIAD Launchpad"] kill' >/dev/null 2>&1
        log "supervisor: both chats open — launchpad closed"
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

# Fail-safe count wrapper: when sway IPC is unavailable the window count is
# UNKNOWN — and unknown is treated as "window present", so the supervisor
# NEVER auto-spawns a duplicate (a boot can only lose a surface, never
# flood). The sway-ready wait below keeps this path exceptional.
count_or_unknown() {
    if command -v swaymsg >/dev/null 2>&1 && swaymsg -t get_tree >/dev/null 2>&1; then
        "$@"
    else
        echo "unknown"
    fi
}

# Wait up to 5 s for sway IPC to answer the window tree. Every spawn /
# count decision below runs only after this, so window counts are real.
sway_ready=0
for _ in $(seq 1 10); do
    if command -v swaymsg >/dev/null 2>&1 && swaymsg -t get_tree >/dev/null 2>&1; then
        sway_ready=1
        break
    fi
    sleep 0.5
done
if [[ ${sway_ready} -eq 0 ]]; then
    log "supervisor: sway IPC unavailable after 5 s — assuming windows present (no auto-spawn)"
fi

# One-shot diagnostics window (RBRO-004 WP-02/WP-07). Opened at most once
# per session, marker-gated, and only after the daemon restart limit is
# reached. Shows the daemon log tail — the single allowed error surface.
show_diagnostics() {
    if [[ -f "${DIAD_HOME}/.diagnostics-shown" ]]; then
        log "supervisor: diagnostics already shown — skipping (never auto-reopened)"
        return
    fi
    touch "${DIAD_HOME}/.diagnostics-shown"
    foot --app-id=foot --title "DIAD diagnostics" \
        -e sh -c 'echo "DIAD diagnostics — daemon.log tail:"; tail -n 20 "${HOME}/.diad/daemon.log" 2>/dev/null; echo; echo "Press Enter to close."; read _' \
        >/dev/null 2>&1 &
    log "supervisor: diagnostics window opened (once per session)"
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
# RBRO-004 WP-03 — spawn-once policy. The GUI is spawned ONLY when the
# daemon health check passed (FALLBACK=0). One spawn + one retry (3 s
# verify each); if the window still does not map, the TUI takes over once.
# Any count of "unknown" (sway IPC down) is treated as "window present" —
# the supervisor never spawns a duplicate it cannot see. The GUI never
# runs together with the TUI; the user can re-open via Super+Shift+A
# (sway), the open_agent tool, or TUI F9.
launch_agent_tui() {
    foot --app-id=foot --title "DIAD AGENT" -e diad-agent --tui >/dev/null 2>&1 &
    log "supervisor: TUI fallback opened on workspace 1"
}
launch_agent_gui() {
    brave --app=http://127.0.0.1:7860 >/dev/null 2>&1 &
}

agent_cnt=$(count_or_unknown agent_window_count)
if [[ ${agent_cnt} != "0" ]]; then
    log "supervisor: agent window already present (count=${agent_cnt}) — skipping WS1 launch"
elif [[ ${FALLBACK} -eq 1 ]]; then
    launch_agent_tui
else
    launch_agent_gui
    sleep 3
    if [[ $(count_or_unknown agent_window_count) != "0" ]]; then
        log "supervisor: DIAD AGENT GUI verified on workspace 1"
    else
        log "supervisor: GUI window not mapped after 3 s — retrying once"
        launch_agent_gui
        sleep 3
        if [[ $(count_or_unknown agent_window_count) != "0" ]]; then
            log "supervisor: DIAD AGENT GUI verified on workspace 1 (retry)"
        else
            log "supervisor: GUI failed twice — engaging TUI fallback (reason in daemon.log)"
            # PROMPT-2 S4.3 — persist the exact reason for the next GUI start
            # (read + cleared via GET/DELETE /api/v1/gui/fallback).
            reason="GUI failed twice: Brave flatpak + chromium shim could not open a window for http://127.0.0.1:7860 — TUI fallback engaged"
            printf '%s\n' "${reason}" > "${DIAD_HOME}/.gui-fallback-reason"
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

# ---- (d) workspace 2: DIAD Apps icon page (RBRO-004 WP-04) ----
# EXACTLY ONE icon page at boot: spawn, verify, one retry — NO respawn
# loop. The page is a launcher; the browser opens ONLY on click
# (POST /api/v1/launch). This kills the old WS2 flood: the browser is no
# longer auto-spawned every 5 s while a window "was missing".
if daemon_healthy; then
    spawn_apps_page() {
        brave --app=http://127.0.0.1:7860/apps >/dev/null 2>&1 &
    }
    spawn_apps_page
    sleep 3
    if [[ $(count_or_unknown apps_page_count) != "0" ]]; then
        log "supervisor: DIAD Apps icon page verified on workspace 2"
    else
        spawn_apps_page
        sleep 3
        if [[ $(count_or_unknown apps_page_count) != "0" ]]; then
            log "supervisor: DIAD Apps icon page verified on workspace 2 (retry)"
        else
            log "supervisor: DIAD Apps icon page did not map — browser opens on click only"
        fi
    fi
else
    log "supervisor: daemon down — DIAD Apps icon page skipped"
fi

# ---- (e) workspace 3: DIAD Launchpad (RBRO-004 WP-05) ----
# EXACTLY ONE launchpad at boot: spawn, verify, one retry — NO respawn
# loop. The page opens Telegram/Slack on click (POST /api/v1/launch) and
# closes itself once BOTH chats are open. The steady-state loop below is
# the enforcement backstop (window.close() can be blocked by the browser).
if daemon_healthy; then
    spawn_launchpad() {
        brave --app=http://127.0.0.1:7860/launch >/dev/null 2>&1 &
    }
    spawn_launchpad
    sleep 3
    if [[ $(count_or_unknown launchpad_window_count) != "0" ]]; then
        log "supervisor: DIAD Launchpad verified on workspace 3"
    else
        spawn_launchpad
        sleep 3
        if [[ $(count_or_unknown launchpad_window_count) != "0" ]]; then
            log "supervisor: DIAD Launchpad verified on workspace 3 (retry)"
        else
            log "supervisor: DIAD Launchpad did not map — chat opens on click only"
        fi
    fi
else
    log "supervisor: daemon down — DIAD Launchpad skipped"
fi

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
    # RBRO-004 WP-05 — launchpad self-closes when both chats open; this is
    # the backstop in case the page's window.close() was blocked.
    if daemon_healthy; then
        enforce_launchpad_close
    fi
    # Daemon health: restart max 2 times, then hand workspace 1 to the TUI.
    # A 30 s grace window after each restart stops the 10 s poll from
    # counting a slow start-up as another failure.
    # RBRO-004 WP-02 crash guard: the TUI fallback is opened only if no
    # agent window exists yet (never a second one), and the ONE allowed
    # error surface (a diagnostics foot, WP-07) appears exactly once.
    if [[ ${FALLBACK} -eq 0 ]] && ! daemon_healthy; then
        if [[ ${DAEMON_RESTARTS} -lt 2 ]]; then
            DAEMON_RESTARTS=$((DAEMON_RESTARTS + 1))
            DAEMON_GRACE_UNTIL=$(( $(date +%s) + 30 ))
            log "supervisor: daemon unhealthy — restart ${DAEMON_RESTARTS}/2"
            nohup diad-agent --daemon >> "${DIAD_HOME}/daemon.log" 2>&1 &
        elif [[ $(date +%s) -ge ${DAEMON_GRACE_UNTIL} ]]; then
            log "supervisor: daemon restart limit reached — no further retries"
            show_diagnostics
            if [[ $(count_or_unknown agent_window_count) -le 0 ]]; then
                log "supervisor: opening TUI fallback (only agent surface)"
                foot --app-id=foot --title "DIAD AGENT" -e diad-agent --tui >/dev/null 2>&1 &
            fi
            FALLBACK=1
        fi
    fi
    cycles=$((cycles + 1))
    if [[ $((cycles % 10)) -eq 0 ]]; then
        rotate_logs
    fi
    sleep 10
done
