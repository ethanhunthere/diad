#!/usr/bin/env bash
# =============================================================================
# DIAD — power actions for the waybar buttons (S3).
# =============================================================================
# Waybar custom modules call this with a fixed action word:
#   suspend | reboot | power
# The sudo grants are argv-pinned in sudoers.d/diad, so only the exact
# `systemctl suspend|reboot|poweroff` invocations are allowed. This script
# adds a wofi y/N confirmation (skipped if wofi is missing — the click is
# already deliberate).
# =============================================================================

set -u

action="${1:-}"
case "${action}" in
    suspend)
        cmd="/usr/bin/systemctl suspend"
        label="Sleep"
        ;;
    reboot)
        cmd="/usr/bin/systemctl reboot"
        label="Restart"
        ;;
    power)
        cmd="/usr/bin/systemctl poweroff"
        label="Shutdown"
        ;;
    *)
        notify-send "diad-power: unknown action '${action}' (suspend|reboot|power)" -u critical
        exit 1
        ;;
esac

confirm() {
    if ! command -v wofi >/dev/null 2>&1; then
        return 0
    fi
    choice="$(printf 'No\nYes\n' | wofi --dmenu --prompt "${label} DIAD?" --insensitive 2>/dev/null)"
    [ "${choice}" = "Yes" ]
}

if confirm; then
    exec sudo "${cmd}"
fi
exit 0
