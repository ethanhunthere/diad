#!/usr/bin/env bash
#
# DIAD — Boot diagnostics
# =============================================================================
# Dumps system state to ~/diad-debug.txt for troubleshooting a live boot:
#   • failed systemd units
#   • last 100 ERROR-priority journal lines from this boot
#   • key DIAD processes (foot, brave, diad, waybar, sway)
#   • availability of the DIAD binaries (diad-agent, foot, brave)
#   • /usr/local/bin contents
#   • all ~/.diad/logs/*.log
#
# Run it in a terminal on the live system, then share ~/diad-debug.txt.
#
set -uo pipefail

HOME_DIR="${HOME:-/home/diad}"
OUT="${HOME_DIR}/diad-debug.txt"

{
echo "=== DIAD diagnostics $(date '+%Y-%m-%d %H:%M:%S') ==="

echo ""
echo "=== systemctl --failed --no-pager ==="
systemctl --failed --no-pager

echo ""
echo "=== journalctl -b -p err --no-pager (last 100) ==="
journalctl -b -p err --no-pager 2>/dev/null | tail -100

echo ""
echo "=== processes: foot/brave/diad/waybar/sway ==="
ps aux | grep -E 'foot|brave|diad|waybar|sway' | grep -v grep

echo ""
echo "=== availability: diad-agent foot brave ==="
for bin in diad-agent foot brave; do
    printf '%-12s %s\n' "${bin}" "$(command -v "${bin}" 2>/dev/null || echo 'NOT FOUND')"
done

echo ""
echo "=== ls -la /usr/local/bin/ ==="
ls -la /usr/local/bin/

echo ""
echo "=== ~/.diad/logs/*.log ==="
for logfile in "${HOME_DIR}/.diad/logs/"*.log; do
    [[ -e "${logfile}" ]] || continue
    echo ""
    echo "----- ${logfile} -----"
    cat "${logfile}"
done

} > "${OUT}" 2>&1

echo "Diagnostics written to ${OUT}"
exit 0
