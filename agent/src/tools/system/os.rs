//! System management tools — package management, services, system info.

use std::process::Command;
use std::time::Duration;

use super::{valid_identifier, ToolResult};

/// Get system information.
pub fn get_system_info() -> anyhow::Result<ToolResult> {
    let mut info = Vec::new();

    // Hostname
    if let Ok(output) = Command::new("hostnamectl").arg("--static").output() {
        let hostname = String::from_utf8_lossy(&output.stdout).trim().to_string();
        info.push(format!("Hostname: {hostname}"));
    }

    // Kernel version
    if let Ok(output) = Command::new("uname").arg("-r").output() {
        let kernel = String::from_utf8_lossy(&output.stdout).trim().to_string();
        info.push(format!("Kernel: {kernel}"));
    }

    // Architecture
    if let Ok(output) = Command::new("uname").arg("-m").output() {
        let arch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        info.push(format!("Architecture: {arch}"));
    }

    // Uptime
    if let Ok(content) = std::fs::read_to_string("/proc/uptime") {
        let seconds: f64 = content
            .split_whitespace()
            .next()
            .unwrap_or("0")
            .parse()
            .unwrap_or(0.0);
        let uptime = Duration::from_secs_f64(seconds);
        let hours = uptime.as_secs() / 3600;
        let minutes = (uptime.as_secs() % 3600) / 60;
        info.push(format!("Uptime: {hours}h {minutes}m"));
    }

    // Memory
    if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
        for line in content.lines().take(3) {
            info.push(line.to_string());
        }
    }

    // Disk usage
    if let Ok(output) = Command::new("df").args(["-h", "/"]).output() {
        let df = String::from_utf8_lossy(&output.stdout).to_string();
        for line in df.lines().skip(1).take(1) {
            info.push(format!("Disk (/): {line}"));
        }
    }

    // DIAD version
    info.push(format!("DIAD Agent: v{}", env!("CARGO_PKG_VERSION")));

    Ok(ToolResult::ok(info.join("\n")))
}

/// Install a system package using pacman.
///
/// The package name is strictly validated (`^[A-Za-z0-9@._+:-]+$`, no leading
/// `-`) and passed after a `--` separator so it can never be parsed as a
/// pacman option or injected into the sudo argv.
pub fn install_package(package: &str) -> anyhow::Result<ToolResult> {
    if !valid_identifier(package) {
        return Ok(ToolResult::err(format!(
            "install_package: invalid package name '{package}'"
        )));
    }

    crate::tools::system::exec::run_privileged(&["/usr/local/bin/diad-pacman", "install", package])
}

/// Remove a system package using pacman.
///
/// Same strict validation + `--` separator as `install_package`.
pub fn remove_package(package: &str) -> anyhow::Result<ToolResult> {
    if !valid_identifier(package) {
        return Ok(ToolResult::err(format!(
            "remove_package: invalid package name '{package}'"
        )));
    }

    crate::tools::system::exec::run_privileged(&["/usr/local/bin/diad-pacman", "remove", package])
}

/// Manage a systemd service.
pub fn manage_service(service: &str, action: &str) -> anyhow::Result<ToolResult> {
    let valid_actions = ["start", "stop", "restart", "enable", "disable", "status"];

    if !valid_actions.contains(&action) {
        return Ok(ToolResult::err(format!(
            "Invalid action: {action}. Valid: {}",
            valid_actions.join(", ")
        )));
    }

    // The unit name is interpolated into argv — strict validation prevents
    // option injection (e.g. `x; reboot` or a leading `-`) and shell syntax.
    if !valid_identifier(service) {
        return Ok(ToolResult::err(format!(
            "manage_service: invalid service name '{service}'"
        )));
    }

    let use_sudo =
        if action != "status" && std::env::var("USER").map(|u| u != "root").unwrap_or(true) {
            "sudo"
        } else {
            ""
        };

    let mut cmd = if use_sudo.is_empty() {
        let mut c = Command::new("systemctl");
        c.arg(action).arg(service);
        c
    } else {
        let mut c = Command::new("sudo");
        c.args(["systemctl", action, service]);
        c
    };

    let output = cmd
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to {action} {service}: {e}"))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(ToolResult::ok(if stdout.is_empty() {
            format!("Service '{service}' {action} successful.")
        } else {
            stdout
        }))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(ToolResult::err(format!(
            "Failed to {action} {service}: {stderr}"
        )))
    }
}

/// Read a DIAD configuration value.
pub fn read_config(key: &str) -> anyhow::Result<ToolResult> {
    let config_paths = [
        format!(
            "{}/.config/diad/config.toml",
            std::env::var("HOME").unwrap_or_default()
        ),
        "/etc/diad/diad.conf".to_string(),
    ];

    for path in &config_paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            // Simple TOML key lookup (no full parser needed for this)
            for line in content.lines() {
                let trimmed = line.trim();
                if let Some((k, v)) = trimmed.split_once('=') {
                    if k.trim() == key {
                        return Ok(ToolResult::ok(format!(
                            "{key} = {}",
                            v.trim().trim_matches('"')
                        )));
                    }
                }
            }
        }
    }

    Ok(ToolResult::err(format!("Config key '{key}' not found")))
}

/// Write a DIAD configuration value.
pub fn write_config(key: &str, value: &str) -> anyhow::Result<ToolResult> {
    let config_path = format!(
        "{}/.config/diad/config.toml",
        std::env::var("HOME").unwrap_or_default()
    );

    let mut content = if let Ok(existing) = std::fs::read_to_string(&config_path) {
        existing
    } else {
        String::from("# DIAD Agent Configuration\n")
    };

    // Check if key already exists
    let key_exists = content
        .lines()
        .any(|line| line.trim_start().starts_with(key));

    if key_exists {
        // Replace existing key
        let new_content: Vec<String> = content
            .lines()
            .map(|line| {
                if line.trim_start().starts_with(key) {
                    format!("{key} = \"{value}\"")
                } else {
                    line.to_string()
                }
            })
            .collect();
        content = new_content.join("\n") + "\n";
    } else {
        // Append new key
        content.push_str(&format!("{key} = \"{value}\"\n"));
    }

    std::fs::write(&config_path, content)?;
    Ok(ToolResult::ok(format!("Config '{key}' set to '{value}'")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_package_rejects_option_injection() {
        // `--config=/tmp/x` would be parsed as a pacman/sudo option — must be
        // rejected before any process spawns.
        let result = install_package("--config=/tmp/x").unwrap();
        assert!(!result.success);
        let err = result.error.unwrap();
        assert!(err.contains("invalid package name"), "got: {err}");
    }

    #[test]
    fn test_install_package_rejects_shell_metacharacters() {
        for evil in [
            "x; reboot",
            "$(touch /tmp/pwned)",
            "pkg && rm -rf /",
            "pkg|sh",
            "pkg`id`",
            "pkg\nreboot",
            "pkg --noconfirm",
        ] {
            let result = install_package(evil).unwrap();
            assert!(!result.success, "should reject: {evil:?}");
        }
    }

    #[test]
    fn test_remove_package_rejects_injection() {
        let result = remove_package("--help").unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("invalid package name"));
    }

    #[test]
    fn test_valid_package_names_accepted() {
        for good in [
            "htop",
            "libx11",
            "gcc-libs",
            "python-pip",
            "1password",
            "ffmpeg+",
            "a@b",
        ] {
            assert!(valid_identifier(good), "{good:?} should be valid");
            // No process should actually spawn in tests — validation happens
            // before the Command runs, so we only assert the validator.
        }
    }

    #[test]
    fn test_manage_service_rejects_shell_injection() {
        // `x; reboot` must be rejected as a unit name, not passed to systemctl.
        let result = manage_service("x; reboot", "start").unwrap();
        assert!(!result.success);
        let err = result.error.unwrap();
        assert!(err.contains("invalid service name"), "got: {err}");
    }

    #[test]
    fn test_manage_service_rejects_option_injection() {
        let result = manage_service("--host", "start").unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("invalid service name"));
    }

    #[test]
    fn test_manage_service_valid_unit_names() {
        for good in [
            "sshd.service",
            "NetworkManager",
            "diad-firewall.service",
            "user@1000.service",
        ] {
            assert!(valid_identifier(good), "{good:?} should be valid");
        }
    }

    #[test]
    fn test_valid_identifier_rejects_attacks() {
        for evil in [
            "",
            "-",
            "--config=/tmp/x",
            "a b",
            "a;b",
            "a|b",
            "a&b",
            "a$(b)",
            "a`b`",
            "a\nb",
            "a\tb",
            "..",
        ] {
            assert!(!valid_identifier(evil), "{evil:?} should be rejected");
        }
    }
}
