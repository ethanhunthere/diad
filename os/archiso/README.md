# `os/archiso/` — The OS Recipe

This folder is the **recipe** that turns DIAD into a bootable, installable
`.iso`. It's driven by `mkarchiso` (Arch Linux's ISO-builder).

| Path | What it is |
|------|------------|
| `profiledef.sh` | The master recipe — what gets included and file permissions |
| `packages.x86_64` | The shopping list of programs to install |
| `pacman.conf` | Which software repositories to use |
| `grub/` | The boot menu and its theme |
| `airootfs/` | **The OS's actual files.** This mirrors the installed system's drive: `airootfs/etc/...` becomes `/etc/...`, `airootfs/usr/local/bin/...` becomes `/usr/local/bin/...`, etc. |

## Key places inside `airootfs/`

- `etc/sway/config` — window manager, workspaces, keybindings
- `etc/waybar/` — the top bar
- `etc/wofi/` — the app launcher
- `etc/mako/config` — notifications
- `etc/systemd/system/` — startup services
- `etc/sudoers.d/` — what may run as administrator
- `usr/local/bin/` — DIAD's own `diad-*` programs
- `home/diad/` — the default user's home folder

See [`../../STRUCTURE.md`](../../STRUCTURE.md) for a full plain-English tour.
