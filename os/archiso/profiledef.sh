#!/usr/bin/env bash
#
# DIAD — Arch ISO Profile Definition
# Used by mkarchiso to build the bootable DIAD ISO.
#

# === ISO Metadata ===
iso_name="diados"
iso_label="DIADOS"
iso_publisher="DIADOS Project"
iso_application="DIADOS — Intent-Driven Operating System"
iso_version="$(date +%Y.%m.%d)"
install_dir="diados"

# === Boot Modes ===
# UEFI with GRUB (uefi.grub — the current boot mode name; the old
# uefi-x64.grub.esp spelling is deprecated)
bootmodes=("uefi.grub")

# === File Permissions ===
# mkarchiso copies airootfs with --no-preserve=ownership,mode, so EVERY file
# that needs non-default ownership or an executable bit MUST be listed here.
# Without this, /usr/local/bin/* would land in the ISO as 0644 root-owned but
# non-executable, and /etc/sudoers.d/diad would be world-readable (sudo would
# refuse to use it). Every path listed below exists in airootfs/.
file_permissions=(
    # --- diad user ---
    ["/home/diad"]="1000:1000:755"
    ["/home/diad/.bashrc"]="1000:1000:644"

    # --- system identity ---
    ["/etc/passwd"]="0:0:644"
    ["/etc/group"]="0:0:644"
    ["/etc/shadow"]="0:0:400"
    ["/etc/hostname"]="0:0:644"
    ["/etc/locale.conf"]="0:0:644"
    ["/etc/localtime"]="0:0:644"
    ["/etc/motd"]="0:0:644"
    ["/etc/diad/diad.conf"]="0:0:644"

    # --- sudo: strict 0440 root-owned or sudo ignores it ---
    ["/etc/sudoers.d/diad"]="0:0:440"

    # --- security / networking ---
    ["/etc/nftables.conf"]="0:0:600"
    ["/etc/NetworkManager/conf.d/diad.conf"]="0:0:600"

    # --- display session ---
    ["/etc/sway/config"]="0:0:644"
    ["/etc/sway/conf.d/media-keys.conf"]="0:0:644"
    ["/etc/waybar/config"]="0:0:644"
    ["/etc/waybar/style.css"]="0:0:644"

    # --- systemd units / drop-ins ---
    ["/etc/systemd/system/diad-firewall.service"]="0:0:644"
    ["/etc/systemd/system/diad-perf-tier.service"]="0:0:644"
    ["/etc/systemd/system/getty@tty1.service.d/autologin.conf"]="0:0:644"

    # --- udev rules ---
    ["/etc/udev/rules.d/99-diad-perf-tier.rules"]="0:0:644"

    # --- initramfs (archiso hooks) ---
    ["/etc/mkinitcpio.conf.d/archiso.conf"]="0:0:644"
    ["/etc/mkinitcpio.d/linux-lts.preset"]="0:0:644"

    # --- DIAD binaries & scripts (executable, root-owned) ---
    ["/usr/local/bin/diad-agent"]="0:0:755"
    ["/usr/local/bin/diad-diagnose.sh"]="0:0:755"
    ["/usr/local/bin/diad-launch.sh"]="0:0:755"
    ["/usr/local/bin/diad-perf-tier.sh"]="0:0:755"
    ["/usr/local/bin/diad-display-setup.sh"]="0:0:755"
    ["/usr/local/bin/diad-first-run.sh"]="0:0:755"
    ["/usr/local/bin/diad-hardware-setup.sh"]="0:0:755"
    ["/usr/local/bin/diad-network-setup.sh"]="0:0:755"
    ["/usr/local/bin/diad-power-setup.sh"]="0:0:755"
    ["/usr/local/bin/diad-splash.sh"]="0:0:755"
    ["/usr/local/bin/diad-storage-setup.sh"]="0:0:755"
    ["/usr/local/bin/diad-supervisor.sh"]="0:0:755"
    ["/usr/local/bin/diad-sysfs-write"]="0:0:755"
    ["/usr/local/bin/brave"]="0:0:755"
)

# === Build Configuration ===
# Base Arch ISO profile
buildmodes=("iso")

# === GRUB theme ===
# The DIAD GRUB theme lives in this profile at grub/themes/grub-theme-diad/
# and grub/grub.cfg references it. mkarchiso copies every non-*.cfg file
# from grub/ verbatim into /boot/grub/ on the ISO (grub_theme= is not
# consumed by archiso 88).

# === Pacman Configuration ===
# Profile-local pacman.conf (official core/extra only — reproducible builds
# on any Arch machine, no host-specific repos).
pacman_conf="pacman.conf"

# === Custom airootfs modifications ===
# Packages to install into the ISO
# (Listed in packages.x86_64 in this profile directory)
airootfs_image_type="erofs"
