#!/usr/bin/env bash
# =============================================================================
# DIAD — Fast Workspace Switcher (2040 Edition)
# =============================================================================
set -uo pipefail

target="${1:-1}"

case "$target" in
    1|agent|diad)
        swaymsg workspace "1: DIAD AGENT" >/dev/null 2>&1 || swaymsg workspace 1 >/dev/null 2>&1
        ;;
    2|browser|web)
        swaymsg workspace "2: BROWSER" >/dev/null 2>&1 || swaymsg workspace 2 >/dev/null 2>&1
        ;;
    3|comms|studio|launch)
        swaymsg workspace "3: COMMS" >/dev/null 2>&1 || swaymsg workspace 3 >/dev/null 2>&1
        ;;
esac
