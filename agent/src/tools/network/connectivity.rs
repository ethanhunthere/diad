//! Network tools — HTTP and connectivity operations.

use std::process::Command;
use std::time::Duration;

use super::{valid_identifier, ToolResult};

/// Perform an HTTP GET request.
pub fn http_get(url: &str) -> anyhow::Result<ToolResult> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    match client.get(url).send() {
        Ok(response) => {
            let status = response.status();
            let body = response.text()?;

            let preview = if body.len() > 2000 {
                format!("{}... (truncated, {} total bytes)", &body[..2000], body.len())
            } else {
                body
            };

            Ok(ToolResult::ok(format!(
                "HTTP {status}\n\n{preview}"
            )))
        }
        Err(e) => Ok(ToolResult::err(format!("HTTP GET {url} failed: {e}"))),
    }
}

/// Perform an HTTP POST request.
pub fn http_post(url: &str, body: &str) -> anyhow::Result<ToolResult> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    match client.post(url).body(body.to_string()).send() {
        Ok(response) => {
            let status = response.status();
            let resp_body = response.text()?;

            let preview = if resp_body.len() > 2000 {
                format!(
                    "{}... (truncated, {} total bytes)",
                    &resp_body[..2000],
                    resp_body.len()
                )
            } else {
                resp_body
            };

            Ok(ToolResult::ok(format!(
                "HTTP {status}\n\n{preview}"
            )))
        }
        Err(e) => Ok(ToolResult::err(format!(
            "HTTP POST {url} failed: {e}"
        ))),
    }
}

/// Ping a host to check connectivity.
pub fn ping_host(host: &str) -> anyhow::Result<ToolResult> {
    // The host is interpolated into argv — strict validation rejects option
    // injection (leading `-`) and shell metacharacters.
    if !valid_identifier(host) {
        return Ok(ToolResult::err(format!(
            "ping_host: invalid host '{host}'"
        )));
    }

    let output = Command::new("ping")
        .args(["-c", "3", "-W", "2", host])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to ping {host}: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(ToolResult::ok(stdout))
    } else {
        Ok(ToolResult::err(format!(
            "Ping {host} failed: {stderr}"
        )))
    }
}

/// Get current network status.
pub fn get_network_status() -> anyhow::Result<ToolResult> {
    let output = Command::new("ip")
        .args(["addr", "show"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to get network info: {e}"))?;

    if output.status.success() {
        Ok(ToolResult::ok(String::from_utf8_lossy(&output.stdout)))
    } else {
        Ok(ToolResult::err("Failed to get network status"))
    }
}
