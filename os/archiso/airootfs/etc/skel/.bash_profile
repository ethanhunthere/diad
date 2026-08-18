# =============================================================================
# DIAD User .bash_profile (skel)
# Auto-launch Sway window manager on login with crash backoff guard (audit R-9)
# =============================================================================

if [ -z "${WAYLAND_DISPLAY:-}" ] && [ "$(tty)" = "/dev/tty1" ]; then
    now=$(date +%s); last=0; [ -f /tmp/.diad-sway-last ] && last=$(cat /tmp/.diad-sway-last 2>/dev/null || echo 0)
    if [ $((now - last)) -lt 10 ]; then
        echo "sway exited rapidly — waiting 10s (Ctrl+C for shell)..."
        sleep 10
    fi
    echo "$now" > /tmp/.diad-sway-last
    exec sway >/tmp/sway.log 2>&1
fi

if [ -f ~/.bashrc ]; then
    . ~/.bashrc
fi
