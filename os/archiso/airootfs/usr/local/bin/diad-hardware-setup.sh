#!/usr/bin/env bash
#
# DIAD — Hardware setup (laptop detection & media keys)
# =============================================================================
# Detects whether the machine is a laptop (any battery in /sys/class/power_supply).
# On laptops it enables the media/backlight key bindings in
# /etc/sway/conf.d/media-keys.conf; on desktops it keeps them disabled.
#
# Runs once on first boot (see the marker file below). Idempotent — safe to
# re-run at any time.
#
set -uo pipefail

HOME_DIR="${HOME:-/home/diad}"
DIAD_HOME="${HOME_DIR}/.diad"
LOG_DIR="${DIAD_HOME}/logs"
MARKER="${DIAD_HOME}/.diad-hardware-setup-done"
MEDIA_KEYS_CONF="/etc/sway/conf.d/media-keys.conf"

mkdir -p "${LOG_DIR}"

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" >> "${LOG_DIR}/boot.log"
}

# ---- First-boot gating (override with --force) ----
if [[ "${1:-}" != "--force" && -f "${MARKER}" ]]; then
    exit 0
fi

# ---- Laptop detection: any battery present? ----
IS_LAPTOP="no"
if compgen -G "/sys/class/power_supply/BAT*" >/dev/null; then
    IS_LAPTOP="yes"
fi

log "diad-hardware-setup: laptop=$IS_LAPTOP"

if [[ ! -f "${MEDIA_KEYS_CONF}" ]]; then
    log "diad-hardware-setup: ${MEDIA_KEYS_CONF} missing — skipping."
    exit 0
fi

# ---- Enable/disable the media-key bindings (idempotent sed) ----
if [[ "${IS_LAPTOP}" == "yes" ]]; then
    # Uncomment the '# bindsym' lines
    sudo sed -i 's/^# bindsym/bindsym/' "${MEDIA_KEYS_CONF}"
    log "diad-hardware-setup: media keys enabled (laptop)."
else
    # Comment any active 'bindsym' lines back out
    sudo sed -i 's/^bindsym/# bindsym/' "${MEDIA_KEYS_CONF}"
    log "diad-hardware-setup: media keys disabled (desktop)."
fi

touch "${MARKER}"
log "diad-hardware-setup: complete."
exit 0
