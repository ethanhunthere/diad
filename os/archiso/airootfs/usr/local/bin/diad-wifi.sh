#!/usr/bin/env bash
# =============================================================================
# DIAD — Universal Industrial Basalt & Crimson Wi-Fi Manager
# =============================================================================
set -euo pipefail

WOFI_STYLE="/etc/wofi/style.css"
WOFI_BASE="--dmenu --insensitive --style $WOFI_STYLE"

# 1. Unblock wireless hardware via rfkill
rfkill unblock wifi 2>/dev/null || true
rfkill unblock all 2>/dev/null || true

# 2. Discover primary Wi-Fi interface
WIFI_DEV=$(nmcli -t -f DEVICE,TYPE dev 2>/dev/null | grep ':wifi' | cut -d: -f1 | head -n 1 || echo "")

if [ -n "$WIFI_DEV" ]; then
    ip link set "$WIFI_DEV" up 2>/dev/null || true
    nmcli dev set "$WIFI_DEV" managed yes 2>/dev/null || true
fi

# 3. Check Wi-Fi radio status
wifi_status=$(nmcli -fields WIFI g 2>/dev/null | tail -n 1 | tr -d ' ' || echo "enabled")
if [ "$wifi_status" = "disabled" ]; then
    choice=$(printf "⏼ Turn On Wi-Fi\n✕ Close" | wofi $WOFI_BASE --width 380 --height 180 --prompt "Wi-Fi is Powered Off" 2>/dev/null || true)
    if [ "$choice" = "⏼ Turn On Wi-Fi" ]; then
        nmcli radio wifi on
        notify-send "DIAD Network" "Wi-Fi radio turned ON" -i network-wireless
        sleep 1.5
    else
        exit 0
    fi
fi

# 4. Scan available wireless networks
raw_list=$(nmcli --terse --fields "IN-USE,SSID,SIGNAL,SECURITY,BARS" dev wifi list 2>/dev/null || true)

if [ -z "$raw_list" ]; then
    notify-send "DIAD Network" "Scanning wireless frequencies..." -t 2000 -i network-wireless 2>/dev/null || true
    nmcli dev wifi rescan 2>/dev/null || true
    for _ in 1 2 3; do
        sleep 1
        raw_list=$(nmcli --terse --fields "IN-USE,SSID,SIGNAL,SECURITY,BARS" dev wifi list 2>/dev/null || true)
        [ -n "$raw_list" ] && break
    done
fi

if [ -z "$raw_list" ]; then
    choice=$(printf "⟳ Scan Again\n+ Connect to Hidden Network\n⏻ Turn Off Wi-Fi\n✕ Close" | wofi $WOFI_BASE --width 420 --height 240 --prompt "No Wi-Fi Found Nearby" 2>/dev/null || true)
    if [ "$choice" = "⟳ Scan Again" ]; then
        nmcli dev wifi rescan 2>/dev/null || true
        sleep 1.5
        exec "$0"
    elif [ "$choice" = "⏻ Turn Off Wi-Fi" ]; then
        nmcli radio wifi off
        notify-send "DIAD Network" "Wi-Fi disabled" -i network-wireless-offline
        exit 0
    fi
    exit 0
fi

# 5. Format networks into high-clarity basalt menu
entries=""
while IFS=':' read -r in_use ssid signal security bars; do
    [ -z "$ssid" ] && continue
    mark="  "
    [ "$in_use" = "*" ] && mark="● "
    sec_label="Open"
    [ -n "$security" ] && [ "$security" != "--" ] && sec_label="$security"
    [ -z "$bars" ] && bars="${signal}%"
    line=$(printf "%s%-22s  [%s]  %s" "$mark" "$ssid" "$bars" "$sec_label")
    entries="${entries}${line}\n"
done <<< "$raw_list"

menu_content=$(printf "✕ Close Menu\n⟳ Rescan Networks\n+ Connect to Hidden Network\n⏻ Turn Off Wi-Fi\n──────────────────────────────\n%b" "$entries")

selected=$(printf "%b" "$menu_content" | wofi $WOFI_BASE --width 480 --height 480 --prompt "✦ Select Wi-Fi Network" 2>/dev/null || true)
[ -z "$selected" ] && exit 0

if [ "$selected" = "✕ Close Menu" ] || [ "$selected" = "✕ Close" ]; then
    exit 0
elif [ "$selected" = "⟳ Rescan Networks" ]; then
    notify-send "DIAD Network" "Rescanning channels..." -t 1500 -i network-wireless 2>/dev/null || true
    nmcli dev wifi rescan 2>/dev/null || true
    sleep 1.5
    exec "$0"
elif [ "$selected" = "⏻ Turn Off Wi-Fi" ]; then
    nmcli radio wifi off
    notify-send "DIAD Network" "Wi-Fi disabled" -i network-wireless-offline
    exit 0
elif [ "$selected" = "+ Connect to Hidden Network" ]; then
    hidden_ssid=$(wofi $WOFI_BASE --width 460 --height 130 --prompt "Hidden SSID ➔ [Press Enter]" 2>/dev/null || true)
    [ -z "$hidden_ssid" ] && exit 0
    
    hidden_pass=$(wofi $WOFI_BASE --password --width 460 --height 130 --prompt "Password for $hidden_ssid ➔ [Press Enter]" 2>/dev/null || true)

    notify-send "DIAD Network" "Connecting to hidden network '$hidden_ssid'..." -i network-wireless
    nmcli connection delete id "$hidden_ssid" 2>/dev/null || true
    if [ -n "$hidden_pass" ]; then
        nmcli dev wifi connect "$hidden_ssid" password "$hidden_pass" hidden yes ${WIFI_DEV:+ifname "$WIFI_DEV"} --wait 25 && notify-send "DIAD Network" "✓ Connected to $hidden_ssid" -i network-wireless || notify-send "DIAD Network" "Connection failed for $hidden_ssid" -u critical
    else
        nmcli dev wifi connect "$hidden_ssid" hidden yes ${WIFI_DEV:+ifname "$WIFI_DEV"} --wait 25 && notify-send "DIAD Network" "✓ Connected to $hidden_ssid" -i network-wireless || notify-send "DIAD Network" "Connection failed" -u critical
    fi
    exit 0
elif echo "$selected" | grep -q '───────'; then
    exit 0
fi

# Extract SSID name
is_active=false
if echo "$selected" | grep -q '^● '; then
    is_active=true
fi
clean_ssid=$(echo "$selected" | sed 's/^[● ]*//' | sed 's/  \[.*$//' | xargs)
[ -z "$clean_ssid" ] && exit 0

# If already connected, offer Disconnect or Reconnect
if [ "$is_active" = true ]; then
    ip_addr=$(ip -4 -o addr show | grep -v '127.0.0.1' | awk '{print $4}' | head -n 1 || echo "Acquiring IP...")
    action=$(printf "✕ Close\n✓ Currently Connected (IP: %s)\n⏼ Disconnect from %s\n⟳ Reconnect" "$ip_addr" "$clean_ssid" | wofi $WOFI_BASE --width 440 --height 240 --prompt "$clean_ssid Status" 2>/dev/null || true)
    if [ "$action" = "✕ Close" ] || [ -z "$action" ]; then
        exit 0
    elif echo "$action" | grep -q "Disconnect"; then
        nmcli con down id "$clean_ssid" 2>/dev/null || ( [ -n "$WIFI_DEV" ] && nmcli dev disconnect "$WIFI_DEV" )
        notify-send "DIAD Network" "Disconnected from $clean_ssid" -i network-wireless-offline
    elif echo "$action" | grep -q "Reconnect"; then
        nmcli con up id "$clean_ssid" && notify-send "DIAD Network" "✓ Reconnected to $clean_ssid" -i network-wireless
    fi
    exit 0
fi

# Check if connection profile is already saved
if nmcli -g NAME connection show 2>/dev/null | grep -Fxq "$clean_ssid"; then
    notify-send "DIAD Network" "Connecting to saved network '$clean_ssid'..." -i network-wireless
    if nmcli connection up "$clean_ssid" --wait 20; then
        notify-send "DIAD Network" "✓ Connected to $clean_ssid" -i network-wireless
        exit 0
    else
        notify-send "DIAD Network" "Saved connection failed for '$clean_ssid' — prompting password..." -u normal
        nmcli connection delete id "$clean_ssid" 2>/dev/null || true
    fi
fi

# Check if network is open or password protected
sec_type=$(echo "$selected" | grep -o '\]  .*' | sed 's/\]  //' || echo "")
if [ "$sec_type" = "Open" ] || [ -z "$sec_type" ]; then
    notify-send "DIAD Network" "Connecting to open network '$clean_ssid'..." -i network-wireless
    if nmcli dev wifi connect "$clean_ssid" ${WIFI_DEV:+ifname "$WIFI_DEV"} --wait 20; then
        notify-send "DIAD Network" "✓ Connected to $clean_ssid" -i network-wireless
    else
        notify-send "DIAD Network" "Connection failed for $clean_ssid" -u critical
    fi
else
    # Prompt password: clean dedicated input without dummy dmenu entries
    prompt_text="🔑 Password for $clean_ssid ➔ [Press Enter to Connect]"
    pass=$(wofi --dmenu --password --insensitive --style "$WOFI_STYLE" --width 480 --height 130 --prompt "$prompt_text" 2>/dev/null || true)
    
    # Exit if user cancelled (ESC / empty)
    [ -z "$pass" ] && exit 0
    
    notify-send "DIAD Network" "Authenticating with '$clean_ssid'..." -i network-wireless
    
    # 1. Clean any broken stale profile first
    nmcli connection delete id "$clean_ssid" 2>/dev/null || true
    nmcli connection delete "$clean_ssid" 2>/dev/null || true

    # 2. Attempt direct connection with interface binding
    connect_out=$(nmcli dev wifi connect "$clean_ssid" password "$pass" ${WIFI_DEV:+ifname "$WIFI_DEV"} --wait 25 2>&1 || true)
    
    if echo "$connect_out" | grep -iq "successfully activated\|connected"; then
        notify-send "DIAD Network" "✓ Successfully connected to $clean_ssid" -i network-wireless
    else
        # 3. Fallback: create explicit WPA-PSK profile and bring up
        nmcli connection delete id "$clean_ssid" 2>/dev/null || true
        if [ -n "$WIFI_DEV" ]; then
            nmcli connection add type wifi ifname "$WIFI_DEV" con-name "$clean_ssid" ssid "$clean_ssid" wifi-sec.key-mgmt wpa-psk wifi-sec.psk "$pass" 2>/dev/null || true
        else
            nmcli connection add type wifi con-name "$clean_ssid" ssid "$clean_ssid" wifi-sec.key-mgmt wpa-psk wifi-sec.psk "$pass" 2>/dev/null || true
        fi
        
        fallback_out=$(nmcli connection up "$clean_ssid" --wait 25 2>&1 || true)
        if echo "$fallback_out" | grep -iq "successfully activated\|connected"; then
            notify-send "DIAD Network" "✓ Successfully connected to $clean_ssid" -i network-wireless
        else
            notify-send "DIAD Network" "Authentication failed for '$clean_ssid'. Please verify password." -u critical -i network-wireless-offline
            # Open nmtui in foot terminal as emergency fallback
            foot --app-id=foot --title "Wi-Fi Configuration" -e nmtui >/dev/null 2>&1 &
        fi
    fi
fi
