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

# ---- HiDPI / fractional scaling (spec 2.4) --------------------------------
# DPI from the EDID physical size (bytes 21/22 = width/height in cm) and the
# active mode width:  <120 → 1.0 · <150 → 1.25 · <200 → 1.5 · else → 2.0.
# Per-output: multi-monitor setups with different DPI scale independently.
compute_scale() {
    local name="$1" px="$2"
    local edid cm dpi
    edid="$(ls "/sys/class/drm/card*-${name}/edid" 2>/dev/null | head -1)"
    [[ -z "${edid}" ]] && { echo "1"; return; }
    cm="$(od -An -tu1 -j21 -N1 "${edid}" 2>/dev/null | tr -d ' ')"
    if [[ -z "${cm}" || "${cm}" == "0" ]]; then
        echo "1"; return
    fi
    dpi=$(( px * 254 / (cm * 100) ))
    if   (( dpi < 120 )); then echo "1"
    elif (( dpi < 150 )); then echo "1.25"
    elif (( dpi < 200 )); then echo "1.5"
    else echo "2"
    fi
}

ALL_ACTIVE="$(printf '%s' "${OUTPUTS_JSON}" | jq -r '.[] | select(.active == true) | .name')"
for out in ${ALL_ACTIVE}; do
    PX="$(printf '%s' "${OUTPUTS_JSON}" | jq -r --arg o "${out}" '.[] | select(.name == $o) | (.current_mode.width // .modes[0].width // 1920)')"
    SCALE="$(compute_scale "${out}" "${PX}")"
    CURRENT="$(printf '%s' "${OUTPUTS_JSON}" | jq -r --arg o "${out}" '.[] | select(.name == $o) | .scale // 1')"
    if [[ "${SCALE}" != "${CURRENT}" ]]; then
        swaymsg output "${out}" scale "${SCALE}" >/dev/null 2>&1 \
            && log "display-setup: ${out} scale ${CURRENT} → ${SCALE} (${PX}px wide)."
    fi
done
# Re-read after scaling: rect dimensions are now logical (scaled) units.
OUTPUTS_JSON="$(swaymsg -t get_outputs 2>/dev/null)" || true

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
