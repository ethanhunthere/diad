# DIAD — Boot & Landing Reference (RBRO-004)

This document is the authoritative reference for the **release-blocker boot
chain** fixed in RBRO-004 (work packages WP-01..WP-08). It covers the GRUB
menu, the post-login landing state, the supervisor boot order, and the
runtime flood backstops.

## 1. GRUB entries (WP-01 — parity)

Three entries are defined in `os/archiso/grub/grub.cfg`. They differ **only**
in `quiet` / `loglevel` / `systemd.show_status` / `copytoram`; everything else
(kernel + initramfs, archisobasedir, archisosearchuuid, fsck.mode=skip,
rd.timeout=10) is identical. Verified at build time by a parity diff gate.

| Entry          | Kernel args (delta)                        | Use case              |
|----------------|--------------------------------------------|-----------------------|
| DIAD (default) | `quiet loglevel=3 systemd.show_status=0`   | Normal boot, fastest  |
| DIAD verbose   | `loglevel=7` (no quiet)                    | Debug / console logs  |
| DIAD RAM       | `copytoram` + `pv` progress                | Whole ISO copied to RAM, then boot |

- `timeout=3`, default = `DIAD`.
- `fsck.mode=skip rd.timeout=10` on every entry — kills the old 2–3 minute
  USB copy-to-RAM stall (root cause E4 in the P3 audit).
- ISO uuid: `2026-08-11-09-38-15-00` (`archisosearchuuid=`).

## 2. Post-login landing state (TARGET STATE)

Within ~6 seconds of login the desktop must show **exactly**:

- **Workspace 1** — one DIAD agent surface:
  - daemon healthy → `brave --app=http://127.0.0.1:7860` (GUI), focused,
    no user click needed; or
  - daemon unhealthy after restart limit → **one** `foot diad-agent --tui`.
  - Never both. Never zero. (WP-03 spawn-once policy + WP-02 crash guard.)
- **Workspace 2** — one DIAD Apps icon page (`/apps`). The browser opens only
  on click (`POST /api/v1/launch`). (WP-04.)
- **Workspace 3** — one DIAD Launchpad (`/launch`). It opens Telegram/Slack on
  click and self-closes once both chats are open (page `window.close()` +
  supervisor sway-kill backstop). (WP-05.)
- **Waybar** — glass pills with Font Awesome glyphs (workspace, clock, power).
  Owned by the supervisor; sway never starts it directly. (WP-06.)
- **Zero** popups, floods, or connection-error pages.

## 3. Supervisor boot order (`diad-supervisor.sh`)

Sway runs `exec diad-supervisor.sh` on login (exactly once). Order:

1. **Idempotent setup** — hardware/network/power/display/storage scripts
   (marker-gated, non-blocking).
2. **Daemon** — start `diad-agent --daemon`, wait for `/api/v1/status` up to
   10 s; on timeout log the tail and set `FALLBACK=1`.
3. **WS1** — spawn GUI once + one retry (3 s verify each); if still not
   mapped, persist the reason to `~/.diad/.gui-fallback-reason` and open ONE
   TUI. Any "unknown" sway count is treated as *window present* — the
   supervisor never spawns a duplicate it cannot see.
4. **First-run wizard** — GUI mode: onboarding is a GUI modal; TUI mode only:
   one foot wizard. Never both.
5. **End-of-boot focus** — `swaymsg workspace 1` + focus primary output.
6. **WS2** — spawn Apps page once + one retry; no respawn loop.
7. **WS3** — spawn Launchpad once + one retry; no respawn loop.
8. **Steady state (10 s cycle)** — waybar / mako / cliphist watchdogs; daemon
   health restart (max 2, 30 s grace) then ONE diagnostics foot (marker-gated,
   never auto-reopened — WP-02/WP-07); launchpad close backstop (WP-05);
   **workspace limits watcher (WP-08)**; log rotation (1 MiB cap).

## 4. Runtime flood backstops

| Surface          | Cap | Action beyond cap                                   |
|------------------|-----|------------------------------------------------------|
| WS1 agent        | 8   | excess windows → sway scratchpad + rate-limited toast |
| WS2 apps page    | 2   | excess windows → sway scratchpad + rate-limited toast |
| WS3 launchpad    | 2   | excess windows → sway scratchpad + rate-limited toast |

Excess windows are **moved, never killed** (`Super+Shift+M` restores them).
The focused window is never touched; toast rate limit = 1/min per workspace.
Log line: `supervisor: workspace N ... over cap ... — K excess window(s) moved
to scratchpad`.

## 5. Power control parity (WP-06)

The GUI top-bar power menu (⏻) and the TUI slash commands are the SAME action
path: `/shutdown`, `/reboot`, `/sleep` → daemon confirm dialog → `device::power`
→ `sudo systemctl poweroff|reboot|suspend`.

- Confirm labels always show the **real verb**: `Run `systemctl poweroff`? This
  will power off the system.` (never `systemctl shutdown` — that is not pinned
  in sudoers).
- `sudoers.d/diad` pins exactly `/usr/bin/systemctl poweroff, reboot, suspend`
  (fixed argv — a bare `systemctl shutdown` would be rejected).
- `diad-power.sh` has a `DIAD_POWER_DRYRUN=1` mode printing
  `WOULD: systemctl <verb>` and exiting 0 before any confirmation — used by the
  regression gates to prove the mapping without touching hardware.

## 6. Verification

- **QEMU (no display):**
  ```
  qemu-system-x86_64 -enable-kvm -m 4096 -machine q35 \
    -kernel work/iso/diados/boot/x86_64/vmlinuz-linux-lts \
    -initrd work/iso/diados/boot/x86_64/initramfs-linux-lts.img \
    -append "console=ttyS0 archisobasedir=diados archisosearchuuid=2026-08-11-09-38-15-00 loglevel=7" \
    -serial file:/tmp/diad-qemu.log -display none \
    -netdev user,id=n0,hostfwd=tcp::7860-:7860 -device e1000,netdev=n0
  ```
- **Real hardware (from the ISO):**
  ```
  sudo dd if=out/diados-*.iso of=/dev/sda bs=4M status=progress conv=fsync
  ```
  Boot the USB stick, pick **DIAD** (default). See the landing checklist in
  `FINAL_REPORT.md` 6.7.
