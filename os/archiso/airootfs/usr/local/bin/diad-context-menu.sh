#!/usr/bin/env bash
# =============================================================================
# DIAD OS — Desktop Right-Click Context Menu (Industrial Basalt & Crimson)
# =============================================================================
set -uo pipefail

WOFI_STYLE="/etc/wofi/style.css"
WOFI_BASE="--dmenu --insensitive --style $WOFI_STYLE"

menu=$(cat << 'MENU'
DIAD AI Agent (Workspace 1)
🌐 Open Web Browser (Workspace 2)
💬 Open Communications (Workspace 3)
💻 Open Terminal Console (Super+Enter)
📁 Open File Manager
📶 Wi-Fi & Network Manager (Super+N)
📋 Clipboard History (Super+V)
🎨 Appearance & Personalization
⚙ System Settings
📷 Capture Interactive Screenshot (Print)
🖼 Capture Fullscreen Screenshot (Shift+Print)
⌨ View Keyboard Shortcuts (Super+K)
🔒 Lock Screen (Super+Shift+P)
⏻ Power & System Menu (Super+P)
MENU
)

choice=$(printf "%b" "$menu" | wofi $WOFI_BASE --width 480 --height 620 --prompt "DIAD Quick Menu" 2>/dev/null || true)
[ -z "$choice" ] && exit 0

case "$choice" in
    *"DIAD AI Agent"*)
        swaymsg workspace "AGENT" 2>/dev/null || true
        ;;
    *"Web Browser"*)
        swaymsg workspace "BROWSER" 2>/dev/null || true
        ;;
    *"Communications"*)
        swaymsg workspace "COMMS" 2>/dev/null || true
        ;;
    *"Terminal Console"*)
        foot &
        ;;
    *"File Manager"*)
        foot -e lf 2>/dev/null || foot -e ranger 2>/dev/null || foot &
        ;;
    *"Wi-Fi"*)
        /usr/local/bin/diad-wifi.sh &
        ;;
    *"Clipboard"*)
        /usr/local/bin/diad-clipboard.sh &
        ;;
    *"Appearance"*)
        foot --title="DIAD Themes" -e diad /tuicolor 2>/dev/null || true
        ;;
    *"System Settings"*)
        foot --title="DIAD Settings" -e diad /status 2>/dev/null || true
        ;;
    *"Interactive Screenshot"*)
        grim -g "$(slurp)" - | wl-copy && notify-send "DIAD Screenshot" "Selected area copied to clipboard" -i camera-photo || true
        ;;
    *"Fullscreen Screenshot"*)
        mkdir -p ~/Pictures/Screenshots 2>/dev/null || true
        ts=$(date +%Y-%m-%d_%H-%M-%S)
        grim ~/Pictures/Screenshots/screenshot_${ts}.png && wl-copy < ~/Pictures/Screenshots/screenshot_${ts}.png && notify-send "DIAD Screenshot" "Saved to Pictures/Screenshots & copied to clipboard" -i camera-photo || true
        ;;
    *"Keyboard Shortcuts"*)
        /usr/local/bin/diad-shortcuts.sh &
        ;;
    *"Lock Screen"*)
        /usr/local/bin/diad-lock &
        ;;
    *"Power"*)
        /usr/local/bin/diad-power.sh &
        ;;
    *)
        exit 0
        ;;
esac
