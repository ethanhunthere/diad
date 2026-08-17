#!/usr/bin/env bash
#
# DIAD — Network setup (Universal Hardware & Wi-Fi Stack)
# =============================================================================
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

# ---- 1. Unblock all physical and kernel wireless switches ----
rfkill unblock wifi 2>/dev/null || true
rfkill unblock all 2>/dev/null || true

# ---- 2. Set regulatory domain to enable all 2.4GHz & 5GHz channels ----
iw reg set US 2>/dev/null || true

# ---- 3. Ensure wpa_supplicant & NetworkManager are active ----
systemctl start wpa_supplicant >/dev/null 2>&1 || true
systemctl start NetworkManager >/dev/null 2>&1 || true
sleep 0.5

# ---- 4. Bring up wireless interfaces and mark managed ----
nmcli radio wifi on >/dev/null 2>&1 || true
for dev in $(nmcli -t -f DEVICE,TYPE dev 2>/dev/null | grep ':wifi' | cut -d: -f1); do
    ip link set "$dev" up 2>/dev/null || true
    nmcli dev set "$dev" managed yes 2>/dev/null || true
    nmcli dev set "$dev" autoconnect yes 2>/dev/null || true
done

# ---- 5. Pre-warm Wi-Fi frequency scan in background ----
(
    sleep 1
    nmcli dev wifi rescan >/dev/null 2>&1 || true
) &

log "diad-network-setup: NetworkManager, wpa_supplicant, and Wi-Fi devices initialized."
touch "${MARKER}"
log "diad-network-setup: complete."
exit 0
