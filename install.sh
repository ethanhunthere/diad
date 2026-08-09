#!/usr/bin/env bash
#
# DIAD OS — Installation Script
# Bootstraps a minimal Arch Linux system with Sway, Brave, and the DIAD Agent.
#
# Usage: sudo ./install.sh
#
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

# ---- Pre-flight checks ----
if [[ $EUID -ne 0 ]]; then
    log_error "This script must be run as root (sudo ./install.sh)"
    exit 1
fi

if ! command -v pacman &>/dev/null; then
    log_error "This script requires Arch Linux (pacman not found)"
    exit 1
fi

INSTALL_USER="${SUDO_USER:-$USER}"
if [[ "$INSTALL_USER" == "root" ]]; then
    log_error "Do not run as root directly. Use: sudo ./install.sh"
    exit 1
fi

USER_HOME=$(eval echo "~$INSTALL_USER")
DIAD_DIR="$(cd "$(dirname "$0")" && pwd)"

log_info "DIAD Installation starting..."
log_info "Target user: $INSTALL_USER"
log_info "User home:   $USER_HOME"

# ---- Step 1: Update System ----
log_info "Step 1/6: Updating package database..."
pacman -Syu --noconfirm

# ---- Step 2: Install Base Packages ----
log_info "Step 2/6: Installing base system packages..."
pacman -S --noconfirm \
    base \
    linux-lts \
    linux-firmware \
    networkmanager \
    sudo \
    git \
    curl \
    jq \
    wget \
    vim \
    unzip

# ---- Step 3: Install Sway & Wayland Components ----
log_info "Step 3/6: Installing Sway WM and Wayland components..."
pacman -S --noconfirm \
    sway \
    swaybg \
    swayidle \
    swaylock \
    waybar \
    wofi \
    foot \
    grim \
    slurp \
    wl-clipboard \
    xdg-desktop-portal-wlr \
    polkit \
    playerctl \
    nftables \
    upower \
    power-profiles-daemon \
    tlp

# ---- Step 4: Install Brave Browser ----
log_info "Step 4/6: Installing Brave Browser..."
if command -v brave &>/dev/null; then
    log_info "Brave is already installed."
else
    # Prefer brave-bin from AUR; fall back to the official Flatpak
    if command -v yay &>/dev/null; then
        sudo -u "$INSTALL_USER" yay -S --noconfirm brave-bin
    elif command -v paru &>/dev/null; then
        sudo -u "$INSTALL_USER" paru -S --noconfirm brave-bin
    fi

    if ! command -v brave &>/dev/null; then
        log_warn "AUR install unavailable. Falling back to the official Flatpak..."
        flatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
        flatpak install -y flathub com.brave.Browser
        # Flatpak does not provide a 'brave' command — install a launcher shim
        if ! command -v brave &>/dev/null; then
            printf '#!/usr/bin/env sh\nexec /usr/bin/flatpak run com.brave.Browser "$@"\n' \
                | sudo tee /usr/local/bin/brave >/dev/null
            sudo chmod 0755 /usr/local/bin/brave
        fi
    fi

    if ! command -v brave &>/dev/null; then
        log_warn "No AUR helper and no Flatpak. Installing brave-bin via manual build..."
        mkdir -p /tmp/diad-aur
        cd /tmp/diad-aur
        sudo -u "$INSTALL_USER" git clone https://aur.archlinux.org/brave-bin.git
        cd brave-bin
        sudo -u "$INSTALL_USER" makepkg -si --noconfirm
        cd "$DIAD_DIR"
        rm -rf /tmp/diad-aur
    fi
fi

# ---- Step 5: Install Rust Toolchain ----
log_info "Step 5/6: Installing Rust toolchain..."
if command -v rustup &>/dev/null; then
    log_info "Rustup is already installed."
else
    sudo -u "$INSTALL_USER" bash -c "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
fi
export PATH="$USER_HOME/.cargo/bin:$PATH"

# Install additional build dependencies
pacman -S --noconfirm \
    base-devel \
    libsecret \
    gnome-keyring \
    pacman-contrib \
    cmake \
    pkg-config \
    openssl \
    fontconfig

# ---- Step 6: Install DIAD Agent ----
log_info "Step 6/6: Building DIAD Agent..."
sudo -u "$INSTALL_USER" bash -c "cd '$DIAD_DIR/agent' && source '$USER_HOME/.cargo/env' && cargo build --release"

# Install binary
cp "$DIAD_DIR/agent/target/release/diad-agent" /usr/local/bin/diad-agent
chmod 755 /usr/local/bin/diad-agent

# ---- Install Configuration ----
log_info "Installing system configuration..."
mkdir -p /etc/diad
if [[ -f "$DIAD_DIR/os/archiso/airootfs/etc/diad/diad.conf" ]]; then
    cp "$DIAD_DIR/os/archiso/airootfs/etc/diad/diad.conf" /etc/diad/diad.conf
fi

# ---- Install Sway Config ----
mkdir -p /etc/skel/.config/sway
mkdir -p "$USER_HOME/.config/sway"
mkdir -p /etc/sway/conf.d
if [[ -f "$DIAD_DIR/os/archiso/airootfs/etc/sway/config" ]]; then
    cp "$DIAD_DIR/os/archiso/airootfs/etc/sway/config" /etc/skel/.config/sway/config
    cp "$DIAD_DIR/os/archiso/airootfs/etc/sway/config" "$USER_HOME/.config/sway/config"
    chown -R "$INSTALL_USER:$INSTALL_USER" "$USER_HOME/.config/sway"
fi
if [[ -f "$DIAD_DIR/os/archiso/airootfs/etc/sway/conf.d/media-keys.conf" ]]; then
    cp "$DIAD_DIR/os/archiso/airootfs/etc/sway/conf.d/media-keys.conf" /etc/sway/conf.d/media-keys.conf
    chmod 644 /etc/sway/conf.d/media-keys.conf
fi

# ---- Install Waybar Config ----
log_info "Installing Waybar configuration..."
mkdir -p /etc/waybar
if [[ -f "$DIAD_DIR/os/archiso/airootfs/etc/waybar/config" ]]; then
    cp "$DIAD_DIR/os/archiso/airootfs/etc/waybar/config" /etc/waybar/config
    chmod 644 /etc/waybar/config
fi
if [[ -f "$DIAD_DIR/os/archiso/airootfs/etc/waybar/style.css" ]]; then
    cp "$DIAD_DIR/os/archiso/airootfs/etc/waybar/style.css" /etc/waybar/style.css
    chmod 644 /etc/waybar/style.css
fi

# ---- Install Sudoers (NOPASSWD for DIAD system-control tools) ----
log_info "Installing sudoers.d/diad..."
if [[ -f "$DIAD_DIR/os/archiso/airootfs/etc/sudoers.d/diad" ]]; then
    cp "$DIAD_DIR/os/archiso/airootfs/etc/sudoers.d/diad" /etc/sudoers.d/diad
    chown root:root /etc/sudoers.d/diad
    chmod 440 /etc/sudoers.d/diad
    if command -v visudo &>/dev/null; then
        visudo -cf /etc/sudoers.d/diad && log_info "sudoers.d/diad validated OK."
    fi
fi

# ---- Install Launch Scripts (DIAD setup scripts) ----
for script in diad-first-run.sh diad-supervisor.sh diad-hardware-setup.sh diad-display-setup.sh diad-network-setup.sh diad-power-setup.sh diad-storage-setup.sh diad-splash.sh; do
    if [[ -f "$DIAD_DIR/os/archiso/airootfs/usr/local/bin/$script" ]]; then
        cp "$DIAD_DIR/os/archiso/airootfs/usr/local/bin/$script" "/usr/local/bin/$script"
        chmod 755 "/usr/local/bin/$script"
        log_info "Installed $script"
    fi
done

# ---- Install NetworkManager Config ----
log_info "Installing NetworkManager configuration..."
mkdir -p /etc/NetworkManager/conf.d
if [[ -f "$DIAD_DIR/os/archiso/airootfs/etc/NetworkManager/conf.d/diad.conf" ]]; then
    cp "$DIAD_DIR/os/archiso/airootfs/etc/NetworkManager/conf.d/diad.conf" /etc/NetworkManager/conf.d/diad.conf
    chmod 644 /etc/NetworkManager/conf.d/diad.conf
fi

# ---- Install Firewall (nftables + systemd unit) ----
log_info "Installing nftables firewall..."
if [[ -f "$DIAD_DIR/os/archiso/airootfs/etc/nftables.conf" ]]; then
    cp "$DIAD_DIR/os/archiso/airootfs/etc/nftables.conf" /etc/nftables.conf
    chmod 644 /etc/nftables.conf
fi
if [[ -f "$DIAD_DIR/os/archiso/airootfs/etc/systemd/system/diad-firewall.service" ]]; then
    cp "$DIAD_DIR/os/archiso/airootfs/etc/systemd/system/diad-firewall.service" /etc/systemd/system/diad-firewall.service
    chmod 644 /etc/systemd/system/diad-firewall.service
fi

# ---- Install Persistent Storage (fstab.d + setup script) ----
log_info "Installing persistent storage config..."
mkdir -p /etc/fstab.d
if [[ -f "$DIAD_DIR/os/archiso/airootfs/etc/fstab.d/diad-persistent.conf" ]]; then
    cp "$DIAD_DIR/os/archiso/airootfs/etc/fstab.d/diad-persistent.conf" /etc/fstab.d/diad-persistent.conf
    chmod 644 /etc/fstab.d/diad-persistent.conf
fi

# ---- Install MOTD ----
if [[ -f "$DIAD_DIR/os/archiso/airootfs/etc/motd" ]]; then
    cp "$DIAD_DIR/os/archiso/airootfs/etc/motd" /etc/motd
    chmod 644 /etc/motd
fi

# ---- Install .bashrc ----
if [[ -f "$DIAD_DIR/os/archiso/airootfs/home/diad/.bashrc" ]]; then
    cp "$DIAD_DIR/os/archiso/airootfs/home/diad/.bashrc" "$USER_HOME/.bashrc"
    chown "$INSTALL_USER:$INSTALL_USER" "$USER_HOME/.bashrc"
fi

# ---- Create Required Directories ----
mkdir -p "$USER_HOME/.diad/sessions"
mkdir -p "$USER_HOME/.diad/logs"
mkdir -p "$USER_HOME/.config/diad"
chown -R "$INSTALL_USER:$INSTALL_USER" "$USER_HOME/.diad"
chown -R "$INSTALL_USER:$INSTALL_USER" "$USER_HOME/.config/diad"

# ---- Enable Services ----
log_info "Enabling system services..."
systemctl enable NetworkManager
systemctl enable gnome-keyring
systemctl enable diad-firewall.service
systemctl enable power-profiles-daemon
systemctl disable tlp 2>/dev/null || true
systemctl mask tlp 2>/dev/null || true

log_info ""
log_info "======================================================"
log_info "  DIAD Installation Complete!"
log_info "======================================================"
log_info ""
log_info "Next steps:"
log_info "  1. Reboot your system"
log_info "  2. At the login screen, select 'Sway' as your session"
log_info "  3. Log in — the DIAD Agent will launch automatically"
log_info "  4. Configure your LLM provider API keys:"
log_info "     diad-agent --configure"
log_info ""
log_info "Enjoy your intent-driven operating system!"
