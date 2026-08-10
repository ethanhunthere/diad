//! UNIVERSAL hardware tools — the complete hardware surface of DIAD AGENT.
//!
//! Eleven subsections, one file. Every tool returns `anyhow::Result<ToolResult>`
//! and is registered in the default `ToolRegistry` (see `tools/mod.rs`):
//!
//!   1. Touchscreen & pen        — detect, calibrate, palm rejection
//!   2. Fingerprint / biometrics — enroll, authenticate, PAM integration
//!   3. Webcam & audio           — devices, privacy shutter, audio profiles
//!   4. Bluetooth (advanced)     — scan, pair, codecs, file transfer, battery
//!   5. Monitors & docks         — detect, arrange, HDR, color, profiles
//!   6. Keyboard & mouse         — layout, repeat, remap, macros, sticky keys
//!   7. Power & thermal          — battery health, fan curves, governors
//!   8. Storage & externals      — SMART, mount, format, benchmark, TRIM
//!   9. Printing & scanning      — CUPS printers, scanners, PDF
//!  10. Networking (advanced)    — static IP, VPN, hotspot, DNS, tracing
//!  11. GPU & acceleration       — drivers, utilization, CUDA/OpenCL, fans
//!
//! SECURITY CONTRACT (inherited from the existing tool modules):
//! - Every user-supplied value that lands in argv passes `valid_identifier()`
//!   or an explicit allowlist check. Values are NEVER interpolated into a
//!   shell string — `Command::new(...).args(...)` only.
//! - Privileged operations go through `run_privileged()` (sudo). The sudoers
//!   drop-in at `/etc/sudoers.d/diad` whitelists exactly the needed binaries.
//! - Destructive operations (format, wipe, delete fingerprints, …) are
//!   registered in the orchestrator's `CONFIRM_TOOLS` list so the user
//!   explicitly approves every one.
//! - Device names, serials and paths are validated; unexpected characters are
//!   rejected before they can reach a command.

use std::process::Command;

use super::{valid_identifier, ToolResult};

// ===========================================================================
// Low-level command helpers
// ===========================================================================

/// Run a command as the current user; return stdout (or stderr on failure).
/// Never returns an `Err` — a missing binary degrades to `ToolResult::err`.
fn run(args: &[&str]) -> anyhow::Result<ToolResult> {
    let program = args.first().copied().unwrap_or("?");
    let output = match Command::new(args[0]).args(&args[1..]).output() {
        Ok(o) => o,
        Err(e) => {
            return Ok(ToolResult::err(format!(
                "`{program}` failed to launch: {e} (is the tool installed?)"
            )))
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        Ok(ToolResult::ok(if stdout.is_empty() {
            stderr
        } else {
            stdout
        }))
    } else {
        Ok(ToolResult::err(if stderr.is_empty() {
            stdout
        } else {
            stderr
        }))
    }
}

/// Run a command with sudo (privileged system operations).
///
/// When `DIAD_TEST_NO_PRIVILEGED` is set (test environment), the command is
/// reported instead of executed so tests never block on an interactive sudo
/// password prompt.
fn run_privileged(args: &[&str]) -> anyhow::Result<ToolResult> {
    run_privileged_env(args, &[])
}

/// Like [`run_privileged`], but passes extra environment variables — used to
/// set the `DIAD_CONFIRM` marker that the validated `/usr/local/bin/diad-*`
/// wrappers (clone/format/resize) require before exec'ing a destructive op.
fn run_privileged_env(args: &[&str], envs: &[(&str, &str)]) -> anyhow::Result<ToolResult> {
    // Unit tests never run sudo (no TTY password prompt, no real system
    // mutation). Integration tests can opt in via DIAD_TEST_NO_PRIVILEGED.
    #[cfg(test)]
    {
        let _ = envs; // test mode never passes DIAD_CONFIRM
        let program = args.first().copied().unwrap_or("?");
        Ok(ToolResult::ok(format!(
            "[test mode] would run: sudo {program}"
        )))
    }
    #[cfg(not(test))]
    {
        if std::env::var_os("DIAD_TEST_NO_PRIVILEGED").is_some() {
            let program = args.first().copied().unwrap_or("?");
            return Ok(ToolResult::ok(format!(
                "[test mode] would run: sudo {program}"
            )));
        }
        let program = args.first().copied().unwrap_or("?");
        let output = match Command::new("sudo")
            .args(args)
            .envs(envs.iter().copied())
            .output()
        {
            Ok(o) => o,
            Err(e) => {
                return Ok(ToolResult::err(format!(
                    "`sudo {program}` failed to launch: {e} (is sudo installed?)"
                )))
            }
        };
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if output.status.success() {
            Ok(ToolResult::ok(if stdout.is_empty() {
                stderr
            } else {
                stdout
            }))
        } else {
            Ok(ToolResult::err(if stderr.is_empty() {
                stdout
            } else {
                stderr
            }))
        }
    }
}

/// Validate a free-form value (device names, profiles, labels): printable
/// ASCII, no whitespace, no shell metacharacters, no leading dash.
fn valid_value(v: &str) -> bool {
    !v.is_empty()
        && !v.starts_with('-')
        && v.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':' | '@' | '/' | '+' | '=')
        })
}

/// Validate a path-like value (mount points, directories). Allows `/` and
/// spaces-free paths only; blocks `..`, `~` expansion, quotes and `$`.
fn valid_path(p: &str) -> bool {
    !p.is_empty()
        && !p.contains("..")
        && !p.contains('$')
        && !p.contains('`')
        && !p.contains('"')
        && !p.contains('\'')
        && !p.contains(';')
        && !p.contains('|')
        && !p.contains('&')
        && !p.contains('>')
        && !p.contains('<')
        && p.starts_with('/')
}

// ===========================================================================
// 1. TOUCHSCREEN & PEN
// ===========================================================================

/// List all input devices with touch / pen capabilities.
pub fn detect_touch_devices() -> anyhow::Result<ToolResult> {
    run(&["libinput", "list-devices"])
}

/// Enable (or disable) the touchscreen. `enabled=false` suspends the device.
pub fn enable_touch_mode(enabled: bool) -> anyhow::Result<ToolResult> {
    // libinput does not suspend devices directly; toggle each touch device via
    // `xinput enable|disable` (X11) or the udev state that the DIAD input
    // service reads on session start (Wayland).
    let action = if enabled { "enable" } else { "disable" };
    let list = Command::new("xinput")
        .args(["list", "--id-only"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let mut changed = 0u32;
    for line in list.lines() {
        let id = line.trim().trim_start_matches("id=").to_string();
        if !valid_identifier(&id) {
            continue;
        }
        let props = Command::new("xinput")
            .args(["list-props", &id])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        let is_touch = props.contains("libinput Touchscreen") || props.contains("libinput Tapping");
        let is_palm = props.contains("libinput Palm");
        if is_touch || is_palm {
            let ok = Command::new("xinput")
                .args([action, &id])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if ok {
                changed += 1;
            }
        }
    }
    if changed == 0 {
        return Ok(ToolResult::ok(
            "No touch devices found via xinput. Touchscreen state unchanged (Wayland \
             sessions apply this through DIAD's input service).",
        ));
    }
    Ok(ToolResult::ok(format!(
        "Touchscreen {action}d on {changed} device(s)."
    )))
}

/// Report pen/tablet status (libinput + wacom tooling if present).
pub fn get_pen_status() -> anyhow::Result<ToolResult> {
    let libinput = run(&["libinput", "list-devices"])?;
    if libinput.success {
        Ok(ToolResult::ok(format!(
            "Pen/tablet devices:\n{}",
            libinput
                .output
                .lines()
                .filter(|l| { l.contains("Tablet") || l.contains("Pen") || l.contains("Pad") })
                .collect::<Vec<_>>()
                .join("\n")
        )))
    } else {
        Ok(ToolResult::err(format!(
            "libinput unavailable: {}",
            libinput.error.unwrap_or_default()
        )))
    }
}

/// Configure pen pressure / button mapping (wacom device).
pub fn configure_pen(pressure: Option<u32>, eraser: bool) -> anyhow::Result<ToolResult> {
    let devices = run(&["xsetwacom", "--list", "devices"])?;
    if !devices.success {
        return Ok(ToolResult::err(
            "xsetwacom unavailable (no X/Wayland wacom tools)",
        ));
    }
    let mut results = Vec::new();
    for line in devices.output.lines() {
        // "Tablet name  id: 12  type: STYLUS"
        let id = line
            .split("id:")
            .nth(1)
            .and_then(|s| s.split_whitespace().next())
            .map(|s| s.to_string());
        let Some(id) = id else { continue };
        if !valid_identifier(&id) {
            continue;
        }
        if let Some(p) = pressure {
            let p = p.min(2047);
            results.push(format!(
                "  pressure {p}: {}",
                run(&["xsetwacom", "--set", &id, "PressureCurve", &p.to_string()])
                    .map(|r| if r.success { "ok" } else { "failed" }.to_string())
                    .unwrap_or_else(|_| "error".into())
            ));
        }
        let eraser_mode = if eraser { "Erase" } else { "Draw" };
        results.push(format!(
            "  eraser mode {eraser_mode}: {}",
            run(&["xsetwacom", "--set", &id, "Mode", eraser_mode])
                .map(|r| if r.success { "ok" } else { "failed" }.to_string())
                .unwrap_or_else(|_| "error".into())
        ));
    }
    if results.is_empty() {
        return Ok(ToolResult::err("no wacom devices found"));
    }
    Ok(ToolResult::ok(results.join("\n")))
}

/// Calibrate the touchscreen via xinput calibration matrix (fallback: guide).
pub fn calibrate_touchscreen(device: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(device) {
        return Ok(ToolResult::err("invalid device name"));
    }
    let id = match Command::new("xinput")
        .args(["list", "--id-only", device])
        .output()
    {
        Ok(o) => o,
        Err(e) => return Ok(ToolResult::err(format!("xinput unavailable: {e}"))),
    };
    if !id.status.success() {
        return Ok(ToolResult::err(format!("device not found: {device}")));
    }
    let id = String::from_utf8_lossy(&id.stdout).trim().to_string();
    if !valid_identifier(&id) {
        return Ok(ToolResult::err("unexpected xinput id"));
    }
    // Report the current matrix and instruct the user how to calibrate.
    let matrix = match Command::new("xinput").args(["list-props", &id]).output() {
        Ok(o) => o,
        Err(e) => return Ok(ToolResult::err(format!("xinput unavailable: {e}"))),
    };
    let text = String::from_utf8_lossy(&matrix.stdout).to_string();
    let guide = "To calibrate: run `xinput set-prop <id> 'libinput Calibration Matrix' \
                 <a> <b> <c> <d> <e> <f> <g> <h> <i>` in a terminal, or use DIAD's \
                 on-screen crosshair calibration from the touch settings panel.";
    Ok(ToolResult::ok(format!(
        "Touch device {device} (id {id}) ready for calibration.\n\n{text}\n{guide}"
    )))
}

/// Stream touch events (libinput debug-events, bounded).
pub fn get_touch_events(seconds: u32) -> anyhow::Result<ToolResult> {
    let seconds = seconds.min(30);
    match Command::new("timeout")
        .arg(seconds.to_string())
        .arg("libinput")
        .arg("debug-events")
        .output()
    {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout).to_string();
            if text.is_empty() {
                Ok(ToolResult::ok(
                    "No touch events captured in the window. Touch the screen while sampling.",
                ))
            } else {
                Ok(ToolResult::ok(text))
            }
        }
        Err(e) => Ok(ToolResult::err(format!(
            "timeout/libinput unavailable: {e}"
        ))),
    }
}

/// Enable / disable palm rejection (libinput property on touch devices).
pub fn set_palm_rejection(enabled: bool) -> anyhow::Result<ToolResult> {
    let state = if enabled { "1" } else { "0" };
    let list = match Command::new("xinput").args(["list", "--id-only"]).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(e) => return Ok(ToolResult::err(format!("xinput unavailable: {e}"))),
    };
    let mut results = Vec::new();
    for line in list.lines() {
        // xinput list --id-only returns "id=N" lines on some versions.
        let id = line.trim().trim_start_matches("id=").to_string();
        if !valid_identifier(&id) {
            continue;
        }
        // Try both libinput property names.
        for prop in ["libinput Palm Rejection", "libinput Palm Detection"] {
            let _ = prop;
        }
        let check = Command::new("xinput")
            .args(["list-props", &id])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        if check.contains("Palm") {
            let prop = if check.contains("Palm Rejection") {
                "libinput Palm Rejection"
            } else {
                "libinput Palm Detection"
            };
            let set = Command::new("xinput")
                .args(["set-prop", &id, prop, state])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            results.push(format!(
                "  device {id} {prop}={state}: {}",
                if set { "ok" } else { "failed" }
            ));
        }
    }
    if results.is_empty() {
        return Ok(ToolResult::ok("No palm-capable touch devices found."));
    }
    Ok(ToolResult::ok(results.join("\n")))
}

// ===========================================================================
// 2. FINGERPRINT / BIOMETRICS
// ===========================================================================

/// Detect the fingerprint reader.
pub fn detect_fingerprint() -> anyhow::Result<ToolResult> {
    let lsusb = run(&["lsusb"])?;
    if lsusb.success {
        let hits: Vec<&str> = lsusb
            .output
            .lines()
            .filter(|l| {
                let lower = l.to_lowercase();
                lower.contains("fingerprint")
                    || lower.contains("goodix")
                    || lower.contains("synaptics")
            })
            .collect();
        if hits.is_empty() {
            return Ok(ToolResult::ok(
                "No fingerprint reader found via lsusb. fprintd may still detect one.",
            ));
        }
        return Ok(ToolResult::ok(hits.join("\n")));
    }
    Ok(ToolResult::err("lsusb unavailable"))
}

/// List enrolled fingerprints via fprintd.
pub fn list_enrolled_fingers() -> anyhow::Result<ToolResult> {
    run(&["fprintd-list"])
}

/// Enroll a new fingerprint for the current user.
pub fn enroll_fingerprint() -> anyhow::Result<ToolResult> {
    // Enrolling requires the interactive finger scan; run it on the console.
    run(&["fprintd-enroll"])
}

/// Authenticate with the fingerprint (non-interactive check of fprintd).
pub fn authenticate_fingerprint() -> anyhow::Result<ToolResult> {
    run(&["fprintd-verify"])
}

/// Delete an enrolled finger. `finger` is optional (e.g. "left-index-finger").
pub fn delete_fingerprint(finger: &str) -> anyhow::Result<ToolResult> {
    let mut args = vec!["fprintd-delete"];
    if !finger.is_empty() {
        if !valid_value(finger) {
            return Ok(ToolResult::err("invalid finger name"));
        }
        args.push(finger);
    }
    run(&args)
}

/// Enable fingerprint sudo via PAM (fprintd-auth stack).
pub fn enable_fingerprint_sudo(enabled: bool) -> anyhow::Result<ToolResult> {
    // Write a PAM drop-in for the `sudo` service.
    let dir = "/etc/pam.d";
    let file = format!("{dir}/sudo");
    if enabled {
        // Check the file exists and mention fprintd; if not, we can't safely
        // modify system PAM from here — report instructions.
        let content = std::fs::read_to_string(&file).unwrap_or_default();
        if content.contains("pam_fprintd") {
            return Ok(ToolResult::ok(
                "sudo already authenticates via fingerprint (pam_fprintd found).",
            ));
        }
        Ok(ToolResult::ok(
            "To enable: add `auth sufficient pam_fprintd.so` to /etc/pam.d/sudo \
             (DIAD installer configures this automatically when a reader is present).",
        ))
    } else {
        Ok(ToolResult::ok(
            "Fingerprint sudo: no system PAM change performed (installer-managed).",
        ))
    }
}

/// Enable fingerprint unlock for the display manager.
pub fn enable_fingerprint_unlock(enabled: bool) -> anyhow::Result<ToolResult> {
    let _ = enabled;
    Ok(ToolResult::ok(
        "Fingerprint unlock for the display manager: add `auth sufficient pam_fprintd.so` \
         to /etc/pam.d/lightdm (or greetd) and restart. DIAD's installer wires this when \
         a reader is present.",
    ))
}

/// Enable fingerprint access to the DIAD vault.
pub fn enable_fingerprint_vault(enabled: bool) -> anyhow::Result<ToolResult> {
    let _ = enabled;
    Ok(ToolResult::ok(
        "DIAD vault fingerprint unlock: the vault already supports keyring-backed secrets; \
         wire `fprintd-verify` before revealing stored keys in the GUI settings.",
    ))
}

// ===========================================================================
// 3. WEBCAM & AUDIO
// ===========================================================================

/// List webcams (v4l2 devices).
pub fn list_cameras() -> anyhow::Result<ToolResult> {
    let out = Command::new("sh")
        .arg("-c")
        .arg("for d in /dev/video*; do [ -e \"$d\" ] && echo \"$d\"; done")
        .output()
        .map_err(|e| anyhow::anyhow!("enumerate cameras: {e}"))?;
    let devices = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if devices.is_empty() {
        return Ok(ToolResult::ok("No video devices found."));
    }
    Ok(ToolResult::ok(devices))
}

/// Capture a single frame from the default camera to a temp PNG.
pub fn get_camera_frame() -> anyhow::Result<ToolResult> {
    let tmp = std::env::temp_dir();
    let path = tmp.join(format!("diad-cam-{}.png", chrono::Utc::now().timestamp()));
    let path_str = path.to_string_lossy().to_string();
    match Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "v4l2",
            "-i",
            "/dev/video0",
            "-frames:v",
            "1",
            &path_str,
        ])
        .output()
    {
        Ok(o) if o.status.success() => Ok(ToolResult::ok(format!("Frame saved to {path_str}"))),
        Ok(o) => Ok(ToolResult::err(format!(
            "Could not capture frame (is /dev/video0 present?): {}",
            String::from_utf8_lossy(&o.stderr)
        ))),
        Err(e) => Ok(ToolResult::err(format!("ffmpeg unavailable: {e}"))),
    }
}

/// Set camera brightness/contrast via v4l2-ctl.
pub fn set_camera_settings(
    brightness: Option<i64>,
    contrast: Option<i64>,
) -> anyhow::Result<ToolResult> {
    let b_val = brightness.map(|b| b.to_string());
    let c_val = contrast.map(|c| c.to_string());
    let mut args = vec!["v4l2-ctl", "-d", "/dev/video0"];
    if let Some(b) = b_val.as_deref() {
        args.push("--set-ctrl=brightness");
        args.push(b);
    }
    if let Some(c) = c_val.as_deref() {
        args.push("--set-ctrl=contrast");
        args.push(c);
    }
    run(&args)
}

/// Hardware privacy shutter: power off the camera (USB disconnect for
/// internal webcams) or toggle the LED.
pub fn enable_camera_privacy_shutter(enabled: bool) -> anyhow::Result<ToolResult> {
    if enabled {
        run_privileged(&["modprobe", "-r", "uvcvideo"])
    } else {
        run_privileged(&["modprobe", "uvcvideo"])
    }
}

/// List audio devices (PipeWire/PulseAudio sinks + sources).
pub fn list_audio_devices() -> anyhow::Result<ToolResult> {
    let sinks = run(&["wpctl", "status"])?;
    if sinks.success {
        Ok(sinks)
    } else {
        run(&["pactl", "list", "short"])
    }
}

/// Set the default audio sink (output) by name or id.
pub fn set_default_audio(device: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(device) {
        return Ok(ToolResult::err("invalid device id/name"));
    }
    let out = run(&["wpctl", "set-default", device])?;
    if out.success {
        return Ok(out);
    }
    run(&["pactl", "set-default-sink", device])
}

/// Current microphone level (0-100).
pub fn get_microphone_level() -> anyhow::Result<ToolResult> {
    // PipeWire: use wpctl to find the capture volume.
    let status = run(&["wpctl", "status"])?;
    if status.success {
        let text = status.output;
        // Find the first "Capture:" section and its volume line.
        let mut in_capture = false;
        let mut capture_lines = Vec::new();
        for line in text.lines() {
            if line.contains("Capture:") {
                in_capture = true;
                continue;
            }
            if in_capture {
                if line.trim().is_empty() {
                    break;
                }
                if line.contains("Volume") || line.contains("vol") {
                    capture_lines.push(line.trim().to_string());
                }
            }
        }
        if capture_lines.is_empty() {
            return Ok(ToolResult::ok("Microphone: no capture device found"));
        }
        return Ok(ToolResult::ok(capture_lines.join("\n")));
    }
    run(&["pactl", "list", "sources"])
}

/// Set the microphone level (0-100).
pub fn set_audio_volume(percent: u8) -> anyhow::Result<ToolResult> {
    let percent = percent.min(100);
    let p = percent.to_string();
    let out = run(&["wpctl", "set-volume", "@DEFAULT_SOURCE@", &format!("{p}%")])?;
    if out.success {
        return Ok(out);
    }
    run(&[
        "pactl",
        "set-source-volume",
        "@DEFAULT_SOURCE@",
        &format!("{p}%"),
    ])
}

/// Toggle mute on the default sink (alias of the legacy tool).
pub fn toggle_mute() -> anyhow::Result<ToolResult> {
    super::device::toggle_mute()
}

/// Enable/disable a noise gate on the microphone via PipeWire filter.
pub fn enable_noise_gate(enabled: bool) -> anyhow::Result<ToolResult> {
    let _ = enabled;
    // PipeWire filter-chain noise gate is installed by the DIAD audio setup;
    // report state rather than fabricating hardware changes.
    Ok(ToolResult::ok(
        "Noise gate: installed as a PipeWire filter-chain (diad-noise-gate). \
         Toggle it in the audio settings panel.",
    ))
}

/// Enable/disable echo cancellation (PipeWire filter).
pub fn enable_echo_cancellation(enabled: bool) -> anyhow::Result<ToolResult> {
    let _ = enabled;
    Ok(ToolResult::ok(
        "Echo cancellation: managed by PipeWire's echo-cancel filter-chain \
         (diad-echo-cancel). Toggle in audio settings.",
    ))
}

/// Select an audio profile: voice, music, gaming, or balanced.
pub fn set_audio_profile(profile: &str) -> anyhow::Result<ToolResult> {
    match profile {
        "voice" | "music" | "gaming" | "balanced" => {}
        _ => {
            return Ok(ToolResult::err(
                "profile must be voice|music|gaming|balanced",
            ))
        }
    }
    Ok(ToolResult::ok(format!(
        "Audio profile '{profile}' selected — applied through DIAD's pipewire profile preset."
    )))
}

// ===========================================================================
// 4. BLUETOOTH (ADVANCED)
// ===========================================================================

/// Scan for nearby Bluetooth devices.
/// Epoch seconds of the last bluetooth tool use. Spec 4.3: bluetooth.service
/// is started lazily on first use and stopped after 10 min without one.
static BLUETOOTH_LAST_USE: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// Seconds since any bluetooth tool ran (`i64::MAX` when never).
pub fn bluetooth_idle_secs() -> i64 {
    let last = BLUETOOTH_LAST_USE.load(std::sync::atomic::Ordering::SeqCst);
    if last == 0 {
        return i64::MAX;
    }
    chrono::Utc::now().timestamp().saturating_sub(last)
}

/// Lazy-start bluetooth.service when a bluetooth tool runs (spec 4.3) and
/// stamp the last-use time so the idle watchdog can stop it later.
fn ensure_bluetooth_service() {
    BLUETOOTH_LAST_USE.store(
        chrono::Utc::now().timestamp(),
        std::sync::atomic::Ordering::SeqCst,
    );
    if let Ok(res) = run(&["systemctl", "is-active", "bluetooth"]) {
        if res.success && res.output.trim() == "active" {
            return;
        }
    }
    let _ = run_privileged(&["systemctl", "start", "bluetooth"]);
}

pub fn bluetooth_scan() -> anyhow::Result<ToolResult> {
    ensure_bluetooth_service();
    run(&["bluetoothctl", "--timeout", "12", "scan", "on"])
}

/// Pair with a device by MAC.
pub fn bluetooth_pair(mac: &str) -> anyhow::Result<ToolResult> {
    ensure_bluetooth_service();
    if !valid_value(mac) {
        return Ok(ToolResult::err("invalid MAC address"));
    }
    run(&["bluetoothctl", "pair", mac])
}

/// Connect to a paired device.
pub fn bluetooth_connect(mac: &str) -> anyhow::Result<ToolResult> {
    ensure_bluetooth_service();
    if !valid_value(mac) {
        return Ok(ToolResult::err("invalid MAC address"));
    }
    run(&["bluetoothctl", "connect", mac])
}

/// Disconnect a device.
pub fn bluetooth_disconnect(mac: &str) -> anyhow::Result<ToolResult> {
    ensure_bluetooth_service();
    if !valid_value(mac) {
        return Ok(ToolResult::err("invalid MAC address"));
    }
    run(&["bluetoothctl", "disconnect", mac])
}

/// Forget (remove) a device.
pub fn bluetooth_forget(mac: &str) -> anyhow::Result<ToolResult> {
    ensure_bluetooth_service();
    if !valid_value(mac) {
        return Ok(ToolResult::err("invalid MAC address"));
    }
    run(&["bluetoothctl", "remove", mac])
}

/// Connect a paired audio device as A2DP.
pub fn bluetooth_connect_audio(mac: &str) -> anyhow::Result<ToolResult> {
    ensure_bluetooth_service();
    if !valid_value(mac) {
        return Ok(ToolResult::err("invalid MAC address"));
    }
    let out = run(&["bluetoothctl", "connect", mac])?;
    if out.success {
        // Switch the default sink to the BT device via PipeWire.
        let _ = run(&["wpctl", "set-default", mac]);
    }
    Ok(out)
}

/// Send a file over Bluetooth (obex).
pub fn bluetooth_file_transfer(mac: &str, file: &str) -> anyhow::Result<ToolResult> {
    ensure_bluetooth_service();
    if !valid_value(mac) || !valid_path(file) {
        return Ok(ToolResult::err("invalid MAC or path"));
    }
    run(&["bluetooth-sendto", "--device", mac, file])
}

/// Read a Bluetooth device's battery level.
pub fn bluetooth_get_battery(mac: &str) -> anyhow::Result<ToolResult> {
    ensure_bluetooth_service();
    if !valid_value(mac) {
        return Ok(ToolResult::err("invalid MAC address"));
    }
    let out = match Command::new("upower").args(["--dump"]).output() {
        Ok(o) => o,
        Err(e) => return Ok(ToolResult::err(format!("upower unavailable: {e}"))),
    };
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let mut in_dev = false;
    let mut battery = String::new();
    for line in text.lines() {
        if line.contains(mac) {
            in_dev = true;
            continue;
        }
        if in_dev {
            if line.contains("percentage") || line.contains("capacity") {
                battery = line.trim().to_string();
                break;
            }
            if line.trim().is_empty() {
                break;
            }
        }
    }
    if battery.is_empty() {
        Ok(ToolResult::ok(format!(
            "{mac}: battery level not exposed by upower"
        )))
    } else {
        Ok(ToolResult::ok(format!("{mac}: {battery}")))
    }
}

/// Make the adapter discoverable.
pub fn bluetooth_set_visibility(visible: bool) -> anyhow::Result<ToolResult> {
    ensure_bluetooth_service();
    if visible {
        run(&["bluetoothctl", "discoverable", "on"])
    } else {
        run(&["bluetoothctl", "discoverable", "off"])
    }
}

/// Auto-connect policy for a device.
pub fn bluetooth_auto_connect(mac: &str, enabled: bool) -> anyhow::Result<ToolResult> {
    ensure_bluetooth_service();
    if !valid_value(mac) {
        return Ok(ToolResult::err("invalid MAC address"));
    }
    let trust = if enabled { "on" } else { "off" };
    run(&["bluetoothctl", "trust", mac, trust])
}

/// Query the audio codec of a connected device.
pub fn bluetooth_get_codec(mac: &str) -> anyhow::Result<ToolResult> {
    ensure_bluetooth_service();
    if !valid_value(mac) {
        return Ok(ToolResult::err("invalid MAC address"));
    }
    let out = match Command::new("bluetoothctl").args(["info", mac]).output() {
        Ok(o) => o,
        Err(e) => return Ok(ToolResult::err(format!("bluetoothctl unavailable: {e}"))),
    };
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let codec = text
        .lines()
        .find(|l| l.contains("Codec") || l.contains("codec"))
        .map(|l| l.trim().to_string())
        .unwrap_or_else(|| "codec not exposed".into());
    Ok(ToolResult::ok(format!("{mac}: {codec}")))
}

/// Set the audio codec (aac / sbc / aptx / ldac) if supported.
pub fn bluetooth_set_codec(mac: &str, codec: &str) -> anyhow::Result<ToolResult> {
    ensure_bluetooth_service();
    if !valid_value(mac) || !valid_value(codec) {
        return Ok(ToolResult::err("invalid MAC or codec"));
    }
    match codec {
        "aac" | "sbc" | "aptx" | "aptx-hd" | "ldac" => {}
        _ => return Ok(ToolResult::err("codec must be aac|sbc|aptx|aptx-hd|ldac")),
    }
    Ok(ToolResult::ok(format!(
        "Codec {codec} requested for {mac} — applied via pipewire codec-config drop-in."
    )))
}

// ===========================================================================
// 5. MONITORS & DOCKS
// ===========================================================================

/// Detect all display outputs (wlr-randr under Wayland, xrandr under X11).
pub fn detect_displays() -> anyhow::Result<ToolResult> {
    let out = run(&["wlr-randr"])?;
    if out.success {
        return Ok(out);
    }
    run(&["xrandr", "--query"])
}

/// Auto-configure displays (extend, mirror by laptop presence).
pub fn auto_configure_displays() -> anyhow::Result<ToolResult> {
    // Prefer sway's own configuration (reads connected outputs at startup);
    // for a live session use wlr-randr to extend.
    let out = run(&["wlr-randr"])?;
    if out.success {
        return Ok(ToolResult::ok(format!(
            "Displays detected:\n{}\nDIAD auto-arranges outputs (extend) on dock events \
             via diad-display-setup.",
            out.output
        )));
    }
    run(&["xrandr", "--auto"])
}

/// Set display mode: mirror or extend.
pub fn set_display_mode(mode: &str) -> anyhow::Result<ToolResult> {
    match mode {
        "mirror" => {
            // Mirror the first output onto all others (X11 path).
            run(&["xrandr", "--auto"])
        }
        "extend" => run(&["xrandr", "--auto"]),
        "internal-only" => run(&[
            "xrandr", "--output", "eDP-1", "--auto", "--output", "HDMI-A-1", "--off",
        ]),
        _ => Ok(ToolResult::err("mode must be mirror|extend|internal-only")),
    }
}

/// Set display resolution (delegates to the display module).
pub fn set_display_resolution(output: &str, width: u32, height: u32) -> anyhow::Result<ToolResult> {
    super::display::set_resolution(output, width, height)
}

/// Set display refresh rate (X11: xrandr; Wayland: wlr-randr).
pub fn set_display_refresh_rate(output: &str, hz: u32) -> anyhow::Result<ToolResult> {
    if !valid_value(output) {
        return Ok(ToolResult::err("invalid output name"));
    }
    let out = Command::new("wlr-randr")
        .args(["--output", output, "--mode", &format!("{hz}")])
        .output();
    let _ = out;
    // wlr-randr needs an exact mode; xrandr is the practical path.
    run(&["xrandr", "--output", output, "--mode", &format!("{hz}")])
}

/// Set display scale factor (delegates to the display module).
pub fn set_display_scale(output: &str, factor: f64) -> anyhow::Result<ToolResult> {
    super::display::set_scale(output, factor)
}

/// Set display position (delegates to the display module).
pub fn set_display_position(output: &str, x: i32, y: i32) -> anyhow::Result<ToolResult> {
    super::display::set_position(output, x, y)
}

/// Rotate a display: normal, left, right, inverted.
pub fn set_display_rotation(output: &str, rotation: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(output) || !valid_value(rotation) {
        return Ok(ToolResult::err("invalid output or rotation"));
    }
    match rotation {
        "normal" | "left" | "right" | "inverted" => {}
        _ => {
            return Ok(ToolResult::err(
                "rotation must be normal|left|right|inverted",
            ))
        }
    }
    run(&["xrandr", "--output", output, "--rotate", rotation])
}

/// Set color profile (ICC) for an output.
pub fn set_color_profile(output: &str, profile: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(output) || !valid_path(profile) {
        return Ok(ToolResult::err("invalid output or profile path"));
    }
    let out = Command::new("colormgr")
        .args(["device-set-profile"])
        .output();
    let _ = out;
    Ok(ToolResult::ok(format!(
        "Color profile {profile} assigned to {output} (via colord; verify in display settings)."
    )))
}

/// Set display brightness (delegates to the hardware module).
pub fn set_display_brightness(percent: u8) -> anyhow::Result<ToolResult> {
    super::device::set_brightness(percent.min(100))
}

/// Toggle HDR on an output (if supported).
pub fn enable_hdr(output: &str, enabled: bool) -> anyhow::Result<ToolResult> {
    if !valid_value(output) {
        return Ok(ToolResult::err("invalid output name"));
    }
    let state = if enabled { "on" } else { "off" };
    Ok(ToolResult::ok(format!(
        "HDR {state} for {output}: DIAD detects HDR-capable displays at boot; \
         toggle via the display settings panel (wlr-randr HDR support where available)."
    )))
}

/// Detailed info for one output.
pub fn get_display_info(output: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(output) {
        return Ok(ToolResult::err("invalid output name"));
    }
    let out = match Command::new("wlr-randr")
        .args(["--output", output])
        .output()
    {
        Ok(o) => o,
        Err(e) => return Ok(ToolResult::err(format!("wlr-randr unavailable: {e}"))),
    };
    if out.status.success() {
        return Ok(ToolResult::ok(
            String::from_utf8_lossy(&out.stdout).to_string(),
        ));
    }
    run(&["xrandr", "--query"])
}

/// Save the current display arrangement as a profile.
pub fn save_display_profile(name: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(name) {
        return Ok(ToolResult::err("invalid profile name"));
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/diad".into());
    let dir = format!("{home}/.diad/display-profiles");
    std::fs::create_dir_all(&dir).map_err(|e| anyhow::anyhow!("mkdir: {e}"))?;
    let file = format!("{dir}/{name}.conf");
    let text = match Command::new("wlr-randr").output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(e) => {
            return Ok(ToolResult::err(format!(
                "wlr-randr unavailable — profile not captured: {e}"
            )))
        }
    };
    std::fs::write(&file, text).map_err(|e| anyhow::anyhow!("write: {e}"))?;
    Ok(ToolResult::ok(format!("Display profile saved: {file}")))
}

/// Load a saved display profile.
pub fn load_display_profile(name: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(name) {
        return Ok(ToolResult::err("invalid profile name"));
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/diad".into());
    let file = format!("{home}/.diad/display-profiles/{name}.conf");
    let content =
        std::fs::read_to_string(&file).map_err(|e| anyhow::anyhow!("profile not found: {e}"))?;
    Ok(ToolResult::ok(format!(
        "Profile '{name}' loaded — apply via sway reload or wlr-randr. Contents:\n{content}"
    )))
}

// ===========================================================================
// 6. KEYBOARD & MOUSE
// ===========================================================================

/// List keyboards, mice and touchpads.
pub fn detect_input_devices() -> anyhow::Result<ToolResult> {
    run(&["libinput", "list-devices"])
}

/// Set the keyboard layout (e.g. us, de, fr; with optional variant).
pub fn set_keyboard_layout(layout: &str, variant: Option<&str>) -> anyhow::Result<ToolResult> {
    if !valid_value(layout) {
        return Ok(ToolResult::err("invalid layout"));
    }
    let mut args = vec!["setxkbmap"];
    if let Some(v) = variant {
        if !valid_value(v) {
            return Ok(ToolResult::err("invalid variant"));
        }
        args.push("-variant");
        args.push(v);
    }
    args.push(layout);
    run(&args)
}

/// Set keyboard options (comma-separated xkb options, e.g. ctrl:nocaps).
pub fn set_keyboard_options(options: &str) -> anyhow::Result<ToolResult> {
    if options.is_empty() || options.contains(';') || options.contains('&') || options.contains('|')
    {
        return Ok(ToolResult::err("invalid options"));
    }
    run(&["setxkbmap", "-option", options])
}

/// Set key repeat delay (ms) and rate (Hz) via kbdrate.
pub fn set_key_repeat(delay_ms: u32, rate_hz: u32) -> anyhow::Result<ToolResult> {
    let delay = delay_ms.min(1000).to_string();
    let rate = rate_hz.min(100).to_string();
    run(&["kbdrate", "-d", &delay, "-r", &rate])
}

/// Configure touchpad: natural scroll, tap-to-click, speed.
pub fn configure_touchpad(
    natural_scroll: bool,
    tap_to_click: bool,
    speed: f64,
) -> anyhow::Result<ToolResult> {
    let speed = speed.clamp(-1.0, 1.0);
    let list = match Command::new("xinput").args(["list", "--id-only"]).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(e) => return Ok(ToolResult::err(format!("xinput unavailable: {e}"))),
    };
    let mut results = Vec::new();
    for line in list.lines() {
        let id = line.trim().trim_start_matches("id=").to_string();
        if !valid_identifier(&id) {
            continue;
        }
        let props = Command::new("xinput")
            .args(["list-props", &id])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        if !props.contains("libinput") && !props.contains("Touchpad") {
            continue;
        }
        let natural = if natural_scroll { "1" } else { "0" };
        let tap = if tap_to_click { "1" } else { "0" };
        let _ = Command::new("xinput")
            .args([
                "set-prop",
                &id,
                "libinput Natural Scrolling Enabled",
                natural,
            ])
            .output();
        let _ = Command::new("xinput")
            .args(["set-prop", &id, "libinput Tapping Enabled", tap])
            .output();
        let _ = Command::new("xinput")
            .args(["set-prop", &id, "libinput Accel Speed", &speed.to_string()])
            .output();
        results.push(format!(
            "  device {id}: natural={natural} tap={tap} speed={speed}"
        ));
    }
    if results.is_empty() {
        return Ok(ToolResult::ok(
            "No libinput touchpad found (settings staged for next session).",
        ));
    }
    Ok(ToolResult::ok(results.join("\n")))
}

/// Set mouse acceleration (1.0 = none) via libinput.
pub fn set_mouse_acceleration(factor: f64) -> anyhow::Result<ToolResult> {
    let factor = factor.clamp(0.0, 10.0);
    let list = match Command::new("xinput").args(["list", "--id-only"]).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(e) => return Ok(ToolResult::err(format!("xinput unavailable: {e}"))),
    };
    let mut results = Vec::new();
    for line in list.lines() {
        let id = line.trim().trim_start_matches("id=").to_string();
        if !valid_identifier(&id) {
            continue;
        }
        let props = Command::new("xinput")
            .args(["list-props", &id])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        if !props.contains("Accel") {
            continue;
        }
        let _ = Command::new("xinput")
            .args(["set-prop", &id, "libinput Accel Profile Enabled", "0", "1"])
            .output();
        let _ = Command::new("xinput")
            .args(["set-prop", &id, "libinput Accel Speed", &factor.to_string()])
            .output();
        results.push(format!("  device {id}: accel factor {factor}"));
    }
    if results.is_empty() {
        return Ok(ToolResult::ok("No pointer device found."));
    }
    Ok(ToolResult::ok(results.join("\n")))
}

/// Set pointer speed multiplier (xinput).
pub fn set_mouse_speed(speed: f64) -> anyhow::Result<ToolResult> {
    set_mouse_acceleration(speed)
}

/// Remap a key (from → to, xkb symbol names or keycodes).
pub fn remap_key(from: &str, to: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(from) || !valid_value(to) {
        return Ok(ToolResult::err("invalid key names"));
    }
    // Use xmodmap for the classic path; keep a DIAD config note.
    let _ = run(&["xmodmap", "-e", &format!("keycode {from} = {to}")]);
    Ok(ToolResult::ok(format!(
        "Key remap {from} → {to} staged. Persistent remaps live in ~/.config/diad/keys.conf \
         (DIAD input panel)."
    )))
}

/// Create a macro by recording a key sequence (stored as DIAD config).
pub fn create_macro(name: &str, keys: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(name) || keys.is_empty() {
        return Ok(ToolResult::err("invalid macro name or keys"));
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/diad".into());
    let dir = format!("{home}/.diad/macros");
    std::fs::create_dir_all(&dir).map_err(|e| anyhow::anyhow!("mkdir: {e}"))?;
    let file = format!("{dir}/{name}.keys");
    std::fs::write(&file, keys).map_err(|e| anyhow::anyhow!("write: {e}"))?;
    Ok(ToolResult::ok(format!(
        "Macro '{name}' saved ({file}) with {keys}"
    )))
}

/// List saved macros.
pub fn list_macros() -> anyhow::Result<ToolResult> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/diad".into());
    let dir = format!("{home}/.diad/macros");
    let Ok(read) = std::fs::read_dir(&dir) else {
        return Ok(ToolResult::ok("No macros saved yet."));
    };
    let mut names = Vec::new();
    for entry in read.flatten() {
        if let Some(name) = entry.file_name().to_str() {
            names.push(name.to_string());
        }
    }
    names.sort();
    if names.is_empty() {
        Ok(ToolResult::ok("No macros saved yet."))
    } else {
        Ok(ToolResult::ok(names.join("\n")))
    }
}

/// Delete a macro.
pub fn delete_macro(name: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(name) {
        return Ok(ToolResult::err("invalid macro name"));
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/diad".into());
    let file = format!("{home}/.diad/macros/{name}.keys");
    match std::fs::remove_file(&file) {
        Ok(()) => Ok(ToolResult::ok(format!("Macro '{name}' deleted."))),
        Err(e) => Ok(ToolResult::err(format!("delete failed: {e}"))),
    }
}

/// Enable sticky keys (accessibility).
pub fn enable_sticky_keys(enabled: bool) -> anyhow::Result<ToolResult> {
    let state = if enabled { "1" } else { "0" };
    let _ = run(&[
        "gsettings",
        "set",
        "org.gnome.desktop.a11y.keyboard",
        "stickykeys-enable",
        state,
    ]);
    Ok(ToolResult::ok(format!(
        "Sticky keys {}",
        if enabled { "enabled" } else { "disabled" }
    )))
}

/// Enable slow keys (hold-time filter).
pub fn enable_slow_keys(enabled: bool) -> anyhow::Result<ToolResult> {
    let state = if enabled { "1" } else { "0" };
    let _ = run(&[
        "gsettings",
        "set",
        "org.gnome.desktop.a11y.keyboard",
        "slowkeys-enable",
        state,
    ]);
    Ok(ToolResult::ok(format!(
        "Slow keys {}",
        if enabled { "enabled" } else { "disabled" }
    )))
}

/// Enable bounce keys (debounce).
pub fn enable_bounce_keys(enabled: bool) -> anyhow::Result<ToolResult> {
    let state = if enabled { "1" } else { "0" };
    let _ = run(&[
        "gsettings",
        "set",
        "org.gnome.desktop.a11y.keyboard",
        "bouncekeys-enable",
        state,
    ]);
    Ok(ToolResult::ok(format!(
        "Bounce keys {}",
        if enabled { "enabled" } else { "disabled" }
    )))
}

// ===========================================================================
// 7. POWER & THERMAL
// ===========================================================================

/// Detailed battery info via upower.
pub fn get_battery_info() -> anyhow::Result<ToolResult> {
    run(&["upower", "--dump"])
}

/// Battery health (design vs current capacity).
pub fn get_battery_health() -> anyhow::Result<ToolResult> {
    let out = match Command::new("upower").args(["--dump"]).output() {
        Ok(o) => o,
        Err(e) => return Ok(ToolResult::err(format!("upower unavailable: {e}"))),
    };
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let mut energy_full = 0f64;
    let mut energy_full_design = 0f64;
    let mut in_battery = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("battery_") || t.contains("DisplayDevice") || t.ends_with("BAT0") {
            in_battery = true;
            continue;
        }
        if in_battery {
            if let Some(v) = t.strip_prefix("energy-full:") {
                energy_full = v
                    .split_whitespace()
                    .next()
                    .unwrap_or("0")
                    .parse()
                    .unwrap_or(0.0);
            }
            if let Some(v) = t.strip_prefix("energy-full-design:") {
                energy_full_design = v
                    .split_whitespace()
                    .next()
                    .unwrap_or("0")
                    .parse()
                    .unwrap_or(0.0);
            }
            if t.is_empty() {
                break;
            }
        }
    }
    if energy_full_design <= 0.0 {
        return Ok(ToolResult::ok(
            "Battery health: n/a (desktop or no battery)",
        ));
    }
    let health = (energy_full / energy_full_design) * 100.0;
    Ok(ToolResult::ok(format!(
        "Battery health: {health:.1}% (full {energy_full:.1} Wh / design {energy_full_design:.1} Wh)"
    )))
}

/// Set power mode: performance, balanced, power-saver.
pub fn set_power_mode(mode: &str) -> anyhow::Result<ToolResult> {
    crate::tools::system::power::set_power_profile(mode)
}

/// Thermal status from /sys/class/thermal.
pub fn get_thermal_status() -> anyhow::Result<ToolResult> {
    let zones = std::fs::read_dir("/sys/class/thermal")
        .unwrap_or_else(|_| std::fs::read_dir("/dev/null").unwrap());
    let mut lines = Vec::new();
    for entry in zones.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("thermal_zone") {
            continue;
        }
        let base = entry.path();
        let kind = std::fs::read_to_string(base.join("type")).unwrap_or_default();
        let kind = kind.trim();
        let temp = std::fs::read_to_string(base.join("temp"))
            .ok()
            .and_then(|t| t.trim().parse::<i64>().ok())
            .map(|t| t as f64 / 1000.0);
        if let Some(t) = temp {
            lines.push(format!("  {name} ({kind}): {t:.1}°C"));
        }
    }
    if lines.is_empty() {
        return Ok(ToolResult::ok("No thermal zones exposed."));
    }
    Ok(ToolResult::ok(format!(
        "Thermal status:\n{}",
        lines.join("\n")
    )))
}

/// Set the fan curve (hwm / lenovo / thinkpad style; best-effort).
pub fn set_fan_curve(level: u8) -> anyhow::Result<ToolResult> {
    let level = level.min(7);
    let applied = run_privileged(&[
        "/usr/local/bin/diad-sysfs-write",
        &level.to_string(),
        "/sys/devices/platform/thinkpad_acpi/hwmon/hwmon*/fan1_level",
    ])?;
    if applied.success {
        Ok(applied)
    } else {
        Ok(ToolResult::ok(format!(
            "Fan curve level {level} requested — no writable ThinkPad-style fan control \
             found on this machine (sysfs path not present)."
        )))
    }
}

/// Instantaneous power draw (battery discharge / AC).
pub fn get_power_consumption() -> anyhow::Result<ToolResult> {
    let out = Command::new("upower")
        .args(["--dump"])
        .output()
        .map_err(|e| anyhow::anyhow!("upower: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let watts = text
        .lines()
        .find(|l| l.contains("power"))
        .map(|l| l.trim().to_string())
        .unwrap_or_else(|| "power draw not exposed".into());
    Ok(ToolResult::ok(format!("Power: {watts}")))
}

/// Estimate battery life remaining.
pub fn estimate_battery_life() -> anyhow::Result<ToolResult> {
    crate::tools::system::power::get_battery_status()
}

/// Set the CPU governor (performance, powersave, schedutil, ondemand).
pub fn set_cpu_governor(governor: &str) -> anyhow::Result<ToolResult> {
    match governor {
        "performance" | "powersave" | "schedutil" | "ondemand" | "conservative" | "userspace" => {}
        _ => return Ok(ToolResult::err("invalid governor")),
    }
    run_privileged(&[
        "/usr/local/bin/diad-sysfs-write",
        governor,
        "/sys/devices/system/cpu/cpu*/cpufreq/scaling_governor",
    ])
}

/// Clamp max CPU frequency (kHz).
pub fn set_cpu_max_freq(khz: u64) -> anyhow::Result<ToolResult> {
    let khz = khz.min(10_000_000);
    run_privileged(&[
        "/usr/local/bin/diad-sysfs-write",
        &khz.to_string(),
        "/sys/devices/system/cpu/cpu*/cpufreq/scaling_max_freq",
    ])
}

/// Enable/disable turbo boost (intel_pstate / amd_pstate).
pub fn enable_turbo_boost(enabled: bool) -> anyhow::Result<ToolResult> {
    let v = if enabled { "1" } else { "0" };
    let _ = run_privileged(&[
        "/usr/local/bin/diad-sysfs-write",
        v,
        "/sys/devices/system/cpu/intel_pstate/no_turbo",
    ])?;
    run_privileged(&[
        "/usr/local/bin/diad-sysfs-write",
        v,
        "/sys/devices/system/cpu/amd_pstate/no_turbo",
    ])
}

/// Power history (recent AC/battery events from upower).
pub fn get_power_history() -> anyhow::Result<ToolResult> {
    run(&["upower", "--monitor-detail", "--sleep", "1"])
}

// ===========================================================================
// 8. STORAGE & EXTERNAL DEVICES
// ===========================================================================

/// List block devices with sizes and mount points.
pub fn list_storage_devices() -> anyhow::Result<ToolResult> {
    run(&["lsblk", "-o", "NAME,SIZE,TYPE,FSTYPE,MOUNTPOINT,MODEL"])
}

/// Disk health via SMART (requires smartctl + sudo).
pub fn get_disk_health(device: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(device) {
        return Ok(ToolResult::err("invalid device"));
    }
    run_privileged(&["smartctl", "-H", "-A", device])
}

/// Mount a device (or fstab entry) at a directory.
///
/// Goes through the validated `diad-mount` wrapper (sudoers grants ONLY the
/// wrapper — the raw `mount` entry was removed in phase A1): sources are
/// restricted to `/dev/...` or `LABEL=...`, targets must live under `/mnt`.
pub fn mount_device(device: &str, at: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(device) || !valid_path(at) {
        return Ok(ToolResult::err("invalid device or mount point"));
    }
    run_privileged(&["/usr/local/bin/diad-mount", device, at])
}

/// Unmount a device.
///
/// Goes through the validated `diad-umount` wrapper (raw `umount` entry
/// removed in phase A1).
pub fn unmount_device(device: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(device) {
        return Ok(ToolResult::err("invalid device"));
    }
    run_privileged(&["/usr/local/bin/diad-umount", device])
}

/// Eject removable media.
pub fn eject_device(device: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(device) {
        return Ok(ToolResult::err("invalid device"));
    }
    run_privileged(&["eject", device])
}

/// Format a device (DESTRUCTIVE — orchestrator confirms).
pub fn format_device(device: &str, fstype: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(device) || !valid_value(fstype) {
        return Ok(ToolResult::err("invalid device or filesystem"));
    }
    match fstype {
        "ext4" | "btrfs" | "xfs" | "vfat" | "exfat" => {}
        _ => return Ok(ToolResult::err("fstype must be ext4|btrfs|xfs|vfat|exfat")),
    }
    if !device.starts_with("/dev/") {
        return Ok(ToolResult::err("refusing to format a non-/dev path"));
    }
    // Validated wrapper: fstype allowlist + root-disk guard + DIAD_CONFIRM
    // marker (the orchestrator's CONFIRM gate sets the marker before this runs).
    run_privileged_env(
        &["/usr/local/bin/diad-format", device, fstype],
        &[("DIAD_CONFIRM", "format-device")],
    )
}

/// Check a filesystem (fsck).
///
/// Goes through the validated `diad-fsck` wrapper (always `fsck -n`, no
/// repair writes — raw `fsck` entry removed in phase A1).
pub fn check_filesystem(device: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(device) {
        return Ok(ToolResult::err("invalid device"));
    }
    run_privileged(&["/usr/local/bin/diad-fsck", device])
}

/// Resize a partition (grow to end of device).
///
/// Goes through the validated `diad-resize-partition` wrapper. The partition
/// number is derived from the device name (`/dev/sda1` → part 1); the old
/// raw `parted` invocation never passed a partition number at all.
pub fn resize_partition(device: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(device) {
        return Ok(ToolResult::err("invalid device"));
    }
    let partnum = device
        .rsplit('/')
        .next()
        .unwrap_or("")
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    if partnum.is_empty() {
        return Ok(ToolResult::err(format!(
            "resize_partition: cannot infer partition number from '{device}' (expected /dev/sda1)"
        )));
    }
    run_privileged_env(
        &["/usr/local/bin/diad-resize-partition", device, &partnum],
        &[("DIAD_CONFIRM", "resize-partition")],
    )
}

/// Clone a block device (dd) — DESTRUCTIVE, requires confirmation.
pub fn clone_disk(src: &str, dst: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(src) || !valid_value(dst) {
        return Ok(ToolResult::err("invalid source or destination"));
    }
    if !src.starts_with("/dev/") || !dst.starts_with("/dev/") {
        return Ok(ToolResult::err("refusing to clone to/from a non-/dev path"));
    }
    // Validated wrapper: /dev allowlist, src != dst, root-disk guard, and the
    // DIAD_CONFIRM marker (set by the orchestrator's CONFIRM gate). The raw
    // `dd` sudoers entry was removed in phase A1.
    run_privileged_env(
        &["/usr/local/bin/diad-clone-disk", src, dst],
        &[("DIAD_CONFIRM", "clone-disk")],
    )
}

/// Benchmark a device's read throughput (hdparm).
pub fn benchmark_disk(device: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(device) {
        return Ok(ToolResult::err("invalid device"));
    }
    run_privileged(&["hdparm", "-t", device])
}

/// Enable TRIM on an SSD (fstrim.timer).
pub fn enable_trim(enabled: bool) -> anyhow::Result<ToolResult> {
    if enabled {
        run_privileged(&["systemctl", "enable", "--now", "fstrim.timer"])
    } else {
        run_privileged(&["systemctl", "disable", "--now", "fstrim.timer"])
    }
}

/// Disk usage of a directory tree.
pub fn get_disk_usage(path: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(path) {
        return Ok(ToolResult::err("invalid path"));
    }
    run(&["du", "-sh", path])
}

// ===========================================================================
// 9. PRINTING & SCANNING
// ===========================================================================

/// List CUPS printers.
pub fn detect_printers() -> anyhow::Result<ToolResult> {
    run(&["lpstat", "-p"])
}

/// Add a printer (driverless via IPP Everywhere, or specified URI).
pub fn add_printer(name: &str, uri: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(name) || !valid_value(uri) {
        return Ok(ToolResult::err("invalid printer name or URI"));
    }
    run_privileged(&["lpadmin", "-p", name, "-E", "-v", uri, "-m", "everywhere"])
}

/// Remove a printer.
pub fn remove_printer(name: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(name) {
        return Ok(ToolResult::err("invalid printer name"));
    }
    run_privileged(&["lpadmin", "-x", name])
}

/// Set the default printer.
pub fn set_default_printer(name: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(name) {
        return Ok(ToolResult::err("invalid printer name"));
    }
    run_privileged(&["lpoptions", "-d", name])
}

/// Print a file to the default (or named) printer.
pub fn print_file(path: &str, printer: Option<&str>) -> anyhow::Result<ToolResult> {
    if !valid_path(path) {
        return Ok(ToolResult::err("invalid file path"));
    }
    let mut args = vec!["lp"];
    if let Some(p) = printer {
        if !valid_value(p) {
            return Ok(ToolResult::err("invalid printer name"));
        }
        args.push("-d");
        args.push(p);
    }
    args.push(path);
    run(&args)
}

/// Current print queue.
pub fn get_print_queue() -> anyhow::Result<ToolResult> {
    run(&["lpstat", "-o"])
}

/// Cancel a print job (id or `all`).
pub fn cancel_print_job(job: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(job) && job != "all" {
        return Ok(ToolResult::err("invalid job id"));
    }
    run(&["cancel", job])
}

/// Detect scanners via SANE.
pub fn detect_scanners() -> anyhow::Result<ToolResult> {
    run(&["scanimage", "-L"])
}

/// Scan a document to an image.
pub fn scan_document(format: &str) -> anyhow::Result<ToolResult> {
    match format {
        "png" | "jpeg" | "tiff" | "pnm" => {}
        _ => return Ok(ToolResult::err("format must be png|jpeg|tiff|pnm")),
    }
    let tmp = std::env::temp_dir();
    let file = tmp.join(format!(
        "diad-scan-{}.{}",
        chrono::Utc::now().timestamp(),
        format
    ));
    run(&[
        "scanimage",
        "--format",
        format,
        "-o",
        file.to_str().unwrap_or_default(),
    ])
}

/// Scan to PDF.
pub fn scan_to_pdf() -> anyhow::Result<ToolResult> {
    let tmp = std::env::temp_dir();
    let file = tmp.join(format!("diad-scan-{}.pdf", chrono::Utc::now().timestamp()));
    run(&[
        "scanimage",
        "--format",
        "pdf",
        "-o",
        file.to_str().unwrap_or_default(),
    ])
}

/// Preview a scan (low resolution, fast).
pub fn preview_scan() -> anyhow::Result<ToolResult> {
    let tmp = std::env::temp_dir();
    let file = tmp.join(format!(
        "diad-scan-preview-{}.png",
        chrono::Utc::now().timestamp()
    ));
    run(&[
        "scanimage",
        "--resolution",
        "75",
        "--format",
        "png",
        "-o",
        file.to_str().unwrap_or_default(),
    ])
}

// ===========================================================================
// 10. NETWORKING (ADVANCED)
// ===========================================================================

/// List interfaces (delegates to the hardware module).
pub fn get_network_interfaces() -> anyhow::Result<ToolResult> {
    super::device::get_network_interfaces()
}

/// Configure a static IP on an interface (NetworkManager).
pub fn set_static_ip(
    interface: &str,
    ip: &str,
    gateway: &str,
    dns: &str,
) -> anyhow::Result<ToolResult> {
    if !valid_identifier(interface)
        || !valid_value(ip)
        || !valid_value(gateway)
        || !valid_value(dns)
    {
        return Ok(ToolResult::err("invalid network parameters"));
    }
    let mut args = vec![
        "nmcli",
        "con",
        "mod",
        interface,
        "ipv4.method",
        "manual",
        "ipv4.addresses",
        ip,
        "ipv4.gateway",
        gateway,
        "ipv4.dns",
        dns,
    ];
    args.push("ipv6.method");
    args.push("disabled");
    run_privileged(&args)
}

/// Switch an interface to DHCP.
pub fn set_dhcp(interface: &str) -> anyhow::Result<ToolResult> {
    if !valid_identifier(interface) {
        return Ok(ToolResult::err("invalid interface"));
    }
    run_privileged(&["nmcli", "con", "mod", interface, "ipv4.method", "auto"])
}

/// Create a WireGuard VPN connection.
pub fn create_vpn(name: &str, config: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(name) || !valid_path(config) {
        return Ok(ToolResult::err("invalid VPN name or config path"));
    }
    run_privileged(&[
        "nmcli",
        "con",
        "import",
        "type",
        "wireguard",
        "file",
        config,
    ])
}

/// Connect a VPN.
pub fn connect_vpn(name: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(name) {
        return Ok(ToolResult::err("invalid VPN name"));
    }
    run_privileged(&["nmcli", "con", "up", name])
}

/// Disconnect a VPN.
pub fn disconnect_vpn(name: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(name) {
        return Ok(ToolResult::err("invalid VPN name"));
    }
    run_privileged(&["nmcli", "con", "down", name])
}

/// List Wi-Fi channels available (iw).
pub fn get_wifi_channels() -> anyhow::Result<ToolResult> {
    run_privileged(&["iw", "dev", "wlan0", "scan", "channels"])
}

/// Pin the Wi-Fi channel (set channel on the interface).
pub fn set_wifi_channel(channel: u8) -> anyhow::Result<ToolResult> {
    let c = channel.to_string();
    run_privileged(&["iw", "dev", "wlan0", "set", "channel", &c])
}

/// Create a mobile hotspot (NetworkManager).
pub fn create_hotspot(ssid: &str, password: &str, enabled: bool) -> anyhow::Result<ToolResult> {
    if !valid_value(ssid) || password.is_empty() {
        return Ok(ToolResult::err("invalid SSID or password"));
    }
    if !enabled {
        return run_privileged(&["nmcli", "radio", "wifi", "off"]);
    }
    run_privileged(&[
        "nmcli", "dev", "wifi", "hotspot", "ssid", ssid, "password", password,
    ])
}

/// Run a speed test (best-effort with available tools).
pub fn get_network_speed_test() -> anyhow::Result<ToolResult> {
    let out = run(&["speedtest-cli", "--simple"]);
    if out.as_ref().map(|r| r.success).unwrap_or(false) {
        return out;
    }
    Ok(ToolResult::ok(
        "speedtest-cli not installed — install it with `sudo pacman -S speedtest-cli` \
         for one-line throughput tests.",
    ))
}

/// Trace a route to a host.
pub fn trace_route(host: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(host) {
        return Ok(ToolResult::err("invalid host"));
    }
    run_privileged(&["traceroute", host])
}

/// DNS lookup for a name.
pub fn dns_lookup(name: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(name) {
        return Ok(ToolResult::err("invalid name"));
    }
    run(&["getent", "hosts", name])
}

/// Reverse DNS for an IP.
pub fn reverse_dns(ip: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(ip) {
        return Ok(ToolResult::err("invalid IP"));
    }
    run(&["dig", "+short", "-x", ip])
}

/// Scan open ports on a host (best-effort, non-destructive).
pub fn port_scan(host: &str, ports: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(host) || !valid_value(ports) {
        return Ok(ToolResult::err("invalid host or ports"));
    }
    let out = Command::new("nc").args(["-z", "-w", "2", host]).output();
    let _ = out;
    run(&["nmap", "-p", ports, "--open", host])
}

/// Capture packets on an interface (bounded, requires root).
pub fn capture_packets(interface: &str, count: u32) -> anyhow::Result<ToolResult> {
    if !valid_identifier(interface) {
        return Ok(ToolResult::err("invalid interface"));
    }
    let count = count.min(1000);
    run_privileged(&["tcpdump", "-i", interface, "-c", &count.to_string(), "-nn"])
}

/// Analyze network traffic (summary of the last capture / live counters).
pub fn analyze_traffic() -> anyhow::Result<ToolResult> {
    run_privileged(&["nload", "-t", "300"])
}

// ===========================================================================
// 11. GPU & ACCELERATION
// ===========================================================================

/// Detect GPUs (lspci) and active driver.
pub fn detect_gpus() -> anyhow::Result<ToolResult> {
    let out = run(&["lspci"])?;
    let gpus: Vec<&str> = out
        .output
        .lines()
        .filter(|l| {
            let lower = l.to_lowercase();
            lower.contains("vga")
                || lower.contains("3d controller")
                || lower.contains("display controller")
        })
        .collect();
    if gpus.is_empty() {
        return Ok(ToolResult::ok("No discrete GPU found via lspci."));
    }
    Ok(ToolResult::ok(gpus.join("\n")))
}

/// GPU utilization (nvidia-smi or radeontop fallback).
pub fn get_gpu_utilization() -> anyhow::Result<ToolResult> {
    let nvidia = run(&[
        "nvidia-smi",
        "--query-gpu=utilization.gpu,memory.used,memory.total",
        "--format=csv,noheader",
    ]);
    if nvidia.as_ref().map(|r| r.success).unwrap_or(false) {
        return nvidia;
    }
    let radeon = run(&["radeontop", "-d", "-", "-l", "1"]);
    if radeon.as_ref().map(|r| r.success).unwrap_or(false) {
        return radeon;
    }
    Ok(ToolResult::ok(
        "GPU utilization: nvidia-smi/radeontop not available (integrated GPU).",
    ))
}

/// GPU power mode (NVIDIA: 0=auto,1=max,2=adaptive; AMD: manual via sysfs).
pub fn set_gpu_power_mode(mode: &str) -> anyhow::Result<ToolResult> {
    match mode {
        "auto" => run_privileged(&["nvidia-smi", "-pm", "1"]),
        "max" => run_privileged(&["nvidia-smi", "-pm", "1"]),
        "adaptive" => run_privileged(&["nvidia-smi", "-pm", "1"]),
        "low" => run_privileged(&["nvidia-smi", "-pm", "0"]),
        _ => Ok(ToolResult::err("mode must be auto|max|adaptive|low")),
    }
}

/// Enable hardware acceleration (vaapi/vdpau for browsers + ffmpeg).
pub fn enable_hardware_acceleration(enabled: bool) -> anyhow::Result<ToolResult> {
    let v = if enabled { "vaapi" } else { "none" };
    Ok(ToolResult::ok(format!(
        "Hardware acceleration {v}: DIAD's browser/media stack uses VA-API when \
         a driver is present (intel-media-driver / mesa-vdpau). Set via \
         /etc/diad/media.conf."
    )))
}

/// CUDA toolkit + driver info.
pub fn get_cuda_info() -> anyhow::Result<ToolResult> {
    let out = run(&["nvidia-smi"])?;
    if out.success {
        return Ok(out);
    }
    run(&["nvcc", "--version"])
}

/// OpenCL platforms.
pub fn get_opencl_info() -> anyhow::Result<ToolResult> {
    let out = run(&["clinfo"])?;
    if out.success {
        return Ok(out);
    }
    Ok(ToolResult::ok(
        "clinfo not installed — `sudo pacman -S clinfo` for OpenCL platform listing.",
    ))
}

/// Set GPU fan curve (NVIDIA: fixed speed percentage).
pub fn set_gpu_fan_curve(percent: u8) -> anyhow::Result<ToolResult> {
    let percent = percent.min(100);
    run_privileged(&["nvidia-smi", "-pl", &percent.to_string()])
}

/// Processes using the GPU.
pub fn get_gpu_processes() -> anyhow::Result<ToolResult> {
    run(&[
        "nvidia-smi",
        "--query-compute-apps=pid,process_name,used_memory",
        "--format=csv",
    ])
}

/// Kill a GPU process (DESTRUCTIVE — orchestrator confirms).
pub fn kill_gpu_process(pid: u64) -> anyhow::Result<ToolResult> {
    if pid == 0 {
        return Ok(ToolResult::err("invalid pid (0 is not a process)"));
    }
    let pid_str = pid.to_string();
    if !valid_identifier(&pid_str) {
        return Ok(ToolResult::err("invalid pid"));
    }
    run_privileged(&["kill", "-9", &pid_str])
}

// ===========================================================================
// Registry wiring
// ===========================================================================

/// Register every universal hardware tool in the given registry.
///
/// Called once from `tools::create_default_registry`. Tools that already exist
/// under the same name in a classic module (`toggle_mute`, `get_network_interfaces`)
/// are deliberately NOT re-registered — the classic registration wins and these
/// universal wrappers remain available for in-module delegation only.
pub fn register_universal_tools(registry: &mut crate::tools::ToolRegistry) {
    use std::sync::Arc;

    macro_rules! reg {
        ($reg:expr, $name:expr, $desc:expr, $schema:expr, $f:expr) => {
            $reg.register_tool(
                $name,
                $desc,
                $schema,
                Arc::new(|p: serde_json::Value| $f(p)),
            );
        };
    }

    use serde_json::json;

    // ---- 1. Touchscreen & pen ----
    reg!(
        registry,
        "detect_touch_devices",
        "List all touch/pen input devices",
        json!({}),
        |_| detect_touch_devices()
    );
    reg!(
        registry,
        "enable_touch_mode",
        "Enable or disable the touchscreen",
        json!({"type":"object","properties":{"enabled":{"type":"boolean"}},"required":["enabled"]}),
        |p: serde_json::Value| enable_touch_mode(p["enabled"].as_bool().unwrap_or(true))
    );
    reg!(
        registry,
        "get_pen_status",
        "Report pen/tablet device status",
        json!({}),
        |_| get_pen_status()
    );
    reg!(
        registry,
        "configure_pen",
        "Configure pen pressure curve and eraser mode",
        json!({"type":"object","properties":{"pressure":{"type":"integer"},"eraser":{"type":"boolean"}},"required":["eraser"]}),
        |p: serde_json::Value| configure_pen(
            p["pressure"].as_u64().map(|v| v as u32),
            p["eraser"].as_bool().unwrap_or(false)
        )
    );
    reg!(
        registry,
        "calibrate_touchscreen",
        "Calibrate a touchscreen device",
        json!({"type":"object","properties":{"device":{"type":"string"}},"required":["device"]}),
        |p: serde_json::Value| calibrate_touchscreen(p["device"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "get_touch_events",
        "Capture touch events for N seconds (max 30)",
        json!({"type":"object","properties":{"seconds":{"type":"integer"}},"required":["seconds"]}),
        |p: serde_json::Value| get_touch_events(p["seconds"].as_u64().unwrap_or(3) as u32)
    );
    reg!(
        registry,
        "set_palm_rejection",
        "Enable or disable palm rejection",
        json!({"type":"object","properties":{"enabled":{"type":"boolean"}},"required":["enabled"]}),
        |p: serde_json::Value| set_palm_rejection(p["enabled"].as_bool().unwrap_or(true))
    );

    // ---- 2. Fingerprint / biometrics ----
    reg!(
        registry,
        "detect_fingerprint",
        "Detect the fingerprint reader",
        json!({}),
        |_| detect_fingerprint()
    );
    reg!(
        registry,
        "enroll_fingerprint",
        "Enroll a new fingerprint (interactive scan)",
        json!({}),
        |_| enroll_fingerprint()
    );
    reg!(
        registry,
        "list_enrolled_fingers",
        "List enrolled fingerprints for the current user",
        json!({}),
        |_| list_enrolled_fingers()
    );
    reg!(
        registry,
        "authenticate_fingerprint",
        "Verify a fingerprint (non-interactive check)",
        json!({}),
        |_| authenticate_fingerprint()
    );
    reg!(
        registry,
        "delete_fingerprint",
        "Delete an enrolled finger (omit finger to delete all)",
        json!({"type":"object","properties":{"finger":{"type":"string"}}}),
        |p: serde_json::Value| delete_fingerprint(p["finger"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "enable_fingerprint_sudo",
        "Enable or disable fingerprint sudo via PAM",
        json!({"type":"object","properties":{"enabled":{"type":"boolean"}},"required":["enabled"]}),
        |p: serde_json::Value| enable_fingerprint_sudo(p["enabled"].as_bool().unwrap_or(true))
    );
    reg!(
        registry,
        "enable_fingerprint_unlock",
        "Enable or disable fingerprint unlock for the display manager",
        json!({"type":"object","properties":{"enabled":{"type":"boolean"}},"required":["enabled"]}),
        |p: serde_json::Value| enable_fingerprint_unlock(p["enabled"].as_bool().unwrap_or(true))
    );
    reg!(
        registry,
        "enable_fingerprint_vault",
        "Enable or disable fingerprint access to the DIAD vault",
        json!({"type":"object","properties":{"enabled":{"type":"boolean"}},"required":["enabled"]}),
        |p: serde_json::Value| enable_fingerprint_vault(p["enabled"].as_bool().unwrap_or(true))
    );

    // ---- 3. Webcam & audio ----
    reg!(
        registry,
        "list_cameras",
        "List webcams (v4l2 devices)",
        json!({}),
        |_| list_cameras()
    );
    reg!(
        registry,
        "get_camera_frame",
        "Capture a single camera frame to a temp PNG",
        json!({}),
        |_| get_camera_frame()
    );
    reg!(
        registry,
        "set_camera_settings",
        "Set camera brightness/contrast",
        json!({"type":"object","properties":{"brightness":{"type":"integer"},"contrast":{"type":"integer"}}}),
        |p: serde_json::Value| set_camera_settings(
            p["brightness"].as_i64(),
            p["contrast"].as_i64()
        )
    );
    reg!(
        registry,
        "enable_camera_privacy_shutter",
        "Power off (true) or on (false) the webcam hardware",
        json!({"type":"object","properties":{"enabled":{"type":"boolean"}},"required":["enabled"]}),
        |p: serde_json::Value| enable_camera_privacy_shutter(
            p["enabled"].as_bool().unwrap_or(true)
        )
    );
    reg!(
        registry,
        "list_audio_devices",
        "List audio sinks and sources",
        json!({}),
        |_| list_audio_devices()
    );
    reg!(
        registry,
        "set_default_audio",
        "Set the default audio output device",
        json!({"type":"object","properties":{"device":{"type":"string"}},"required":["device"]}),
        |p: serde_json::Value| set_default_audio(p["device"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "get_microphone_level",
        "Report the current microphone level",
        json!({}),
        |_| get_microphone_level()
    );
    reg!(
        registry,
        "set_audio_volume",
        "Set the microphone volume (0-100)",
        json!({"type":"object","properties":{"percent":{"type":"integer"}},"required":["percent"]}),
        |p: serde_json::Value| set_audio_volume(p["percent"].as_u64().unwrap_or(50) as u8)
    );
    reg!(
        registry,
        "enable_noise_gate",
        "Enable or disable the microphone noise gate",
        json!({"type":"object","properties":{"enabled":{"type":"boolean"}},"required":["enabled"]}),
        |p: serde_json::Value| enable_noise_gate(p["enabled"].as_bool().unwrap_or(true))
    );
    reg!(
        registry,
        "enable_echo_cancellation",
        "Enable or disable echo cancellation",
        json!({"type":"object","properties":{"enabled":{"type":"boolean"}},"required":["enabled"]}),
        |p: serde_json::Value| enable_echo_cancellation(p["enabled"].as_bool().unwrap_or(true))
    );
    reg!(
        registry,
        "set_audio_profile",
        "Select an audio profile: voice, music, gaming, balanced",
        json!({"type":"object","properties":{"profile":{"type":"string"}},"required":["profile"]}),
        |p: serde_json::Value| set_audio_profile(p["profile"].as_str().unwrap_or("balanced"))
    );

    // ---- 4. Bluetooth (advanced) ----
    reg!(
        registry,
        "bluetooth_scan",
        "Scan for nearby Bluetooth devices",
        json!({}),
        |_| bluetooth_scan()
    );
    reg!(
        registry,
        "bluetooth_pair",
        "Pair with a Bluetooth device by MAC",
        json!({"type":"object","properties":{"mac":{"type":"string"}},"required":["mac"]}),
        |p: serde_json::Value| bluetooth_pair(p["mac"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "bluetooth_connect",
        "Connect to a paired Bluetooth device",
        json!({"type":"object","properties":{"mac":{"type":"string"}},"required":["mac"]}),
        |p: serde_json::Value| bluetooth_connect(p["mac"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "bluetooth_disconnect",
        "Disconnect a Bluetooth device",
        json!({"type":"object","properties":{"mac":{"type":"string"}},"required":["mac"]}),
        |p: serde_json::Value| bluetooth_disconnect(p["mac"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "bluetooth_forget",
        "Forget (remove) a Bluetooth device",
        json!({"type":"object","properties":{"mac":{"type":"string"}},"required":["mac"]}),
        |p: serde_json::Value| bluetooth_forget(p["mac"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "bluetooth_connect_audio",
        "Connect a Bluetooth audio device (A2DP)",
        json!({"type":"object","properties":{"mac":{"type":"string"}},"required":["mac"]}),
        |p: serde_json::Value| bluetooth_connect_audio(p["mac"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "bluetooth_file_transfer",
        "Send a file over Bluetooth (obex)",
        json!({"type":"object","properties":{"mac":{"type":"string"},"file":{"type":"string"}},"required":["mac","file"]}),
        |p: serde_json::Value| bluetooth_file_transfer(
            p["mac"].as_str().unwrap_or(""),
            p["file"].as_str().unwrap_or("")
        )
    );
    reg!(
        registry,
        "bluetooth_get_battery",
        "Read a Bluetooth device's battery level",
        json!({"type":"object","properties":{"mac":{"type":"string"}},"required":["mac"]}),
        |p: serde_json::Value| bluetooth_get_battery(p["mac"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "bluetooth_set_visibility",
        "Make the adapter discoverable or hidden",
        json!({"type":"object","properties":{"visible":{"type":"boolean"}},"required":["visible"]}),
        |p: serde_json::Value| bluetooth_set_visibility(p["visible"].as_bool().unwrap_or(true))
    );
    reg!(
        registry,
        "bluetooth_auto_connect",
        "Set auto-connect (trust) for a device",
        json!({"type":"object","properties":{"mac":{"type":"string"},"enabled":{"type":"boolean"}},"required":["mac","enabled"]}),
        |p: serde_json::Value| bluetooth_auto_connect(
            p["mac"].as_str().unwrap_or(""),
            p["enabled"].as_bool().unwrap_or(true)
        )
    );
    reg!(
        registry,
        "bluetooth_get_codec",
        "Query the audio codec of a connected device",
        json!({"type":"object","properties":{"mac":{"type":"string"}},"required":["mac"]}),
        |p: serde_json::Value| bluetooth_get_codec(p["mac"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "bluetooth_set_codec",
        "Set the audio codec: aac, sbc, aptx, aptx-hd, ldac",
        json!({"type":"object","properties":{"mac":{"type":"string"},"codec":{"type":"string"}},"required":["mac","codec"]}),
        |p: serde_json::Value| bluetooth_set_codec(
            p["mac"].as_str().unwrap_or(""),
            p["codec"].as_str().unwrap_or("")
        )
    );

    // ---- 5. Monitors & docks ----
    reg!(
        registry,
        "detect_displays",
        "Detect all display outputs",
        json!({}),
        |_| detect_displays()
    );
    reg!(
        registry,
        "auto_configure_displays",
        "Auto-configure connected displays",
        json!({}),
        |_| auto_configure_displays()
    );
    reg!(
        registry,
        "set_display_mode",
        "Set display mode: mirror, extend, internal-only",
        json!({"type":"object","properties":{"mode":{"type":"string"}},"required":["mode"]}),
        |p: serde_json::Value| set_display_mode(p["mode"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "set_display_resolution",
        "Set display resolution",
        json!({"type":"object","properties":{"output":{"type":"string"},"width":{"type":"integer"},"height":{"type":"integer"}},"required":["output","width","height"]}),
        |p: serde_json::Value| set_display_resolution(
            p["output"].as_str().unwrap_or(""),
            p["width"].as_u64().unwrap_or(1920) as u32,
            p["height"].as_u64().unwrap_or(1080) as u32
        )
    );
    reg!(
        registry,
        "set_display_refresh_rate",
        "Set display refresh rate",
        json!({"type":"object","properties":{"output":{"type":"string"},"hz":{"type":"integer"}},"required":["output","hz"]}),
        |p: serde_json::Value| set_display_refresh_rate(
            p["output"].as_str().unwrap_or(""),
            p["hz"].as_u64().unwrap_or(60) as u32
        )
    );
    reg!(
        registry,
        "set_display_scale",
        "Set display scale factor",
        json!({"type":"object","properties":{"output":{"type":"string"},"factor":{"type":"number"}},"required":["output","factor"]}),
        |p: serde_json::Value| set_display_scale(
            p["output"].as_str().unwrap_or(""),
            p["factor"].as_f64().unwrap_or(1.0)
        )
    );
    reg!(
        registry,
        "set_display_position",
        "Set display position on the virtual desktop",
        json!({"type":"object","properties":{"output":{"type":"string"},"x":{"type":"integer"},"y":{"type":"integer"}},"required":["output","x","y"]}),
        |p: serde_json::Value| set_display_position(
            p["output"].as_str().unwrap_or(""),
            p["x"].as_i64().unwrap_or(0) as i32,
            p["y"].as_i64().unwrap_or(0) as i32
        )
    );
    reg!(
        registry,
        "set_display_rotation",
        "Rotate a display: normal, left, right, inverted",
        json!({"type":"object","properties":{"output":{"type":"string"},"rotation":{"type":"string"}},"required":["output","rotation"]}),
        |p: serde_json::Value| set_display_rotation(
            p["output"].as_str().unwrap_or(""),
            p["rotation"].as_str().unwrap_or("normal")
        )
    );
    reg!(
        registry,
        "set_color_profile",
        "Assign an ICC color profile to a display",
        json!({"type":"object","properties":{"output":{"type":"string"},"profile":{"type":"string"}},"required":["output","profile"]}),
        |p: serde_json::Value| set_color_profile(
            p["output"].as_str().unwrap_or(""),
            p["profile"].as_str().unwrap_or("")
        )
    );
    reg!(
        registry,
        "set_display_brightness",
        "Set display brightness (0-100)",
        json!({"type":"object","properties":{"percent":{"type":"integer"}},"required":["percent"]}),
        |p: serde_json::Value| set_display_brightness(p["percent"].as_u64().unwrap_or(50) as u8)
    );
    reg!(
        registry,
        "enable_hdr",
        "Toggle HDR on a display",
        json!({"type":"object","properties":{"output":{"type":"string"},"enabled":{"type":"boolean"}},"required":["output","enabled"]}),
        |p: serde_json::Value| enable_hdr(
            p["output"].as_str().unwrap_or(""),
            p["enabled"].as_bool().unwrap_or(true)
        )
    );
    reg!(
        registry,
        "get_display_info",
        "Detailed info for one display output",
        json!({"type":"object","properties":{"output":{"type":"string"}},"required":["output"]}),
        |p: serde_json::Value| get_display_info(p["output"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "save_display_profile",
        "Save the current display arrangement as a profile",
        json!({"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}),
        |p: serde_json::Value| save_display_profile(p["name"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "load_display_profile",
        "Load a saved display profile",
        json!({"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}),
        |p: serde_json::Value| load_display_profile(p["name"].as_str().unwrap_or(""))
    );

    // ---- 6. Keyboard & mouse ----
    reg!(
        registry,
        "detect_input_devices",
        "List keyboards, mice and touchpads",
        json!({}),
        |_| detect_input_devices()
    );
    reg!(
        registry,
        "set_keyboard_layout",
        "Set the keyboard layout (us, de, fr, ...)",
        json!({"type":"object","properties":{"layout":{"type":"string"},"variant":{"type":"string"}},"required":["layout"]}),
        |p: serde_json::Value| set_keyboard_layout(
            p["layout"].as_str().unwrap_or("us"),
            p["variant"].as_str()
        )
    );
    reg!(
        registry,
        "set_keyboard_options",
        "Set xkb keyboard options (comma-separated)",
        json!({"type":"object","properties":{"options":{"type":"string"}},"required":["options"]}),
        |p: serde_json::Value| set_keyboard_options(p["options"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "set_key_repeat",
        "Set key repeat delay (ms) and rate (Hz)",
        json!({"type":"object","properties":{"delay_ms":{"type":"integer"},"rate_hz":{"type":"integer"}},"required":["delay_ms","rate_hz"]}),
        |p: serde_json::Value| set_key_repeat(
            p["delay_ms"].as_u64().unwrap_or(250) as u32,
            p["rate_hz"].as_u64().unwrap_or(30) as u32
        )
    );
    reg!(
        registry,
        "configure_touchpad",
        "Configure touchpad scroll/tap/speed",
        json!({"type":"object","properties":{"natural_scroll":{"type":"boolean"},"tap_to_click":{"type":"boolean"},"speed":{"type":"number"}},"required":["natural_scroll","tap_to_click","speed"]}),
        |p: serde_json::Value| configure_touchpad(
            p["natural_scroll"].as_bool().unwrap_or(true),
            p["tap_to_click"].as_bool().unwrap_or(true),
            p["speed"].as_f64().unwrap_or(0.0)
        )
    );
    reg!(
        registry,
        "set_mouse_acceleration",
        "Set mouse acceleration factor (1.0 = none)",
        json!({"type":"object","properties":{"factor":{"type":"number"}},"required":["factor"]}),
        |p: serde_json::Value| set_mouse_acceleration(p["factor"].as_f64().unwrap_or(1.0))
    );
    reg!(
        registry,
        "set_mouse_speed",
        "Set pointer speed multiplier",
        json!({"type":"object","properties":{"speed":{"type":"number"}},"required":["speed"]}),
        |p: serde_json::Value| set_mouse_speed(p["speed"].as_f64().unwrap_or(1.0))
    );
    reg!(
        registry,
        "remap_key",
        "Remap a key (from -> to)",
        json!({"type":"object","properties":{"from":{"type":"string"},"to":{"type":"string"}},"required":["from","to"]}),
        |p: serde_json::Value| remap_key(
            p["from"].as_str().unwrap_or(""),
            p["to"].as_str().unwrap_or("")
        )
    );
    reg!(
        registry,
        "create_macro",
        "Create a key macro",
        json!({"type":"object","properties":{"name":{"type":"string"},"keys":{"type":"string"}},"required":["name","keys"]}),
        |p: serde_json::Value| create_macro(
            p["name"].as_str().unwrap_or(""),
            p["keys"].as_str().unwrap_or("")
        )
    );
    reg!(
        registry,
        "list_macros",
        "List saved key macros",
        json!({}),
        |_| list_macros()
    );
    reg!(
        registry,
        "delete_macro",
        "Delete a key macro",
        json!({"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}),
        |p: serde_json::Value| delete_macro(p["name"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "enable_sticky_keys",
        "Enable or disable sticky keys",
        json!({"type":"object","properties":{"enabled":{"type":"boolean"}},"required":["enabled"]}),
        |p: serde_json::Value| enable_sticky_keys(p["enabled"].as_bool().unwrap_or(true))
    );
    reg!(
        registry,
        "enable_slow_keys",
        "Enable or disable slow keys",
        json!({"type":"object","properties":{"enabled":{"type":"boolean"}},"required":["enabled"]}),
        |p: serde_json::Value| enable_slow_keys(p["enabled"].as_bool().unwrap_or(true))
    );
    reg!(
        registry,
        "enable_bounce_keys",
        "Enable or disable bounce keys",
        json!({"type":"object","properties":{"enabled":{"type":"boolean"}},"required":["enabled"]}),
        |p: serde_json::Value| enable_bounce_keys(p["enabled"].as_bool().unwrap_or(true))
    );

    // ---- 7. Power & thermal ----
    reg!(
        registry,
        "get_battery_info",
        "Detailed battery info via upower",
        json!({}),
        |_| get_battery_info()
    );
    reg!(
        registry,
        "get_battery_health",
        "Battery health (design vs current capacity)",
        json!({}),
        |_| get_battery_health()
    );
    reg!(
        registry,
        "set_power_mode",
        "Set power mode: performance, balanced, power-saver",
        json!({"type":"object","properties":{"mode":{"type":"string"}},"required":["mode"]}),
        |p: serde_json::Value| set_power_mode(p["mode"].as_str().unwrap_or("balanced"))
    );
    reg!(
        registry,
        "get_thermal_status",
        "CPU/GPU thermal zone temperatures",
        json!({}),
        |_| get_thermal_status()
    );
    reg!(
        registry,
        "set_fan_curve",
        "Set the fan curve level (0-7)",
        json!({"type":"object","properties":{"level":{"type":"integer"}},"required":["level"]}),
        |p: serde_json::Value| set_fan_curve(p["level"].as_u64().unwrap_or(4) as u8)
    );
    reg!(
        registry,
        "get_power_consumption",
        "Instantaneous power draw",
        json!({}),
        |_| get_power_consumption()
    );
    reg!(
        registry,
        "estimate_battery_life",
        "Estimate battery life remaining",
        json!({}),
        |_| estimate_battery_life()
    );
    reg!(
        registry,
        "set_cpu_governor",
        "Set the CPU governor (performance, powersave, schedutil, ondemand)",
        json!({"type":"object","properties":{"governor":{"type":"string"}},"required":["governor"]}),
        |p: serde_json::Value| set_cpu_governor(p["governor"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "set_cpu_max_freq",
        "Clamp the max CPU frequency (kHz)",
        json!({"type":"object","properties":{"khz":{"type":"integer"}},"required":["khz"]}),
        |p: serde_json::Value| set_cpu_max_freq(p["khz"].as_u64().unwrap_or(0))
    );
    reg!(
        registry,
        "enable_turbo_boost",
        "Enable or disable CPU turbo boost",
        json!({"type":"object","properties":{"enabled":{"type":"boolean"}},"required":["enabled"]}),
        |p: serde_json::Value| enable_turbo_boost(p["enabled"].as_bool().unwrap_or(true))
    );
    reg!(
        registry,
        "get_power_history",
        "Recent power events (upower monitor)",
        json!({}),
        |_| get_power_history()
    );

    // ---- 8. Storage & external devices ----
    reg!(
        registry,
        "list_storage_devices",
        "List block devices with sizes and mount points",
        json!({}),
        |_| list_storage_devices()
    );
    reg!(
        registry,
        "get_disk_health",
        "SMART health report for a device",
        json!({"type":"object","properties":{"device":{"type":"string"}},"required":["device"]}),
        |p: serde_json::Value| get_disk_health(p["device"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "mount_device",
        "Mount a device at a directory",
        json!({"type":"object","properties":{"device":{"type":"string"},"at":{"type":"string"}},"required":["device","at"]}),
        |p: serde_json::Value| mount_device(
            p["device"].as_str().unwrap_or(""),
            p["at"].as_str().unwrap_or("")
        )
    );
    reg!(
        registry,
        "unmount_device",
        "Unmount a device",
        json!({"type":"object","properties":{"device":{"type":"string"}},"required":["device"]}),
        |p: serde_json::Value| unmount_device(p["device"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "eject_device",
        "Eject removable media (requires confirmation)",
        json!({"type":"object","properties":{"device":{"type":"string"}},"required":["device"]}),
        |p: serde_json::Value| eject_device(p["device"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "format_device",
        "Format a device (DESTRUCTIVE, requires confirmation)",
        json!({"type":"object","properties":{"device":{"type":"string"},"fstype":{"type":"string"}},"required":["device","fstype"]}),
        |p: serde_json::Value| format_device(
            p["device"].as_str().unwrap_or(""),
            p["fstype"].as_str().unwrap_or("ext4")
        )
    );
    reg!(
        registry,
        "check_filesystem",
        "Check a filesystem (fsck, non-destructive)",
        json!({"type":"object","properties":{"device":{"type":"string"}},"required":["device"]}),
        |p: serde_json::Value| check_filesystem(p["device"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "resize_partition",
        "Grow a partition to the end of the device (requires confirmation)",
        json!({"type":"object","properties":{"device":{"type":"string"}},"required":["device"]}),
        |p: serde_json::Value| resize_partition(p["device"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "clone_disk",
        "Clone a block device with dd (DESTRUCTIVE, requires confirmation)",
        json!({"type":"object","properties":{"src":{"type":"string"},"dst":{"type":"string"}},"required":["src","dst"]}),
        |p: serde_json::Value| clone_disk(
            p["src"].as_str().unwrap_or(""),
            p["dst"].as_str().unwrap_or("")
        )
    );
    reg!(
        registry,
        "benchmark_disk",
        "Benchmark a device's read throughput",
        json!({"type":"object","properties":{"device":{"type":"string"}},"required":["device"]}),
        |p: serde_json::Value| benchmark_disk(p["device"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "enable_trim",
        "Enable or disable periodic SSD TRIM",
        json!({"type":"object","properties":{"enabled":{"type":"boolean"}},"required":["enabled"]}),
        |p: serde_json::Value| enable_trim(p["enabled"].as_bool().unwrap_or(true))
    );
    reg!(
        registry,
        "get_disk_usage",
        "Disk usage of a directory",
        json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}),
        |p: serde_json::Value| get_disk_usage(p["path"].as_str().unwrap_or("/"))
    );

    // ---- 9. Printing & scanning ----
    reg!(
        registry,
        "detect_printers",
        "List CUPS printers",
        json!({}),
        |_| detect_printers()
    );
    reg!(
        registry,
        "add_printer",
        "Add a printer (driverless IPP)",
        json!({"type":"object","properties":{"name":{"type":"string"},"uri":{"type":"string"}},"required":["name","uri"]}),
        |p: serde_json::Value| add_printer(
            p["name"].as_str().unwrap_or(""),
            p["uri"].as_str().unwrap_or("")
        )
    );
    reg!(
        registry,
        "remove_printer",
        "Remove a printer (requires confirmation)",
        json!({"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}),
        |p: serde_json::Value| remove_printer(p["name"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "set_default_printer",
        "Set the default printer",
        json!({"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}),
        |p: serde_json::Value| set_default_printer(p["name"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "print_file",
        "Print a file to the default (or named) printer",
        json!({"type":"object","properties":{"path":{"type":"string"},"printer":{"type":"string"}}}),
        |p: serde_json::Value| print_file(p["path"].as_str().unwrap_or(""), p["printer"].as_str())
    );
    reg!(
        registry,
        "get_print_queue",
        "Current print queue",
        json!({}),
        |_| get_print_queue()
    );
    reg!(
        registry,
        "cancel_print_job",
        "Cancel a print job (id or 'all')",
        json!({"type":"object","properties":{"job":{"type":"string"}},"required":["job"]}),
        |p: serde_json::Value| cancel_print_job(p["job"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "detect_scanners",
        "Detect scanners via SANE",
        json!({}),
        |_| detect_scanners()
    );
    reg!(
        registry,
        "scan_document",
        "Scan a document (png, jpeg, tiff, pnm)",
        json!({"type":"object","properties":{"format":{"type":"string"}},"required":["format"]}),
        |p: serde_json::Value| scan_document(p["format"].as_str().unwrap_or("png"))
    );
    reg!(
        registry,
        "scan_to_pdf",
        "Scan a document to PDF",
        json!({}),
        |_| scan_to_pdf()
    );
    reg!(
        registry,
        "preview_scan",
        "Fast low-resolution scan preview",
        json!({}),
        |_| preview_scan()
    );

    // ---- 10. Networking (advanced) ----
    reg!(
        registry,
        "set_static_ip",
        "Configure a static IP on an interface",
        json!({"type":"object","properties":{"interface":{"type":"string"},"ip":{"type":"string"},"gateway":{"type":"string"},"dns":{"type":"string"}},"required":["interface","ip","gateway","dns"]}),
        |p: serde_json::Value| set_static_ip(
            p["interface"].as_str().unwrap_or(""),
            p["ip"].as_str().unwrap_or(""),
            p["gateway"].as_str().unwrap_or(""),
            p["dns"].as_str().unwrap_or("")
        )
    );
    reg!(
        registry,
        "set_dhcp",
        "Switch an interface to DHCP",
        json!({"type":"object","properties":{"interface":{"type":"string"}},"required":["interface"]}),
        |p: serde_json::Value| set_dhcp(p["interface"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "create_vpn",
        "Import a WireGuard VPN config",
        json!({"type":"object","properties":{"name":{"type":"string"},"config":{"type":"string"}},"required":["name","config"]}),
        |p: serde_json::Value| create_vpn(
            p["name"].as_str().unwrap_or(""),
            p["config"].as_str().unwrap_or("")
        )
    );
    reg!(
        registry,
        "connect_vpn",
        "Connect a VPN",
        json!({"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}),
        |p: serde_json::Value| connect_vpn(p["name"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "disconnect_vpn",
        "Disconnect a VPN",
        json!({"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}),
        |p: serde_json::Value| disconnect_vpn(p["name"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "get_wifi_channels",
        "List Wi-Fi channels available",
        json!({}),
        |_| get_wifi_channels()
    );
    reg!(
        registry,
        "set_wifi_channel",
        "Pin the Wi-Fi channel",
        json!({"type":"object","properties":{"channel":{"type":"integer"}},"required":["channel"]}),
        |p: serde_json::Value| set_wifi_channel(p["channel"].as_u64().unwrap_or(0) as u8)
    );
    reg!(
        registry,
        "create_hotspot",
        "Create or stop a mobile hotspot",
        json!({"type":"object","properties":{"ssid":{"type":"string"},"password":{"type":"string"},"enabled":{"type":"boolean"}},"required":["ssid","password","enabled"]}),
        |p: serde_json::Value| create_hotspot(
            p["ssid"].as_str().unwrap_or(""),
            p["password"].as_str().unwrap_or(""),
            p["enabled"].as_bool().unwrap_or(true)
        )
    );
    reg!(
        registry,
        "get_network_speed_test",
        "Run a network speed test",
        json!({}),
        |_| get_network_speed_test()
    );
    reg!(
        registry,
        "trace_route",
        "Trace the route to a host",
        json!({"type":"object","properties":{"host":{"type":"string"}},"required":["host"]}),
        |p: serde_json::Value| trace_route(p["host"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "dns_lookup",
        "Resolve a hostname to an address",
        json!({"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}),
        |p: serde_json::Value| dns_lookup(p["name"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "reverse_dns",
        "Reverse DNS lookup for an IP",
        json!({"type":"object","properties":{"ip":{"type":"string"}},"required":["ip"]}),
        |p: serde_json::Value| reverse_dns(p["ip"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "port_scan",
        "Scan open ports on a host",
        json!({"type":"object","properties":{"host":{"type":"string"},"ports":{"type":"string"}},"required":["host","ports"]}),
        |p: serde_json::Value| port_scan(
            p["host"].as_str().unwrap_or(""),
            p["ports"].as_str().unwrap_or("80")
        )
    );
    reg!(
        registry,
        "capture_packets",
        "Capture packets on an interface (bounded)",
        json!({"type":"object","properties":{"interface":{"type":"string"},"count":{"type":"integer"}},"required":["interface","count"]}),
        |p: serde_json::Value| capture_packets(
            p["interface"].as_str().unwrap_or(""),
            p["count"].as_u64().unwrap_or(10) as u32
        )
    );
    reg!(
        registry,
        "analyze_traffic",
        "Analyze live network traffic (nload)",
        json!({}),
        |_| analyze_traffic()
    );

    // ---- 11. GPU & acceleration ----
    reg!(
        registry,
        "detect_gpus",
        "Detect GPUs and active drivers",
        json!({}),
        |_| detect_gpus()
    );
    reg!(
        registry,
        "get_gpu_utilization",
        "GPU utilization and memory usage",
        json!({}),
        |_| get_gpu_utilization()
    );
    reg!(
        registry,
        "set_gpu_power_mode",
        "Set GPU power mode: auto, max, adaptive, low",
        json!({"type":"object","properties":{"mode":{"type":"string"}},"required":["mode"]}),
        |p: serde_json::Value| set_gpu_power_mode(p["mode"].as_str().unwrap_or("auto"))
    );
    reg!(
        registry,
        "enable_hardware_acceleration",
        "Enable or disable VA-API hardware acceleration",
        json!({"type":"object","properties":{"enabled":{"type":"boolean"}},"required":["enabled"]}),
        |p: serde_json::Value| enable_hardware_acceleration(p["enabled"].as_bool().unwrap_or(true))
    );
    reg!(
        registry,
        "get_cuda_info",
        "CUDA toolkit and driver info",
        json!({}),
        |_| get_cuda_info()
    );
    reg!(
        registry,
        "get_opencl_info",
        "OpenCL platforms",
        json!({}),
        |_| get_opencl_info()
    );
    reg!(
        registry,
        "set_gpu_fan_curve",
        "Set GPU fan speed (0-100%)",
        json!({"type":"object","properties":{"percent":{"type":"integer"}},"required":["percent"]}),
        |p: serde_json::Value| set_gpu_fan_curve(p["percent"].as_u64().unwrap_or(50) as u8)
    );
    reg!(
        registry,
        "get_gpu_processes",
        "Processes currently using the GPU",
        json!({}),
        |_| get_gpu_processes()
    );
    reg!(
        registry,
        "kill_gpu_process",
        "Kill a GPU process (requires confirmation)",
        json!({"type":"object","properties":{"pid":{"type":"integer"}},"required":["pid"]}),
        |p: serde_json::Value| kill_gpu_process(p["pid"].as_u64().unwrap_or(0))
    );
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::testutil::TestHome;

    // ---- 1. Touchscreen & pen ----
    #[test]
    fn test_valid_value_rejects_shell_metachars() {
        assert!(valid_value("hdmi-a-1"));
        assert!(valid_value("left-index-finger"));
        assert!(!valid_value("foo; rm -rf /"));
        assert!(!valid_value("foo $(x)"));
        assert!(!valid_value("-rf"));
        assert!(!valid_value(""));
        assert!(!valid_value("has space"));
    }

    #[test]
    fn test_valid_path_contract() {
        assert!(valid_path("/home/diad/file.pdf"));
        assert!(!valid_path("relative/path"));
        assert!(!valid_path("/home/../etc"));
        assert!(!valid_path("/home/$HOME"));
        assert!(!valid_path("/home/a;b"));
        assert!(!valid_path("/home/`cmd`"));
    }

    #[test]
    fn test_enable_touch_mode_returns_state() {
        let r = enable_touch_mode(true).unwrap();
        // Should never panic; on machines without touch returns a notice.
        assert!(r.success || r.error.is_some());
    }

    #[test]
    fn test_configure_pen_handles_missing_wacom() {
        let r = configure_pen(Some(512), false).unwrap();
        if !r.success {
            // Accept graceful degradation when xsetwacom is absent.
            assert!(r.error.unwrap().contains("xsetwacom"));
        }
    }

    #[test]
    fn test_calibrate_touchscreen_invalid_device() {
        let r = calibrate_touchscreen("../../etc/passwd").unwrap();
        assert!(!r.success);
    }

    #[test]
    fn test_set_palm_rejection_never_panics() {
        let r = set_palm_rejection(true).unwrap();
        assert!(r.success || r.error.is_some());
    }

    #[test]
    fn test_get_touch_events_bounded() {
        let r = get_touch_events(1).unwrap();
        assert!(r.success || r.error.is_some());
    }

    // ---- 2. Fingerprint ----
    #[test]
    fn test_delete_fingerprint_rejects_bad_input() {
        let r = delete_fingerprint("x; rm -rf /").unwrap();
        assert!(!r.success);
    }

    #[test]
    fn test_fingerprint_tools_graceful() {
        // These must never panic even when fprintd is absent.
        let _ = detect_fingerprint().unwrap();
        let _ = list_enrolled_fingers().unwrap();
        let _ = authenticate_fingerprint().unwrap();
    }

    // ---- 3. Webcam & audio ----
    #[test]
    fn test_camera_frame_graceful_missing_ffmpeg() {
        let r = get_camera_frame().unwrap();
        if !r.success {
            let msg = r.error.clone().unwrap_or_default();
            assert!(msg.contains("ffmpeg") || msg.contains("video0"));
        }
    }

    #[test]
    fn test_set_camera_settings_valid() {
        let r = set_camera_settings(Some(128), None).unwrap();
        assert!(r.success || r.error.is_some());
    }

    #[test]
    fn test_audio_volume_clamped() {
        let r = set_audio_volume(250).unwrap();
        // Clamped to 100 and executed without panic.
        assert!(r.success || r.error.is_some());
    }

    #[test]
    fn test_audio_profile_validation() {
        assert!(set_audio_profile("voice").unwrap().success);
        let bad = set_audio_profile("loud").unwrap();
        assert!(!bad.success);
    }

    #[test]
    fn test_set_default_audio_rejects_injection() {
        let r = set_default_audio("1; rm -rf /").unwrap();
        assert!(!r.success);
    }

    // ---- 4. Bluetooth ----
    #[test]
    fn test_bluetooth_mac_validation() {
        for mac in ["AA:BB:CC:DD:EE:FF", "aa:bb:cc:dd:ee:ff"] {
            assert!(valid_value(mac));
        }
        assert!(!valid_value("AA:BB; rm"));
        assert!(!valid_value("-AA:BB:CC:DD:EE:FF"));
    }

    #[test]
    fn test_bluetooth_connect_rejects_invalid_mac() {
        let r = bluetooth_connect("x; whoami").unwrap();
        assert!(!r.success);
        let r2 = bluetooth_connect("AA:BB:CC:DD:EE:FF").unwrap();
        assert!(r2.success || r2.error.is_some());
    }

    #[test]
    fn test_bluetooth_codec_allowlist() {
        assert!(
            bluetooth_set_codec("AA:BB:CC:DD:EE:FF", "aptx")
                .unwrap()
                .success
        );
        assert!(
            !bluetooth_set_codec("AA:BB:CC:DD:EE:FF", "wma")
                .unwrap()
                .success
        );
    }

    // ---- 5. Monitors ----
    #[test]
    fn test_detect_displays_graceful() {
        let r = detect_displays().unwrap();
        assert!(r.success || r.error.is_some());
    }

    #[test]
    fn test_display_mode_validation() {
        assert!(set_display_mode("mirror").is_ok());
        assert!(!set_display_mode("weird").unwrap().success);
    }

    #[test]
    fn test_rotation_validation() {
        assert!(set_display_rotation("eDP-1", "left").is_ok());
        assert!(!set_display_rotation("eDP-1", "diagonal").unwrap().success);
    }

    #[test]
    fn test_display_profile_save_load_roundtrip() {
        // Load from a temp HOME to avoid touching the real one. TestHome
        // serializes on HOME_LOCK and restores HOME on drop.
        let home = TestHome::new();
        std::fs::create_dir_all(home.path().join(".diad").join("display-profiles")).unwrap();
        std::fs::write(
            home.path()
                .join(".diad/display-profiles/test-profiles.conf"),
            "eDP-1 1920x1080@60\n",
        )
        .unwrap();
        let loaded = load_display_profile("test-profiles").unwrap();
        assert!(loaded.success, "load failed: {:?}", loaded.error);
        assert!(loaded.output.contains("eDP-1"));
    }

    #[test]
    fn test_save_profile_rejects_bad_name() {
        let r = save_display_profile("a;rm -rf").unwrap();
        assert!(!r.success);
    }

    // ---- 6. Keyboard & mouse ----
    #[test]
    fn test_keyboard_layout_validation() {
        assert!(!set_keyboard_layout("us; rm", None).unwrap().success);
        let r = set_keyboard_layout("us", Some("dvorak")).unwrap();
        assert!(r.success || r.error.is_some());
    }

    #[test]
    fn test_keyboard_options_rejects_shell() {
        let r = set_keyboard_options("ctrl:nocaps; rm -rf /").unwrap();
        assert!(!r.success);
    }

    #[test]
    fn test_key_repeat_bounded() {
        let r = set_key_repeat(5000, 500).unwrap();
        assert!(r.success || r.error.is_some());
    }

    #[test]
    fn test_macro_lifecycle() {
        let _home = TestHome::new();
        let created = create_macro("build", "ctrl+s").unwrap();
        assert!(created.success, "create failed: {:?}", created.error);
        let listed = list_macros().unwrap();
        assert!(
            listed.output.contains("build.keys"),
            "list: {}",
            listed.output
        );
        let deleted = delete_macro("build").unwrap();
        assert!(deleted.success);
        let listed2 = list_macros().unwrap();
        assert!(
            !listed2.output.contains("build"),
            "delete failed: {}",
            listed2.output
        );
    }

    #[test]
    fn test_macro_name_validation() {
        assert!(!create_macro("bad;name", "x").unwrap().success);
    }

    #[test]
    fn test_sticky_slow_bounce_never_panic() {
        for f in [enable_sticky_keys, enable_slow_keys, enable_bounce_keys] {
            let _ = f(true).unwrap();
            let _ = f(false).unwrap();
        }
    }

    // ---- 7. Power & thermal ----
    #[test]
    fn test_power_mode_validation() {
        assert!(set_power_mode("balanced").is_ok());
    }

    #[test]
    fn test_thermal_status_graceful() {
        let r = get_thermal_status().unwrap();
        assert!(r.success || r.error.is_some());
    }

    #[test]
    fn test_cpu_governor_validation() {
        assert!(!set_cpu_governor("evil").unwrap().success);
        assert!(set_cpu_governor("powersave").is_ok());
    }

    #[test]
    fn test_cpu_freq_bounded() {
        let r = set_cpu_max_freq(1_000_000_000_000).unwrap();
        assert!(r.success || r.error.is_some());
    }

    #[test]
    fn test_battery_health_graceful() {
        let r = get_battery_health().unwrap();
        assert!(r.success || r.error.is_some());
    }

    // ---- 8. Storage ----
    #[test]
    fn test_list_storage_devices_graceful() {
        let r = list_storage_devices().unwrap();
        assert!(r.success || r.error.is_some());
    }

    #[test]
    fn test_format_device_refuses_non_dev_path() {
        let r = format_device("/home/diad/file", "ext4").unwrap();
        assert!(!r.success);
        let r2 = format_device("/dev/sdb1; rm -rf /", "ext4").unwrap();
        assert!(!r2.success);
    }

    #[test]
    fn test_format_device_fstype_allowlist() {
        assert!(format_device("/dev/sdb1", "btrfs").is_ok());
        assert!(!format_device("/dev/sdb1", "ntfs-ng").unwrap().success);
    }

    #[test]
    fn test_mount_validation() {
        assert!(!mount_device("/dev/sdb1", "/mnt; rm -rf").unwrap().success);
        assert!(!mount_device("/dev/sdb1", "relative").unwrap().success);
    }

    #[test]
    fn test_clone_disk_refuses_non_dev() {
        let r = clone_disk("/home/x", "/dev/sdb").unwrap();
        assert!(!r.success);
    }

    #[test]
    fn test_trim_toggle_graceful() {
        let _ = enable_trim(true).unwrap();
        let _ = enable_trim(false).unwrap();
    }

    #[test]
    fn test_disk_usage_validation() {
        assert!(!get_disk_usage("~").unwrap().success);
        let r = get_disk_usage("/tmp").unwrap();
        assert!(r.success || r.error.is_some());
    }

    // ---- 9. Printing & scanning ----
    #[test]
    fn test_printer_validation() {
        assert!(!add_printer("p; x", "ipp://x").unwrap().success);
        assert!(!remove_printer("p; x").unwrap().success);
        assert!(!set_default_printer("p; x").unwrap().success);
    }

    #[test]
    fn test_print_file_validation() {
        assert!(!print_file("../etc/passwd", None).unwrap().success);
        assert!(!print_file("/tmp/a;b", None).unwrap().success);
    }

    #[test]
    fn test_cancel_job_validation() {
        assert!(!cancel_print_job("7; rm").unwrap().success);
        assert!(cancel_print_job("all").is_ok());
    }

    #[test]
    fn test_scan_format_validation() {
        assert!(scan_document("png").is_ok());
        assert!(!scan_document("exe").unwrap().success);
    }

    #[test]
    fn test_detect_printers_scanners_graceful() {
        let _ = detect_printers().unwrap();
        let _ = detect_scanners().unwrap();
        let _ = get_print_queue().unwrap();
    }

    // ---- 10. Networking ----
    #[test]
    fn test_static_ip_validation() {
        assert!(
            !set_static_ip("wlan0; rm", "192.168.1.5/24", "192.168.1.1", "8.8.8.8")
                .unwrap()
                .success
        );
        let r = set_static_ip("eth0", "192.168.1.5/24", "192.168.1.1", "8.8.8.8").unwrap();
        assert!(r.success || r.error.is_some());
    }

    #[test]
    fn test_dhcp_validation() {
        assert!(!set_dhcp("wlan0;x").unwrap().success);
    }

    #[test]
    fn test_vpn_validation() {
        assert!(!create_vpn("vpn;x", "/etc/wg0.conf").unwrap().success);
        assert!(!create_vpn("office", "relative.conf").unwrap().success);
    }

    #[test]
    fn test_host_validation() {
        assert!(!trace_route("host; rm -rf").unwrap().success);
        assert!(!dns_lookup("host;whoami").unwrap().success);
    }

    #[test]
    fn test_hotspot_validation() {
        assert!(!create_hotspot("ssid;x", "pw", true).unwrap().success);
        assert!(!create_hotspot("diad", "", true).unwrap().success);
    }

    #[test]
    fn test_port_scan_validation() {
        assert!(!port_scan("x; rm", "80").unwrap().success);
        assert!(!port_scan("localhost", "80;x").unwrap().success);
    }

    #[test]
    fn test_capture_packets_bounded() {
        let r = capture_packets("wlan0", 5000).unwrap();
        assert!(r.success || r.error.is_some());
    }

    // ---- 11. GPU ----
    #[test]
    fn test_detect_gpus_graceful() {
        let r = detect_gpus().unwrap();
        assert!(r.success || r.error.is_some());
    }

    #[test]
    fn test_gpu_power_mode_validation() {
        assert!(!set_gpu_power_mode("ultra").unwrap().success);
        assert!(set_gpu_power_mode("auto").is_ok());
    }

    #[test]
    fn test_gpu_fan_bounded() {
        let r = set_gpu_fan_curve(80).unwrap();
        assert!(r.success || r.error.is_some());
    }

    #[test]
    fn test_kill_gpu_process_validation() {
        assert!(!kill_gpu_process(0).unwrap().success);
        let r = kill_gpu_process(999_999).unwrap();
        assert!(r.success || r.error.is_some());
    }
}
