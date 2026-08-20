#!/usr/bin/env bash
# =============================================================================
# DIAD OS — Universal Clipboard History Manager (Super+V)
# =============================================================================
set -uo pipefail

WOFI_STYLE="/etc/wofi/style.css"
WOFI_BASE="--dmenu --insensitive --style $WOFI_STYLE"

if command -v cliphist >/dev/null 2>&1; then
    selected=$(cliphist list 2>/dev/null | wofi $WOFI_BASE --width 580 --height 440 --prompt "📋 Clipboard History [Super+V]" 2>/dev/null || true)
    if [ -n "$selected" ]; then
        echo "$selected" | cliphist decode | wl-copy
        notify-send "DIAD Clipboard" "Copied item to clipboard" -i edit-copy -t 1500
    fi
else
    current=$(wl-paste 2>/dev/null || echo "")
    if [ -z "$current" ]; then
        notify-send "DIAD Clipboard" "Clipboard is currently empty" -i edit-copy -t 1500
    else
        notify-send "DIAD Clipboard" "Current clipboard: ${current:0:60}..." -i edit-copy -t 2500
    fi
fi
