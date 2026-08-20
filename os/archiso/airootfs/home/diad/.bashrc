# =============================================================================
# DIAD User .bashrc
# Minimal shell configuration for the DIAD user.
# =============================================================================

# If not running interactively, don't do anything
[[ $- != *i* ]] && return

# ---- Environment ----
export PATH="/usr/local/bin:$HOME/.cargo/bin:$PATH"
export EDITOR="diad-agent"
export PAGER="less"

# ---- Aliases ----
alias ls="ls --color=auto"
alias ll="ls -lah"
alias la="ls -A"
alias grep="grep --color=auto"
alias ip="ip -color=auto"

# ---- Prompt ----
PS1='\[\033[01;36m\]\u@diad\[\033[00m\]:\[\033[01;34m\]\w\[\033[00m\]\$ '

# ---- Auto-launch DIAD on first login TTY ----
if [ -z "${WAYLAND_DISPLAY:-}" ] && [ "$(tty)" = "/dev/tty1" ]; then
    echo ""
    echo "  ╔═══════════════════════════════════════════╗"
    echo "  ║         Welcome to DIAD                   ║"
    echo "  ║  Intent-Driven Operating System           ║"
    echo "  ║                                           ║"
    echo "  ║  Launching DIAD Agent...                  ║"
    echo "  ╚═══════════════════════════════════════════╝"
    echo ""

    # Launch Sway (which will auto-start diad-agent + brave)
    if command -v sway &>/dev/null; then
        exec sway
    else
        # Fall back to running diad-agent directly
        exec diad-agent
    fi
fi

# ---- History Settings ----
HISTSIZE=10000
HISTFILESIZE=20000
HISTCONTROL=ignoreboth:erasedups
shopt -s histappend

# ---- Completion ----
if [ -f /usr/share/bash-completion/bash_completion ]; then
    . /usr/share/bash-completion/bash_completion
fi
