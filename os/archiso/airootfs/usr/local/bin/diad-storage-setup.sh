#!/usr/bin/env bash
#
# DIAD — Persistent storage setup
# =============================================================================
# Runs on every boot:
#   • If a partition labelled DIAD-DATA exists, mount it at ~/data
#     (fstab.d already handles this; the mount here is a fallback).
#   • Otherwise create a plain directory fallback at ~/data.
#   • Symlink standard user folders (~/Documents, ~/Downloads, ~/Pictures,
#     ~/Music, ~/Videos) into ~/data so user files survive reinstallation.
#
# Never clobbers an existing real directory — symlinks are only created where
# nothing exists yet.
#
set -uo pipefail

HOME_DIR="${HOME:-/home/diad}"
DIAD_HOME="${HOME_DIR}/.diad"
LOG_DIR="${DIAD_HOME}/logs"
DATA_DIR="${DATA_DIR:-${HOME_DIR}/data}"

mkdir -p "${LOG_DIR}"

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" >> "${LOG_DIR}/boot.log"
}

mkdir -p "${DATA_DIR}"

# ---- Mount the DIAD-DATA partition if present (fallback to fstab.d) ----
if [[ -e /dev/disk/by-label/DIAD-DATA ]]; then
    if ! findmnt -rn "${DATA_DIR}" >/dev/null 2>&1; then
        sudo mount LABEL=DIAD-DATA "${DATA_DIR}" >/dev/null 2>&1 \
            && log "storage-setup: mounted DIAD-DATA at ${DATA_DIR}." \
            || log "storage-setup: DIAD-DATA present but mount failed (non-fatal)."
    else
        log "storage-setup: DIAD-DATA already mounted at ${DATA_DIR}."
    fi
else
    log "storage-setup: no DIAD-DATA partition — using local fallback at ${DATA_DIR}."
fi

# ---- Standard-folder symlinks ----
for sub in Documents Downloads Pictures Music Videos; do
    if [[ ! -e "${HOME_DIR}/${sub}" ]]; then
        mkdir -p "${DATA_DIR}/${sub}"
        ln -s "${DATA_DIR}/${sub}" "${HOME_DIR}/${sub}" >/dev/null 2>&1 \
            && log "storage-setup: linked ~/${sub} → ${DATA_DIR}/${sub}"
    fi
done

log "storage-setup: complete."
exit 0
