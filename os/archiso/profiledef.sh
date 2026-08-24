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
# F-23 (spec 6.8) — reproducible builds: when the Makefile exports
# SOURCE_DATE_EPOCH, derive the version from it so two builds of the same
# tree are byte-identical; otherwise fall back to today's date.
if [[ -n "${SOURCE_DATE_EPOCH:-}" ]]; then
    iso_version="$(date -u -d "@${SOURCE_DATE_EPOCH}" +%Y.%m.%d)"
else
    iso_version="$(date +%Y.%m.%d)"
fi
install_dir="diados"

bootmodes=("uefi.grub")
airootfs_image_type="squashfs"
airootfs_image_tool_options=('-comp' 'zstd' '-Xcompression-level' '1' '-b' '1M')

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
    ["/home/diad/.bash_profile"]="1000:1000:644"
    ["/home/diad/.config/foot/foot.ini"]="1000:1000:644"
    ["/home/diad/.config/sway/config"]="1000:1000:644"
    ["/etc/skel/.bash_profile"]="0:0:644"
    ["/etc/skel/.config/foot/foot.ini"]="0:0:644"
    ["/etc/xdg/foot/foot.ini"]="0:0:644"

    # --- system identity ---
    ["/etc/passwd"]="0:0:644"
    ["/etc/group"]="0:0:644"
    ["/etc/shadow"]="0:0:400"
    ["/etc/hostname"]="0:0:644"
    ["/etc/locale.conf"]="0:0:644"
    ["/etc/localtime"]="0:0:644"
    ["/etc/motd"]="0:0:644"
    ["/etc/issue"]="0:0:644"
    ["/etc/diad/diad.conf"]="0:0:644"

    # --- sudo: strict 0440 root-owned or sudo ignores it ---
    ["/etc/sudoers.d/diad"]="0:0:440"

    # --- security / networking ---
    ["/etc/nftables.conf"]="0:0:600"
    ["/etc/NetworkManager/conf.d/diad.conf"]="0:0:600"
    ["/etc/polkit-1/rules.d/00-diad-admin.rules"]="0:0:644"

    # --- display session ---
    ["/etc/mako/config"]="0:0:644"
    ["/etc/sway/config"]="0:0:644"
    ["/etc/sway/conf.d/media-keys.conf"]="0:0:644"
    ["/etc/waybar/config"]="0:0:644"
    ["/etc/waybar/config.compact"]="0:0:644"
    ["/etc/waybar/config.full"]="0:0:644"
    ["/etc/waybar/style.css"]="0:0:644"
    ["/etc/diad/sway-overrides.conf"]="0:0:644"

    # --- systemd units / drop-ins ---
    ["/etc/systemd/system/getty@tty1.service.d/autologin.conf"]="0:0:644"
    ["/etc/systemd/system/diad-firewall.service"]="0:0:644"
    ["/etc/systemd/system/diad-identity-setup.service"]="0:0:644"
    ["/etc/systemd/system/diad-perf-tier.service"]="0:0:644"

    # --- udev rules ---
    ["/etc/udev/rules.d/98-diad-runtime-pm.rules"]="0:0:644"
    ["/etc/udev/rules.d/99-diad-perf-tier.rules"]="0:0:644"

    # --- system tuning (sysctl / zram / modules / presets / tmpfiles) ---
    ["/etc/sysctl.d/99-diad.conf"]="0:0:644"
    ["/etc/zram-generator.conf"]="0:0:644"
    ["/etc/modprobe.d/diad-blacklist.conf"]="0:0:644"
    ["/etc/tmpfiles.d/diad-power.conf"]="0:0:644"
    ["/etc/systemd/system-preset/00-diad.preset"]="0:0:644"
    ["/usr/share/glib-2.0/schemas/99_diad_dark.gschema.override"]="0:0:644"
    ["/usr/share/backgrounds/diad/wallpaper.png"]="0:0:644"
    ["/usr/share/backgrounds/diad/wallpaper.svg"]="0:0:644"
    ["/usr/share/backgrounds/diad/lockscreen.png"]="0:0:644"
    ["/usr/share/pixmaps/diad-logo-white.png"]="0:0:644"
    ["/usr/share/pixmaps/diad-logo-black.png"]="0:0:644"
    ["/usr/share/pixmaps/diad.png"]="0:0:644"

    # --- initramfs (archiso hooks) ---
    ["/etc/mkinitcpio.conf.d/archiso.conf"]="0:0:644"
    ["/etc/mkinitcpio.d/linux-lts.preset"]="0:0:644"

    # --- DIAD binaries & scripts (executable, root-owned) ---
    ["/usr/local/bin/diad"]="0:0:755"
    ["/usr/local/bin/diad-agent"]="0:0:755"
    ["/usr/local/bin/diad-diagnose.sh"]="0:0:755"
    ["/usr/local/bin/diad-launch.sh"]="0:0:755"
    ["/usr/local/bin/diad-perf-tier.sh"]="0:0:755"
    ["/usr/local/bin/diad-snap.sh"]="0:0:755"
    ["/usr/local/bin/diad-bench.sh"]="0:0:755"
    ["/usr/local/bin/diad-display-setup.sh"]="0:0:755"
    ["/usr/local/bin/diad-first-run.sh"]="0:0:755"
    ["/usr/local/bin/diad-hardware-setup.sh"]="0:0:755"
    ["/usr/local/bin/diad-identity-setup.sh"]="0:0:755"
    ["/usr/local/bin/diad-network-setup.sh"]="0:0:755"
    ["/usr/local/bin/diad-power-setup.sh"]="0:0:755"
    ["/usr/local/bin/diad-power.sh"]="0:0:755"
    ["/usr/local/bin/diad-splash.sh"]="0:0:755"
    ["/usr/local/bin/diad-storage-setup.sh"]="0:0:755"
    ["/usr/local/bin/diad-supervisor.sh"]="0:0:755"
    ["/usr/local/bin/diad-windowctl"]="0:0:755"
    ["/usr/local/bin/diad-open"]="0:0:755"
    ["/usr/local/bin/diad-set-password"]="0:0:755"
    ["/usr/local/bin/diad-sysfs-write"]="0:0:755"
    ["/usr/local/bin/diad-pacman"]="0:0:755"
    ["/usr/local/bin/diad-killpid"]="0:0:755"
    ["/usr/local/bin/diad-clone-disk"]="0:0:755"
    ["/usr/local/bin/diad-conf-write"]="0:0:755"
    ["/usr/local/bin/diad-format"]="0:0:755"
    ["/usr/local/bin/diad-fsck"]="0:0:755"
    ["/usr/local/bin/diad-install-conf"]="0:0:755"
    ["/usr/local/bin/diad-mount"]="0:0:755"
    ["/usr/local/bin/diad-resize-partition"]="0:0:755"
    ["/usr/local/bin/diad-selfupdate"]="0:0:755"
    ["/usr/local/bin/diad-umount"]="0:0:755"
    ["/usr/local/bin/diad-wifi.sh"]="0:0:755"
    ["/usr/local/bin/diad-telegram"]="0:0:755"
    ["/usr/local/bin/diad-slack"]="0:0:755"
    ["/usr/local/bin/diad-workspace.sh"]="0:0:755"
    ["/usr/local/bin/diad-workspace-jump.sh"]="0:0:755"
    ["/usr/local/bin/diad-waybar"]="0:0:755"
    ["/usr/local/bin/diad-rfkill"]="0:0:755"
    ["/usr/local/bin/diad-screenshot"]="0:0:755"
    # Web lockscreen: kiosk orchestrator (sway lock mode) + PAM auth helper
    # (compiled from os/lock-auth/diad-lock-auth.c at ISO build time).
    ["/usr/local/bin/diad-lock"]="0:0:755"
    ["/usr/local/bin/diad-lock-auth"]="0:0:755"
    ["/etc/wofi/style.css"]="0:0:644"
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

