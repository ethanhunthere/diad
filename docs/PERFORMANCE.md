# DIAD — Performance Budgets & Tuning

DIAD must feel instant on a 2026 workstation and stay usable on a 2010
netbook. This document is the contract: every number is a requirement
(spec §5.1), and `diad-bench.sh --check` is the enforcement tool.

## Budget table (spec 5.1)

| Metric                     | Budget                                  |
|----------------------------|-----------------------------------------|
| Boot to GUI                | < 10 s (SSD/NVMe) · < 20 s (HDD)        |
| Daemon RSS, idle stage 1   | < 80 MB                                  |
| Daemon RSS, idle stage 2   | < 50 MB                                  |
| Daemon RSS, active         | < 300 MB                                 |
| Window open                | < 300 ms                                 |
| GUI wake (idle → open)     | < 200 ms                                 |
| UI frame budget            | 16 ms on MID+ tiers · 33 ms on LOW/ULTRA_LOW |
| Idle CPU (agent closed)    | ~0 % (≤ 1 % averaged over 60 s)          |
| Total idle RAM             | < 900 MB (MID) · < 600 MB (LOW)          |
| Initial JS bundle          | < 300 KB gzipped (core entry is ~19 KB; vendor-react ~43 KB) |

## Performance tiers (spec 1.3)

Detection runs at boot (`diad-perf-tier.service` → `diad-agent --perf-tier`)
and on major hotplug (udev). The most restrictive matching rule wins:

| Tier        | Condition                                                        |
|-------------|------------------------------------------------------------------|
| `ULTRA_LOW` | RAM ≤ 2 GB · 1 core · software renderer (llvmpipe)               |
| `LOW`       | RAM ≤ 4 GB · ≤ 2 cores · GPU older than Intel HD 4000 / GCN gen1 |
| `MID`       | RAM ≤ 8 GB · integrated GPU · HDD storage                        |
| `HIGH`      | RAM ≤ 16 GB with dGPU, or modern iGPU                            |
| `ULTRA`     | RAM > 16 GB · modern dGPU · NVMe                                 |

Files written: `/etc/diad/perf-tier` (one word) and
`/etc/diad/hw-profile.json` (full dump). Any detection failure defaults to
`MID` with a log line — detection never crashes the boot (spec 1.9).

### Effect matrix (spec 1.5)

| Tier        | Blur | Animations | Shadows | Notes                          |
|-------------|------|------------|---------|--------------------------------|
| `ULTRA_LOW` | off  | none       | none    | static waybar, TUI preferred   |
| `LOW`       | off  | fade-only  | none    | simple waybar                  |
| `MID`       | off  | full       | subtle  | default                        |
| `HIGH`      | on   | full       | full    |                                |
| `ULTRA`     | on   | full       | HQ      | 120 Hz-aware animations        |

### Thread policy (spec 1.6)

| Tier        | Tokio worker threads | Prefetch          |
|-------------|----------------------|-------------------|
| `ULTRA_LOW` | 1                    | off               |
| `LOW`       | 2                    | off               |
| `MID`       | min(4, cores)        | light (hover)     |
| `HIGH`      | cores                | after first paint |
| `ULTRA`     | cores                | aggressive        |

The daemon reads the tier at startup and builds its tokio runtime with that
worker count; the GUI reads it from `/api/v1/status` (`perf_tier`) and
applies the matching prefetch policy (`frontend/src/lib/prefetch.ts`).

## Idle engine (spec §3)

The daemon tracks connected WebSocket clients. When the last client leaves:

| Stage | After          | Action                                                     | Target RSS |
|-------|----------------|------------------------------------------------------------|------------|
| 1     | 30 s           | periodic tasks off, tool cache dropped, history capped 200 | < 80 MB    |
| 2     | 5 min          | + PTY children shut down                                   | < 50 MB    |

Wake path: the first new connection re-enables every registered task and
restores `ACTIVE` in O(number of tasks) — well under the 200 ms budget.
Caches re-populate lazily. Log lines: `diad daemon idle (stage1|2)` /
`diad daemon active`.

Every periodic task lives in the central `TaskRegistry` with an atomic
enable flag; there are no spin loops and no `sleep(0)` patterns. Registered
tasks: `session-autosave`, `env-monitor` (battery + thermal),
`bluetooth-idle-stop`.

## Tuning knobs

| Knob                          | Where                                          |
|-------------------------------|------------------------------------------------|
| Tier override for experiments | `echo MID > /etc/diad/perf-tier` (reboot-free for the GUI; daemon reads at start) |
| zram size                     | `/etc/zram-generator.conf` (perf-tier script keeps it tier-correct) |
| Governor                      | written by `diad-perf-tier.sh` (schedutil / powersave) |
| Autosave interval             | `diad.toml` → `auto_save_interval` (0 = off)   |
| Stream tokens                 | `diad.toml` → `stream_tokens`                  |
| GUI font scale / density      | Settings panel (persisted in localStorage)     |

## Bench & regression guard (spec 5.2 / 5.3)

```
diad-bench.sh           # one-shot report
diad-bench.sh --watch   # 1 s sampling
diad-bench.sh --json    # machine-readable
diad-bench.sh --check   # baseline at /etc/diad/perf-baseline.json;
                        # fails (exit 1) on any metric worse than 15 %
```
