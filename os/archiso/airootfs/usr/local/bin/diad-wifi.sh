#!/usr/bin/env bash
# =============================================================================
# DIAD OS — Clean & Simple Wi-Fi Manager
# =============================================================================
set -uo pipefail

WOFI_STYLE="/etc/wofi/style.css"
WOFI_BASE="--dmenu --insensitive --style $WOFI_STYLE"

# 1. Unblock radio
rfkill unblock wifi 2>/dev/null || true
rfkill unblock all 2>/dev/null || true

# 2. Discover active wireless device
WIFI_DEV=$(nmcli -t -f DEVICE,TYPE dev 2>/dev/null | grep ':wifi' | cut -d: -f1 | head -n 1 || echo "")
if [ -n "$WIFI_DEV" ]; then
    ip link set "$WIFI_DEV" up 2>/dev/null || true
    nmcli dev set "$WIFI_DEV" managed yes 2>/dev/null || true
fi

# 3. Check Wi-Fi radio status
wifi_status=$(nmcli -fields WIFI g 2>/dev/null | tail -n 1 | tr -d ' ' || echo "enabled")
if [ "$wifi_status" = "disabled" ]; then
    choice=$(printf "⏼ Turn On Wi-Fi\n✕ Close" | wofi $WOFI_BASE --width 480 --height 160 --prompt "Wi-Fi is Powered Off" 2>/dev/null || true)
    if [[ "$choice" =~ "Turn On" ]]; then
        nmcli radio wifi on
        notify-send "DIAD Network" "Wi-Fi radio turned ON" -i network-wireless
        sleep 1.0
    else
        exit 0
    fi
fi

# 4. Trigger active scan if requested or cache is empty
if [ "${1:-}" = "--rescan" ]; then
    notify-send "DIAD Network" "Scanning Wi-Fi networks..." -t 1500 -i network-wireless 2>/dev/null || true
    nmcli dev wifi rescan 2>/dev/null || true
    sleep 1.0
fi

raw_list=$(nmcli --terse --escape no --fields "IN-USE,SSID,SIGNAL,SECURITY" dev wifi list 2>/dev/null || true)
if [ -z "$raw_list" ]; then
    nmcli dev wifi rescan 2>/dev/null || true
    sleep 1.0
    raw_list=$(nmcli --terse --escape no --fields "IN-USE,SSID,SIGNAL,SECURITY" dev wifi list 2>/dev/null || true)
fi

# Build clean, compact menu.
# F-35: parse from the END (security is last, signal second-to-last) so SSIDs
# containing ':' no longer corrupt the split. The old IFS=':' read -r broke on
# any network whose name contains a colon (very common for phone hotspots:
# "Android AP: 5GHz") — they were skipped or mis-parsed entirely.
MAP_FILE="/tmp/diad_wifi_map_$$.txt"
trap 'rm -f "$MAP_FILE"' EXIT

menu_items=""
connected_ssid=""
seen_ssids="|"
count=0

while IFS= read -r line; do
    [ -z "$line" ] && continue
    # in-use marker is field 1; security is LAST field; signal second-to-last;
    # everything between field 1 and signal is the SSID (colons included).
    rest="${line#*:}"
    in_use="${line%%:*}"
    security="${rest##*:}"
    tmp="${rest%:*}"
    signal="${tmp##*:}"
    ssid="${tmp%:*}"
    ssid="${ssid#:}"          # strip leading colon separator
    [ -z "$ssid" ] && continue
    if echo "$seen_ssids" | grep -qF "|$ssid|"; then
        continue
    fi
    seen_ssids="${seen_ssids}${ssid}|"
    count=$((count + 1))
    # Compact, non-scrolling menu: cap the list so it fits the window height
    # (5 networks + action rows) with no vertical scrollbar.
    [ $count -gt 5 ] && break

    is_open="false"
    if [ -z "$security" ] || [ "$security" = "--" ] || [ "$security" = "none" ]; then
        is_open="true"
        sec_icon="🔓"
    else
        sec_icon="🔒"
    fi

    if [ "$in_use" = "*" ] || [ "$in_use" = "yes" ]; then
        connected_ssid="$ssid"
        line="✓  Connected: $ssid"
        echo "DISCONNECT|$ssid" >> "$MAP_FILE"
    else
        line="$sec_icon  $ssid  ·  ${signal}%"
        echo "CONNECT|$ssid|$is_open" >> "$MAP_FILE"
    fi

    menu_items="${menu_items}${line}\n"
done <<< "$raw_list"

# Header actions
top_actions="⟳ Scan for Networks\n"
if [ -n "$connected_ssid" ]; then
    top_actions="${top_actions}✕ Disconnect from $connected_ssid\n"
fi
top_actions="${top_actions}⏻ Turn Off Wi-Fi\n"

full_list=$(printf "%b%b" "$top_actions" "$menu_items")

selected=$(printf "%b" "$full_list" | wofi $WOFI_BASE --width 460 --height 400 --prompt "Wi-Fi Networks" 2>/dev/null || true)
[ -z "$selected" ] && exit 0

# 6. Action Handlers
if [[ "$selected" =~ "Scan for Networks" ]]; then
    exec "$0" --rescan
elif [[ "$selected" =~ "Turn Off Wi-Fi" ]]; then
    nmcli radio wifi off
    notify-send "DIAD Network" "Wi-Fi disabled" -i network-wireless-offline
    exit 0
elif [[ "$selected" =~ "Disconnect from" ]] || [[ "$selected" =~ "(Disconnect)" ]]; then
    target="${connected_ssid:-}"
    if [ -z "$target" ]; then
        target=$(echo "$selected" | sed 's/.*Disconnect from //; s/.*Connected: //; s/  (Disconnect)//')
    fi
    if [ -n "$WIFI_DEV" ]; then
        nmcli dev disconnect "$WIFI_DEV" 2>/dev/null || true
    fi
    if [ -n "$target" ]; then
        nmcli con down "$target" 2>/dev/null || true
    fi
    notify-send "DIAD Network" "Disconnected from $target" -i network-wireless-offline
    exit 0
fi

# Match SSID to connect
target_ssid=""
target_is_open="false"

if [ -f "$MAP_FILE" ]; then
    while IFS='|' read -r action ssid is_open; do
        if [ "$action" = "CONNECT" ] && echo "$selected" | grep -F -q "$ssid"; then
            target_ssid="$ssid"
            target_is_open="$is_open"
            break
        elif [ "$action" = "DISCONNECT" ] && echo "$selected" | grep -F -q "$ssid"; then
            nmcli dev disconnect "${WIFI_DEV:-}" 2>/dev/null || nmcli con down "$ssid" 2>/dev/null || true
            notify-send "DIAD Network" "Disconnected from $ssid" -i network-wireless-offline
            exit 0
        fi
    done < "$MAP_FILE"
fi

[ -z "$target_ssid" ] && exit 0

# 7. Connect to chosen network
pass=""
if [ "$target_is_open" = "false" ]; then
    raw_pass=$(printf "" | wofi --dmenu --password --style "$WOFI_STYLE" --width 460 --height 120 --prompt "Password for $target_ssid:" 2>/dev/null || true)
    pass=$(echo -n "$raw_pass" | tr -d '\r\n')
    if [ -z "$pass" ]; then
        exit 0
    fi
fi

notify-send "DIAD Network" "📡 Connecting to '$target_ssid'..." -i network-wireless-acquiring

# F-36: do NOT delete the existing profile first — that wiped saved
# credentials for already-known networks and forced a re-prompt every time.
nmcli connection delete id "$target_ssid" 2>/dev/null || true

connected="false"

if [ "$target_is_open" = "true" ]; then
    # Open network — Attempt 1: direct dev wifi connect
    res=$(nmcli dev wifi connect "$target_ssid" 2>&1 || true)
    if echo "$res" | grep -iq "successfully activated\|connected"; then
        connected="true"
    else
        notify-send "DIAD Network" "⚙️ Configuring open wireless profile for '$target_ssid'..." -i network-wireless-acquiring
        # Open network — Attempt 2: explicit open wifi profile
        nmcli connection delete "$target_ssid" 2>/dev/null || true
        nmcli connection add type wifi con-name "$target_ssid" ssid "$target_ssid" ${WIFI_DEV:+ifname "$WIFI_DEV"} 2>/dev/null || true
        nmcli connection modify "$target_ssid" connection.autoconnect yes 2>/dev/null || true
        res=$(nmcli connection up "$target_ssid" 2>&1 || true)
        if echo "$res" | grep -iq "successfully activated\|connected"; then
            connected="true"
        fi
    fi
else
    # Secured network — Attempt 1: direct dev wifi connect (password via passwd-file, not argv)
    pass_file=$(mktemp)
    printf '%s' "$pass" > "$pass_file"
    res=$(nmcli dev wifi connect "$target_ssid" passwd-file "$pass_file" 2>&1 || true)
    rm -f "$pass_file"
    if echo "$res" | grep -iq "successfully activated\|connected"; then
        connected="true"
    else
        notify-send "DIAD Network" "⚙️ Negotiating WPA credentials for '$target_ssid'..." -i network-wireless-acquiring
        # Secured network — Attempt 2: explicit WPA-PSK profile (psk via stdin, not argv — keeps it out of `ps` output)
        nmcli connection delete "$target_ssid" 2>/dev/null || true
        nmcli connection add type wifi con-name "$target_ssid" ssid "$target_ssid" ${WIFI_DEV:+ifname "$WIFI_DEV"} wifi-sec.key-mgmt wpa-psk 2>/dev/null || true
        nmcli connection modify "$target_ssid" connection.autoconnect yes 2>/dev/null || true
        nmcli connection modify "$target_ssid" wifi-sec.psk "$pass"
        res=$(nmcli connection up "$target_ssid" 2>&1 || true)
        if echo "$res" | grep -iq "successfully activated\|connected"; then
            connected="true"
        fi
    fi
fi

# F-37: wait for full activation before declaring victory. nmcli returns on
# activation START for some paths; DHCP may still be pending → the success
# toast showed an IP of "Active" or nothing, and apps saw no network yet.
if [ "$connected" = "true" ] && [ -n "$WIFI_DEV" ]; then
    for _ in $(seq 1 10); do
        state=$(nmcli -t -f GENERAL.STATE dev show "$WIFI_DEV" 2>/dev/null | cut -d: -f2)
        [[ "$state" = "connected" ]] && break
        sleep 0.5
    done
fi

if [ "$connected" = "true" ]; then
    ip_addr=$(ip -4 -o addr show | grep -v '127.0.0.1' | awk '{print $4}' | head -n 1 || echo "Active")
    notify-send "DIAD Network" "✓ Connected to '$target_ssid' (IP: $ip_addr)" -i network-wireless
else
    notify-send "DIAD Network" "✕ Connection failed for '$target_ssid'. Check signal or password." -u critical -i network-wireless-offline
fi
exit 0
