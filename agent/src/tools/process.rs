//! Process management tools.

use std::io::Write;
use std::path::Path;
use std::process::Command;

use super::{valid_identifier, ToolResult};

/// argv[0] values that are hard-rejected: shells and privilege-elevation
/// binaries. These are the classic escape hatches for prompt-injection or
/// sandbox-escape attacks, so `spawn_process` refuses them outright.
const BANNED_ARGV0: &[&str] = &["sudo", "su", "doas", "pkexec", "bash", "sh", "zsh", "fish"];

/// Check whether a command name (argv[0]) is a banned shell/superuser binary.
///
/// The check compares the basename so `/usr/bin/bash` and `bash` are both
/// rejected.
fn is_banned_shell(command: &str) -> bool {
    let base = Path::new(command)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| command.to_string());
    BANNED_ARGV0.iter().any(|b| base == *b)
}

/// Append one audit line to `~/.diad/logs/audit.log`.
///
/// Every `spawn_process` invocation is recorded with its full command line,
/// an RFC3339 timestamp, and the approval status. The orchestrator shows the
/// exact argv to the user and only then executes, so `approved:true` is
/// recorded for every line appended here. Secrets are never logged.
fn append_audit_log(command: &str, args: &[String], approved: bool) {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/diad".into());
    let dir = format!("{home}/.diad/logs");
    let file = format!("{dir}/audit.log");

    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let timestamp = chrono::Utc::now().to_rfc3339();
    let line = format!(
        "[{timestamp}] approved:{approved} command={command} args={}\n",
        args.join(" ")
    );
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
    {
        let _ = f.write_all(line.as_bytes());
    }
}

/// Spawn a new process and return its output.
///
/// Security contract (mandatory, enforced here):
/// 1. argv[0] must not be a shell or privilege-elevation binary
///    (`sudo`, `su`, `doas`, `pkexec`, `bash`, `sh`, `zsh`, `fish`).
/// 2. The process is spawned with an argv array (`Command::new(...).args(...)`)
///    — never through a shell — so arguments are passed verbatim and cannot
///    be interpreted as shell syntax or options of a wrapping shell.
/// 3. The orchestrator always shows the EXACT argv to the user for approval
///    before this function runs, and every invocation is appended to
///    `~/.diad/logs/audit.log` with `approved:true`.
pub fn spawn_process(command: &str, args: &[String]) -> anyhow::Result<ToolResult> {
    if command.trim().is_empty() {
        return Ok(ToolResult::err("spawn_process: no command specified."));
    }
    if is_banned_shell(command) {
        return Ok(ToolResult::err(format!(
            "Refusing to spawn {command}: shells and privilege-elevation \
             binaries are not allowed (banned argv[0])."
        )));
    }

    let output = Command::new(command)
        .args(args)
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to spawn {command}: {e}"))?;

    // Record the invocation — the orchestrator already obtained explicit user
    // approval showing the exact argv before this point.
    append_audit_log(command, args, true);

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        let mut result = stdout;
        if !stderr.is_empty() {
            result.push_str(&format!("\n[stderr]\n{stderr}"));
        }
        Ok(ToolResult::ok(result))
    } else {
        Ok(ToolResult::err(format!(
            "Process exited with code {}: {stderr}",
            output.status.code().unwrap_or(-1)
        )))
    }
}

/// List all running processes.
pub fn list_processes() -> anyhow::Result<ToolResult> {
    let output = Command::new("ps")
        .args(["aux", "--no-headers"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run ps: {e}"))?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        // Only return first 40 lines to avoid overwhelming output
        let lines: Vec<&str> = text.lines().take(40).collect();
        let preview = if lines.len() >= 40 {
            format!("{}\n... (showing first 40 processes)", lines.join("\n"))
        } else {
            lines.join("\n")
        };
        Ok(ToolResult::ok(preview))
    } else {
        Ok(ToolResult::err("Failed to list processes"))
    }
}

/// Kill a process by PID.
pub fn kill_process(pid: u32) -> anyhow::Result<ToolResult> {
    let output = Command::new("kill")
        .arg(pid.to_string())
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to kill process {pid}: {e}"))?;

    if output.status.success() {
        Ok(ToolResult::ok(format!("Process {pid} killed.")))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(ToolResult::err(format!(
            "Failed to kill process {pid}: {stderr}"
        )))
    }
}

/// Get logs from a running process (last N lines of journalctl).
pub fn get_process_logs(unit: &str, lines: u32) -> anyhow::Result<ToolResult> {
    // The unit name is interpolated into argv — validate it strictly so it
    // can never inject options (e.g. a leading `-`) or shell syntax.
    if !valid_identifier(unit) {
        return Ok(ToolResult::err(format!(
            "get_process_logs: invalid unit name '{unit}'"
        )));
    }

    let output = Command::new("journalctl")
        .args(["-u", unit, "-n", &lines.to_string(), "--no-pager"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to get logs for {unit}: {e}"))?;

    if output.status.success() {
        Ok(ToolResult::ok(String::from_utf8_lossy(&output.stdout)))
    } else {
        Ok(ToolResult::err(format!(
            "Failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_banned_shells_rejected() {
        for cmd in ["sudo", "su", "doas", "pkexec", "bash", "sh", "zsh", "fish"] {
            assert!(is_banned_shell(cmd), "{cmd} should be banned");
        }
        // Absolute paths to banned binaries are rejected too.
        assert!(is_banned_shell("/usr/bin/bash"));
        assert!(is_banned_shell("/bin/sh"));
    }

    #[test]
    fn test_non_shell_commands_allowed() {
        assert!(!is_banned_shell("ls"));
        assert!(!is_banned_shell("/usr/bin/ping"));
        assert!(!is_banned_shell("python3"));
        assert!(!is_banned_shell("bashful")); // different basename
    }

    #[test]
    fn test_spawn_process_empty_command_rejected() {
        let result = spawn_process("", &[]).unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("no command"));
    }

    #[test]
    fn test_spawn_process_rejects_shell_escape() {
        // Attempted privilege/shell escape via argv[0] — must be hard-rejected
        // before any process is spawned.
        let result = spawn_process("sudo", &["rm".to_string(), "-rf".to_string(), "/".to_string()]).unwrap();
        assert!(!result.success);
        let err = result.error.unwrap();
        assert!(err.contains("Refusing to spawn sudo"), "got: {err}");
    }

    #[test]
    fn test_spawn_process_audit_log_written() {
        // Point HOME at a temp dir so the audit log lands somewhere writable.
        // HOME is process-global and shared with other tool tests, so
        // serialize the mutation (see tools::testutil).
        let _guard = crate::tools::testutil::HOME_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", dir.path());
        let result = spawn_process("true", &["--flag".to_string(), "value".to_string()]).unwrap();
        assert!(result.success);

        let log = std::fs::read_to_string(dir.path().join(".diad/logs/audit.log"))
            .expect("audit log should exist");
        assert!(log.contains("command=true"), "log: {log}");
        assert!(log.contains("--flag value"), "log: {log}");
        assert!(log.contains("approved:true"), "log: {log}");
        // RFC3339 timestamp in square brackets, e.g. [2026-08-09T16:45:43+00:00]
        assert!(log.starts_with('['), "expected bracketed timestamp in: {log}");
        assert!(log.contains('T'), "expected RFC3339 timestamp in: {log}");
    }
}
