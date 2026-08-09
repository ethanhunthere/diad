#!/usr/bin/env bash
#
# DIAD — Session supervisor
# =============================================================================
# Entry point launched by Sway on login. Runs the onboarding wizard and all
# setup scripts, then launches the two DIAD surfaces:
#   • workspace 1: DIAD AGENT (daemon + Brave app window on 127.0.0.1:7860)
#   • workspace 2: Brave browser
#
# A failing setup step never blocks the desktop (no `set -e`). The first-run
# wizard opens in its OWN foot window and never blocks the rest of the session.
#
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

log "supervisor: session start"

# ---- First-boot hardware/network/power setup (marker-gated inside) ----
# Run BEFORE the wizard so NetworkManager/power-profiles-daemon are up when
# the wizard asks about Wi-Fi or Brave.
for script in diad-hardware-setup.sh diad-network-setup.sh diad-power-setup.sh; do
    if command -v "${script}" >/dev/null 2>&1; then
        "${script}" >> "${LOG_FILE}" 2>&1 || log "supervisor: ${script} failed (non-fatal)"
    else
        log "supervisor: ${script} not found"
    fi
done

# ---- First-run onboarding (independent window — never blocks boot) ----
if [[ ! -f "${DIAD_HOME}/.first-run-complete" ]]; then
    log "supervisor: launching first-run wizard..."
    foot --app-id=foot --title "DIAD Setup" -e diad-first-run.sh >/dev/null 2>&1 &
fi

# ---- Every-boot display/storage setup ----
for script in diad-display-setup.sh diad-storage-setup.sh; do
    if command -v "${script}" >/dev/null 2>&1; then
        "${script}" >> "${LOG_FILE}" 2>&1 || log "supervisor: ${script} failed (non-fatal)"
    else
        log "supervisor: ${script} not found"
    fi
done

# ---- Launch DIAD daemon (headless, serves the GUI + WS API) ----
# The daemon must be up BEFORE the GUI window opens: it serves the static
# frontend on http://127.0.0.1:7860 and the JSON API on /ws.
if command -v diad-agent >/dev/null 2>&1; then
    nohup diad-agent --daemon >> "${DIAD_HOME}/daemon.log" 2>&1 &
    log "supervisor: DIAD daemon starting (pid $!)"
    # Wait up to 20 s for the health endpoint.
    healthy=0
    for _ in $(seq 1 40); do
        if curl -sf http://127.0.0.1:7860/health >/dev/null 2>&1; then
            healthy=1
            break
        fi
        sleep 0.5
    done
    if [[ ${healthy} -eq 1 ]]; then
        log "supervisor: DIAD daemon healthy on 127.0.0.1:7860"
    else
        log "supervisor: DIAD daemon did not become healthy — tail of log:"
        tail -n 5 "${DIAD_HOME}/daemon.log" >> "${LOG_FILE}" 2>&1 || true
    fi
else
    log "supervisor: diad-agent not found"
fi

# ---- Launch DIAD AGENT GUI on workspace 1 (Brave app window) ----
# The GUI is the static frontend served by the daemon itself, opened as a
# chromeless app window. Fall back to the default browser if brave is gone.
if command -v brave >/dev/null 2>&1; then
    brave --app=http://127.0.0.1:7860 >/dev/null 2>&1 &
    log "supervisor: DIAD AGENT GUI opened on workspace 1"
else
    xdg-open http://127.0.0.1:7860 >/dev/null 2>&1 &
    log "supervisor: brave not found, using default browser"
fi

# ---- Launch Brave on workspace 2 (waits for first-run to install it) ----
# Brave ships via Flatpak; the first-run wizard installs it. Poll every 5 s
# until the Flatpak app exists (or the wizard finishes without it), then
# launch once. The brave shim exits quietly while it is not installed yet.
# NOTE: the DIAD AGENT GUI (workspace 1) is also a Brave app window, so match
# the --new-window flag to avoid skipping this launch while the GUI runs.
if command -v brave >/dev/null 2>&1; then
    for _ in $(seq 1 120); do
        if flatpak list --app --columns=application 2>/dev/null | grep -qx 'com.brave.Browser'; then
            if ! pgrep -f -- '--new-window' >/dev/null 2>&1; then
                brave --new-window >/dev/null 2>&1 &
                log "supervisor: Brave launched"
            else
                log "supervisor: Brave already running"
            fi
            break
        fi
        # Stop waiting as soon as the wizard finished without installing it
        if [[ -f "${DIAD_HOME}/.first-run-complete" ]]; then
            log "supervisor: first-run complete, Brave not installed — skipping"
            break
        fi
        sleep 5
    done
else
    log "supervisor: brave not found"
fi

log "supervisor: complete"
exit 0
