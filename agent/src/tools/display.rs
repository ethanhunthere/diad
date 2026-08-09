//! Display & multi-monitor control tools (swaymsg IPC).
//!
//! All commands talk to the running Sway compositor over its IPC socket, so
//! they run as the current user and need no privileges.

use super::{valid_identifier, ToolResult};
use crate::tools::exec;

/// Validate an output name for argv safety.
///
/// Output names are real device names like `HDMI-A-1` / `eDP-1`; the strict
/// identifier rule (no leading `-`, no shell metacharacters) prevents option
/// injection into the `swaymsg` argv.
fn valid_output(name: &str) -> bool {
    valid_identifier(name)
}

/// List all outputs and their current state (JSON from `swaymsg`).
pub fn list_outputs() -> anyhow::Result<ToolResult> {
    exec::run(&["swaymsg", "-t", "get_outputs"])
}

/// Set an output's resolution, e.g. `set_resolution("HDMI-A-1", 2560, 1440)`.
pub fn set_resolution(output: &str, width: u32, height: u32) -> anyhow::Result<ToolResult> {
    if !valid_output(output) {
        return Ok(ToolResult::err(format!(
            "set_resolution: invalid output name '{output}'"
        )));
    }
    if width == 0 || height == 0 {
        return Ok(ToolResult::err(
            "set_resolution: width and height must be greater than 0.",
        ));
    }
    exec::run(&[
        "swaymsg",
        "output",
        output,
        "resolution",
        &format!("{width}x{height}"),
    ])
}

/// Set an output's scale factor (fractional scaling supported, e.g. 1.25).
pub fn set_scale(output: &str, factor: f64) -> anyhow::Result<ToolResult> {
    if !valid_output(output) {
        return Ok(ToolResult::err(format!(
            "set_scale: invalid output name '{output}'"
        )));
    }
    if !factor.is_finite() || factor <= 0.0 || factor > 10.0 {
        return Ok(ToolResult::err(
            "set_scale: factor must be a positive number in (0, 10].",
        ));
    }
    exec::run(&["swaymsg", "output", output, "scale", &factor.to_string()])
}

/// Set an output's position in the global layout (pixels).
pub fn set_position(output: &str, x: i32, y: i32) -> anyhow::Result<ToolResult> {
    if !valid_output(output) {
        return Ok(ToolResult::err(format!(
            "set_position: invalid output name '{output}'"
        )));
    }
    exec::run(&[
        "swaymsg",
        "output",
        output,
        "position",
        &x.to_string(),
        &y.to_string(),
    ])
}

/// Mirror `secondary` onto `primary`.
///
/// NOTE: Sway has no native display-clone feature; this mirrors the secondary
/// output's image (flipped transform) as requested. On compositors with true
/// mirroring support, replace with the compositor's clone command.
pub fn mirror_outputs(primary: &str, secondary: &str) -> anyhow::Result<ToolResult> {
    if !valid_output(primary) || !valid_output(secondary) {
        return Ok(ToolResult::err(format!(
            "mirror_outputs: invalid output name(s) '{primary}' / '{secondary}'"
        )));
    }
    exec::run(&["swaymsg", "output", secondary, "transform", "scale", "-1", "-1"])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_resolution_requires_output() {
        let result = set_resolution("", 1920, 1080).unwrap();
        assert!(!result.success);
    }

    #[test]
    fn test_output_names_reject_option_injection() {
        assert!(!set_resolution("--help", 1920, 1080).unwrap().success);
        assert!(!set_scale("-o", 1.0).unwrap().success);
        assert!(!set_position("--reload", 0, 0).unwrap().success);
        assert!(!mirror_outputs("--help", "HDMI-A-1").unwrap().success);
        assert!(!mirror_outputs("HDMI-A-1", "x; reboot").unwrap().success);
        // Real output names still validate.
        assert!(valid_output("HDMI-A-1"));
        assert!(valid_output("eDP-1"));
        assert!(valid_output("DP-3"));
    }

    #[test]
    fn test_set_resolution_rejects_zero() {
        let result = set_resolution("HDMI-A-1", 0, 1080).unwrap();
        assert!(!result.success);
    }

    #[test]
    fn test_set_scale_validates_range() {
        assert!(!set_scale("eDP-1", 0.0).unwrap().success);
        assert!(!set_scale("eDP-1", -1.5).unwrap().success);
        assert!(!set_scale("eDP-1", 11.0).unwrap().success);
    }

    #[test]
    fn test_mirror_outputs_requires_both() {
        assert!(!mirror_outputs("eDP-1", "").unwrap().success);
        assert!(!mirror_outputs("", "HDMI-A-1").unwrap().success);
    }
}
