# DIAD OS — Boot Audit Report (PROMPT #3, Phase 1)

Date: 2026-08-10 · Repo HEAD: `5836304` · ISO: `out/diados-2026.08.10-x86_64.iso` (3,187,470,336 B, built 20:00)
Method: read-only audit of repo + built artifacts in `work/` (authoritative chroot copies), live QEMU/KVM boot measurements, hook source forensics.

---

## 1. BOOT ERROR INVENTORY (before)

Observed in live verbose QEMU boot (`qemu-verbose-baseline.log`, kernel 6.18.39-1-lts, initramfs from the ISO):

| # | Error / event | Where | Severity | Root cause |
|---|---|---|---|---|
| E1 | `/init: line 7: memdiskfind: not found` | initramfs, `memdisk` hook | MEDIUM (noise, tiny delay) | `memdisk` hook enabled in `HOOKS` but `memdiskfind` binary absent from initramfs |
| E2 | `archiso_pxe_common/nbd/http/nfs` hooks run, probe missing tools (`ipconfig`, `nbd-client`, `nfsmount`) | initramfs | LOW (verbose noise) | PXE hooks enabled but PXE tooling not installed |
| E3 | `systemd-firstboot.service` runs at every boot | systemd | LOW (extra unit, potential interactive stall) | no `/etc/machine-id` in the airootfs image → firstboot triggers |
| E4 | **`:: Copying image to RAM…` (2–3 min on real USB)** | archiso hook `_mnt_fs` | **CRITICAL** | `copytoram` unset → hook default `auto`; on non-optical media (`/dev/sd*`, i.e. real USB boot) with erofs 2.9 GB < 4 GiB and enough RAM → **auto-copy 2.9 GB to RAM at boot**. Verified in hook source (`work/x86_64/airootfs/usr/lib/initcpio/hooks/archiso` L221, L340–351). QEMU `-cdrom` presents as `/dev/sr*` (optical) → copy NOT triggered in QEMU → QEMU baseline misses it; real USB hardware shows it. |
| E5 | `NetworkManager-wait-online.service` re-enabled by `enable *` in preset | systemd | MEDIUM (network stall risk) | preset `mask` list covers only rfkill-poll/debug-shell/serial-getty; `enable *` re-enables wait-online units → multi-user.target waits on network |
| E6 | initramfs 216.9 MB, `xz -9e` single-threaded decompress | kernel boot | MEDIUM (boot latency; scales with CPU) | `COMPRESSION="xz"` + `COMPRESSION_OPTIONS=(-9e)` + PXE/memdisk hooks bloating image |
| E7 | `systemd-timesyncd` + `seatd` not enabled | systemd | LOW (missing features) | absent from preset enable list; units exist in image |

Not a bug (harness artifact): direct `-kernel/-initrd` boot **without** the ISO attached produced `ERROR: Device '2026-08-10-17-58-23-00' not found` → the archiso UUID search correctly failed on an empty `/dev/sr0`. With the ISO attached (real boot path) the search succeeds and login is reached. UUID file confirmed present at ISO root `/boot/2026-08-10-17-58-23-00.uuid` (0 bytes, correct).

---

## 2. SERVICE TABLE (before)

Source: `os/archiso/airootfs/usr/share/systemd/diad.preset` + `airootfs/etc/systemd/system/` + chroot units.

| Unit | State | Notes |
|---|---|---|
| diad-firewall.service | enabled | oneshot `nft -f /etc/nftables.conf`, After=network-pre.target |
| diad-perf-tier.service | enabled | oneshot, Before=multi-user.target, cheap |
| fstrim.timer | enabled | good |
| NetworkManager.service | enabled | good |
| power-profiles-daemon.service | enabled | idle daemon, negligible boot cost |
| bluetooth / cups / avahi-daemon / geoclue / ModemManager / tracker-miner | disabled | lazy on demand |
| systemd-rfkill-poll / debug-shell / serial-getty@ | masked (declared) | symlinks materialized at build (preset-all) |
| **NetworkManager-wait-online / systemd-networkd-wait-online** | **enabled via `enable *`** | **boot stall risk — must mask** |
| **systemd-timesyncd.service** | **not enabled** | must enable |
| **seatd.service** | **not enabled** | must enable (sway seat management) |
| systemd-firstboot.service | runs at boot | machine-id absent; completes non-interactively but is an extra unit |
| daemon units in `airootfs/etc/systemd/system/` | diad-firewall + diad-perf-tier only | no other diad units; no getty/basic.target wiring |

---

## 3. BASELINE TIMES (before)

| Metric | Value | How measured |
|---|---|---|
| QEMU start → login prompt (`diad login:` on ttyS0) | **12.33 s** | kernel+initrd direct boot, ISO attached, `quiet loglevel=3 systemd.show_status=0 console=ttyS0`, KVM, log mtime − t0 |
| Kernel → `/init` (initramfs unpacked) | 2.42 s | verbose log timestamps |
| initramfs size | **216,931,740 B (216.9 MB)** | `ls -l work/iso/diados/boot/x86_64/initramfs-linux-lts.img` |
| erofs image size | **2,916,855,808 B (2.9 GB)** | `work/iso/diados/x86_64/airootfs.erofs` |
| Real-USB boot stall (copy to RAM) | **2–3 min** (user-observed) | E4 auto-copy of 2.9 GB on non-optical media |

QEMU note: `-cdrom` is optical (`/dev/sr*`) so the RAM-copy auto-trigger is a **real-USB phenomenon**, not reproducible in the default QEMU harness. The optimized ISO must therefore (a) default to `copytoram=n` (boot straight from media) and (b) expose an explicit RAM mode for testing.

---

*Phase 1 complete. No project files modified. Phase 2 (S2.1–S2.8) follows.*
