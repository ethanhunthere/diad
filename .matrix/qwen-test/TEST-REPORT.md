# DIAD OS — VM & Configuration Test Report (2026-08-18)

- **Artifact:** `out/diados-2026.08.18-x86_64.iso` (1,704,062,976 B; built 2026-08-18 11:31)
- **VM:** QEMU 11 + KVM, q35, 2 vCPU, 3072 MB, virtio VGA, user-net; entry = GRUB default + `break=postmount` console injection (project `.matrix` harness, extended clone `.matrix/qwen-vmtest.py`)
- **Streams:** VM full-session (18 check groups + screenshot) · A: cargo ×3 · B: ISO artifact · C: static config checks
- **Raw evidence:** `.matrix/transcript-q1-full.log`, `.matrix/shot-q1-full.png`, `.matrix/qwen-test/{A,B,C}-*.md`

## Summary

| Area | Verdict | Evidence |
|---|---|---|
| Boot → interactive desktop | ✅ PASS | session ready ≤ first 5 s probe after settle; autologin→sway clean; no swaynag |
| Sway config integrity | ✅ PASS | `get_config` errors=0; `sway -C` clean; reload rc=0; 58 unique bindsyms, no dupes, no `fullscreen disable` |
| 3-workspace routing | ✅ PASS | WS1 exactly one DIAD AGENT window; no floods; scratchpad holds 1; supervisor watchdogs live |
| Daemon HTTP surface | ✅ PASS | `/ /health /api/v1/status /apps /launch` all 200 |
| Origin gate (P0 remediation) | ✅ PASS | cross-origin REST→403, WS→403; local/no-origin→200/101 |
| Sudo least privilege | ✅ PASS | no shell-capable grants; pacman/kill wrapper-only; forbidden probes denied; wrappers reject bad args |
| Firewall ruleset | ✅ PASS | input policy drop; only lo+established accepted; zero open ports |
| Visual design | ✅ PASS | screenshot: obsidian/amber desktop, 3 waybar pills, GUI intent shell, AAA-contrast |
| Test suite (A) | ✅ PASS | 386 tests × 3 runs, 0 failures; vault flake not reproduced; release build clean |
| Static config checks (C) | ✅ PASS | 37/37 scripts `bash -n`; JSON/TOML valid; theme consistent |
| ISO size invariant (B) | ❌ FAIL | 1704 MB = 1.89× the 900 MB ceiling; every ISO ever built violates it |
| Notifications (mako) | ❌ FAIL | crash-loop: supervisor relaunches every 10 s, no mako process; `ttf-inter` absent while mako config demands `Inter 10` |
| App-container isolation | ❌ FAIL (effective) | scripts carry `--profile`+`--application-mode` but epiphany is **not installed** → runtime falls to chromium `--app` fallback **without isolation** |
| Media/VA-API | ⚠️ PARTIAL | intel-media-driver+libva present (chromium VA path possible); `vainfo` missing; GStreamer/libav stack gone (chromium built-in codecs now) |
| Boot timing invariant | ⚠️ UNMEASURED | `systemd-analyze time` failed in VM; blame shows journal-upload 1.54 s, sshdgenkeys 0.53 s, accounts-daemon 0.46 s |
| Failed units | ❌ 11 | oomd, resolved(+2 sockets), sysusers, timesyncd, avahi×2, krb5×2, lm_sensors |
| Service bloat | ❌ | sshd listening :22, sudo_logsrvd on 0.0.0.0:30343, gpsd, journal-remote/git sockets; systemd-journal-upload enabled |
| Waybar height | ⚠️ | renders 38 px vs 28 px spec text |

**Overall: the OS boots and behaves correctly; the security remediations are verified live; but the appliance has drifted from its minimal-ISO mandate (1.7 GB, 461 pkgs, dev toolchain + nvidia firmware + sshd/krb5/avahi) and carries two live defects (mako crash-loop, dead isolation fallback).**

## 1. VM full-session results

### Passes (highlights)
- **Origin gate works exactly as remediated** — the P0 hole from the 2026-08-17 audit is closed and regression-proof in the shipped binary: `REST evil-origin 403`, `WS evil-origin 403`, legitimate clients unaffected.
- **Sudo surface is least-privilege in practice**: `sudo -l -U diad` shows wrapper-only grants for destructive ops; live probes `sudo -n /bin/sh`, `sudo -n pacman -S -- /tmp/x.pkg`, `sudo -n kill -9` all denied with "a password is required"; `diad-pacman install '../evil'` → "invalid package name", `diad-killpid abc/1` → rejected, `diad-format` without `--confirm` → rejected. (Note: my rc capture printed 0 due to a `head` pipeline bug in the check; the rejection messages are the evidence.)
- **Desktop renders the 2040 design** (screenshot): waybar pills DIAD AGENT/BROWSER/COMMS, amber accents, GUI with command palette (Ctrl K), provider selector, TUI button, intent input.
- **Daemon discipline**: perf-tier LOW detected on 2-core VM; vault correctly warns memory-only keys on live ISO; loopback-only listener.
- **Storage setup**: DIAD-DATA absent → local fallback + Documents/Downloads/… symlinks created cleanly.

### Findings (VM)
1. **mako crash-loop** — `supervisor: mako not running — relaunching` every 10 s; no mako process in [1]. `packages.x86_64` has no `ttf-inter` while `/etc/mako/config` sets `font=Inter 10`. Fix: add `ttf-inter` or switch mako font to `Liberation Sans 10`.
2. **`diad-power-setup` privilege gap** — boot.log shows two `sudo: a password is required` lectures; the script calls sudo commands not in the NOPASSWD list (it recovers and finishes, but the power-profiles enable fails: "could not enable power-profiles-daemon").
3. **11 failed units** — oomd, resolved (+resolved-monitor/varlink sockets), sysusers, timesyncd, avahi-daemon, avahi-dnsconfd, krb5-kdc, krb5-kadmind, lm_sensors. sysusers/resolved/oomd failing on a fresh boot is a hygiene problem; krb5/avahi shouldn't be installed at all.
4. **Listener bloat** — `sshd` on :22 (firewall input-drop hides it, but the service exists), `sudo_logsrvd` on 0.0.0.0:30343, `gpsd` on 127.0.0.1:2947, systemd sockets for git (:9418) and journal-remote (:19531/2); `systemd-journal-upload.service` enabled (1.54 s of boot) — a telemetry-ish unit in a zero-telemetry OS.
5. **Isolation fallback dead** — `diad-telegram`/`diad-slack` epiphany branch can never fire (epiphany absent); chromium `--app` fallback has no `--profile` → Telegram/Slack share the main browser profile. Fix: add `--user-data-dir`/profile handling to the chromium fallback, or ship epiphany.
6. **Waybar 38 px** — still above the 28 px spec text (module minimum height).
7. **`systemd-analyze time` unavailable** in the VM context → invariant 1 remains uncertified; blame top shows non-minimal units (journal-upload, sshdgenkeys, accounts-daemon, homed-firstboot).
8. **WS2/WS3 surfaces are on-demand now** (`/apps`, `/launch` pages, bindings) — a deliberate design change vs the spec's persistent hubs; spec §12 is stale.
9. **Qt dark theme not enforced** (no `QT_STYLE_OVERRIDE`) — cosmetic.
10. **nftables.conf header comment stale** ("and SSH (port 22)") — comment-only.

## 2. Agent A — cargo tests ×3 (PASS)

386 tests per run (347 lib + 39 integration), **0 failures in all 3 runs**; `auth::vault::tests::test_vault_roundtrip` passed 3× parallel + 1× isolated → the historical flake did not reproduce. `cargo build --release` clean, 0 warnings (4 m 19 s).

## 3. Agent B — ISO artifact (FAIL on size)

- ISO 1704.06 MB vs 900 MB ceiling → **+804 MB over**; uncompressed tree 1.43 GB (< 1.6 GB, pass).
- sfs (1449.7 MB) is *larger* than its uncompressed content (1429.2 MB) — content is incompressible; compression is not the lever.
- Removable mass inside sfs: **firmware 414 MB** (incl. nvidia 109 M, mediatek 39 M), **chromium 397 MB**, **dev toolchain ~380 MB** (gcc/headers/gdb/cmake/rustup/chromedriver), initramfs 200.7 MB (zstd -1; recompress → ~70 MB).
- Plan to close the gap: drop toolchain (~380) + firmware trim (~250–300) + initramfs recompress (~130) ≈ 760–810 MB → lands ~895–945 MB; all three plus minor sweeps needed for the band.
- Consistency: ISO is a faithful image of the staging tree (hash-identical kernel/initrd/sfs; UUID matches).

## 4. Agent C — static checks (PASS with notes)

37/37 scripts `bash -n` clean; waybar JSON ×3 + diad.conf TOML valid; sudoers: no shell-capable binaries, wrappers 755, pacman/kill wrapper-only (note: staged sudoers file is 644 f0id:f0id in-tree — build must enforce 0440 root:root, which profiledef does); nftables default-deny; sway: no dup bindsyms, no fullscreen-disable, residual unanchored title regexes limited to workspace-move impact and co-constrained by `app_id="chrome-.*"`; theme consistent except Qt.

## 5. Harness notes (for the record)

- `qwen-vmtest.py` `expect("VFY_DONE")` self-matches the tty echo of the sent script; the run still completes because the poweroff is queued until `vfy.sh` finishes. Cosmetic; fix by using a marker string not present in the script text (e.g. `VFY_DONE_$RANDOM`).
- [12] wrapper rc values were captured after `head` (always 0); rejection *messages* are the valid evidence.

## 6. Recommended next actions (priority)

1. **Size**: drop dev toolchain + chromedriver from the live image; trim firmware to intel/amdgpu/atheros/realtek; recompress initramfs (zstd -19). Gate the Makefile at 900 MB.
2. **mako**: install `ttf-inter` or change mako font.
3. **Isolation**: give the chromium fallback a dedicated `--user-data-dir` per app.
4. **Bloat purge**: remove openssh/krb5/avahi/gpsd/sudo_logsrvd/journal-upload/journal-remote/git sockets; fix `diad-power-setup` sudo gap (pin the missing verbs or route through an existing wrapper).
5. **Measure boot**: make `systemd-analyze` work in the matrix (it failed here) and publish per-run numbers.
