#!/usr/bin/env bash
#
# DIAD Launch Script
# Ensures both DIAD surfaces (Agent + Brave) are running.
# Called on login and can be invoked manually.
#

set -euo pipefail

# ---- Environment ----
export WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}"
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"

DIAD_HOME="${HOME}/.diad"
LOG_DIR="${DIAD_HOME}/logs"
LOG_FILE="${LOG_DIR}/boot.log"

# Ensure log directory exists
mkdir -p "${LOG_DIR}"

# ---- Logging ----
log() {
    local timestamp
    timestamp="$(date '+%Y-%m-%d %H:%M:%S')"
    echo "[${timestamp}] $*" | tee -a "${LOG_FILE}"
}

log "DIAD Launch Script starting..."

# ---- Check prerequisites ----
if ! command -v foot &>/dev/null; then
    log "ERROR: foot terminal emulator not found. Install with: sudo pacman -S foot"
    exit 1
fi

if ! command -v brave &>/dev/null; then
    log "WARNING: Brave browser not found. Install brave-bin from AUR."
fi

if ! command -v diad-agent &>/dev/null; then
    log "WARNING: diad-agent not found in PATH. Make sure it's installed at /usr/local/bin/diad-agent"
fi

# ---- Launch Brave Browser ----
if command -v brave &>/dev/null; then
    if pgrep -x brave > /dev/null; then
        log "Brave is already running."
    else
        log "Launching Brave browser..."
        brave --new-window &
        sleep 1
        log "Brave launched (PID: $!)"
    fi
else
    log "Skipping Brave — not installed."
fi

# ---- Launch DIAD Agent ----
if command -v diad-agent &>/dev/null; then
    if pgrep -x diad-agent > /dev/null; then
        log "DIAD Agent is already running."
    else
        log "Launching DIAD Agent in foot terminal..."
        # If we're in a graphical session, launch in foot
        if [ -n "${WAYLAND_DISPLAY:-}" ]; then
            foot --title "DIAD Agent" -e diad-agent &
        else
            # Running in TTY — start directly
            diad-agent &
        fi
        sleep 1
        log "DIAD Agent launched (PID: $!)"
    fi
else
    log "Skipping DIAD Agent — not installed."
fi

log "DIAD Launch Script complete."
