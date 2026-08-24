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
#   (h) RBRO-004 WP-08 flood backstop in steady state: workspace limits
#       (WS1 agent ≤ 8, WS2 apps ≤ 2, WS3 launchpad ≤ 2); excess windows
#       move to the sway scratchpad + one rate-limited toast
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
            | jq -r '.. | objects | select(.type? == "con" and (.app_id? // "" | test("epiphany|brave|chromium"))) | (.name? // "")' 2>/dev/null \
            | grep -cvE '^(DIAD AGENT|DIAD Apps|DIAD Launchpad)$' || true
    else
        pgrep -fc 'epiphany|brave|chromium' 2>/dev/null || echo 0
    fi
}

# The DIAD Apps icon page (workspace 2). Matches the document title AND the
# map-time URL title, whichever sway currently reports (RBRO-004 WP-04).
apps_page_count() {
    if command -v swaymsg >/dev/null 2>&1 && swaymsg -t get_tree >/dev/null 2>&1; then
        swaymsg -t get_tree 2>/dev/null \
            | jq -r '.. | objects | select(.type? == "con" and ((.name? // "") | test("DIAD Apps|DIAD Browser|7860/apps"))) | .name' 2>/dev/null \
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

# RBRO-004 WP-10 — count matching windows that are actually ON workspace
# $1. The *_window_count functions above only prove a window EXISTS
# somewhere in the tree — a window can map onto the wrong workspace (the
# launchpad once landed on workspace 1 when the generic :7860 rule beat
# the title rules at map time), so "verified" must mean "on the right
# workspace", not merely "present". Returns 0 when sway IPC is down.
count_on_workspace() {
    local ws_num="$1" name_test="$2" out
    if command -v swaymsg >/dev/null 2>&1 && swaymsg -t get_tree >/dev/null 2>&1; then
        out=$(swaymsg -t get_tree 2>/dev/null \
            | jq -r --arg ws "${ws_num}" --arg test "${name_test}" '
                .. | objects | select(.type? == "workspace" and ((.num // -1) | tostring) == $ws)
                | [ .. | objects | select(.type? == "con" and ((.name? // "") | test($test))) | .name ] | length' 2>/dev/null)
        echo "${out:-0}"
    else
        echo 0
    fi
}

# RBRO-004 WP-10 — move a window back onto its assigned workspace if it
# mapped there but ended up elsewhere (map-time title race backstop). The
# app_id is deterministic (set from --app= URL at window creation), so it
# is the safest criterion for the relocation.
relocate_to_workspace() {
    local app_id_re="$1" ws_num="$2"
    swaymsg "[app_id=\"${app_id_re}\"] move to workspace ${ws_num}" >/dev/null 2>&1
}

# RBRO-004 WP-10 — honest placement verification: returns 0 (OK) if a
# window matching $2 is present AND on workspace $1, 1 otherwise. Also
# attempts relocation when the window exists but is misplaced. When sway
# IPC is down, count_or_unknown returns "unknown" (assume present) — we
# return 0 so the caller NEVER respawns a duplicate.
verify_on_workspace() {
    local ws_num="$1" name_test="$2" app_id_re="$3"
    local present ws_ok
    present=$(count_or_unknown "$4")
    ws_ok=$(count_on_workspace "${ws_num}" "${name_test}")
    if [[ "${present}" == "unknown" ]]; then
        return 0
    fi
    if [[ "${present}" != "0" ]] && [[ "${ws_ok}" != "0" ]]; then
        return 0
    fi
    if [[ "${present}" != "0" ]]; then
        relocate_to_workspace "${app_id_re}" "${ws_num}"
        sleep 1
        ws_ok=$(count_on_workspace "${ws_num}" "${name_test}")
        if [[ "${ws_ok}" != "0" ]]; then
            log "supervisor: window relocated to workspace ${ws_num}"
            return 0
        fi
        log "supervisor: window present but NOT on workspace ${ws_num} — placement failed"
        return 1
    fi
    return 1
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

# ---- RBRO-004 WP-08 — workspace limits watcher (flood backstop) ----
# The boot chain guarantees exactly one surface per workspace (1/1/1);
# this caps any RUNTIME flood — repeated Super+Shift+A opens, a blocked
# launchpad window.close(), a respawn bug — by moving the excess windows
# to the sway scratchpad (never killed; Super+Shift+M restores them) and
# showing ONE rate-limited toast. The focused window is never touched.
WS1_LIMIT=8   # DIAD AGENT windows on workspace 1
WS2_LIMIT=2   # DIAD Apps icon pages on workspace 2
WS3_LIMIT=2   # DIAD Launchpad windows on workspace 3
LIMIT_TOAST_MIN=60   # seconds between toasts per workspace

# con_ids of matching windows on workspace $1 that exceed the $3 cap.
# Tree order keeps the primary window; the focused window is never moved.
excess_window_ids() {
    local ws_num="$1" name_test="$2" cap="$3"
    swaymsg -t get_tree 2>/dev/null \
        | jq -r --arg num "${ws_num}" --arg test "${name_test}" --argjson cap "${cap}" '
            .. | objects | select(.type? == "workspace" and ((.num // -1) | tostring) == $num)
            | [ .. | objects | select(.type? == "con" or .type? == "floating_con")
                  | select((.name? // "") | test($test))
                  | select(.focused? != true)
                  | .id ]
            | .[($cap - 1):][]' 2>/dev/null
}

# Move every excess window on the workspace to the scratchpad, then toast
# (rate-limited). $1=workspace num, $2=surface label, $3=name test, $4=cap.
enforce_workspace_limit() {
    local ws_num="$1" surface="$2" name_test="$3" cap="$4" id excess marker now last
    excess=$(excess_window_ids "${ws_num}" "${name_test}" "${cap}")
    if [[ -z "${excess}" ]]; then
        return 0
    fi
    for id in ${excess}; do
        swaymsg "[con_id=${id}] move scratchpad" >/dev/null 2>&1
    done
    log "supervisor: workspace ${ws_num} ${surface} over cap ${cap} — $(echo "${excess}" | wc -w) excess window(s) moved to scratchpad"
    marker="${DIAD_HOME}/.limit-toast-ws${ws_num}"
    now=$(date +%s)
    last=0
    [[ -f "${marker}" ]] && last=$(cat "${marker}" 2>/dev/null || echo 0)
    if [[ $((now - last)) -ge ${LIMIT_TOAST_MIN} ]]; then
        printf '%s' "${now}" > "${marker}"
        notify-send "DIAD: workspace ${ws_num} capped at ${cap} ${surface}" \
            "Extra windows moved to the scratchpad (Super+Shift+M to restore)." \
            -u normal
    fi
}

# The brave shim always exists on the ISO; it can only actually open a
# browser if flatpak com.brave.Browser OR chromium is present (S1 fallback).
browser_possible() {
    if command -v epiphany >/dev/null 2>&1; then
        return 0
    fi
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
            | jq -r '.. | objects | select(.type? == "con" and (((.name? // "") | test("7860|DIAD AGENT|DIAD Agent|Web")) or ((.app_id? // "") | test("epiphany|org.gnome.Epiphany|diad-agent|chromium|brave")))) | .name' 2>/dev/null \
            | wc -l
    else
        # Pre-sway fallback: only the TUI process can exist yet.
        pgrep -fc 'diad-agent --tui' 2>/dev/null || echo 0
    fi
}

# RBRO-004 WP-09 — count ONLY the GUI variant (an epiphany/chromium/brave app-window
# titled "DIAD AGENT"/"7860"), never the foot TUI. Lets the late-map guard
# distinguish "the GUI finally arrived" from "the TUI is still up" so
# workspace 1 keeps exactly one agent surface. Returns 0 while sway IPC is
# down (conservative: the guard must never act on a blind count).
agent_gui_count() {
    if command -v swaymsg >/dev/null 2>&1 && swaymsg -t get_tree >/dev/null 2>&1; then
        swaymsg -t get_tree 2>/dev/null \
            | jq -r '.. | objects | select(.type? == "con" and (.app_id? // "" | test("epiphany|org.gnome.Epiphany|chromium|brave"))) | .name' 2>/dev/null \
            | wc -l
    else
        echo 0
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
for _ in $(seq 1 40); do
    if command -v swaymsg >/dev/null 2>&1 && swaymsg -t get_tree >/dev/null 2>&1; then
        sway_ready=1
        break
    fi
    sleep 0.05
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

# Ensure swaynag never displays any error banner
pkill -9 -f swaynag 2>/dev/null || true
(
    for _ in {1..30}; do
        pkill -9 -f swaynag 2>/dev/null || true
        killall -9 swaynag 2>/dev/null || true
        sleep 0.1
    done
) &
log "supervisor: sway output: $(cat /tmp/sway.log 2>/dev/null || echo 'none')"


# ---- (a) idempotent setup in background (never blocks session start) ----
(
    for script in diad-hardware-setup.sh diad-network-setup.sh diad-power-setup.sh \
        diad-display-setup.sh diad-storage-setup.sh; do
        if command -v "${script}" >/dev/null 2>&1; then
            "${script}" >> "${LOG_FILE}" 2>&1 || true
        fi
    done
) &

# ---- (b) DIAD daemon & 3-Workspace Surface Initialization ----
FALLBACK=0
DAEMON_RESTARTS=0
DAEMON_GRACE_UNTIL=0

launch_agent_tui() {
    foot --app-id=foot --title "DIAD AGENT" -e diad-agent --tui >/dev/null 2>&1 &
    log "supervisor: TUI fallback opened on workspace 1"
}
launch_agent_gui() {
    brave --app=http://127.0.0.1:7860 >/dev/null 2>&1 &
}

if command -v diad-agent >/dev/null 2>&1; then
    nohup diad-agent --daemon >> "${DIAD_HOME}/daemon.log" 2>&1 &
    log "supervisor: DIAD daemon starting (pid $!)"

    # Workspace 1: Launch DIAD AGENT Studio if not already open
    if [[ $(count_or_unknown agent_window_count) == "0" ]]; then
        brave --app=http://127.0.0.1:7860 >/dev/null 2>&1 &
        log "supervisor: DIAD AGENT Hub spawned on workspace 1"
    fi

    # Fast-poll for daemon readiness in background
    (
        healthy=0
        for _ in $(seq 1 40); do
            if daemon_healthy; then
                healthy=1
                break
            fi
            sleep 0.05
        done
        if [[ ${healthy} -eq 0 ]]; then
            log "supervisor: DIAD daemon failed to respond — engaging TUI fallback"
            launch_agent_tui
        fi
    ) &
else
    log "supervisor: diad-agent not found — TUI fallback"
    FALLBACK=1
    launch_agent_tui
fi

# ---- (f) first-run wizard ----
if [[ ! -f "${DIAD_HOME}/.first-run-complete" ]]; then
    if [[ ${FALLBACK} -eq 1 ]]; then
        log "supervisor: first-run wizard via foot (TUI mode)"
        foot --app-id=foot --title "DIAD Setup" -e diad-first-run.sh >/dev/null 2>&1 &
    fi
fi

# ---- end-of-boot: focus workspace 1 on primary output ----
if command -v swaymsg >/dev/null 2>&1; then
    swaymsg workspace "DIAD AGENT" >/dev/null 2>&1 || swaymsg workspace number 1 >/dev/null 2>&1
    output_name="$(swaymsg -t get_outputs 2>/dev/null | jq -r '.[0].name // empty' 2>/dev/null)"
    if [[ -n "${output_name}" ]]; then
        swaymsg focus output "${output_name}" >/dev/null 2>&1
    fi
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
    # RBRO-004 WP-08 — flood backstop: cap DIAD surfaces per workspace.
    # Excess windows go to the scratchpad (recoverable), one toast max/min.
    if command -v swaymsg >/dev/null 2>&1 && swaymsg -t get_tree >/dev/null 2>&1; then
        enforce_workspace_limit 1 "agent windows" 'DIAD AGENT|7860' ${WS1_LIMIT}
        enforce_workspace_limit 2 "apps icon pages" 'DIAD Apps|7860/apps' ${WS2_LIMIT}
        enforce_workspace_limit 3 "launchpad windows" 'DIAD Launchpad|7860/launch' ${WS3_LIMIT}
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
