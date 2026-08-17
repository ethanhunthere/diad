#!/usr/bin/env bash
#
# DIAD — Power management setup
# =============================================================================
# Runs once on first boot (marker-gated, idempotent):
#   • Ensures TLP is installed (kept disabled — power-profiles-daemon is the
#     active manager; running both conflicts).
#   • Enables power-profiles-daemon (performance/balanced/power-saver).
#   • Sets laptop lid behavior: suspend on battery, ignore on external power.
#
# logind changes take effect on the NEXT login (the script deliberately does
# not restart systemd-logind, which would kill the current session).
#
set -uo pipefail

HOME_DIR="${HOME:-/home/diad}"
DIAD_HOME="${HOME_DIR}/.diad"
LOG_DIR="${DIAD_HOME}/logs"
MARKER="${DIAD_HOME}/.diad-power-setup-done"
LOGIND_CONF="/etc/systemd/logind.conf"

mkdir -p "${LOG_DIR}"

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" >> "${LOG_DIR}/boot.log"
}

if [[ "${1:-}" != "--force" && -f "${MARKER}" ]]; then
    exit 0
fi

# ---- TLP: ensure masked (power-profiles-daemon is active manager) ----
sudo systemctl disable tlp >/dev/null 2>&1 || true
sudo systemctl mask tlp >/dev/null 2>&1 || true
log "diad-power-setup: tlp masked (power-profiles-daemon active)."

# ---- power-profiles-daemon ----
sudo systemctl enable --now power-profiles-daemon >/dev/null 2>&1 \
    && log "diad-power-setup: power-profiles-daemon enabled." \
    || log "diad-power-setup: could not enable power-profiles-daemon."

# ---- Lid behavior in logind.conf ----
set_lid_setting() {
    local key="$1" value="$2" file="$3"
    if grep -q "^[# ]*${key}=" "${file}" 2>/dev/null; then
        sudo sed -i "s/^[# ]*${key}=.*/${key}=${value}/" "${file}"
    else
        echo "${key}=${value}" | sudo tee -a "${file}" >/dev/null
    fi
}

set_lid_setting "HandleLidSwitch" "suspend" "${LOGIND_CONF}"
set_lid_setting "HandleLidSwitchExternalPower" "ignore" "${LOGIND_CONF}"
log "diad-power-setup: lid behavior set (suspend on battery, ignore on AC)."

touch "${MARKER}"
log "diad-power-setup: complete."
exit 0
