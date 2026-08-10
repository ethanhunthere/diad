# DIAD OS — Testing Guide

This document describes how to verify every feature of the DIAD OS release.
Each section maps to a subsystem and lists concrete manual test steps plus the
automated tests that cover the underlying code.

Run the full automated suite first:

```bash
cd agent
cargo test          # 311 tests (282 unit + 29 integration)
cargo build --all-targets   # must be 0 errors, 0 warnings
cargo clippy --all-targets -- -D warnings   # must be clean
```

Integration tests are split by domain under `agent/tests/`:

| File                        | Tests | Coverage                              |
|-----------------------------|-------|---------------------------------------|
| `tui_app_tests.rs`          | 10    | app init, submit, modes, F6 picker, `/model`, `/connect` |
| `tools_sandbox_tests.rs`    | 8     | sandbox *rejects* privileged paths, shells, traversal |
| `tools_system_tests.rs`     | 4     | registry completeness, list_dir, system info |
| `orchestrator_tests.rs`     | 4     | intent classification, sessions, message constructors |
| `config_tests.rs`           | 2     | defaults, TOML roundtrip |
| `providers_tests.rs`        | 1     | provider factory resolution |

`agent/tests/common/mod.rs` holds shared helpers (e.g. `default_registry()`).

---

## 1. Laptop Hardware (media keys, brightness, volume, airplane mode)

**Automated:** `agent/src/tools/input.rs` — `test_airplane_mode_on_turns_wifi_off`,
`test_airplane_mode_off_turns_wifi_on`.

| # | Test | Expected result |
|---|------|-----------------|
| 1 | Boot on a laptop | `~/.diad/logs/boot.log` contains `diad-hardware-setup: media keys enabled (laptop)`; `/etc/sway/conf.d/media-keys.conf` has active `bindsym` lines |
| 2 | Boot on a desktop | media-keys.conf stays fully commented; `diad-hardware-setup: media keys disabled (desktop)` in the log |
| 3 | `XF86MonBrightnessUp` / `Down` | Screen brightness steps ±10% |
| 4 | `XF86AudioRaiseVolume` / `LowerVolume` / `Mute` | Volume steps ±5%, mute toggles |
| 5 | `XF86AudioPlay` / `Next` / `Prev` | Playback controls the active media player |
| 6 | `XF86RFKill` | Wi-Fi radio toggles off/on |
| 7 | In the agent: *"turn on airplane mode"* / *"turn it off"* | `nmcli radio wifi off/on` runs via sudo, no password prompt |
| 8 | Re-run setup: `diad-hardware-setup.sh --force` | Idempotent — no duplicate bindings, log lines appended |

## 2. Display & Multi-Monitor

**Automated:** `agent/src/tools/display.rs` — `test_set_resolution_requires_output`,
`test_set_resolution_rejects_zero`, `test_set_scale_validates_range`,
`test_mirror_outputs_requires_both`.

| # | Test | Expected result |
|---|------|-----------------|
| 1 | Laptop with external monitor plugged in | External appears to the right of the built-in (`~/.diad/logs/display.log` shows the position) |
| 2 | Laptop, no external | Built-in at native resolution |
| 3 | In the agent: *"set resolution of HDMI-A-1 to 1920x1080"* | Resolution applies |
| 4 | In the agent: *"set scale of eDP-1 to 1.5"* | Scaling applies |
| 5 | In the agent: *"mirror HDMI-A-1 to eDP-1"* | Screens mirrored |
| 6 | In the agent: *"what displays are connected?"* | `swaymsg -t get_outputs` summary |

## 3. Network

**Automated:** `agent/src/tools/hardware.rs` — `test_connect_ethernet_requires_interface`,
`test_set_hostname_requires_name`.

| # | Test | Expected result |
|---|------|-----------------|
| 1 | First boot | `diad-network-setup.sh` runs once; NetworkManager enabled; Wi-Fi profiles hardened (`wifi.cloned-mac-address=random`, `connection.autoconnect=yes`) |
| 2 | In the agent: *"connect to wifi network X"* | `nmcli` connects |
| 3 | In the agent: *"wifi status"* / *"internet status"* | SSID, signal %, and IPv4 addresses shown |
| 4 | In the agent: *"show network interfaces"* | `ip -j addr show` JSON formatted |
| 5 | In the agent: *"set hostname to my-pc"* | `hostnamectl set-hostname` via sudo |
| 6 | Connectivity check | `/etc/NetworkManager/conf.d/diad.conf` present with `uri` + `interval` |

## 4. Power Management

**Automated:** `agent/src/tools/power.rs` — `test_set_power_profile_validates`,
`test_set_sleep_timeout_bounds`, `test_summarize_upower`.

| # | Test | Expected result |
|---|------|-----------------|
| 1 | First boot | `power-profiles-daemon` enabled; `tlp` masked; lid behavior set (suspend on battery, ignore on AC) |
| 2 | In the agent: *"set power profile to power-saver"* | `powerprofilesctl set power-saver` via sudo |
| 3 | In the agent: *"battery status"* | State, percentage, time-to-empty shown |
| 4 | In the agent: *"set sleep timeout to 30 minutes"* | `gsettings` timeout applied; invalid values rejected (1–1440) |
| 5 | Invalid profile: *"set power profile to turbo"* | Rejected with clear error |

## 5. Security

**Automated:** integration tests in `agent/tests/integration_test.rs`
(`test_tool_registry_registers_all_tools` asserts the security tools exist).

| # | Test | Expected result |
|---|------|-----------------|
| 1 | `Super+L` | Screen locks immediately (swaylock, black). `$mod+Right` still moves focus right (no shadowed binding) |
| 2 | `$mod+Ctrl+l` | Removed — use `Super+L` |
| 3 | Idle 5 min (300 s) | Screen locks automatically (swayidle) |
| 4 | Reboot | `nft list ruleset` shows input policy *drop*, established/related accepted, loopback accepted, port 22 accepted |
| 5 | `systemctl status diad-firewall` | Active (exited), enabled at boot |
| 6 | In the agent: *"enable firewall"* / *"disable firewall"* | `systemctl enable/disable --now diad-firewall` via sudo, no password |
| 7 | In the agent: *"lock screen"* | swaylock runs |
| 8 | External host scans the machine | Only SSH (22) reachable from outside; everything else filtered |
| 9 | `sudo visudo -cf /etc/sudoers.d/diad` | Validates OK (single-line rule) |

## 6. Onboarding & First Run

| # | Test | Expected result |
|---|------|-----------------|
| 1 | Fresh install, first login | Sway starts; foot opens with the DIAD Setup wizard (logo, prompts) |
| 2 | *"Connect to an AI provider now?" — y* | `diad-agent --configure` wizard runs |
| 3 | *"Connect to a Wi-Fi network?" — y* | Network list shown; SSID prompt; password entered directly in the terminal |
| 4 | Timezone prompt with a valid zone | `timedatectl set-timezone` applies |
| 5 | Re-login | Wizard does **not** reappear (`~/.diad/.first-run-complete` exists) |
| 6 | Agent launches | Splash (DIAD logo + progress bar) then the agent UI in foot |
| 7 | Brave opens on workspace 2 | Window appears in workspace 2 |
| 8 | `~/.diad/logs/boot.log` | Full chronological log of the supervisor + all setup scripts |

## 7. Update Mechanism

**Automated:** `agent/src/tools/update.rs` — none of the update tools spawn
privileged commands in tests; argument construction is unit-covered via the
registry test.

| # | Test | Expected result |
|---|------|-----------------|
| 1 | In the agent: *"check for system updates"* | `checkupdates` runs; up-to-date or a list of pending packages |
| 2 | In the agent: *"apply system updates"* | Confirmation prompt shown (`CONFIRM_TOOLS`), then `pacman -Syu --noconfirm` via sudo |
| 3 | In the agent: *"check for agent updates"* | GitHub releases API queried; version compared |
| 4 | In the agent: *"update the agent"* | Confirmation, download to /tmp, `install -b -S .bak` to `/usr/local/bin/diad-agent` |
| 5 | Sudoers | `/usr/bin/pacman` and `/usr/bin/checkupdates` whitelisted, no password prompt |

## 8. Storage (DIAD-DATA)

**Automated:** `agent/src/config/settings.rs` — `test_default_config`,
`test_serialize_deserialize` (both assert `storage.persistent_path`).

| # | Test | Expected result |
|---|------|-----------------|
| 1 | System with a `DIAD-DATA`-labelled partition | Mounted at `/home/diad/data` (fstab.d); `findmnt /home/diad/data` succeeds |
| 2 | System without the partition | Plain directory fallback created; no mount errors |
| 3 | Fresh home | `~/Documents`, `~/Downloads`, `~/Pictures`, `~/Music`, `~/Videos` symlink into `~/data` |
| 4 | Existing real folders | Symlinks are **not** created over them (no data loss) |
| 5 | `diad.conf` | Contains `[storage] persistent_path = "/home/diad/data"`; agent loads it (`cargo test` covers the round-trip) |

## 9. ISO & Boot

Build prerequisites on the host:

- `archiso` (provides `mkarchiso`) and `grub` (provides `grub-mkstandalone` for the embedded UEFI bootloader)
- ~10 GB free disk space; `make iso` emits `out/diados-*.iso`

| # | Test | Expected result |
|---|------|-----------------|
| 1 | `make iso` | Builds `out/diados-*.iso` without errors |
| 2 | Boot the ISO in UEFI | GRUB shows the DIAD theme (title "DIAD OS — Intent is the only interface"), default entry "DIAD OS (x86_64)" |
| 3 | tty1 autologin | Boots directly to the `diad` user (empty password, standard for live ISOs) |
| 4 | Desktop session | `.bashrc` auto-starts Sway → supervisor → Waybar, DIAD Agent (with splash), and Brave all present |
| 5 | First-run wizard | Offers: AI provider, Wi-Fi, timezone, and Brave installation (Flatpak) |
| 6 | Brave | Installed via Flatpak (`com.brave.Browser`); `brave` command resolves through the `/usr/local/bin/brave` shim; skipped if declined |
| 7 | `/etc/motd` | Shows the DIAD welcome message |
| 8 | Packages | `playerctl`, `swaylock`, `nftables`, `upower`, `power-profiles-daemon`, `tlp`, `mkinitcpio-archiso`, `grub`, `flatpak`, `pacman-mirrorlist` installed |
| 9 | Live updates | `pacman -Syu` works inside the live ISO (mirrorlist + `pacman-mirrorlist` present) |

## 10. Regression Checklist

- [ ] `cd agent && cargo build --all-targets` → 0 errors, 0 warnings
- [ ] `cd agent && cargo test` → 57/57 passing
- [ ] `bash -n` passes on every shell script
- [ ] `python3 -m json.tool` validates `etc/waybar/config`
- [ ] `sudo visudo -cf /etc/sudoers.d/diad` passes
- [ ] The agent's `/help` shows all new tool categories (hardware, input, display,
      power, security, update, storage, help)
- [ ] `/tools` lists all 26 new tools; `/version` shows the version + profile
