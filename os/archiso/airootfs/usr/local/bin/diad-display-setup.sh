#!/usr/bin/env bash
#
# DIAD — Display & multi-monitor setup
# =============================================================================
# Runs on every boot (after sway is up):
#   • Built-in + external output present  → position each external output to
#     the right of the built-in screen.
#   • Built-in only                       → force the built-in's preferred
#     (native) resolution.
#
# Logs to ~/.diad/logs/display.log. Never fails the boot — if sway isn't
# reachable yet, it logs and exits 0.
#
set -uo pipefail

HOME_DIR="${HOME:-/home/diad}"
DIAD_HOME="${HOME_DIR}/.diad"
LOG_DIR="${DIAD_HOME}/logs"
LOG_FILE="${LOG_DIR}/display.log"

mkdir -p "${LOG_DIR}"

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" >> "${LOG_FILE}"
}

if ! command -v swaymsg >/dev/null 2>&1 || ! command -v jq >/dev/null 2>&1; then
    log "display-setup: swaymsg or jq not available — skipping."
    exit 0
fi

if [[ -z "${WAYLAND_DISPLAY:-}" ]]; then
    log "display-setup: not in a Wayland session — skipping."
    exit 0
fi

# ---- Enumerate outputs ----
OUTPUTS_JSON="$(swaymsg -t get_outputs 2>/dev/null)" || {
    log "display-setup: swaymsg failed — is sway running?"
    exit 0
}

BUILTIN="$(printf '%s' "${OUTPUTS_JSON}" | jq -r '.[] | select(.name | test("^(eDP|LVDS|DSI)")) | .name' | head -1)"
EXTERNALS="$(printf '%s' "${OUTPUTS_JSON}" | jq -r '.[] | select(.name | test("^(eDP|LVDS|DSI)") | not) | select(.active == true) | .name')"

# ---- Only built-in: enforce native resolution ----
if [[ -z "${BUILTIN}" && -z "${EXTERNALS}" ]]; then
    log "display-setup: no active outputs detected."
    exit 0
fi

if [[ -z "${EXTERNALS}" && -n "${BUILTIN}" ]]; then
    MODE="$(printf '%s' "${OUTPUTS_JSON}" | jq -r --arg o "${BUILTIN}" '.[] | select(.name == $o) | .modes[0] | "\(.width)x\(.height)"')"
    if [[ -n "${MODE}" ]]; then
        swaymsg output "${BUILTIN}" resolution "${MODE}" >/dev/null 2>&1
        log "display-setup: ${BUILTIN} → native ${MODE}."
    else
        log "display-setup: ${BUILTIN} active, no preferred mode listed."
    fi
    exit 0
fi

# ---- External present: extend to the right of the built-in ----
if [[ -n "${BUILTIN}" ]]; then
    BUILTIN_WIDTH="$(printf '%s' "${OUTPUTS_JSON}" | jq -r --arg o "${BUILTIN}" '.[] | select(.name == $o) | .rect.width')"
    swaymsg output "${BUILTIN}" position 0 0 >/dev/null 2>&1
    for ext in ${EXTERNALS}; do
        swaymsg output "${ext}" position "${BUILTIN_WIDTH}" 0 >/dev/null 2>&1
        log "display-setup: ${ext} positioned at ${BUILTIN_WIDTH},0 (right of ${BUILTIN})."
    done
else
    # No built-in detected (desktop) — just ensure each external is enabled
    for ext in ${EXTERNALS}; do
        swaymsg output "${ext}" enable >/dev/null 2>&1
        log "display-setup: ${ext} enabled."
    done
fi

log "display-setup: complete."
exit 0
