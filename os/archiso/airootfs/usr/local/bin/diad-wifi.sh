#!/usr/bin/env bash
# =============================================================================
# DIAD — Universal Industrial Basalt & Crimson Wi-Fi Manager
# =============================================================================
set -uo pipefail

WOFI_STYLE="/etc/wofi/style.css"
WOFI_BASE="--dmenu --insensitive --style $WOFI_STYLE"

# 1. Unblock wireless hardware via rfkill
rfkill unblock wifi 2>/dev/null || true
rfkill unblock all 2>/dev/null || true

# 2. Discover primary Wi-Fi interface
WIFI_DEV=$(nmcli -t -f DEVICE,TYPE dev 2>/dev/null | grep ':wifi' | cut -d: -f1 | head -n 1 || echo "")
if [ -z "$WIFI_DEV" ] && command -v iw >/dev/null 2>&1; then
    WIFI_DEV=$(iw dev 2>/dev/null | grep Interface | awk '{print $2}' | head -n 1 || echo "")
fi

if [ -n "$WIFI_DEV" ]; then
    ip link set "$WIFI_DEV" up 2>/dev/null || true
    nmcli dev set "$WIFI_DEV" managed yes 2>/dev/null || true
fi

# 3. Check Wi-Fi radio status
wifi_status=$(nmcli -fields WIFI g 2>/dev/null | tail -n 1 | tr -d ' ' || echo "enabled")
if [ "$wifi_status" = "disabled" ]; then
    choice=$(printf "[S] ⏼ Turn On Wi-Fi\n[X] ✕ Close Menu" | wofi $WOFI_BASE --width 480 --height 180 --prompt "Wi-Fi is Powered Off" 2>/dev/null || true)
    if [[ "$choice" =~ "Turn On" ]]; then
        nmcli radio wifi on
        notify-send "DIAD Network" "Wi-Fi radio turned ON" -i network-wireless
        sleep 1.5
    else
        exit 0
    fi
fi

# 4. Notify scan start and scan available wireless networks
notify-send "DIAD Network" "Scanning Wi-Fi networks..." -t 2000 -i network-wireless 2>/dev/null || true
nmcli dev wifi rescan 2>/dev/null || true
sleep 1.2

raw_list=$(nmcli --terse --fields "IN-USE,SSID,SIGNAL,SECURITY,BARS" dev wifi list 2>/dev/null || true)

if [ -z "$raw_list" ]; then
    sleep 1.0
    raw_list=$(nmcli --terse --fields "IN-USE,SSID,SIGNAL,SECURITY,BARS" dev wifi list 2>/dev/null || true)
fi

if [ -z "$raw_list" ]; then
    choice=$(printf "[S] ⟳ Rescan Networks\n[X] ✕ Close Menu\n[C] + Connect to Hidden Network\n⏻ Turn Off Wi-Fi" | wofi $WOFI_BASE --width 480 --height 240 --prompt "No Networks Found Nearby" 2>/dev/null || true)
    if [[ "$choice" =~ "Rescan" ]] || [ "$choice" = "s" ] || [ "$choice" = "S" ]; then
        exec "$0"
    elif [[ "$choice" =~ "Turn Off" ]]; then
        nmcli radio wifi off
        notify-send "DIAD Network" "Wi-Fi disabled" -i network-wireless-offline
        exit 0
    fi
    exit 0
fi

# 5. Format top networks into compact, clean, non-scrollable basalt menu
entries=""
seen_ssids="|"
count=0

while IFS=':' read -r in_use ssid signal security bars; do
    [ -z "$ssid" ] && continue
    # De-duplicate identical SSIDs (keep strongest)
    if echo "$seen_ssids" | grep -q "|$ssid|"; then
        continue
    fi
    seen_ssids="${seen_ssids}${ssid}|"
    count=$((count + 1))
    [ $count -gt 10 ] && break

    mark="  "
    status_suffix=""
    if [ "$in_use" = "*" ]; then
        mark="● "
        status_suffix=" [Connected]"
    fi
    sec_label="Open"
    [ -n "$security" ] && [ "$security" != "--" ] && sec_label="$security"
    [ -z "$bars" ] && bars="${signal}%"
    
    line=$(printf "%s%-20s   [%s]  %s%s" "$mark" "$ssid" "$bars" "$sec_label" "$status_suffix")
    entries="${entries}${line}\n"
done <<< "$raw_list"

menu_header=$(printf "[S] ⟳ Rescan Networks\n[C] ➔ Connect to Selected Network\n[X] ✕ Close Menu\n──────────────────────────────────────────────\n")
full_menu=$(printf "%b%b" "$menu_header" "$entries")

selected=$(printf "%b" "$full_menu" | wofi $WOFI_BASE --width 560 --height 540 --prompt "✦ Wi-Fi Networks [S:Scan / C:Connect / X:Close]" 2>/dev/null || true)
[ -z "$selected" ] && exit 0

# 6. Handle shortcut and menu actions
if [[ "$selected" =~ "Close Menu" ]] || [ "$selected" = "x" ] || [ "$selected" = "X" ]; then
    exit 0
elif [[ "$selected" =~ "Rescan" ]] || [ "$selected" = "s" ] || [ "$selected" = "S" ]; then
    exec "$0"
elif echo "$selected" | grep -q '───────'; then
    exit 0
fi

# If user clicked "[C] Connect to Selected Network", prompt for SSID or reconnect
if [[ "$selected" =~ "Connect to Selected Network" ]] || [ "$selected" = "c" ] || [ "$selected" = "C" ]; then
    # Filter list for just networks
    net_only=$(printf "%b" "$entries" | wofi $WOFI_BASE --width 560 --height 420 --prompt "Select Network to Connect:" 2>/dev/null || true)
    [ -z "$net_only" ] && exit 0
    selected="$net_only"
fi

# 7. Parse selected network
is_active=false
if echo "$selected" | grep -q '● ' || echo "$selected" | grep -q '\[Connected\]'; then
    is_active=true
fi

clean_ssid=$(echo "$selected" | sed 's/^[● ]*//' | sed 's/   \[.*$//' | sed 's/  \[.*$//' | sed 's/ \[.*$//' | xargs)
[ -z "$clean_ssid" ] && exit 0

# 8. If already connected, display notification back immediately
if [ "$is_active" = true ]; then
    ip_addr=$(ip -4 -o addr show | grep -v '127.0.0.1' | awk '{print $4}' | head -n 1 || echo "Active")
    notify-send "DIAD Network" "✓ Connected to '$clean_ssid' (IP: $ip_addr)" -i network-wireless
    exit 0
fi

# 9. Prompt for password if not connected
prompt_text="User's Password for \"$clean_ssid\"                                                     ↑"
raw_pass=$(printf " " | wofi --dmenu --password --insensitive --style "$WOFI_STYLE" --width 560 --height 120 --prompt "$prompt_text" 2>/dev/null || true)

pass=$(echo -n "$raw_pass" | tr -d '\r\n')
# If user pressed Escape / cancelled
if [ -z "$pass" ] || [ "$pass" = " " ]; then
    exit 0
fi

notify-send "DIAD Network" "Authenticating with '$clean_ssid'..." -i network-wireless

# 10. Clean any broken stale profiles
nmcli connection delete id "$clean_ssid" 2>/dev/null || true
nmcli connection delete "$clean_ssid" 2>/dev/null || true

# 11. Connect attempt 1: Direct dev wifi connect
connect_out=$(nmcli dev wifi connect "$clean_ssid" password "$pass" ${WIFI_DEV:+ifname "$WIFI_DEV"} --wait 30 2>&1 || true)

if echo "$connect_out" | grep -iq "successfully activated\|connected"; then
    notify-send "DIAD Network" "✓ Connected to $clean_ssid" -i network-wireless
else
    # Connect attempt 2: Explicit WPA-PSK profile creation fallback
    nmcli connection delete id "$clean_ssid" 2>/dev/null || true
    if [ -n "$WIFI_DEV" ]; then
        nmcli connection add type wifi ifname "$WIFI_DEV" con-name "$clean_ssid" ssid "$clean_ssid" wifi-sec.key-mgmt wpa-psk wifi-sec.psk "$pass" 2>/dev/null || true
    else
        nmcli connection add type wifi con-name "$clean_ssid" ssid "$clean_ssid" wifi-sec.key-mgmt wpa-psk wifi-sec.psk "$pass" 2>/dev/null || true
    fi
    nmcli connection modify "$clean_ssid" connection.autoconnect yes 2>/dev/null || true
    
    fallback_out=$(nmcli connection up "$clean_ssid" --wait 30 2>&1 || true)
    if echo "$fallback_out" | grep -iq "successfully activated\|connected"; then
        notify-send "DIAD Network" "✓ Connected to $clean_ssid" -i network-wireless
    else
        notify-send "DIAD Network" "Authentication failed for '$clean_ssid'. Please verify password." -u critical -i network-wireless-offline
        # Open nmtui terminal fallback
        foot --app-id=foot --title "Wi-Fi Configuration" -e nmtui >/dev/null 2>&1 &
    fi
fi
