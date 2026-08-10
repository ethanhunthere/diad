//! Performance tier detection — DIAD runs on anything from a 2010 netbook
//! to an 8K workstation.
//!
//! Single source of truth: `/etc/diad/hw-profile.json` (+ the one-word
//! `/etc/diad/perf-tier`), written by `diad-agent --perf-tier` at boot via
//! `diad-perf-tier.service` (and re-checked on major hotplug). Every
//! component — daemon thread policy, compositor effects, GUI prefetch —
//! reads the tier instead of guessing.
//!
//! Principles honored here:
//! - DETECT, NEVER ASSUME — every input comes from live sysfs/procfs/lspci.
//! - FAIL-SAFE — any detection failure falls back to `Tier::Mid` (1.9); the
//!   daemon must always boot.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

/// Where the one-word tier file lives (read by scripts + the daemon).
pub const PERF_TIER_PATH: &str = "/etc/diad/perf-tier";
/// Full machine-readable detection dump.
pub const HW_PROFILE_PATH: &str = "/etc/diad/hw-profile.json";

/// Hardware capability tier. Ordered from weakest to strongest.
///
/// Serialized as the spec's SCREAMING_SNAKE names (`ULTRA_LOW` … `ULTRA`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tier {
    #[serde(rename = "ULTRA_LOW")]
    UltraLow,
    #[serde(rename = "LOW")]
    Low,
    #[serde(rename = "MID")]
    Mid,
    #[serde(rename = "HIGH")]
    High,
    #[serde(rename = "ULTRA")]
    Ultra,
}

impl Tier {
    /// Short letter used by the status bar / bench report.
    pub fn letter(self) -> &'static str {
        match self {
            Tier::UltraLow => "UL",
            Tier::Low => "LO",
            Tier::Mid => "MID",
            Tier::High => "HI",
            Tier::Ultra => "ULT",
        }
    }

    /// Tokio worker-thread policy (spec 1.6), capped by the core count so a
    /// 1-core box never gets more workers than it has cores.
    pub fn tokio_workers(self, cores: usize) -> usize {
        let cores = cores.max(1);
        match self {
            Tier::UltraLow => 1,
            Tier::Low => 2.min(cores),
            Tier::Mid => 4.min(cores),
            Tier::High | Tier::Ultra => cores,
        }
    }

    /// Whether background prefetch of GUI chunks is allowed (spec 1.6/6.4).
    pub fn prefetch_allowed(self) -> bool {
        !matches!(self, Tier::UltraLow | Tier::Low)
    }
}

/// GPU class — drives both tier rules and the driver strategy notes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpuKind {
    IntelIgpu,
    AmdIgpu,
    AmdDgpu,
    Nvidia,
    /// VMware / VirtIO / VirtualBox virtual display adapters.
    Virtual,
    /// No accelerator detected — software rendering (llvmpipe).
    Software,
    Unknown,
}

impl GpuKind {
    fn is_integrated(self) -> bool {
        matches!(self, GpuKind::IntelIgpu | GpuKind::AmdIgpu)
    }

    fn is_dedicated(self) -> bool {
        matches!(self, GpuKind::AmdDgpu | GpuKind::Nvidia)
    }
}

/// Rotational vs solid-state boot storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageKind {
    Hdd,
    Ssd,
    Nvme,
    /// Live ISO / ram-backed root — treated like an SSD (no readahead).
    Ram,
    Unknown,
}

/// Everything detected about this machine. Serialized verbatim to
/// `/etc/diad/hw-profile.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HwProfile {
    pub ram_kb: u64,
    pub cpu_cores: usize,
    pub cpu_model: String,
    pub has_avx2: bool,
    pub has_sse4_2: bool,
    pub cpu_max_freq_khz: Option<u64>,
    pub gpu_kind: GpuKind,
    pub gpu_desc: String,
    /// Heuristic flag: GPU older than Intel HD 4000 / AMD GCN gen 1.
    pub gpu_legacy: bool,
    pub storage: StorageKind,
    pub has_thermal: bool,
    pub has_battery: bool,
    pub tier: Tier,
    pub timestamp: String,
}

impl HwProfile {
    fn ram_mb(&self) -> u64 {
        self.ram_kb / 1024
    }
}

// ---------------------------------------------------------------------------
// Detection (fail-safe: every probe defaults, never panics)
// ---------------------------------------------------------------------------

/// Parse `MemTotal` from a /proc/meminfo-format string.
fn parse_memtotal_kb(meminfo: &str) -> u64 {
    for line in meminfo.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            let kb = rest.split_whitespace().next().unwrap_or("0");
            return kb.parse().unwrap_or(0);
        }
    }
    0
}

fn read_file_trimmed(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
}

/// CPU core count, model and feature flags from /proc/cpuinfo + nproc.
fn detect_cpu() -> (usize, String, bool, bool) {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let model = cpuinfo
        .lines()
        .find_map(|l| l.strip_prefix("model name"))
        .and_then(|rest| rest.split(':').nth(1))
        .map(str::trim)
        .unwrap_or("unknown CPU")
        .to_string();
    let flags = cpuinfo
        .lines()
        .find(|l| l.starts_with("flags"))
        .unwrap_or("");
    let has_avx2 = flags.split_whitespace().any(|f| f == "avx2");
    let has_sse4_2 = flags.split_whitespace().any(|f| f == "sse4_2");
    (cores, model, has_avx2, has_sse4_2)
}

fn detect_cpu_max_freq_khz() -> Option<u64> {
    read_file_trimmed("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq")
        .and_then(|s| s.parse().ok())
}

/// Classify the first VGA/3D controller line from `lspci -nn` output.
fn parse_gpu(lspci: &str) -> (GpuKind, String, bool) {
    let line = lspci
        .lines()
        .find(|l| l.contains("VGA") || l.contains("3D controller") || l.contains("Display"))
        .unwrap_or("")
        .to_string();
    let lower = line.to_lowercase();

    if line.is_empty() || lower.contains("llvmpipe") {
        return (GpuKind::Software, line, false);
    }
    if lower.contains("vmware") || lower.contains("virtualbox") || lower.contains("virtio") {
        return (GpuKind::Virtual, line, false);
    }
    if lower.contains("nvidia") {
        return (GpuKind::Nvidia, line, false);
    }

    if lower.contains("intel") {
        // Legacy Intel: HD Graphics 2000/3000 (Sandy/Ivy bridge entry).
        let legacy = lower.contains("hd graphics 2") || lower.contains("hd graphics 3");
        return (GpuKind::IntelIgpu, line, legacy);
    }
    if lower.contains("amd") || lower.contains("ati") || lower.contains("radeon") {
        // Legacy AMD: pre-GCN naming (Radeon HD 2xxx–7xxx, no "RX"/"Vega"/"RDNA").
        let legacy = lower.contains("radeon hd")
            && !(lower.contains("rx") || lower.contains("vega") || lower.contains("rdna"));
        let integrated = lower.contains("radeon graphics")
            || lower.contains("vega")
            || (lower.contains("amd") && !lower.contains("radeon rx"));
        return (
            if integrated {
                GpuKind::AmdIgpu
            } else {
                GpuKind::AmdDgpu
            },
            line,
            legacy,
        );
    }

    (GpuKind::Unknown, line, false)
}

fn detect_gpu() -> (GpuKind, String, bool) {
    let out = Command::new("lspci").arg("-nn").output();
    match out {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout).to_string();
            parse_gpu(&text)
        }
        // No lspci (or it failed): assume software rendering so the UI
        // degrades to the safest tier instead of enabling effects a
        // nonexistent GPU cannot render (1.7 fallback).
        _ => (GpuKind::Software, "lspci unavailable".into(), false),
    }
}

/// Root storage class from /sys/block (skips virtual devices).
fn detect_storage() -> StorageKind {
    let Ok(entries) = std::fs::read_dir("/sys/block") else {
        return StorageKind::Unknown;
    };
    let mut saw_rotational = false;
    let mut saw_ssd = false;
    let mut saw_nvme = false;

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("loop")
            || name.starts_with("ram")
            || name.starts_with("zram")
            || name.starts_with("dm-")
            || name.starts_with("sr")
        {
            continue;
        }
        if name.starts_with("nvme") {
            saw_nvme = true;
            continue;
        }
        let rot =
            read_file_trimmed(&format!("/sys/block/{name}/queue/rotational")).unwrap_or_default();
        match rot.as_str() {
            "1" => saw_rotational = true,
            "0" => saw_ssd = true,
            _ => {}
        }
    }

    if saw_nvme {
        StorageKind::Nvme
    } else if saw_rotational {
        StorageKind::Hdd
    } else if saw_ssd {
        StorageKind::Ssd
    } else {
        // Live ISO root lives in RAM — no physical disk matters.
        StorageKind::Ram
    }
}

fn detect_battery() -> bool {
    std::fs::read_dir("/sys/class/power_supply")
        .map(|entries| {
            entries
                .flatten()
                .any(|e| e.file_name().to_string_lossy().starts_with("BAT"))
        })
        .unwrap_or(false)
}

fn detect_thermal() -> bool {
    std::fs::read_dir("/sys/class/thermal")
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false)
}

/// Run every probe and classify. Never fails — unknown inputs default so the
/// result is always usable (spec 1.9: default MID, log, never crash).
pub fn detect_profile() -> HwProfile {
    let mem_kb = std::fs::read_to_string("/proc/meminfo")
        .map(|s| parse_memtotal_kb(&s))
        .unwrap_or(0);
    let (cores, model, avx2, sse42) = detect_cpu();
    let (gpu_kind, gpu_desc, gpu_legacy) = detect_gpu();
    let storage = detect_storage();
    let has_battery = detect_battery();
    let has_thermal = detect_thermal();

    let profile = HwProfile {
        ram_kb: mem_kb,
        cpu_cores: cores,
        cpu_model: model,
        has_avx2: avx2,
        has_sse4_2: sse42,
        cpu_max_freq_khz: detect_cpu_max_freq_khz(),
        gpu_kind,
        gpu_desc,
        gpu_legacy,
        storage,
        has_thermal,
        has_battery,
        tier: Tier::Mid, // replaced below
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    let tier = classify(&profile);
    HwProfile { tier, ..profile }
}

/// Tier classification (spec 1.3) — the MOST RESTRICTIVE matching rule wins.
///
/// Pure function over a profile so the whole matrix is unit-testable with
/// mocked inputs (13.1).
pub fn classify(hw: &HwProfile) -> Tier {
    let ram_mb = hw.ram_mb();

    if ram_mb > 0 && ram_mb <= 2048 || hw.cpu_cores <= 1 || hw.gpu_kind == GpuKind::Software {
        return Tier::UltraLow;
    }
    if ram_mb <= 4096 || hw.cpu_cores <= 2 || hw.gpu_legacy {
        return Tier::Low;
    }
    if ram_mb <= 8192 || hw.gpu_kind.is_integrated() || hw.storage == StorageKind::Hdd {
        return Tier::Mid;
    }
    if ram_mb > 16384
        && hw.gpu_kind.is_dedicated()
        && matches!(hw.storage, StorageKind::Nvme | StorageKind::Ram)
    {
        return Tier::Ultra;
    }
    Tier::High
}

/// This process's resident set size in KB (from `/proc/self/status`).
/// 0 when unavailable (non-Linux test hosts).
pub fn read_self_rss_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("VmRSS:"))
                .and_then(|rest| rest.split_whitespace().next())
                .and_then(|kb| kb.parse().ok())
        })
        .unwrap_or(0)
}

/// Read the first battery's state from sysfs: `(on_battery, percent)`.
/// `None` when the machine has no battery (desktop / VM).
pub fn read_battery_state() -> Option<(bool, u8)> {
    let entries = std::fs::read_dir("/sys/class/power_supply").ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("BAT") {
            continue;
        }
        let base = format!("/sys/class/power_supply/{name}");
        let status = read_file_trimmed(&format!("{base}/status")).unwrap_or_default();
        let pct = read_file_trimmed(&format!("{base}/capacity"))
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(100);
        return Some((status.eq_ignore_ascii_case("discharging"), pct.min(100)));
    }
    None
}

/// Hottest thermal zone temperature in millidegrees Celsius, or `None`
/// when no zone exposes a reading.
pub fn max_thermal_millic() -> Option<u64> {
    let entries = std::fs::read_dir("/sys/class/thermal").ok()?;
    let mut max: Option<u64> = None;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("thermal_zone") {
            continue;
        }
        let temp = read_file_trimmed(&format!("/sys/class/thermal/{name}/temp"))
            .and_then(|s| s.parse().ok());
        if let Some(v) = temp {
            max = Some(max.map_or(v, |m: u64| m.max(v)));
        }
    }
    max
}

/// Read the tier chosen at boot (falls back to on-the-spot detection when the
/// file is missing — e.g. running the daemon on a dev box).
pub fn active_tier() -> Tier {
    match read_file_trimmed(PERF_TIER_PATH) {
        Some(word) => match word.as_str() {
            "ULTRA_LOW" => Tier::UltraLow,
            "LOW" => Tier::Low,
            "MID" => Tier::Mid,
            "HIGH" => Tier::High,
            "ULTRA" => Tier::Ultra,
            _ => detect_profile().tier,
        },
        None => detect_profile().tier,
    }
}

// ---------------------------------------------------------------------------
// GUI layout primitives (mirrored by CSS media queries; tested here, spec 13.1)
// ---------------------------------------------------------------------------

/// Named GUI breakpoint for a window width (spec 2.1).
pub fn breakpoint(width: u32) -> &'static str {
    match width {
        0..=799 => "TINY",
        800..=1279 => "COMPACT",
        1280..=1919 => "NORMAL",
        1920..=2999 => "WIDE",
        _ => "ULTRAWIDE",
    }
}

/// Integer-pixel snap rectangles for `n` windows inside a `w x h` output
/// (spec 2.6). Landscape: 2 = left/right, 3 = half + stacked half, 4 = 2x2.
/// Callers pass portrait outputs with swapped dimensions for vertical stacks.
///
/// Guarantees: tiles never overlap, never extend past the output, and cover
/// the whole area (no gaps) at every resolution in the spec matrix.
pub fn snap_rects(w: u32, h: u32, n: usize) -> Vec<(u32, u32, u32, u32)> {
    if w == 0 || h == 0 || n == 0 {
        return Vec::new();
    }
    match n {
        1 => vec![(0, 0, w, h)],
        2 => {
            let half = w / 2;
            vec![(0, 0, half, h), (half, 0, w - half, h)]
        }
        3 => {
            let half_w = w / 2;
            let half_h = h / 2;
            vec![
                (0, 0, half_w, h),
                (half_w, 0, w - half_w, half_h),
                (half_w, half_h, w - half_w, h - half_h),
            ]
        }
        4 => {
            let half_w = w / 2;
            let half_h = h / 2;
            vec![
                (0, 0, half_w, half_h),
                (half_w, 0, w - half_w, half_h),
                (0, half_h, half_w, h - half_h),
                (half_w, half_h, w - half_w, h - half_h),
            ]
        }
        // More than 4: rows of 2. An odd count's last tile spans the full
        // width so the output stays fully covered (no gaps, spec 2.6).
        _ => {
            let rows = n.div_ceil(2);
            let row_h = h / rows as u32;
            let half_w = w / 2;
            let mut rects = Vec::with_capacity(n);
            for i in 0..n {
                let row = (i / 2) as u32;
                let col = (i % 2) as u32;
                let y = row * row_h;
                let rh = if row + 1 == rows as u32 { h - y } else { row_h };
                let last_odd_tile = i + 1 == n && n % 2 == 1;
                if last_odd_tile {
                    rects.push((0, y, w, rh));
                } else {
                    let x = col * half_w;
                    let rw = if col == 0 { half_w } else { w - half_w };
                    rects.push((x, y, rw, rh));
                }
            }
            rects
        }
    }
}

/// Write the tier word + JSON profile. Only callable with permission to write
/// `/etc/diad` (systemd unit runs as root). Errors are returned, not fatal —
/// the caller logs and moves on (P2: degrade gracefully).
pub fn write_profiles(profile: &HwProfile) -> std::io::Result<()> {
    if let Some(parent) = Path::new(PERF_TIER_PATH).parent() {
        std::fs::create_dir_all(parent)?;
    }
    let word = serde_json::to_string(&profile.tier)
        .unwrap_or_default()
        .replace('"', "");
    std::fs::write(PERF_TIER_PATH, format!("{word}\n"))?;
    let json = serde_json::to_string_pretty(profile).unwrap_or_default();
    std::fs::write(HW_PROFILE_PATH, format!("{json}\n"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(ram_mb: u64, cores: usize, gpu: GpuKind, storage: StorageKind) -> HwProfile {
        HwProfile {
            ram_kb: ram_mb * 1024,
            cpu_cores: cores,
            cpu_model: "mock".into(),
            has_avx2: false,
            has_sse4_2: true,
            cpu_max_freq_khz: None,
            gpu_kind: gpu,
            gpu_desc: String::new(),
            gpu_legacy: false,
            storage,
            has_thermal: true,
            has_battery: false,
            tier: Tier::Mid,
            timestamp: "t".into(),
        }
    }

    // ---- Tier classification (spec 1.3, most restrictive wins) -----------

    #[test]
    fn ultra_low_rules() {
        assert_eq!(
            classify(&profile(2048, 4, GpuKind::IntelIgpu, StorageKind::Ssd)),
            Tier::UltraLow
        );
        assert_eq!(
            classify(&profile(4096, 1, GpuKind::IntelIgpu, StorageKind::Ssd)),
            Tier::UltraLow
        );
        assert_eq!(
            classify(&profile(8192, 8, GpuKind::Software, StorageKind::Ssd)),
            Tier::UltraLow
        );
    }

    #[test]
    fn low_rules() {
        assert_eq!(
            classify(&profile(4096, 8, GpuKind::IntelIgpu, StorageKind::Ssd)),
            Tier::Low
        );
        assert_eq!(
            classify(&profile(8192, 2, GpuKind::IntelIgpu, StorageKind::Ssd)),
            Tier::Low
        );
        let mut legacy = profile(8192, 8, GpuKind::IntelIgpu, StorageKind::Ssd);
        legacy.gpu_legacy = true;
        assert_eq!(classify(&legacy), Tier::Low);
    }

    #[test]
    fn mid_rules() {
        assert_eq!(
            classify(&profile(8192, 8, GpuKind::IntelIgpu, StorageKind::Ssd)),
            Tier::Mid
        );
        // 32 GB but only an iGPU → still MID (most restrictive wins).
        assert_eq!(
            classify(&profile(32768, 16, GpuKind::IntelIgpu, StorageKind::Nvme)),
            Tier::Mid
        );
        // Plenty of RAM + dGPU but spinning rust → MID.
        assert_eq!(
            classify(&profile(16384, 8, GpuKind::Nvidia, StorageKind::Hdd)),
            Tier::Mid
        );
    }

    #[test]
    fn high_and_ultra_rules() {
        assert_eq!(
            classify(&profile(16384, 8, GpuKind::Nvidia, StorageKind::Ssd)),
            Tier::High
        );
        assert_eq!(
            classify(&profile(32768, 16, GpuKind::Nvidia, StorageKind::Nvme)),
            Tier::Ultra
        );
        assert_eq!(
            classify(&profile(65536, 32, GpuKind::AmdDgpu, StorageKind::Nvme)),
            Tier::Ultra
        );
    }

    #[test]
    fn worker_policy() {
        assert_eq!(Tier::UltraLow.tokio_workers(8), 1);
        assert_eq!(Tier::Low.tokio_workers(8), 2);
        assert_eq!(Tier::Low.tokio_workers(1), 1);
        assert_eq!(Tier::Mid.tokio_workers(16), 4);
        assert_eq!(Tier::Mid.tokio_workers(2), 2);
        assert_eq!(Tier::High.tokio_workers(8), 8);
        assert_eq!(Tier::Ultra.tokio_workers(32), 32);
        assert!(!Tier::UltraLow.prefetch_allowed());
        assert!(!Tier::Low.prefetch_allowed());
        assert!(Tier::Mid.prefetch_allowed());
    }

    // ---- Breakpoints (spec 2.1) -------------------------------------------

    #[test]
    fn breakpoint_boundaries() {
        assert_eq!(breakpoint(640), "TINY");
        assert_eq!(breakpoint(799), "TINY");
        assert_eq!(breakpoint(800), "COMPACT");
        assert_eq!(breakpoint(1279), "COMPACT");
        assert_eq!(breakpoint(1280), "NORMAL");
        assert_eq!(breakpoint(1919), "NORMAL");
        assert_eq!(breakpoint(1920), "WIDE");
        assert_eq!(breakpoint(2999), "WIDE");
        assert_eq!(breakpoint(3000), "ULTRAWIDE");
        assert_eq!(breakpoint(7680), "ULTRAWIDE");
    }

    // ---- Snap geometry (spec 2.6 — every resolution in the matrix) -------

    const RESOLUTIONS: &[(u32, u32)] = &[
        (800, 600),
        (1024, 768),
        (1366, 768),
        (1440, 900),
        (1600, 900),
        (1920, 1080),
        (2048, 1152),
        (2560, 1440),
        (3440, 1440),
        (3840, 2160),
        (5120, 1440),
        (7680, 4320),
        (1080, 1920), // portrait
    ];

    /// Tiles cover the output exactly: full area, no overlaps.
    fn assert_tiling(w: u32, h: u32, n: usize) {
        let rects = snap_rects(w, h, n);
        assert_eq!(rects.len(), n, "{w}x{h} n={n}");

        let area: u64 = rects
            .iter()
            .map(|&(x, y, rw, rh)| {
                assert!(rw > 0 && rh > 0, "zero-sized tile at {w}x{h} n={n}");
                assert!(
                    x + rw <= w && y + rh <= h,
                    "tile out of bounds at {w}x{h} n={n}"
                );
                (rw as u64) * (rh as u64)
            })
            .sum();
        assert_eq!(
            area,
            (w as u64) * (h as u64),
            "gaps/overlaps at {w}x{h} n={n}"
        );

        // No pairwise overlap (area equality + containment already implies
        // it for these layouts; check explicitly anyway).
        for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                let (ax, ay, aw, ah) = rects[i];
                let (bx, by, bw, bh) = rects[j];
                let overlap = ax < bx + bw && bx < ax + aw && ay < by + bh && by < ay + ah;
                assert!(!overlap, "overlap at {w}x{h} n={n}: tiles {i},{j}");
            }
        }
    }

    #[test]
    fn snap_matrix_covers_exactly() {
        for &(w, h) in RESOLUTIONS {
            for n in 1..=6 {
                assert_tiling(w, h, n);
            }
        }
    }

    #[test]
    fn snap_two_is_left_right_landscape() {
        let r = snap_rects(1920, 1080, 2);
        assert_eq!(r[0], (0, 0, 960, 1080));
        assert_eq!(r[1], (960, 0, 960, 1080));
    }

    #[test]
    fn snap_odd_width_uses_remainder() {
        // 1366/2 = 683; the right tile absorbs the odd pixel.
        let r = snap_rects(1366, 768, 2);
        assert_eq!(r[0].2, 683);
        assert_eq!(r[1].0, 683);
        assert_eq!(r[1].2, 683);
    }

    #[test]
    fn snap_empty_inputs() {
        assert!(snap_rects(0, 1080, 2).is_empty());
        assert!(snap_rects(1920, 0, 2).is_empty());
        assert!(snap_rects(1920, 1080, 0).is_empty());
    }

    // ---- Detection parsers ------------------------------------------------

    #[test]
    fn meminfo_parser() {
        assert_eq!(
            parse_memtotal_kb("MemTotal:        8048948 kB\nOther: 1\n"),
            8048948
        );
        assert_eq!(parse_memtotal_kb("nothing here"), 0);
    }

    #[test]
    fn gpu_parser_vendors() {
        assert_eq!(parse_gpu("").0, GpuKind::Software);
        assert_eq!(
            parse_gpu(
                "00:02.0 VGA compatible controller [0300]: Intel Corporation HD Graphics 620"
            )
            .0,
            GpuKind::IntelIgpu
        );
        assert_eq!(
            parse_gpu("01:00.0 VGA compatible controller: NVIDIA Corporation GP104").0,
            GpuKind::Nvidia
        );
        assert_eq!(
            parse_gpu("00:02.0 VGA compatible controller: VMware SVGA II Adapter").0,
            GpuKind::Virtual
        );
        let (kind, _, legacy) =
            parse_gpu("00:02.0 VGA compatible controller: Intel Corporation 2nd Generation Core HD Graphics 3000");
        assert_eq!(kind, GpuKind::IntelIgpu);
        assert!(legacy);
    }
}
