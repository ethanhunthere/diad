#!/usr/bin/env bash
# =============================================================================
# DIAD OS — Universal Wi-Fi Connection Manager (Zero-Delay, Non-Scrollable)
# =============================================================================
set -uo pipefail

WOFI_STYLE="/etc/wofi/style.css"
WOFI_BASE="--dmenu --insensitive --style $WOFI_STYLE"

# 1. Ensure Wi-Fi radio and kernel interfaces are unblocked
rfkill unblock wifi 2>/dev/null || true
rfkill unblock all 2>/dev/null || true

# 2. Discover active wireless interface
WIFI_DEV=$(nmcli -t -f DEVICE,TYPE dev 2>/dev/null | grep ':wifi' | cut -d: -f1 | head -n 1 || echo "")
if [ -z "$WIFI_DEV" ] && command -v iw >/dev/null 2>&1; then
    WIFI_DEV=$(iw dev 2>/dev/null | grep Interface | awk '{print $2}' | head -n 1 || echo "")
fi

if [ -n "$WIFI_DEV" ]; then
    ip link set "$WIFI_DEV" up 2>/dev/null || true
    nmcli dev set "$WIFI_DEV" managed yes 2>/dev/null || true
fi

# 3. Check Wi-Fi radio power
wifi_status=$(nmcli -fields WIFI g 2>/dev/null | tail -n 1 | tr -d ' ' || echo "enabled")
if [ "$wifi_status" = "disabled" ]; then
    choice=$(printf "[S] ⏼ Turn On Wi-Fi\n[X] ✕ Close Menu" | wofi $WOFI_BASE --width 580 --height 180 --prompt "Wi-Fi is Currently Powered Off" 2>/dev/null || true)
    if [[ "$choice" =~ "Turn On" ]]; then
        nmcli radio wifi on
        notify-send "DIAD Network" "Wi-Fi radio turned ON" -i network-wireless
        sleep 1.0
    else
        exit 0
    fi
fi

# 4. If requested with --rescan or user triggered rescan, do active channel scan
if [ "${1:-}" = "--rescan" ]; then
    notify-send "DIAD Network" "Scanning Wi-Fi networks..." -t 2000 -i network-wireless 2>/dev/null || true
    nmcli dev wifi rescan 2>/dev/null || true
    sleep 1.2
fi

# 5. Fetch wireless network list from NetworkManager cache (instant 0.05s response)
raw_list=$(nmcli --terse --fields "IN-USE,SSID,SIGNAL,SECURITY,BARS" dev wifi list 2>/dev/null || true)

# If cache was completely empty, trigger quick scan
if [ -z "$raw_list" ]; then
    notify-send "DIAD Network" "Scanning Wi-Fi networks..." -t 2000 -i network-wireless 2>/dev/null || true
    nmcli dev wifi rescan 2>/dev/null || true
    sleep 1.0
    raw_list=$(nmcli --terse --fields "IN-USE,SSID,SIGNAL,SECURITY,BARS" dev wifi list 2>/dev/null || true)
fi

if [ -z "$raw_list" ]; then
    choice=$(printf "[S] ⟳ Rescan Networks\n[X] ✕ Close Menu\n[C] + Connect to Hidden Network\n⏻ Turn Off Wi-Fi" | wofi $WOFI_BASE --width 580 --height 240 --prompt "No Wireless Networks Detected" 2>/dev/null || true)
    if [[ "$choice" =~ "Rescan" ]] || [ "$choice" = "s" ] || [ "$choice" = "S" ]; then
        exec "$0" --rescan
    elif [[ "$choice" =~ "Turn Off" ]]; then
        nmcli radio wifi off
        notify-send "DIAD Network" "Wi-Fi disabled" -i network-wireless-offline
        exit 0
    fi
    exit 0
fi

# 6. Parse and build numbered lookup table for 100% exact SSID and security matching
MAP_FILE="/tmp/diad_wifi_map_$$.txt"
trap 'rm -f "$MAP_FILE"' EXIT

entries=""
seen_ssids="|"
count=0

while IFS=':' read -r in_use ssid signal security bars; do
    [ -z "$ssid" ] && continue
    # De-duplicate identical SSIDs, retaining highest signal strength
    if echo "$seen_ssids" | grep -q "|$ssid|"; then
        continue
    fi
    seen_ssids="${seen_ssids}${ssid}|"
    count=$((count + 1))
    [ $count -gt 8 ] && break

    # Security label
    is_open=false
    if [ -z "$security" ] || [ "$security" = "--" ] || [ "$security" = "none" ]; then
        sec_label="Open"
        sec_icon="🔓"
        is_open=true
    else
        sec_label="$security"
        sec_icon="🔒"
    fi

    # Connected status
    is_connected=false
    if [ "$in_use" = "*" ]; then
        is_connected=true
        status_tag="● [CONNECTED]"
    else
        status_tag="  ${sec_icon} [${sec_label}]"
    fi

    [ -z "$bars" ] && bars="${signal}%"

    # Save to map file: index|in_use|is_open|security|ssid
    echo "${count}|${is_connected}|${is_open}|${sec_label}|${ssid}" >> "$MAP_FILE"

    # Display line in Wofi
    line=$(printf "%-18s %-26s  %s  %3s%%" "$status_tag" "$ssid" "$bars" "$signal")
    entries="${entries}${line}\n"
done <<< "$raw_list"

# Menu header with standard keyboard controls
menu_header=$(printf "[S] ⟳ Rescan Networks\n[C] ➔ Connect to Selected Network\n[X] ✕ Close Menu\n────────────────────────────────────────────────────────\n")
full_menu=$(printf "%b%b" "$menu_header" "$entries")

selected=$(printf "%b" "$full_menu" | wofi $WOFI_BASE --width 640 --height 520 --prompt "Wi-Fi Networks [Enter:Connect / S:Scan / X:Close]" 2>/dev/null || true)
[ -z "$selected" ] && exit 0

# 7. Handle menu actions and keyboard shortcuts
if [[ "$selected" =~ "Close Menu" ]] || [ "$selected" = "x" ] || [ "$selected" = "X" ]; then
    exit 0
elif [[ "$selected" =~ "Rescan" ]] || [ "$selected" = "s" ] || [ "$selected" = "S" ]; then
    exec "$0" --rescan
elif echo "$selected" | grep -q '───────'; then
    exit 0
fi

# If user selected "[C] Connect to Selected Network", show network selection
if [[ "$selected" =~ "Connect to Selected Network" ]] || [ "$selected" = "c" ] || [ "$selected" = "C" ]; then
    net_only=$(printf "%b" "$entries" | wofi $WOFI_BASE --width 640 --height 420 --prompt "Select Wi-Fi Network:" 2>/dev/null || true)
    [ -z "$net_only" ] && exit 0
    selected="$net_only"
fi

# 8. Match selected row against lookup table
matched_ssid=""
matched_is_connected="false"
matched_is_open="false"
matched_sec=""

if [ -f "$MAP_FILE" ]; then
    while IFS='|' read -r idx conn open sec ssid; do
        if echo "$selected" | grep -F -q "$ssid"; then
            matched_ssid="$ssid"
            matched_is_connected="$conn"
            matched_is_open="$open"
            matched_sec="$sec"
            break
        fi
    done < "$MAP_FILE"
fi

# Fallback string extraction if map missed
if [ -z "$matched_ssid" ]; then
    matched_ssid=$(echo "$selected" | awk '{$1=""; print $0}' | sed 's/^[ \t]*//' | sed 's/  .*$//' | xargs)
fi

[ -z "$matched_ssid" ] && exit 0

# 9. If already connected, notify immediately
if [ "$matched_is_connected" = "true" ]; then
    ip_addr=$(ip -4 -o addr show | grep -v '127.0.0.1' | awk '{print $4}' | head -n 1 || echo "Active")
    notify-send "DIAD Network" "✓ Already connected to '$matched_ssid' (IP: $ip_addr)" -i network-wireless
    exit 0
fi

# 10. Handle Open vs Secured Wi-Fi
pass=""
if [ "$matched_is_open" = "false" ]; then
    prompt_text="User's Password for \"$matched_ssid\"                                                ↑"
    raw_pass=$(printf " " | wofi --dmenu --password --insensitive --style "$WOFI_STYLE" --width 640 --height 130 --prompt "$prompt_text" 2>/dev/null || true)
    pass=$(echo -n "$raw_pass" | tr -d '\r\n')
    
    # If user cancelled password entry
    if [ -z "$pass" ] || [ "$pass" = " " ]; then
        exit 0
    fi
fi

notify-send "DIAD Network" "Connecting to '$matched_ssid'..." -i network-wireless

# 11. Clean broken stale connection profiles
nmcli connection delete id "$matched_ssid" 2>/dev/null || true
nmcli connection delete "$matched_ssid" 2>/dev/null || true

# 12. Connection Engine
if [ "$matched_is_open" = "true" ]; then
    # Open network connection
    connect_out=$(nmcli dev wifi connect "$matched_ssid" ${WIFI_DEV:+ifname "$WIFI_DEV"} --wait 25 2>&1 || true)
else
    # Secured network connection (Attempt 1: direct dev wifi connect)
    connect_out=$(nmcli dev wifi connect "$matched_ssid" password "$pass" ${WIFI_DEV:+ifname "$WIFI_DEV"} --wait 25 2>&1 || true)
fi

if echo "$connect_out" | grep -iq "successfully activated\|connected"; then
    ip_addr=$(ip -4 -o addr show | grep -v '127.0.0.1' | awk '{print $4}' | head -n 1 || echo "Active")
    notify-send "DIAD Network" "✓ Connected to '$matched_ssid' (IP: $ip_addr)" -i network-wireless
else
    # Attempt 2: Explicit WPA-PSK profile fallback
    if [ "$matched_is_open" = "false" ]; then
        nmcli connection delete id "$matched_ssid" 2>/dev/null || true
        if [ -n "$WIFI_DEV" ]; then
            nmcli connection add type wifi ifname "$WIFI_DEV" con-name "$matched_ssid" ssid "$matched_ssid" wifi-sec.key-mgmt wpa-psk wifi-sec.psk "$pass" 2>/dev/null || true
        else
            nmcli connection add type wifi con-name "$matched_ssid" ssid "$matched_ssid" wifi-sec.key-mgmt wpa-psk wifi-sec.psk "$pass" 2>/dev/null || true
        fi
        nmcli connection modify "$matched_ssid" connection.autoconnect yes 2>/dev/null || true
        
        fallback_out=$(nmcli connection up "$matched_ssid" --wait 25 2>&1 || true)
        if echo "$fallback_out" | grep -iq "successfully activated\|connected"; then
            notify-send "DIAD Network" "✓ Connected to '$matched_ssid'" -i network-wireless
            exit 0
        fi
    fi

    notify-send "DIAD Network" "Authentication failed for '$matched_ssid'. Please verify password or signal." -u critical -i network-wireless-offline
    # Open nmtui terminal fallback for manual diagnostic configuration
    foot --app-id=foot --title "Wi-Fi Configuration" -e nmtui >/dev/null 2>&1 &
fi
