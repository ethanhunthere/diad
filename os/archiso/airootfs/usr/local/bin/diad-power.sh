#!/usr/bin/env bash
# =============================================================================
# DIAD — Universal Power & Session Manager (Edition)
# =============================================================================
set -euo pipefail

action="${1:-}"

if [ -z "$action" ] || [ "$action" = "dialog" ] || [ "$action" = "menu" ]; then
    menu=$(printf "⏻ Power Off\n⟳ Restart System\n⏾ Suspend / Sleep\n🔒 Lock Screen\n🚪 Log Out" | wofi --dmenu --insensitive --style /etc/wofi/style.css --width 340 --height 280 --prompt "System Power" 2>/dev/null || true)
    case "$menu" in
        "⏻ Power Off"|*"Power Off"*)
            action="power"
            ;;
        "⟳ Restart System"|*"Restart"*)
            action="reboot"
            ;;
        "⏾ Suspend / Sleep"|*"Suspend"*)
            action="suspend"
            ;;
        "🔒 Lock Screen"|*"Lock"*)
            action="lock"
            ;;
        "🚪 Log Out"|*"Log Out"*)
            action="logout"
            ;;
        *)
            exit 0
            ;;
    esac
fi

case "${action}" in
    reboot-confirm)
        # Waybar reboot icon: confirm first — a misclick must never reboot the
        # machine (this plus the unguarded power-key was the "random reboot" UX).
        if wofi --dmenu --insensitive --style /etc/wofi/style.css --width 360 --height 120 \
                --prompt "Restart System?" <<< "⟳ Restart now" >/dev/null 2>&1; then
            systemctl --no-wall reboot 2>/dev/null || sudo /usr/bin/systemctl --no-wall reboot 2>/dev/null || sudo reboot -f 2>/dev/null || systemctl -i reboot
        fi
        ;;
    reboot)
        systemctl --no-wall reboot 2>/dev/null || sudo /usr/bin/systemctl --no-wall reboot 2>/dev/null || sudo reboot -f 2>/dev/null || systemctl -i reboot
        ;;
    power|shutdown|poweroff)
        systemctl --no-wall poweroff 2>/dev/null || sudo /usr/bin/systemctl --no-wall poweroff 2>/dev/null || sudo poweroff -f 2>/dev/null || systemctl -i poweroff
        ;;
    suspend|sleep)
        systemctl suspend 2>/dev/null || sudo /usr/bin/systemctl suspend 2>/dev/null || systemctl -i suspend
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
