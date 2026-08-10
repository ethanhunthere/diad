# DIAD — Hardware Compatibility

DIAD detects, never assumes (P1). This matrix documents what it expects,
how drivers are chosen, and where the known edges are.

## Supported hardware matrix (spec 11.1)

| Class                                  | Expected tier     | Notes                                |
|----------------------------------------|-------------------|--------------------------------------|
| 2010 netbook, 1–2 GB RAM               | `ULTRA_LOW`       | TUI preferred; no effects; 1 worker  |
| 2013 laptop, 4 GB + HDD                | `LOW`             | fade-only animations; 2 workers      |
| 2017 business laptop, 8 GB SSD iGPU    | `MID`             | default experience                   |
| 2020 laptop, 16 GB iGPU                | `HIGH`            | blur + full animations               |
| 2023 gaming laptop, dGPU               | `HIGH` / `ULTRA`  | dGPU + NVMe + >16 GB → `ULTRA`       |
| 2026 workstation, 32–64 GB, dGPU, NVMe | `ULTRA`           | HQ shadows, 120 Hz-aware animations  |
| VMs (QEMU/KVM, VirtualBox, VMware)     | forced `LOW` effects | virtual display adapters detected |

Boots on all of the above; anything that fails to classify lands on `MID`
by design (spec 1.9).

## GPU driver strategy (spec 1.7)

| GPU                          | Strategy                                             |
|------------------------------|------------------------------------------------------|
| Intel / AMD iGPU             | kernel modesetting — no extra driver                 |
| NVIDIA                       | `nouveau` by default; proprietary only if the user already installed nvidia-dkms on a Turing+ card — DIAD never auto-installs it (FOSS baseline) |
| VMware / VirtIO / VirtualBox | their DRM driver; effects capped at `LOW`            |
| No KMS device                | llvmpipe fallback → `ULTRA_LOW`; the GUI still boots |

## Storage policy (spec 1.8 / 4.6)

| Device     | Readahead | Extras                                |
|------------|-----------|---------------------------------------|
| HDD        | 256 KB    | `noatime` recommended on installs     |
| SSD / NVMe | 0         | `fstrim.timer` enabled; zram writeback off on HDD |

Live-ISO caveat: the running system is RAM-backed (erofs root + cow), so
storage policy matters for installed systems and for any `DIAD-DATA`
labelled persistence partition (mounted read-write at `/mnt/data` by
`diad-storage-setup.sh`).

## Resolution & orientation matrix (spec 11.2)

Snap geometry is exact-tiling (tested) at: 800×600, 1024×768, 1366×768,
1440×900, 1600×900, 1920×1080, 2560×1440, 3440×1440, 3840×2160, 7680×4320
and portrait 1080×1920 (vertical stacks). GUI breakpoints re-evaluate on
every resize: TINY <800 · COMPACT <1280 · NORMAL <1920 · WIDE <3000 ·
ULTRAWIDE ≥3000 (content capped at 2200 px). HiDPI scaling is computed per
output from EDID physical size: <120 dpi → 1.0 · <150 → 1.25 · <200 → 1.5 ·
≥200 → 2.0.

## Known limitations

- **Live ISO root is RAM-backed**: `vm.swappiness` etc. matter most on
  installed systems; zram still helps low-RAM live sessions.
- **Battery/thermal signals** come from sysfs polling (15 s, halved on
  battery, suspended when idle) rather than UPower D-Bus signals — no
  extra daemon dependency; the interval is idle-gated so it costs nothing
  when the agent is closed.
- **NVIDIA proprietary** is never installed automatically; on very new
  cards `nouveau` re-clocking may reduce GPU performance (CPU rendering
  budget still applies).
- **ULTRAWIDE multi-window** sway workspaces: snap layouts tile the first
  N toplevel windows; windows beyond N keep their geometry.
