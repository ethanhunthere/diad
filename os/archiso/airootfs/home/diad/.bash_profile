# =============================================================================
# DIAD User .bash_profile
# Auto-launch Sway window manager on tty1 login
# =============================================================================

if [ -z "${WAYLAND_DISPLAY:-}" ] && [ "$(tty)" = "/dev/tty1" ]; then
    export WLR_NO_HARDWARE_CURSORS=1
    exec sway >/tmp/sway.log 2>&1
fi

if [ -f ~/.bashrc ]; then
    . ~/.bashrc
fi
