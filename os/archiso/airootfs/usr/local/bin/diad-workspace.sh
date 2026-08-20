#!/usr/bin/env bash
# =============================================================================
# DIAD — Smart Workspace Switcher & Window Manager
# =============================================================================
set -uo pipefail

TARGET_WS="${1:-1}"

case "${TARGET_WS}" in
    1|"DIAD AGENT")
        swaymsg workspace "DIAD AGENT" >/dev/null 2>&1 || swaymsg workspace number 1 >/dev/null 2>&1
        # Check if window exists on DIAD AGENT workspace
        ws_count=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.type? == "workspace" and ((.name? // "") | test("(?i)agent|1"))) | [ .. | objects | select(.type? == "con" and .name? != null) ] | length' 2>/dev/null || echo 0)
        if [[ "${ws_count}" == "0" ]]; then
            nohup /usr/local/bin/brave --app=http://127.0.0.1:7860 >/dev/null 2>&1 &
        fi
        ;;
    2|"BROWSER")
        swaymsg workspace "BROWSER" >/dev/null 2>&1 || swaymsg workspace number 2 >/dev/null 2>&1
        # Check if window exists on BROWSER workspace
        ws_count=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.type? == "workspace" and ((.name? // "") | test("(?i)browser|2"))) | [ .. | objects | select(.type? == "con" and .name? != null) ] | length' 2>/dev/null || echo 0)
        if [[ "${ws_count}" == "0" ]]; then
            nohup /usr/local/bin/brave http://127.0.0.1:7860/apps >/dev/null 2>&1 &
        fi
        ;;
    3|"COMMS")
        swaymsg workspace "COMMS" >/dev/null 2>&1 || swaymsg workspace number 3 >/dev/null 2>&1
        # Check if window exists on COMMS workspace
        ws_count=$(swaymsg -t get_tree 2>/dev/null | jq -r '.. | objects | select(.type? == "workspace" and ((.name? // "") | test("(?i)comms|3"))) | [ .. | objects | select(.type? == "con" and .name? != null) ] | length' 2>/dev/null || echo 0)
        if [[ "${ws_count}" == "0" ]]; then
            nohup /usr/local/bin/brave --app=http://127.0.0.1:7860/launch >/dev/null 2>&1 &
        fi
        ;;
esac
