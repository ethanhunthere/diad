//! System & agent update tools.
//!
//! - `check_system_updates` / `apply_system_updates` manage Arch packages
//!   (pacman). Applying updates requires an explicit user confirmation in the
//!   orchestrator (see `CONFIRM_TOOLS` in `orchestrator/agent.rs`) before the
//!   `sudo pacman -Syu` executes.
//! - `check_agent_updates` / `apply_agent_updates` are placeholders wired to
//!   the future `diados/diados` GitHub releases endpoint.

use std::process::Command;

use super::ToolResult;
use crate::tools::system::exec;

// ===========================================================================
// System updates (pacman)
// ===========================================================================

/// Check for pending system updates via `checkupdates` (pacman-contrib).
///
/// `checkupdates` exits 0 when updates are available, 2 when the system is up
/// to date, and non-zero on error.
pub fn check_system_updates() -> anyhow::Result<ToolResult> {
    let output = Command::new("checkupdates")
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run checkupdates: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if output.status.code() == Some(2) {
        return Ok(ToolResult::ok("System is up to date."));
    }
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Ok(ToolResult::err(if stderr.is_empty() {
            format!("checkupdates failed (exit {:?}).", output.status.code())
        } else {
            stderr
        }));
    }
    if stdout.is_empty() {
        return Ok(ToolResult::ok("System is up to date."));
    }

    let count = stdout.lines().count();
    Ok(ToolResult::ok(format!(
        "{count} update(s) available:\n{stdout}"
    )))
}

/// Apply all system updates (`sudo pacman -Syu --noconfirm`).
///
/// The orchestrator requires an explicit user confirmation (y/N) before this
/// tool executes — see `CONFIRM_TOOLS` in `orchestrator/agent.rs`.
pub fn apply_system_updates() -> anyhow::Result<ToolResult> {
    exec::run_privileged(&["pacman", "-Syu", "--noconfirm"])
}

// ===========================================================================
// Agent updates (placeholder)
// ===========================================================================

/// Check for a newer DIAD Agent release.
///
/// PLACEHOLDER: points at the `diados/diados` GitHub releases API. Until the
/// repo publishes releases, this reports the placeholder response.
pub fn check_agent_updates() -> anyhow::Result<ToolResult> {
    let output = Command::new("curl")
        .args([
            "-s",
            "--max-time",
            "10",
            "https://api.github.com/repos/diados/diados/releases/latest",
        ])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run curl: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Ok(ToolResult::err(
            "check_agent_updates: empty response (offline, or the releases endpoint is not live yet).",
        ));
    }

    match serde_json::from_str::<serde_json::Value>(&stdout) {
        Ok(json) => {
            let tag = json["tag_name"].as_str().unwrap_or("unknown");
            let name = json["name"].as_str().unwrap_or("latest release");
            Ok(ToolResult::ok(format!("Latest release: {tag} — {name}")))
        }
        Err(_) => {
            // Not JSON (e.g. GitHub rate-limit message) — surface it as-is.
            Ok(ToolResult::ok(stdout))
        }
    }
}

/// Apply the latest DIAD Agent binary.
///
/// Downloads a release asset over the current binary. The download URL can be
/// overridden at build time via `DIAD_AGENT_DOWNLOAD_URL`; the default points
/// at the future `diados/diados` releases asset.
///
/// # Supply-chain hardening (audit F-12 / phase A1)
///
/// The whole operation (download + checksum fetch + digest verify + install +
/// cleanup) runs inside the root-owned `/usr/local/bin/diad-selfupdate`
/// wrapper, which is the ONLY sudoers entry for self-update — the old open
/// `wget`/`sha256sum`/`install`/`rm` entries are gone. The wrapper refuses to
/// install unless the `DIAD_CONFIRM=selfupdate` marker is present AND the
/// downloaded binary's SHA-256 matches the `.sha256` sidecar from the same
/// HTTPS origin.
pub fn apply_agent_updates() -> anyhow::Result<ToolResult> {
    let url = option_env!("DIAD_AGENT_DOWNLOAD_URL").unwrap_or(
        "https://api.github.com/repos/diados/diados/releases/latest/download/diad-agent",
    );
    let checksum_url = format!("{url}.sha256");
    exec::run_privileged_env(
        &["/usr/local/bin/diad-selfupdate", url, &checksum_url],
        &[("DIAD_CONFIRM", "selfupdate")],
    )
}
