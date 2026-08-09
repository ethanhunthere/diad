#!/usr/bin/env bash
#
# DIAD — Network setup
# =============================================================================
# Runs once on first boot (marker-gated, idempotent):
#   • Ensures NetworkManager is enabled and running.
#   • Enables Wi-Fi MAC randomization (cloned-mac-address=random) and
#     automatic reconnection on every existing Wi-Fi connection profile.
#
# Global scan-MAC randomization and the keyfile backend are configured in
# /etc/NetworkManager/conf.d/diad.conf.
#
set -uo pipefail

HOME_DIR="${HOME:-/home/diad}"
DIAD_HOME="${HOME_DIR}/.diad"
LOG_DIR="${DIAD_HOME}/logs"
MARKER="${DIAD_HOME}/.diad-network-setup-done"

mkdir -p "${LOG_DIR}"

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" >> "${LOG_DIR}/boot.log"
}

if [[ "${1:-}" != "--force" && -f "${MARKER}" ]]; then
    exit 0
fi

if ! command -v nmcli >/dev/null 2>&1; then
    log "diad-network-setup: nmcli not available — skipping."
    exit 0
fi

# ---- Ensure NetworkManager is running ----
sudo systemctl enable NetworkManager >/dev/null 2>&1 || true
sudo systemctl start NetworkManager >/dev/null 2>&1 || true
log "diad-network-setup: NetworkManager ensured."

# ---- Harden existing Wi-Fi profiles ----
for conn in $(nmcli -t -f NAME,TYPE connection show | awk -F: '$2 == "802-11-wireless" {print $1}'); do
    sudo nmcli connection modify "${conn}" \
        wifi.cloned-mac-address random \
        connection.autoconnect yes >/dev/null 2>&1
    log "diad-network-setup: hardened profile '${conn}' (random MAC + autoconnect)."
done

touch "${MARKER}"
log "diad-network-setup: complete."
exit 0
