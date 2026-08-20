# C — Static Config Checks (airootfs)

Date: 2026-08-18 · Tree: `os/archiso/airootfs` · Read-only checks, no repo files modified.

| # | Check | Result |
|---|-------|--------|
| 1 | `bash -n` on all usr/local/bin scripts | PASS (37/37) |
| 2 | JSON validity (waybar ×3, etc/diad) | PASS |
| 3 | TOML validity (diad.conf) | PASS |
| 4 | Sudoers review | PASS w/ notes |
| 5 | Firewall inbound rules | PASS w/ note |
| 6 | Sway config (dupes / fullscreen / regex) | PASS w/ notes |
| 7 | Theme consistency (dark) | PASS w/ note |
| 8 | mkinitcpio hooks + compression | PASS |

## 1. Syntax — PASS
37 shebang scripts, all `bash -n` clean: brave, diad-*.sh ×13, diad-{clone-disk,conf-write,format,fsck,install-conf,killpid,mount,open,pacman,resize-partition,selfupdate,set-password,slack,sysfs-write,telegram,umount,waybar,windowctl}.
Shebangs: 35× `#!/usr/bin/env bash`, 2× `#!/usr/bin/env sh` (`diad-open`, `diad-sysfs-write`); both also pass `sh -n`. No mismatches. `diad-agent` is an ELF binary (skipped).

## 2. JSON — PASS
`json.load` OK: `etc/waybar/config`, `config.compact`, `config.full`. No JSON files exist under `etc/diad/` (only `diad.conf`, `sway-overrides.conf`).

## 3. TOML — PASS
`tomllib.load` OK on `etc/diad/diad.conf`. Keys: allowed_paths, auto_save_interval, confirm_destructive, default_provider, log_level, max_history, storage, stream_tokens, theme.

## 4. Sudoers (`etc/sudoers.d/diad`) — PASS with notes
(a) Shell-capable binaries (sh/bash/zsh/su/sudo/pkexec/doas/runuser): **NONE granted**. rm/sed/tee also absent.
(b) Trailing open-wildcard entries (none shell-capable; value/device/host args per header comments): `brightnessctl set *`, `powerprofilesctl set [a-z-]*`, `hostnamectl set-hostname *`, `nmcli dev wifi|connect|disconnect *`, `nmcli radio *`, `nmcli con *`, `nmcli general *`, `smartctl -H -A *`, `hdparm -t *`, `eject *`, `iw dev wlan0 set channel *`, `traceroute *`, `nvidia-smi -pl *`, `lpadmin -x *`, `lpoptions -d *`. (`tcpdump -i * -c * -nn` has interior wildcards only.)
(c) Bare entries w/o pinned argv: the 13 validated wrappers (`diad-pacman, -killpid, -clone-disk, -mount, -umount, -format, -fsck, -resize-partition, -conf-write, -install-conf, -selfupdate, -sysfs-write, -set-password`) — by design; wrappers enforce their own allowlists. All wrappers are 755 in-tree.
pacman/kill wrapper-only: **YES** — no direct `/usr/bin/pacman` or `/usr/bin/kill` grants (only wrapper paths).
Notes: `nmcli con *`/`nmcli general *` are the broadest family grants (many subcommands). Staged file is `644 f0id:f0id`; file's own comment requires 0440 root:root at install — ISO build must enforce.

## 5. Firewall (`etc/nftables.conf`) — PASS
`input` policy **drop**; accepts only `ct state established,related` and `iif "lo"`. Zero open ports. `forward` policy drop; `output` accept.
Note: header comment claims "and SSH (port 22)" but **no** `tcp dport 22` rule exists — stale comment only; ruleset itself is default-deny.

## 6. Sway (`etc/sway/config` + `conf.d/media-keys.conf`) — PASS with notes
(a) 58 unique bindsym combos (45 + 13), **no duplicates** (`sway-overrides.conf` is comments-only).
(b) `fullscreen disable`: **none**.
(c) for_window regexes: catch-alls `[class|^.*] / [app_id|^.*] / [title|^.*]` lines 64–66 set only `border pixel 0` (benign). Effectively-unanchored title matches (leading/embedded `.*` → substring semantics, title is client-settable): L78 `^.*Telegram.*$`, L79 `^.*Slack.*$`, L80 `^.*web\.telegram\.org.*$`, L81 `^.*app\.slack\.com.*$` (title-only; hijack impact limited to workspace move), L76 `.*7860/launch.*`, L91 `.*7860/apps.*`, L101 `.*7860$` (these three are co-constrained by `app_id="chrome-.*"`, mitigating). All other title rules are `^…$` prefix-anchored.

## 7. Theme consistency — PASS with note
`etc/environment`: `GTK_THEME=Adwaita:dark`, `ADW_COLOR_SCHEME=prefer-dark` (+ QT_QPA_PLATFORM=wayland). `gtk-3.0` & `gtk-4.0` settings.ini: `gtk-theme-name = Adwaita-dark`, `gtk-application-prefer-dark-theme = 1` (identical in etc/, skel/, home/diad/). mako: dark `#0d0e15ea` bg / `#f8fafc` text. gschema override: `color-scheme='prefer-dark'`, `gtk-theme='Adwaita-dark'`.
Note: **no Qt dark enforcement** anywhere — no `QT_STYLE_OVERRIDE`/`QT_QPA_PLATFORMTHEME`, no qt5ct/qt6ct/kvantum files in tree. Qt apps render in default (possibly light) style.

## 8. mkinitcpio (`etc/mkinitcpio.conf.d/archiso.conf`) — PASS
`HOOKS=(base udev microcode modconf kms archiso archiso_loop_mnt block filesystems keyboard)`; `COMPRESSION="zstd"`, `COMPRESSION_OPTIONS=(-1 -T0)`.
Order sane: `archiso`/`archiso_loop_mnt` precede `block`/`filesystems` (required — archiso hook mounts rootfs), matching upstream archiso layout; `kms` early; `keyboard` last is fine (tty1 autologin, no boot-time entry). zstd -1 trades ratio for decompression speed (documented rationale).
