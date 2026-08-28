# =============================================================================
# DIAD User .bash_profile
# Auto-launch Sway window manager on tty1 login
# =============================================================================

if [ -z "${WAYLAND_DISPLAY:-}" ]; then
    clear
    export XDG_CURRENT_DESKTOP=sway
    export XDG_SESSION_DESKTOP=sway
    export XDG_SESSION_TYPE=wayland
    export WLR_NO_HARDWARE_CURSORS=1
    export WLR_RENDERER_ALLOW_SOFTWARE=1
    export WLR_DRM_NO_MODIFIERS=1

    exec sway --unsupported-gpu >~/.sway.log 2>&1
fi

if [ -f ~/.bashrc ]; then
    . ~/.bashrc
fi
