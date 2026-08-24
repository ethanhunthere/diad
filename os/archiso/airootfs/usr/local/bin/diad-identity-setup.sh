#!/usr/bin/env bash
# =============================================================================
# DIAD — First-boot identity setup (username + password)
# =============================================================================
# Runs ONCE as root on tty1 BEFORE getty/autologin starts. Asks the user for:
#   1. Username   (Enter keeps the factory default "diad")
#   2. Password   (min 8 chars, asked twice; blank = keep shipped default)
#
# That password becomes THE account password — used by sudo (via the pinned
# wrappers that need it), the web lockscreen (PAM through diad-lock-auth),
# and any console login.
#
# If a custom username is given, the account is properly renamed:
#   usermod/groupmod + home move, sudoers grant rewritten, autologin updated.
# Every privileged wrapper derives the user dynamically afterwards, so the
# rename is transparent to the agent/lockscreen/tools.
#
# Completion marker: /var/lib/diad/identity-setup (skips on every later boot).
# =============================================================================

set -u

MARKER="/var/lib/diad/identity-setup"
DEFAULT_USER="diad"

[[ $EUID -eq 0 ]] || { echo "must run as root" >&2; exit 1; }

if [[ -f "${MARKER}" ]]; then
    exit 0
fi

mkdir -p /var/lib/diad

cat <<'EOF'

   ██████╗ ██╗     █████╗ ██████╗
   ██╔══██╗██║    ██╔══██╗██╔══██╗
   ██║  ██╗██║    ██╔══██╗██║  ██║
   ██║  ██║██║    ███████║██║  ██║
   ╚██████╔╝██╗   ██║    ██████╔╝
    ╚═════╝ ╚═╝   ╚═╝    ╚═════╝

   Welcome to DIAD OS — Intent is the only interface.

   One-time setup: choose your account name and password.
   This password protects your session (lockscreen) and system changes.

EOF

# ---- Username ---------------------------------------------------------------
NAME_RE='^[a-z_][a-z0-9_-]{0,31}$'
read -r -p "Username [${DEFAULT_USER}]: " USERNAME
USERNAME="${USERNAME:-${DEFAULT_USER}}"

until [[ "${USERNAME}" =~ ${NAME_RE} ]]; do
    echo "  Invalid username (lowercase letters, digits, '-', '_')." >&2
    read -r -p "Username [${DEFAULT_USER}]: " USERNAME
    USERNAME="${USERNAME:-${DEFAULT_USER}}"
done

if id "${USERNAME}" &>/dev/null && [[ "${USERNAME}" != "${DEFAULT_USER}" ]]; then
    echo "  User '${USERNAME}' already exists — keeping '${DEFAULT_USER}'." >&2
    USERNAME="${DEFAULT_USER}"
fi

# ---- Password ----------------------------------------------------------------
while true; do
    read -r -s -p "Password (min 8 chars, blank = keep factory default): " PASS1
    echo ""
    if [[ -z "${PASS1}" ]]; then
        PASSWORD_SET=0
        break
    fi
    if [[ ${#PASS1} -lt 8 ]]; then
        echo "  Too short — at least 8 characters." >&2
        continue
    fi
    read -r -s -p "Confirm password: " PASS2
    echo ""
    if [[ "${PASS1}" == "${PASS2}" ]]; then
        PASSWORD_SET=1
        break
    fi
    echo "  Passwords do not match — try again." >&2
done

# ---- Apply: rename -----------------------------------------------------------
renamed=0
if [[ "${USERNAME}" != "${DEFAULT_USER}" ]]; then
    # Rename account, group, and move the home directory atomically-ish.
    if ! usermod -l "${USERNAME}" -d "/home/${USERNAME}" -m "${DEFAULT_USER}"; then
        echo "  WARNING: rename failed — continuing as '${DEFAULT_USER}'." >&2
        USERNAME="${DEFAULT_USER}"
    else
        groupmod -n "${USERNAME}" "${DEFAULT_USER}" 2>/dev/null || true
        # Rewrite ONLY the user token of every sudoers grant line
        # ("diad ALL=..." -> "<user> ALL=..."). Binary paths (diad-pacman,
        # diad-mount, ...) are untouched because they never start a line.
        sed -i "s/^diad ALL=/${USERNAME} ALL=/" /etc/sudoers.d/diad
        chmod 0440 /etc/sudoers.d/diad
        # Autologin must log the renamed user in.
        sed -i "s/--autologin ${DEFAULT_USER} /--autologin ${USERNAME} /" \
            /etc/systemd/system/getty@tty1.service.d/autologin.conf \
            /etc/systemd/system/getty@.service.d/autologin.conf 2>/dev/null
        visudo -cf /etc/sudoers.d/diad >/dev/null 2>&1 \
            || echo "  WARNING: sudoers validation failed — check /etc/sudoers.d/diad" >&2
        renamed=1
    fi
fi

# ---- Apply: password ----------------------------------------------------------
if [[ "${PASSWORD_SET:-0}" -eq 1 ]]; then
    printf '%s:%s\n' "${USERNAME}" "${PASS1}" | chpasswd
    echo ""
    echo "  ✔ Password set for '${USERNAME}'."
elif [[ ${renamed} -eq 1 ]]; then
    echo ""
    echo "  ✔ Account renamed to '${USERNAME}' (password unchanged)."
else
    echo ""
    echo "  · Keeping factory defaults."
fi

echo ""
echo "  Your password unlocks the screen and authorizes system actions."
echo "  Continuing to the desktop..."

mkdir -p /var/lib/diad
touch "${MARKER}"
sleep 2
exit 0
