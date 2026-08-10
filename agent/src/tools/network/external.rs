//! External AI coding CLI bridge ("every IDE in one place").
//!
//! DIAD can delegate tasks to the local AI coding CLIs the user already has
//! installed — Claude Code, OpenAI Codex, Aider, OpenCode, and the Gemini CLI.
//! They run non-interactively and stream their output into the chat pane as
//! `[agent:<name>]` lines, so the user can mix cloud models and local coding
//! agents from a single interface.
//!
//! The `delegate_to_agent` tool is registered in the ToolRegistry, which also
//! lets the cloud LLM delegate to local agents. Spawning always requires an
//! explicit user confirmation (handled by the orchestrator via CONFIRM_TOOLS).

use std::path::PathBuf;
use std::sync::OnceLock;

use tokio::sync::mpsc::UnboundedSender;

use super::ToolResult;
use crate::tui::event::Event;

/// CLI names the bridge knows how to invoke, in a stable display order.
pub const SUPPORTED_AGENTS: &[&str] = &["claude", "codex", "aider", "opencode", "gemini"];

/// Global event sender, set once at startup by `main.rs`.
///
/// Lets the (synchronous) tool handler stream stdout lines into the chat
/// pane as `[agent:<name>]` messages while it runs.
static EVENT_SENDER: OnceLock<UnboundedSender<Event>> = OnceLock::new();

/// Register the event sender so `delegate_to_agent` can stream into the TUI.
pub fn set_event_sender(tx: UnboundedSender<Event>) {
    let _ = EVENT_SENDER.set(tx);
}

fn event_sender() -> Option<UnboundedSender<Event>> {
    EVENT_SENDER.get().cloned()
}

/// Detect which supported CLI agents are installed (via `which`/PATH search).
pub fn detect_agents() -> Vec<String> {
    detect_agents_in_path(&default_search_paths())
}

/// Detect installed agents among a given set of search paths (testable with
/// a mocked PATH). A binary counts as installed if it exists and is
/// executable.
pub fn detect_agents_in_path(paths: &[PathBuf]) -> Vec<String> {
    SUPPORTED_AGENTS
        .iter()
        .filter(|name| find_binary_in_paths(name, paths).is_some())
        .map(|s| s.to_string())
        .collect()
}

/// Delegate a task to a local AI coding CLI (using the real PATH).
pub fn delegate_to_agent(agent: &str, task: &str) -> anyhow::Result<ToolResult> {
    delegate_to_agent_in_paths(agent, task, &default_search_paths())
}

/// Delegate a task to a local AI coding CLI, searching `paths` for the binary
/// (testable with a mocked PATH).
pub fn delegate_to_agent_in_paths(
    agent: &str,
    task: &str,
    paths: &[PathBuf],
) -> anyhow::Result<ToolResult> {
    let agent = agent.trim().to_lowercase();

    if !SUPPORTED_AGENTS.contains(&agent.as_str()) {
        return Ok(ToolResult::err(format!(
            "Unknown agent '{agent}'. Supported agents: {}.",
            SUPPORTED_AGENTS.join(", ")
        )));
    }

    let Some(binary) = find_binary_in_paths(&agent, paths) else {
        return Ok(ToolResult::err(format!(
            "'{agent}' is not installed. Install it (see README → External agents), \
             or ask the DIAD Agent to handle the task directly."
        )));
    };

    let args = command_args(&agent, task);
    tracing::info!("delegate_to_agent: spawning {:?} {:?}", binary, args);

    let mut child = std::process::Command::new(&binary)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("failed to capture stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("failed to capture stderr"))?;

    // Stream stdout lines into the chat as [agent:<name>] messages while
    // collecting them for the tool result.
    let agent_tag = format!("[agent:{agent}]");
    let event_tx = event_sender();

    let out_handle = std::thread::spawn(move || {
        use std::io::BufRead;
        let reader = std::io::BufReader::new(stdout);
        let mut collected: Vec<String> = Vec::new();
        for line in reader.lines() {
            let line = line.unwrap_or_default();
            if !line.is_empty() {
                collected.push(line.clone());
            }
            if let Some(tx) = &event_tx {
                let _ = tx.send(Event::AppendMessage(format!("{agent_tag} {line}")));
            }
        }
        collected
    });

    let err_handle = std::thread::spawn(move || {
        use std::io::Read;
        let mut buf = String::new();
        let _ = std::io::BufReader::new(stderr).read_to_string(&mut buf);
        buf
    });

    let status = child.wait()?;
    let out_lines = out_handle.join().unwrap_or_default();
    let err_text = err_handle.join().unwrap_or_default();

    let mut output = out_lines.join("\n");
    if !err_text.trim().is_empty() {
        output.push_str("\n[stderr]\n");
        output.push_str(err_text.trim_end());
    }

    if !status.success() {
        return Ok(ToolResult::err(format!(
            "'{agent}' exited with {status}\n{output}"
        )));
    }
    Ok(ToolResult::ok(output))
}

/// Build the non-interactive command-line arguments for an agent.
pub fn command_args(agent: &str, task: &str) -> Vec<String> {
    match agent {
        "claude" => vec!["-p".into(), task.to_string()],
        "codex" => vec!["exec".into(), task.to_string()],
        "aider" => vec!["--message".into(), task.to_string()],
        "opencode" => vec!["run".into(), task.to_string()],
        "gemini" => vec!["-p".into(), task.to_string()],
        _ => vec![task.to_string()],
    }
}

/// PATH directories from the environment.
fn default_search_paths() -> Vec<PathBuf> {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default()
}

/// Find an executable named `name` in the given search directories.
fn find_binary_in_paths(name: &str, paths: &[PathBuf]) -> Option<PathBuf> {
    for dir in paths {
        let candidate = dir.join(name);
        if candidate.is_file() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = std::fs::metadata(&candidate) {
                    if meta.permissions().mode() & 0o111 != 0 {
                        return Some(candidate);
                    }
                }
            }
            #[cfg(not(unix))]
            {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_args() {
        assert_eq!(command_args("claude", "fix the bug"), ["-p", "fix the bug"]);
        assert_eq!(
            command_args("codex", "fix the bug"),
            ["exec", "fix the bug"]
        );
        assert_eq!(
            command_args("aider", "fix the bug"),
            ["--message", "fix the bug"]
        );
        assert_eq!(
            command_args("opencode", "fix the bug"),
            ["run", "fix the bug"]
        );
        assert_eq!(command_args("gemini", "fix the bug"), ["-p", "fix the bug"]);
    }

    #[test]
    fn test_detect_agents_mocked_path() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["claude", "codex", "opencode"] {
            let path = dir.path().join(name);
            std::fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
        }

        let found = detect_agents_in_path(&[dir.path().to_path_buf()]);
        assert!(found.contains(&"claude".to_string()));
        assert!(found.contains(&"codex".to_string()));
        assert!(found.contains(&"opencode".to_string()));
        assert!(!found.contains(&"aider".to_string()));
        assert!(!found.contains(&"gemini".to_string()));
    }

    #[test]
    fn test_detect_agents_ignores_non_executable() {
        let dir = tempfile::tempdir().unwrap();
        // Written but NOT chmod +x → must not be detected.
        let path = dir.path().join("claude");
        std::fs::write(&path, "#!/bin/sh\n").unwrap();

        let found = detect_agents_in_path(&[dir.path().to_path_buf()]);
        assert!(found.is_empty());
    }

    #[test]
    fn test_delegate_missing_binary_is_friendly() {
        let dir = tempfile::tempdir().unwrap();
        let result =
            delegate_to_agent_in_paths("claude", "task", &[dir.path().to_path_buf()]).unwrap();
        assert!(!result.success);
        let err = result.error.unwrap();
        assert!(err.contains("not installed"), "got: {err}");
    }

    #[test]
    fn test_delegate_unknown_agent_is_friendly() {
        let result = delegate_to_agent_in_paths("wat", "task", &[]).unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Unknown agent"));
    }
}
