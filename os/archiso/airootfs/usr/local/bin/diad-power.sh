#!/usr/bin/env bash
# =============================================================================
# DIAD — Universal Power & Session Manager (Edition)
# =============================================================================
set -euo pipefail

action="${1:-}"

if [ -z "$action" ]; then
    menu=$(printf "⏻ Power Off\n⟳ Restart System\n⏾ Suspend / Sleep\n🔒 Lock Screen\n🚪 Log Out" | wofi --dmenu --insensitive --style /etc/wofi/style.css --width 340 --height 240 --prompt "✦ System Power" 2>/dev/null || true)
    case "$menu" in
        "⏻ Power Off")
            action="power"
            ;;
        "⟳ Restart System")
            action="reboot"
            ;;
        "⏾ Suspend / Sleep")
            action="suspend"
            ;;
        "🔒 Lock Screen")
            action="lock"
            ;;
        "🚪 Log Out")
            action="logout"
            ;;
        *)
            exit 0
            ;;
    esac
fi

case "${action}" in
    reboot)
        systemctl reboot 2>/dev/null || sudo systemctl reboot
        ;;
    power|shutdown|poweroff)
        systemctl poweroff 2>/dev/null || sudo systemctl poweroff
        ;;
    suspend|sleep)
        systemctl suspend 2>/dev/null || sudo systemctl suspend
        ;;
    lock)
        /usr/local/bin/diad-lock
        ;;
    logout)
        swaymsg exit 2>/dev/null || true
        ;;
    *)
        notify-send "DIAD Power" "Unknown action '${action}'" -u critical
        exit 1
        ;;
esac
