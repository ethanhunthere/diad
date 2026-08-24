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

# ---- Audio Subsystem Initialization (PipeWire & ALSA) ----
systemctl --user start pipewire pipewire-pulse wireplumber 2>/dev/null || true
(
    sleep 0.5
    wpctl set-mute @DEFAULT_AUDIO_SINK@ 0 2>/dev/null || true
    wpctl set-volume @DEFAULT_AUDIO_SINK@ 0.8 2>/dev/null || true
    amixer -q sset Master unmute 2>/dev/null || true
    amixer -q sset Master 80% 2>/dev/null || true
    amixer -q sset Speaker unmute 2>/dev/null || true
    amixer -q sset Headphone unmute 2>/dev/null || true
) &

touch "${MARKER}"
log "diad-hardware-setup: complete."
exit 0
