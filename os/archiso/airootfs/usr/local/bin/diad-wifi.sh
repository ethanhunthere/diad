#!/usr/bin/env bash
# =============================================================================
# DIAD — Universal Glassy Wi-Fi Network Manager (2040 Edition)
# =============================================================================
set -euo pipefail

WOFI_OPTS="--dmenu --insensitive --style /etc/wofi/style.css --width 460 --height 440 --prompt '✦ Wi-Fi Networks'"

# Ensure rfkill unblocked
rfkill unblock wifi 2>/dev/null || true
rfkill unblock all 2>/dev/null || true

# Check Wi-Fi radio status
wifi_status=$(nmcli -fields WIFI g 2>/dev/null | tail -n 1 | tr -d ' ' || echo "enabled")
if [ "$wifi_status" = "disabled" ]; then
    choice=$(printf "⏼ Turn On Wi-Fi\nCancel" | wofi --dmenu --insensitive --style /etc/wofi/style.css --width 360 --height 180 --prompt "Wi-Fi is Powered Off")
    if [ "$choice" = "⏼ Turn On Wi-Fi" ]; then
        nmcli radio wifi on
        notify-send "DIAD Network" "Wi-Fi radio turned ON" -i network-wireless
        sleep 1
    else
        exit 0
    fi
fi

# Ensure wireless interfaces are UP and managed
for dev in $(nmcli -t -f DEVICE,TYPE dev 2>/dev/null | grep ':wifi' | cut -d: -f1); do
    ip link set "$dev" up 2>/dev/null || true
    nmcli dev set "$dev" managed yes 2>/dev/null || true
done

# Retrieve network list with intelligent async scan polling
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
    choice=$(printf "⟳ Scan Again\n+ Connect to Hidden Network\n⏻ Turn Off Wi-Fi" | wofi --dmenu --insensitive --style /etc/wofi/style.css --width 400 --height 220 --prompt "No Wi-Fi Found Nearby")
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

# Build high-clarity formatted list
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

menu_content=$(printf "⟳ Rescan Networks\n+ Connect to Hidden Network\n⏻ Turn Off Wi-Fi\n──────────────────────────────\n%b" "$entries")

selected=$(printf "%b" "$menu_content" | eval wofi $WOFI_OPTS 2>/dev/null || true)
[ -z "$selected" ] && exit 0

if [ "$selected" = "⟳ Rescan Networks" ]; then
    notify-send "DIAD Network" "Rescanning channels..." -t 1500 -i network-wireless 2>/dev/null || true
    nmcli dev wifi rescan 2>/dev/null || true
    sleep 1.5
    exec "$0"
elif [ "$selected" = "⏻ Turn Off Wi-Fi" ]; then
    nmcli radio wifi off
    notify-send "DIAD Network" "Wi-Fi disabled" -i network-wireless-offline
    exit 0
elif [ "$selected" = "+ Connect to Hidden Network" ]; then
    hidden_ssid=$(wofi --dmenu --insensitive --style /etc/wofi/style.css --width 400 --height 180 --prompt "Hidden SSID Name" 2>/dev/null || true)
    [ -z "$hidden_ssid" ] && exit 0
    hidden_pass=$(wofi --dmenu --password --insensitive --style /etc/wofi/style.css --width 400 --height 180 --prompt "Password for $hidden_ssid" 2>/dev/null || true)
    notify-send "DIAD Network" "Connecting to hidden network '$hidden_ssid'..." -i network-wireless
    if [ -n "$hidden_pass" ]; then
        nmcli dev wifi connect "$hidden_ssid" password "$hidden_pass" hidden yes && notify-send "DIAD Network" "Connected to $hidden_ssid" -i network-wireless || notify-send "DIAD Network" "Connection failed" -u critical
    else
        nmcli dev wifi connect "$hidden_ssid" hidden yes && notify-send "DIAD Network" "Connected to $hidden_ssid" -i network-wireless || notify-send "DIAD Network" "Connection failed" -u critical
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

# If already connected, offer Disconnect or Status
if [ "$is_active" = true ]; then
    ip_addr=$(ip -4 -o addr show | grep -v '127.0.0.1' | awk '{print $4}' | head -n 1 || echo "Acquiring IP...")
    action=$(printf "✓ Currently Connected (IP: %s)\n⏼ Disconnect from %s\n⟳ Reconnect" "$ip_addr" "$clean_ssid" | wofi --dmenu --insensitive --style /etc/wofi/style.css --width 420 --height 200 --prompt "$clean_ssid Status")
    if echo "$action" | grep -q "Disconnect"; then
        nmcli con down id "$clean_ssid" 2>/dev/null || nmcli dev disconnect $(nmcli -t -f DEVICE,TYPE dev | grep ':wifi' | cut -d: -f1 | head -n 1)
        notify-send "DIAD Network" "Disconnected from $clean_ssid" -i network-wireless-offline
    elif echo "$action" | grep -q "Reconnect"; then
        nmcli con up id "$clean_ssid" && notify-send "DIAD Network" "Reconnected to $clean_ssid" -i network-wireless
    fi
    exit 0
fi

# Check if connection profile is already saved
if nmcli -g NAME connection show 2>/dev/null | grep -Fxq "$clean_ssid"; then
    notify-send "DIAD Network" "Connecting to saved network $clean_ssid..." -i network-wireless
    if nmcli connection up "$clean_ssid"; then
        notify-send "DIAD Network" "Connected to $clean_ssid" -i network-wireless
        exit 0
    else
        notify-send "DIAD Network" "Failed to connect to $clean_ssid — retrying with fresh auth..." -u normal
        nmcli connection delete "$clean_ssid" 2>/dev/null || true
    fi
fi

# Check if network is open or password protected
sec_type=$(echo "$selected" | grep -o '\]  .*' | sed 's/\]  //' || echo "")
if [ "$sec_type" = "Open" ] || [ -z "$sec_type" ]; then
    notify-send "DIAD Network" "Connecting to $clean_ssid..." -i network-wireless
    if nmcli dev wifi connect "$clean_ssid"; then
        notify-send "DIAD Network" "Connected to $clean_ssid" -i network-wireless
    else
        notify-send "DIAD Network" "Connection failed" -u critical
    fi
else
    # Prompt password
    pass=$(wofi --dmenu --password --insensitive --style /etc/wofi/style.css --width 420 --height 180 --prompt "Enter password for $clean_ssid" 2>/dev/null || true)
    if [ -z "$pass" ]; then
        exit 0
    fi
    notify-send "DIAD Network" "Authenticating with $clean_ssid..." -i network-wireless
    if nmcli dev wifi connect "$clean_ssid" password "$pass"; then
        notify-send "DIAD Network" "Successfully connected to $clean_ssid" -i network-wireless
    else
        notify-send "DIAD Network" "Authentication failed for $clean_ssid" -u critical -i network-wireless-offline
        # Launch nmtui fallback for manual connection
        foot --app-id=foot --title "Wi-Fi Connections" -e nmtui >/dev/null 2>&1 &
    fi
fi
