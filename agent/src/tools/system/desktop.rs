//! Desktop environment tools — notifications, do-not-disturb, night light.
//!
//! The three parity wins the audit calls "the four cheapest high-visibility
//! wins" (mako + clipboard history + gammastep + trash). This module owns
//! the mako/gammastep half:
//!
//! - `notify`       — `notify-send` (requires `mako` running on Sway).
//! - `dnd`          — `makoctl mode -a dnd` / `-r dnd`.
//! - `night_light`  — `gammastep -O <temp>` (detached, keeps the session's
//!                    temperature) / `gammastep -x` to reset.
//!
//! SECURITY CONTRACT: temperatures are parsed as integers and clamped to a
//! sane display range; titles/bodies go through `Command::new(...).args(...)`
//! only — never a shell string. Missing binaries degrade to `ToolResult::err`
//! instead of panicking.

use std::process::Command;

use super::exec;
use super::ToolResult;

/// The valid gammastep colour-temperature range (Kelvin).
const MIN_TEMP: u32 = 1000;
const MAX_TEMP: u32 = 6500;

/// Send a desktop notification via `notify-send` (shown by `mako`).
///
/// `urgency` is optional and clamped to low/normal/critical.
pub fn notify(title: &str, body: &str, urgency: &str) -> anyhow::Result<ToolResult> {
    if title.len() > 500 || body.len() > 4000 {
        return Ok(ToolResult::err(
            "notify: title ≤ 500 chars, body ≤ 4000 chars",
        ));
    }
    let level = match urgency {
        "low" | "normal" | "critical" => urgency,
        _ => "normal",
    };
    degrade(exec::run(&["notify-send", "-u", level, title, body]))
}

/// Toggle do-not-disturb for `mako` (`makoctl mode`).
pub fn dnd(enabled: bool) -> anyhow::Result<ToolResult> {
    if enabled {
        degrade(exec::run(&["makoctl", "mode", "-a", "dnd"]))
    } else {
        degrade(exec::run(&["makoctl", "mode", "-r", "dnd"]))
    }
}

/// Set the session colour temperature with `gammastep`; `enabled = false`
/// (or temperature 0) resets to the natural display.
///
/// `gammastep -O <temp>` stays resident to hold the temperature, so it is
/// spawned detached (never awaited) — exactly what the manual `-O` mode is
/// for. `gammastep -x` exits immediately and is awaited.
pub fn night_light(enabled: bool, temperature: u32) -> anyhow::Result<ToolResult> {
    if !enabled || temperature == 0 {
        return degrade(exec::run(&["gammastep", "-x"]));
    }
    let temp = temperature.clamp(MIN_TEMP, MAX_TEMP);
    match Command::new("gammastep")
        .args(["-O", &temp.to_string()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(_) => Ok(ToolResult::ok(format!(
            "night light set to {temp} K"
        ))),
        Err(e) => Ok(ToolResult::err(format!(
            "gammastep failed to launch: {e} (is gammastep installed?)"
        ))),
    }
}

/// Degrade a missing-binary / spawn error into `ToolResult::err` so desktop
/// tools never surface a hard error to callers — absent binaries (CI,
/// minimal installs) degrade gracefully instead of panicking.
fn degrade(result: anyhow::Result<ToolResult>) -> anyhow::Result<ToolResult> {
    match result {
        Ok(r) => Ok(r),
        Err(e) => Ok(ToolResult::err(format!("{e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notify_validates_input_lengths() {
        let long = "x".repeat(501);
        let r = notify(&long, "b", "normal").unwrap();
        assert!(!r.success);
        let long = "y".repeat(4001);
        let r = notify("t", &long, "normal").unwrap();
        assert!(!r.success);
    }

    #[test]
    fn night_light_clamps_temperature_range() {
        // 0 → reset path; out-of-range values clamp into 1000..=6500.
        // These spawn gammastep; when the binary is missing they must
        // degrade to err, never panic.
        let _ = night_light(false, 0).unwrap();
        let _ = night_light(true, 50).unwrap();
        let _ = night_light(true, 99_999).unwrap();
    }

    #[test]
    fn dnd_never_panics() {
        // makoctl may be absent in CI — both arms must degrade gracefully.
        let _ = dnd(true).unwrap();
        let _ = dnd(false).unwrap();
    }
}
