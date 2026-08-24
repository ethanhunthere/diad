#!/usr/bin/env bash
# =============================================================================
# DIAD — profiledef.sh file_permissions validator
# =============================================================================
# mkarchiso hard-fails with "Outside of valid path" if any file_permissions
# entry in profiledef.sh does not exist inside the profile's airootfs/ tree
# (realpath -q returns empty for missing paths, which fails the traversal
# check). This gate catches stale entries BEFORE the ~10-minute mkarchiso run.
#
# Exit 0: every entry resolves. Exit 1: at least one entry is stale.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ISO_DIR="${1:-${SCRIPT_DIR}/../os/archiso}"
PROFILE="${ISO_DIR}/profiledef.sh"
AIROOTFS="${ISO_DIR}/airootfs"

if [[ ! -f "${PROFILE}" || ! -d "${AIROOTFS}" ]]; then
    echo "validate-profile: cannot find profiledef.sh or airootfs under ${ISO_DIR}" >&2
    exit 1
fi

missing=0
count=0
# Extract only the file_permissions=( ... ) block, then each ["/path"] key.
while IFS= read -r p; do
    count=$((count + 1))
    if [[ ! -e "${AIROOTFS}${p}" ]]; then
        echo "STALE file_permissions entry (not in airootfs): ${p}" >&2
        missing=$((missing + 1))
    fi
done < <(sed -n '/^file_permissions=(/,/^)/p' "${PROFILE}" \
    | grep -oP '^\s*\["\K[^\"]+(?="\])')

if (( missing > 0 )); then
    echo "validate-profile: FAILED — ${missing} of ${count} entries are stale." >&2
    echo "  Remove them from profiledef.sh or restore the files in airootfs/." >&2
    exit 1
fi

echo "validate-profile: OK — all ${count} file_permissions entries exist in airootfs."
