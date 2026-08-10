//! Power management tools — battery status, power profiles, sleep timeouts.
//!
//! - `get_battery_status` reads UPower (laptops; BAT0).
//! - Power profiles go through `powerprofilesctl` (performance / balanced /
//!   power-saver). Setting a profile requires `sudo` (whitelisted in
//!   `/etc/sudoers.d/diad`); reading it is a plain user D-Bus call.
//! - Sleep timeout is configured via GSettings
//!   (`org.gnome.settings-daemon.plugins.power`).

use super::ToolResult;
use crate::tools::system::exec;

/// The UPower device path for the primary battery.
pub const BATTERY_UPOWER_PATH: &str = "/org/freedesktop/UPower/devices/battery_BAT0";

/// Get battery status (state, charge percentage, time remaining).
///
/// NOTE: `upower -i` has no JSON output mode; the raw key/value listing is
/// parsed into a compact summary.
pub fn get_battery_status() -> anyhow::Result<ToolResult> {
    let raw = exec::run(&["upower", "-i", BATTERY_UPOWER_PATH])?;
    if !raw.success {
        return Ok(ToolResult::err(
            "get_battery_status: no battery found (BAT0). Desktop machines have no battery.",
        ));
    }
    Ok(ToolResult::ok(summarize_upower(&raw.output)))
}

/// Parse `upower -i` output into a compact summary (state / percentage /
/// time to empty).
fn summarize_upower(raw: &str) -> String {
    let mut state = String::new();
    let mut percentage = String::new();
    let mut time_to_empty = String::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(v) = trimmed.strip_prefix("state:") {
            state = v.trim().to_string();
        } else if let Some(v) = trimmed.strip_prefix("percentage:") {
            percentage = v.trim().to_string();
        } else if let Some(v) = trimmed.strip_prefix("time to empty:") {
            time_to_empty = v.trim().to_string();
        }
    }

    let mut out = format!("state: {}", if state.is_empty() { "unknown" } else { &state });
    if !percentage.is_empty() {
        out.push_str(&format!("\npercentage: {percentage}"));
    }
    if !time_to_empty.is_empty() && !time_to_empty.contains("unknown") {
        out.push_str(&format!("\ntime to empty: {time_to_empty}"));
    }
    out
}

/// Validate a power profile name. Returns `Ok` for the three supported
/// profiles, else `Err` with a descriptive message.
fn validate_profile(profile: &str) -> Result<(), String> {
    match profile {
        "performance" | "balanced" | "power-saver" => Ok(()),
        other => Err(format!(
            "set_power_profile: invalid profile '{other}'. Valid profiles: performance, balanced, power-saver"
        )),
    }
}

/// Set the power profile: "performance", "balanced", or "power-saver".
pub fn set_power_profile(profile: &str) -> anyhow::Result<ToolResult> {
    match validate_profile(profile) {
        Ok(()) => exec::run_privileged(&["powerprofilesctl", "set", profile]),
        Err(msg) => Ok(ToolResult::err(msg)),
    }
}

/// Get the currently active power profile.
pub fn get_power_profile() -> anyhow::Result<ToolResult> {
    exec::run(&["powerprofilesctl", "get"])
}

/// Set the AC (plugged-in) sleep timeout in minutes via GSettings.
///
/// Requires the `org.gnome.settings-daemon.plugins.power` schema (shipped by
/// gnome-settings-daemon); on systems without it, gsettings returns an error.
pub fn set_sleep_timeout(minutes: u32) -> anyhow::Result<ToolResult> {
    if minutes == 0 || minutes > 1440 {
        return Ok(ToolResult::err(
            "set_sleep_timeout: minutes must be between 1 and 1440 (24 hours).",
        ));
    }
    let seconds = minutes.saturating_mul(60);
    exec::run(&[
        "gsettings",
        "set",
        "org.gnome.settings-daemon.plugins.power",
        "sleep-inactive-ac-timeout",
        &seconds.to_string(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_power_profile_validates() {
        // Validation only — never spawns sudo.
        assert!(validate_profile("balanced").is_ok());
        assert!(validate_profile("performance").is_ok());
        assert!(validate_profile("power-saver").is_ok());
        let err = validate_profile("turbo").unwrap_err();
        assert!(err.contains("invalid profile"));
        assert!(err.contains("turbo"));
    }

    #[test]
    fn test_set_sleep_timeout_bounds() {
        assert!(!set_sleep_timeout(0).unwrap().success);
        assert!(!set_sleep_timeout(1441).unwrap().success);
    }

    #[test]
    fn test_summarize_upower() {
        let raw = "\n  native-path:          BAT0\n  battery:\n    state:               discharging\n    percentage:          72%\n    energy:              38.6 Wh\n    time to empty:        2.1 hours\n";
        let summary = summarize_upower(raw);
        assert!(summary.contains("state: discharging"));
        assert!(summary.contains("percentage: 72%"));
        assert!(summary.contains("time to empty: 2.1 hours"));
    }
}
