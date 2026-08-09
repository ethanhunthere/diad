#!/usr/bin/env bash
#
# DIAD — First-run onboarding
# =============================================================================
# Interactive setup wizard, shown once inside a foot terminal on first boot:
#   1. AI provider setup (launches `diad-agent --configure`)
#   2. Wi-Fi connection (nmcli; SSID + password entered directly, no agent)
#   3. Timezone (timedatectl)
#   4. Brave Browser (Flatpak — only attempted with a live network connection)
#
# Plain bash — no DIAD Agent dependency. Completion is recorded in
# ~/.diad/.first-run-complete on EVERY exit path (including Ctrl+C) so the
# wizard only ever runs once. Re-run manually with: diad-first-run.sh --force
#
set -uo pipefail

HOME_DIR="${HOME:-/home/diad}"
DIAD_HOME="${HOME_DIR}/.diad"
MARKER="${DIAD_HOME}/.first-run-complete"

# --force re-runs the wizard even after completion
if [[ "${1:-}" == "--force" ]]; then
    rm -f "${MARKER}"
fi

# Record completion on any exit path (incl. Ctrl+C) so the wizard runs once.
mkdir -p "${DIAD_HOME}"
trap 'touch "${MARKER}"' EXIT

# ---- Already completed? ----
if [[ -f "${MARKER}" ]]; then
    exit 0
fi

# ---- Network availability helper (never fails the wizard) ----
have_network() {
    [[ "$(nmcli -t -f STATE g 2>/dev/null)" == "connected" ]]
}

# ---- ASCII logo ----
cat <<'EOF'

   ██████╗ ██╗     █████╗ ██████╗
   ██╔══██╗██║    ██╔══██╗██╔══██╗
   ██║  ██║██║    ███████║██║  ██║
   ██║  ██║██║    ██╔══██║██║  ██║
   ██████╔╝██║    ██║  ██║██████╔╝
   ╚═════╝ ╚═╝    ╚═╝  ╚═╝╚═════╝

   Welcome to DIAD — Intent is the only interface.
   Let's set up your system.

EOF

# ---- 1. AI provider ----
read -r -p "Connect to an AI provider now? [y/N] " connect_ai
if [[ "${connect_ai}" =~ ^[Yy]$ ]]; then
    if command -v diad-agent >/dev/null 2>&1; then
        echo ""
        echo "  Launching the provider wizard (diad-agent --configure)..."
        echo "  Press Ctrl+C at any time to skip."
        echo ""
        diad-agent --configure || echo "  (provider setup skipped or failed)"
    else
        echo ""
        echo "  diad-agent is not available yet — skipping provider setup."
    fi
fi

# ---- 2. Wi-Fi ----
if have_network; then
    echo ""
    echo "  Already connected to a network — skipping Wi-Fi setup."
else
    read -r -p "Connect to a Wi-Fi network? [y/N] " connect_wifi
    if [[ "${connect_wifi}" =~ ^[Yy]$ ]]; then
        echo ""
        echo "  Available networks:"
        sudo systemctl start NetworkManager >/dev/null 2>&1 || true
        nmcli device wifi rescan >/dev/null 2>&1 || true
        nmcli -f SSID,SIGNAL,SECURITY device wifi list 2>/dev/null | head -20 || true
        echo ""
        read -r -p "  Enter the SSID to connect to: " ssid
        if [[ -n "${ssid}" ]]; then
            read -r -s -p "  Enter the Wi-Fi password (hidden): " wifipass
            echo ""
            if sudo nmcli device wifi connect "${ssid}" password "${wifipass}" >/dev/null 2>&1; then
                echo "  Connected to '${ssid}'."
            else
                echo "  Connection failed — retry with: sudo nmcli device wifi connect '${ssid}'"
            fi
        fi
    fi
fi

# ---- 3. Timezone ----
read -r -p "Set timezone (e.g. America/New_York) [skip]: " tz
if [[ -n "${tz}" ]]; then
    if timedatectl list-timezones 2>/dev/null | grep -Fxq "${tz}"; then
        sudo timedatectl set-timezone "${tz}" \
            && echo "  Timezone set to ${tz}." \
            || echo "  Could not set timezone."
    else
        echo "  '${tz}' is not a valid timezone — skipping."
    fi
fi

# ---- 4. Brave Browser (Flatpak) ----
# brave-bin is an AUR-only package, so the ISO installs Brave via Flatpak.
# The /usr/local/bin/brave shim ships in the ISO and resolves to the
# Flatpak runtime, so Sway can keep using the plain 'brave' command.
# Only attempted when a network connection is actually available.
read -r -p "Install the Brave browser (Flatpak, ~300 MB)? [y/N] " install_brave
if [[ "${install_brave}" =~ ^[Yy]$ ]]; then
    if have_network; then
        echo ""
        echo "  Installing Brave via Flatpak (this may take a few minutes)..."
        if flatpak remote-add --user --if-not-exists flathub \
            https://dl.flathub.org/repo/flathub.flatpakrepo >/dev/null 2>&1 \
            && flatpak install --user -y flathub com.brave.Browser; then
            echo "  Brave installed — launch it with 'brave' or ask the DIAD Agent."
        else
            echo "  Brave installation failed. Retry later with:"
            echo "    flatpak install --user -y flathub com.brave.Browser"
        fi
    else
        echo ""
        echo "  No network connection yet — skipping Brave."
        echo "  Later, run: flatpak install --user -y flathub com.brave.Browser"
    fi
fi

echo ""
echo "  Setup complete! Press Enter to continue."
read -r -s -p ""

exit 0
