#!/usr/bin/env bash
# =============================================================================
# DIAD OS — Universal Keyboard Shortcuts Guide (Super+K)
# =============================================================================
set -uo pipefail

WOFI_STYLE="/etc/wofi/style.css"
WOFI_BASE="--dmenu --insensitive --style $WOFI_STYLE"

shortcuts=$(cat << 'KEYS'
[F1] / [Super+A]        ➔ Switch to DIAD AI Agent (Workspace 1)
[F2] / [Super+W]        ➔ Switch to Web Browser (Workspace 2)
[F3] / [Super+C]        ➔ Switch to Communications (Workspace 3)
[Super+Enter]           ➔ Launch Foot Terminal Console
[Super+Shift+Enter]     ➔ Launch Floating Terminal
[Super+Space] / [Super+D]➔ Open App Launcher (Wofi)
[Super+V]               ➔ Open Clipboard History Manager
[Super+N]               ➔ Open Wi-Fi & Network Manager
[Super+P]               ➔ Open Power & System Verbs Menu
[Super+K]               ➔ Open Keyboard Shortcuts Guide
[Super+Q]               ➔ Close / Kill Focused Window
[Super+F] / [F11]       ➔ Toggle Fullscreen Window
[Super+Shift+Space]     ➔ Toggle Floating / Tiling Mode
[Super+E]               ➔ Toggle Split Layout (Horizontal / Vertical)
[Super+S]               ➔ Switch to Stacking Window Layout
[Super+T]               ➔ Switch to Tabbed Window Layout
[Super+H/J/K/L]         ➔ Navigate Focus (Left / Down / Up / Right)
[Super+Arrows]          ➔ Navigate Focus (Left / Down / Up / Right)
[Super+Shift+H/J/K/L]   ➔ Move Window (Left / Down / Up / Right)
[Super+Shift+Arrows]    ➔ Move Window (Left / Down / Up / Right)
[Super+1..9]            ➔ Switch to Workspace 1..9
[Super+Shift+1..9]      ➔ Move Focused Window to Workspace 1..9
[Super+R]               ➔ Enter Resize Mode (Arrows/HJKL, Esc to exit)
[Super+Minus]           ➔ Send Window to Scratchpad
[Super+Shift+Minus]     ➔ Cycle / Show Scratchpad Window
[Print]                 ➔ Interactive Screenshot (Drag Area)
[Shift+Print]           ➔ Fullscreen Screenshot (Auto-save to Pictures)
[Super+Shift+P] / [Super+L] ➔ Lock Screen (diad-lock)
[Super+Escape]          ➔ Instant System Suspend / Sleep
[Ctrl+Shift+C] (Term)   ➔ Copy Selected Terminal Text
[Ctrl+Shift+V] (Term)   ➔ Paste Terminal Clipboard
[F6] (TUI) / /model     ➔ Switch LLM AI Provider & Model
[F9] (TUI)              ➔ Switch from TUI Terminal to GUI Studio
[Super+Shift+C]         ➔ Reload Sway Compositor Config
[Super+Shift+E]         ➔ Exit Sway / Logout
KEYS
)

printf "%b" "$shortcuts" | wofi $WOFI_BASE --width 680 --height 580 --prompt "⌨ DIAD OS Keybindings [40+ Shortcuts]" 2>/dev/null || true
