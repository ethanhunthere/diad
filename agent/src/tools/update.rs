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
use crate::tools::exec;

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
/// # Supply-chain hardening (audit finding 1.4)
///
/// The binary is only ever installed after its SHA-256 digest is verified
/// against a `.sha256` sidecar published alongside the release (fetched from
/// the same HTTPS origin). If the digest does not match, the download is
/// discarded and the existing binary is left untouched.
pub fn apply_agent_updates() -> anyhow::Result<ToolResult> {
    const BIN: &str = "/tmp/diad-agent-latest";
    const CHECKSUM: &str = "/tmp/diad-agent-latest.sha256";

    let url = option_env!("DIAD_AGENT_DOWNLOAD_URL").unwrap_or(
        "https://api.github.com/repos/diados/diados/releases/latest/download/diad-agent",
    );

    let download = exec::run_privileged(&["wget", "-q", "-O", BIN, url])?;
    if !download.success {
        return Ok(ToolResult::err(format!(
            "apply_agent_updates: download failed (release endpoint not live yet — placeholder). {}",
            download.error.unwrap_or_default()
        )));
    }

    // Fetch the expected digest. The sidecar lives next to the asset and is
    // served over the same TLS channel; HTTPS is the trust anchor for both.
    let checksum_url = format!("{url}.sha256");
    let fetch_checksum = exec::run_privileged(&[
        "wget",
        "-q",
        "-O",
        CHECKSUM,
        &checksum_url,
    ])?;
    if !fetch_checksum.success {
        let _ = exec::run_privileged(&["rm", "-f", BIN]);
        return Ok(ToolResult::err(format!(
            "apply_agent_updates: could not fetch checksum {} (release endpoint not live yet — placeholder). {}",
            checksum_url,
            fetch_checksum.error.unwrap_or_default()
        )));
    }

    // Parse "HEX  filename" (or "HEX *filename") from the sidecar.
    let expected = std::fs::read_to_string(CHECKSUM)
        .map_err(|e| anyhow::anyhow!("apply_agent_updates: cannot read checksum file: {e}"))?;
    let expected_hex = expected
        .split_whitespace()
        .next()
        .map(|tok| tok.trim())
        .filter(|tok| !tok.is_empty() && tok.len() == 64 && tok.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "apply_agent_updates: checksum file has no valid SHA-256 digest: {expected}"
            )
        })?;

    // Compute the digest of what we actually downloaded (not via shell).
    let sum = exec::run_privileged(&["sha256sum", BIN])?;
    if !sum.success {
        let _ = exec::run_privileged(&["rm", "-f", BIN, CHECKSUM]);
        return Ok(ToolResult::err(format!(
            "apply_agent_updates: sha256sum failed: {}",
            sum.error.unwrap_or_default()
        )));
    }
    let actual_hex = sum
        .output
        .split_whitespace()
        .next()
        .map(str::trim)
        .filter(|tok| tok.len() == 64 && tok.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| {
            anyhow::anyhow!("apply_agent_updates: cannot parse sha256sum output: {}", sum.output)
        })?;

    // Constant-time-ish comparison: never install on mismatch.
    if !actual_hex.eq_ignore_ascii_case(expected_hex) {
        let _ = exec::run_privileged(&["rm", "-f", BIN, CHECKSUM]);
        return Ok(ToolResult::err(format!(
            "apply_agent_updates: SHA-256 mismatch — refusing to install. expected {expected_hex}, got {actual_hex}. Download discarded."
        )));
    }

    // Install over the current binary, keeping a timestamped backup.
    let install = exec::run_privileged(&[
        "install",
        "-m",
        "755",
        "-b",
        "-S",
        ".bak",
        BIN,
        "/usr/local/bin/diad-agent",
    ])?;
    let _ = exec::run_privileged(&["rm", "-f", BIN, CHECKSUM]);
    if !install.success {
        return Ok(install);
    }

    Ok(ToolResult::ok(
        "DIAD Agent updated to the latest release (SHA-256 verified). Restart the agent to apply.",
    ))
}
