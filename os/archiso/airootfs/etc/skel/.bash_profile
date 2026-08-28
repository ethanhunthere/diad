# =============================================================================
# DIAD User .bash_profile
# Auto-launch Sway window manager on tty1 login
# =============================================================================

if [ -z "${WAYLAND_DISPLAY:-}" ] && [ "$(tty 2>/dev/null)" = "/dev/tty1" ]; then
    clear
    export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
    mkdir -p "${XDG_RUNTIME_DIR}"
    chmod 0700 "${XDG_RUNTIME_DIR}"
    export XDG_CURRENT_DESKTOP=sway
    export XDG_SESSION_DESKTOP=sway
    export XDG_SESSION_TYPE=wayland
    export WLR_NO_HARDWARE_CURSORS=1
    export WLR_RENDERER_ALLOW_SOFTWARE=1
    export WLR_DRM_NO_MODIFIERS=1

    sway --unsupported-gpu >>~/.sway.log 2>&1

    echo "========================================================"
    echo "DIAD OS: Sway compositor exited. Log saved to ~/.sway.log"
    tail -n 10 ~/.sway.log 2>/dev/null || true
    echo "========================================================"
    echo "Starting interactive fallback shell..."
    exec bash
fi

if [ -f ~/.bashrc ]; then
    . ~/.bashrc
fi
