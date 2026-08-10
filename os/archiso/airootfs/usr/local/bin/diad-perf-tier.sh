#!/usr/bin/env bash
#
# DIAD — Performance tier detection (spec section 1)
# =============================================================================
# Runs once at boot (diad-perf-tier.service) and on major hotplug (udev rule
# 99-diad-perf-tier.rules). Delegates detection + classification to the
# agent binary so the exact same tested code path runs everywhere:
#
#   diad-agent --perf-tier  →  /etc/diad/perf-tier  +  /etc/diad/hw-profile.json
#
# Afterwards applies the tier-dependent STORAGE POLICY (spec 1.8):
#   HDD      → readahead 256 KB on every rotational disk
#   SSD/NVMe → readahead 0 (the device already queues optimally)
#
# Never fails the boot: every step is guarded and logged.
#
set -uo pipefail

LOG_DIR="${HOME:-/root}/.diad/logs"
LOG_FILE="${LOG_DIR}/perf-tier.log"
TIER_FILE="/etc/diad/perf-tier"
PROFILE_FILE="/etc/diad/hw-profile.json"

mkdir -p "${LOG_DIR}" 2>/dev/null || LOG_FILE="/tmp/diad-perf-tier.log"

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" >> "${LOG_FILE}" 2>/dev/null
}

# ---- Hotplug debounce: skip when the profile is fresher than 10 s ---------
if [[ -f "${PROFILE_FILE}" ]]; then
    age=$(( $(date +%s) - $(stat -c %Y "${PROFILE_FILE}" 2>/dev/null || echo 0) ))
    if (( age < 10 )); then
        exit 0
    fi
fi

# ---- Detect + classify (the binary owns the logic; see core/perf.rs) ------
if ! command -v diad-agent >/dev/null 2>&1; then
    log "perf-tier: diad-agent not found — skipping detection."
    exit 0
fi

if ! diad-agent --perf-tier >> "${LOG_FILE}" 2>&1; then
    log "perf-tier: detection failed — keeping previous tier (or MID)."
fi

# Fail-safe (spec 1.9): no tier file after detection → MID, log, never crash.
if [[ ! -s "${TIER_FILE}" ]]; then
    mkdir -p /etc/diad 2>/dev/null || true
    echo "MID" > "${TIER_FILE}" 2>/dev/null || true
    log "perf-tier: defaulted to MID."
fi
TIER="$(cat "${TIER_FILE}" 2>/dev/null || echo MID)"
log "perf-tier: tier=${TIER}"

# ---- Storage policy (spec 1.8) ---------------------------------------------
# readahead is expressed in 512-byte sectors: 256 KB = 512 sectors.
if command -v blockdev >/dev/null 2>&1; then
    for disk in /sys/block/*; do
        name="$(basename "${disk}")"
        case "${name}" in
            loop*|ram*|zram*|dm-*|sr*|md*) continue ;;
        esac
        if [[ "${name}" == nvme* ]]; then
            blockdev --setra 0 "/dev/${name}" >/dev/null 2>&1 \
                && log "perf-tier: ${name} readahead=0 (nvme)."
        elif [[ "$(cat "${disk}/queue/rotational" 2>/dev/null)" == "1" ]]; then
            blockdev --setra 512 "/dev/${name}" >/dev/null 2>&1 \
                && log "perf-tier: ${name} readahead=256KB (hdd)."
        else
            blockdev --setra 0 "/dev/${name}" >/dev/null 2>&1 \
                && log "perf-tier: ${name} readahead=0 (ssd)."
        fi
    done
fi

# ---- CPU governor (spec 4.8) -----------------------------------------------
# schedutil everywhere; powersave on the weakest tiers while on battery.
GOVERNOR="schedutil"
ON_BATTERY="no"
for bat in /sys/class/power_supply/BAT*; do
    [[ -e "${bat}/status" ]] || continue
    if [[ "$(cat "${bat}/status" 2>/dev/null)" == "Discharging" ]]; then
        ON_BATTERY="yes"
    fi
done
if [[ "${ON_BATTERY}" == "yes" ]] && [[ "${TIER}" == "ULTRA_LOW" || "${TIER}" == "LOW" ]]; then
    GOVERNOR="powersave"
fi
for gov in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
    [[ -w "${gov}" ]] || continue
    echo "${GOVERNOR}" > "${gov}" 2>/dev/null \
        && log "perf-tier: governor=${GOVERNOR} (${gov})."
done

# ---- zram sizing (spec 4.2) --------------------------------------------------
# ULTRA_LOW (<= 2 GB RAM) gets 100% of RAM as zstd-compressed swap; every
# other tier keeps the default 50%. Only rewritten when the size changes.
ZRAM_CONF="/etc/zram-generator.conf"
ZRAM_SIZE_EXPR="ram / 2"
[[ "${TIER}" == "ULTRA_LOW" ]] && ZRAM_SIZE_EXPR="ram"
if [[ -f "${ZRAM_CONF}" ]] && ! grep -q "^zram-size = ${ZRAM_SIZE_EXPR}$" "${ZRAM_CONF}"; then
    sed -i "s/^zram-size = .*/zram-size = ${ZRAM_SIZE_EXPR}/" "${ZRAM_CONF}" 2>/dev/null \
        && log "perf-tier: zram-size=${ZRAM_SIZE_EXPR}."
    systemctl restart systemd-zram-setup@zram0.service >/dev/null 2>&1 \
        && log "perf-tier: zram0 reconfigured."
fi

exit 0
