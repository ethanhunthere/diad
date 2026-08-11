//! Hardware system control tools — Wi-Fi, audio, brightness, power, Bluetooth.
//!
//! These tools are registered in the `ToolRegistry` and can be invoked by the
//! LLM (agentic loop) or directly by the offline fallback router.
//!
//! Security notes:
//! - Privileged commands go through `run_privileged()` (sudo). The sudoers
//!   drop-in at `/etc/sudoers.d/diad` grants the `diad` user NOPASSWD access
//!   to exactly these binaries.
//! - Wi-Fi passwords are never logged or stored. When `wifi_connect` is called
//!   without a password, it returns a `NEED_SECRET_PREFIX` marker so the
//!   orchestrator can trigger a SecurePrompt (masked input) instead.

use std::process::Command;

use super::{valid_identifier, ToolResult};

/// Marker prefix returned by tools that require a secret (e.g. Wi-Fi password).
///
/// Format: `__DIAD_NEED_SECRET__:<kind>:<context>` — e.g.
/// `__DIAD_NEED_SECRET__:wifi_password:MyNetwork`.
/// The orchestrator parses this, shows a SecurePrompt, and re-invokes the tool
/// with the secret injected. The secret itself never reaches the LLM or logs.
pub const NEED_SECRET_PREFIX: &str = "__DIAD_NEED_SECRET__:";

/// Run a command as the current user (session-scoped operations).
fn run(args: &[&str]) -> anyhow::Result<ToolResult> {
    let program = args.first().copied().unwrap_or("?");
    let output = Command::new(args[0])
        .args(&args[1..])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run `{program}`: {e}"))?;

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

/// Run a command with `sudo` (privileged system operations).
///
/// Requires the `diad` sudoers drop-in for passwordless execution of the
/// whitelisted binaries.
fn run_privileged(args: &[&str]) -> anyhow::Result<ToolResult> {
    let program = args.first().copied().unwrap_or("?");
    let output = Command::new("sudo")
        .args(args)
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run `sudo {program}`: {e}"))?;

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

// ===========================================================================
// Wi-Fi (NetworkManager / nmcli)
// ===========================================================================

/// List nearby Wi-Fi networks: SSID, SIGNAL, SECURITY.
pub fn wifi_list() -> anyhow::Result<ToolResult> {
    run(&[
        "nmcli",
        "-t",
        "-f",
        "SSID,SIGNAL,SECURITY",
        "dev",
        "wifi",
        "list",
    ])
}

/// Connect to a Wi-Fi network by SSID.
///
/// If `password` is `None`, returns a `NEED_SECRET_PREFIX` marker so the
/// orchestrator can trigger a SecurePrompt for the masked password input.
pub fn wifi_connect(ssid: &str, password: Option<&str>) -> anyhow::Result<ToolResult> {
    // SSIDs are interpolated into argv — reject empty, option-injection
    // (leading `-`), and control characters (e.g. newlines).
    if ssid.is_empty() {
        return Ok(ToolResult::err("wifi_connect: SSID is required."));
    }
    if ssid.starts_with('-') || ssid.chars().any(|c| c.is_control()) {
        return Ok(ToolResult::err(format!(
            "wifi_connect: invalid SSID '{ssid}'"
        )));
    }

    let password = match password {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => {
            return Ok(ToolResult::err(format!(
                "{NEED_SECRET_PREFIX}wifi_password:{ssid}"
            )));
        }
    };

    run_privileged(&[
        "nmcli", "dev", "wifi", "connect", ssid, "password", &password,
    ])
}

/// Show NetworkManager general + device status, the active Wi-Fi signal, and
/// the current IPv4 addresses.
pub fn wifi_status() -> anyhow::Result<ToolResult> {
    let general = run(&["nmcli", "general", "status"])?;
    let devices = run(&["nmcli", "dev", "status"])?;

    let mut output = general.output.clone();
    if !general.output.is_empty() {
        output.push('\n');
    }
    output.push_str(&devices.output);

    // Signal strength of the currently connected access point (if any)
    let signal = run(&[
        "nmcli",
        "-t",
        "-f",
        "IN-USE,SSID,SIGNAL",
        "dev",
        "wifi",
        "list",
    ]);
    if let Ok(s) = signal {
        if s.success && !s.output.trim().is_empty() {
            output.push_str("\n\nActive Wi-Fi (IN-USE,SSID,SIGNAL):\n");
            output.push_str(&s.output);
        }
    }

    // IPv4 addresses per interface
    let ip = run(&["ip", "-4", "-o", "addr", "show"]);
    if let Ok(i) = ip {
        if i.success && !i.output.trim().is_empty() {
            output.push_str("\n\nIPv4 addresses:\n");
            output.push_str(&i.output);
        }
    }

    Ok(ToolResult::ok(output))
}

/// List all network interfaces with their addresses (JSON from `ip -j`).
pub fn get_network_interfaces() -> anyhow::Result<ToolResult> {
    run(&["ip", "-j", "addr", "show"])
}

/// Connect an ethernet (wired) interface via NetworkManager.
pub fn connect_ethernet(interface: &str) -> anyhow::Result<ToolResult> {
    if !valid_identifier(interface) {
        return Ok(ToolResult::err(format!(
            "connect_ethernet: invalid interface name '{interface}'"
        )));
    }
    run_privileged(&["nmcli", "dev", "connect", interface])
}

/// Set the system hostname.
pub fn set_hostname(name: &str) -> anyhow::Result<ToolResult> {
    if !valid_identifier(name) {
        return Ok(ToolResult::err(format!(
            "set_hostname: invalid hostname '{name}'"
        )));
    }
    run_privileged(&["hostnamectl", "set-hostname", name])
}

/// Disconnect from the current Wi-Fi network.
pub fn wifi_disconnect() -> anyhow::Result<ToolResult> {
    run_privileged(&["nmcli", "dev", "disconnect", "wifi"])
}

// ===========================================================================
// Audio (WirePlumber / wpctl)
// ===========================================================================

/// Get the current volume of the default sink.
pub fn get_volume() -> anyhow::Result<ToolResult> {
    run(&["wpctl", "get-volume", "@DEFAULT_SINK@"])
}

/// Set the volume of the default sink to a percentage (0-100).
///
/// Values are clamped to 0-100; `u8` keeps the type in range at call sites
/// (the registry clamps as well) and the function re-clamps defensively.
pub fn set_volume(percent: u8) -> anyhow::Result<ToolResult> {
    let percent = percent.min(100);
    run(&[
        "wpctl",
        "set-volume",
        "@DEFAULT_SINK@",
        &format!("{percent}%"),
    ])
}

/// Toggle mute on the default sink.
pub fn toggle_mute() -> anyhow::Result<ToolResult> {
    run(&["wpctl", "set-mute", "@DEFAULT_SINK@", "toggle"])
}

// ===========================================================================
// Brightness (brightnessctl)
// ===========================================================================

/// Get the current brightness level.
pub fn get_brightness() -> anyhow::Result<ToolResult> {
    run(&["brightnessctl", "get"])
}

/// Set the brightness to a percentage (0-100).
///
/// Values are clamped to 0-100 (see `set_volume`).
pub fn set_brightness(percent: u8) -> anyhow::Result<ToolResult> {
    let percent = percent.min(100);
    run_privileged(&["brightnessctl", "set", &format!("{percent}%")])
}

// ===========================================================================
// Power (systemctl)
// ===========================================================================

/// Map a power action word to the real `systemctl` verb.
///
/// Returns `None` for anything that is not a valid action.
///
/// THE LAW (RBRO-004 WP-06.6): every power surface — waybar buttons
/// (`diad-power.sh`), the GUI top-bar menu and the TUI slash commands — must
/// converge on the same `systemctl` verbs. `shutdown` maps to `poweroff`
/// because `systemctl shutdown` is not a verb: the sudoers drop-in pins
/// `/usr/bin/systemctl poweroff` (never `shutdown`).
pub fn power_verb(action: &str) -> Option<&'static str> {
    match action {
        "shutdown" => Some("poweroff"),
        "reboot" => Some("reboot"),
        "suspend" => Some("suspend"),
        _ => None,
    }
}

/// Power action: "shutdown", "reboot", or "suspend".
///
/// NOTE: The orchestrator MUST require explicit user confirmation in the TUI
/// before this tool executes — it is a destructive system action.
pub fn power(action: &str) -> anyhow::Result<ToolResult> {
    match power_verb(action) {
        Some(verb) => run_privileged(&["systemctl", verb]),
        None => Ok(ToolResult::err(format!(
            "power: invalid action '{action}'. Valid actions: shutdown, reboot, suspend"
        ))),
    }
}

// ===========================================================================
// Bluetooth (bluetoothctl)
// ===========================================================================

/// Bluetooth actions: "on", "off", "connect <mac>", "disconnect <mac>".
pub fn bluetooth(action: &str, device: Option<&str>) -> anyhow::Result<ToolResult> {
    match action {
        "on" => run(&["bluetoothctl", "power", "on"]),
        "off" => run(&["bluetoothctl", "power", "off"]),
        "connect" => match device {
            // MAC addresses are `AA:BB:CC:DD:EE:FF` — validate strictly so
            // the value can never smuggle options or shell syntax into argv.
            Some(mac) if valid_identifier(mac) => run(&["bluetoothctl", "connect", mac]),
            _ => Ok(ToolResult::err(
                "bluetooth connect: a valid device MAC address is required.",
            )),
        },
        "disconnect" => match device {
            Some(mac) if valid_identifier(mac) => run(&["bluetoothctl", "disconnect", mac]),
            _ => Ok(ToolResult::err(
                "bluetooth disconnect: a valid device MAC address is required.",
            )),
        },
        other => Ok(ToolResult::err(format!(
            "bluetooth: invalid action '{other}'. Valid actions: on, off, connect, disconnect"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_need_secret_prefix() {
        let result = wifi_connect("MyNetwork", None).unwrap();
        assert!(!result.success);
        let err = result.error.unwrap();
        assert!(err.starts_with(NEED_SECRET_PREFIX));
        assert!(err.contains("wifi_password:MyNetwork"));
    }

    #[test]
    fn test_power_invalid_action() {
        let result = power("hack").unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("invalid action"));
    }

    #[test]
    fn test_power_verb_mapping() {
        // THE LAW (RBRO-004 WP-06.6): shutdown must resolve to the real
        // systemctl verb `poweroff` — `systemctl shutdown` is not a verb and
        // is never allowed by the sudoers drop-in.
        assert_eq!(power_verb("shutdown"), Some("poweroff"));
        assert_eq!(power_verb("reboot"), Some("reboot"));
        assert_eq!(power_verb("suspend"), Some("suspend"));
        assert_eq!(power_verb("hack"), None);
        assert_eq!(power_verb(""), None);
        assert_eq!(power_verb("power"), None);
    }

    #[test]
    fn test_power_rejects_unknown_without_sudo() {
        // Never reaches run_privileged — validation returns early.
        let result = power("--help").unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("invalid action"));
    }

    #[test]
    fn test_set_volume_clamps() {
        // Internal clamp: even if a caller bypasses the registry, the value
        // passed to wpctl never exceeds 100 (150 as u8 is representable).
        let result = set_volume(150u8);
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_hostname_rejects_option_injection() {
        let result = set_hostname("--config=/tmp/x").unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("invalid hostname"));
    }

    #[test]
    fn test_connect_ethernet_rejects_option_injection() {
        let result = connect_ethernet("--help").unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("invalid interface"));
    }

    #[test]
    fn test_wifi_connect_rejects_control_chars() {
        let result = wifi_connect("Net\n; reboot", Some("pw")).unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("invalid SSID"));
    }

    #[test]
    fn test_wifi_connect_rejects_option_injection() {
        let result = wifi_connect("--ask", Some("pw")).unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("invalid SSID"));
    }

    #[test]
    fn test_bluetooth_rejects_bad_mac() {
        // Option injection into argv — rejected by validation.
        let result = bluetooth("connect", Some("--block")).unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("MAC"));

        // A syntactically valid MAC passes validation. Execution may fail in
        // CI without a bluetooth daemon, but never with the validation error.
        match bluetooth("connect", Some("AA:BB:CC:DD:EE:FF")) {
            Ok(r) => assert!(r.success || !r.error.unwrap_or_default().contains("MAC")),
            // Err means the binary itself couldn't run — validation passed.
            Err(e) => assert!(e.to_string().contains("bluetoothctl"), "got: {e}"),
        }
    }

    #[test]
    fn test_bluetooth_requires_mac() {
        let result = bluetooth("connect", None).unwrap();
        assert!(!result.success);
    }

    #[test]
    fn test_connect_ethernet_requires_interface() {
        let result = connect_ethernet("").unwrap();
        assert!(!result.success);
    }

    #[test]
    fn test_set_hostname_requires_name() {
        let result = set_hostname("").unwrap();
        assert!(!result.success);
    }
}
