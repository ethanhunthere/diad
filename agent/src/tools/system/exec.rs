//! Shared command-execution helpers for system-control tools.
//!
//! Two execution modes:
//! - [`run`] — run a command as the current user (session-scoped operations:
//!   `wpctl`, `playerctl`, `swaymsg`, `brightnessctl` reads, ...).
//! - [`run_privileged`] — wrap a command in `sudo`. The `/etc/sudoers.d/diad`
//!   drop-in grants the `diad` user NOPASSWD access to **exactly** the
//!   binaries the DIAD tools invoke (least privilege — nothing else).
//!
//! Both return the captured stdout (falling back to stderr) as a
//! [`ToolResult`]; non-zero exit codes become `ToolResult::err`.

use std::process::Command;
use std::time::Duration;

use super::ToolResult;

const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Spawns and waits for a child process with a hard timeout, killing the process if it hangs.
fn run_command_with_timeout(
    mut cmd: Command,
    timeout: Duration,
) -> std::io::Result<std::process::Output> {
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let start = std::time::Instant::now();
    loop {
        match child.try_wait()? {
            Some(_status) => {
                return child.wait_with_output();
            }
            None => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        format!("command execution timed out after {}s", timeout.as_secs()),
                    ));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

/// Run a command as the current user.
pub fn run(args: &[&str]) -> anyhow::Result<ToolResult> {
    if args.is_empty() {
        return Ok(ToolResult::err("run: no command given."));
    }

    let program = args.first().copied().unwrap_or("?");
    let mut cmd = Command::new(args[0]);
    cmd.args(&args[1..]);
    let output = run_command_with_timeout(cmd, Duration::from_secs(DEFAULT_TIMEOUT_SECS))
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
pub fn run_privileged(args: &[&str]) -> anyhow::Result<ToolResult> {
    run_privileged_env(args, &[])
}

/// Run a command with `sudo`, passing extra environment variables (used to
/// set the `DIAD_CONFIRM` marker the validated `/usr/local/bin/diad-*`
/// wrappers require before they will exec a destructive operation).
///
/// When `DIAD_TEST_NO_PRIVILEGED` is set (test environment), the command is
/// reported instead of executed so tests never block on an interactive sudo
/// password prompt.
pub fn run_privileged_env(args: &[&str], envs: &[(&str, &str)]) -> anyhow::Result<ToolResult> {
    if args.is_empty() {
        return Ok(ToolResult::err("run_privileged: no command given."));
    }

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
        let mut cmd = Command::new("sudo");
        cmd.args(args);
        cmd.envs(envs.iter().copied());
        let output = run_command_with_timeout(cmd, Duration::from_secs(DEFAULT_TIMEOUT_SECS))
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
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Phase 7: argv-leak regression tests ─────────────────────────────

    #[test]
    fn test_run_error_leaks_program_name_only_not_args() {
        // A binary that does not exist, invoked with a secret-looking arg.
        // The error message must contain the program name (for debuggability)
        // but must NOT echo the remaining argv (a Wi-Fi password / clipboard
        // content would otherwise land in the LLM context via ToolResult).
        let secret = "hunter2-super-secret";
        let result = run(&[
            "/nonexistent/diad-binary-xyz",
            "password",
            secret,
            "--token",
            "abcd1234",
        ]);
        assert!(result.is_err(), "expected Err for missing binary");
        let err = format!("{:?}", result.err().unwrap());
        assert!(
            err.contains("diad-binary-xyz"),
            "error should name the program: {err}"
        );
        assert!(
            !err.contains(secret),
            "error must not leak argv secrets: {err}"
        );
        assert!(
            !err.contains("abcd1234"),
            "error must not leak argv secrets: {err}"
        );
    }

    #[test]
    fn test_run_privileged_test_mode_does_not_leak_args() {
        // Even in test mode the simulated command line must not echo args —
        // DIAD_TEST_NO_PRIVILEGED must never be able to leak a password.
        let secret = "wifi-password-s3cret";
        let result = run_privileged(&[
            "nmcli", "dev", "wifi", "connect", "MyNet", "password", secret,
        ])
        .unwrap();
        let msg = format!("{:?}", result);
        assert!(msg.contains("nmcli"), "should name the program: {msg}");
        assert!(!msg.contains(secret), "test-mode must not leak argv: {msg}");
    }

    #[test]
    fn test_run_privileged_no_args_is_error() {
        let result = run_privileged(&[]).unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("no command"));
    }

    #[test]
    fn test_run_no_args_is_error() {
        let result = run(&[]).unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("no command"));
    }
}
