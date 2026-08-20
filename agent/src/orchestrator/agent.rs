//! Agentic tool loop — orchestrates LLM reasoning ↔ tool execution.
//!
//! The loop for a single user turn:
//! 1. Sends the conversation history + the full tool registry (definitions)
//!    to the provider and streams the assistant's text live to the TUI.
//! 2. If the model requests tool calls (`stop_reason = "tool_use"`), each tool
//!    is executed via the `ToolRegistry`:
//!    - `power` actions require an explicit user confirmation (y/N) in the TUI.
//!    - Tools that return a `NEED_SECRET_PREFIX` marker (e.g. Wi-Fi connect
//!      without a password) trigger a masked SecurePrompt; the secret is
//!      injected into the tool call and never logged or sent to the LLM.
//! 3. Tool results are appended back into the conversation as `tool` messages
//!    and the provider is re-called — until it produces a final answer
//!    (`end_turn`) or the iteration budget (`MAX_ITERATIONS`) is exhausted.

use std::sync::Arc;

use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::providers::traits::{LlmProvider, Message, StreamEvent, ToolDefinition};
use crate::tools::hardware::device::NEED_SECRET_PREFIX;
use crate::tools::{ToolRegistry, ToolResult};
use crate::tui::event::Event;

/// Maximum number of tool-call rounds within a single agentic turn.
pub const MAX_ITERATIONS: usize = 5;

/// Master AI System Prompt for DIAD OS.
pub const DIAD_SYSTEM_PROMPT: &str = r#"You are DIAD — the sovereign operating system intelligence for DIAD OS.
You have direct, full-spectrum authority to execute native tools to configure, inspect, automate, and control the user's computer.

SYSTEM ARCHITECTURE:
- Workspace 1: DIAD AGENT (AI Core, Dev, Terminal, System Operations)
- Workspace 2: Web Browser (Fast Wayland WebKit Epiphany)
- Workspace 3: Communications (Telegram & Slack Dedicated Desktop Apps)
- Hardware & Networking: Universal Wi-Fi/Bluetooth stack (NetworkManager, wpa_supplicant, rfkill), multi-display manager, audio sinks (PipeWire), power profiles (power-profiles-daemon, TLP), GPU acceleration.
- Window Management: Sway Wayland Compositor (0-gap full tiling under Waybar capsule).

CORE OPERATING DIRECTIVES:
1. Sovereign Execution: Whenever the user asks to configure the system, connect Wi-Fi, adjust brightness, change volume, install/remove packages, set night light, manage files, or inspect performance, USE YOUR REGISTERED TOOLS PROACTIVELY. Never ask the user to run manual shell commands when a tool exists.
2. Clean, Futuristic Output: Format responses with high-clarity markdown, concise bullet points, and code snippets where appropriate.
3. System Safety: Destructive operations (disk wiping, formatting, process termination) are automatically confirmed. Wi-Fi passwords are secure-prompted."#;

/// Hard cap on conversation messages sent to the provider (spec 2.8:
/// context growth must be bounded or long sessions blow the context
/// window and every fallback provider burns the same 400).
pub const MAX_CONTEXT_MESSAGES: usize = 200;

/// Trim the oldest messages once the cap is exceeded. The cut point always
/// advances to a `user` message so assistant-tool_use / tool_result pairs are
/// never split (a dangling tool result is a 400 on most providers).
fn trim_context(messages: &mut Vec<Message>, cap: usize) {
    if messages.len() <= cap {
        return;
    }
    let mut cut = messages.len() - cap;
    while cut < messages.len() && messages[cut].role != "user" {
        cut += 1;
    }
    messages.drain(..cut);
}

/// Tools that always require an explicit user confirmation (y/N) before the
/// orchestrator executes them.
pub const CONFIRM_TOOLS: &[&str] = &[
    "power",
    "apply_system_updates",
    "delegate_to_agent",
    "spawn_process",
    // Universal hardware tools (destructive or system-altering)
    "format_device",
    "clone_disk",
    "eject_device",
    "resize_partition",
    "kill_gpu_process",
    "delete_fingerprint",
    "remove_printer",
    "bluetooth_forget",
    "cancel_print_job",
    "unmount_device",
    "enable_camera_privacy_shutter",
    "enable_fingerprint_sudo",
    "enable_fingerprint_unlock",
    "set_cpu_governor",
    "enable_turbo_boost",
    "set_static_ip",
    "set_dhcp",
    "create_hotspot",
    "connect_vpn",
    "disconnect_vpn",
    "mount_device",
    // Universal file tools (destructive or data-altering)
    "delete_download",
    "organize_downloads",
    "clean_old_downloads",
    "move_download",
    "clear_clipboard_history",
    "trash_empty",
];

/// A single tool call requested by the LLM.
#[derive(Debug, Clone)]
pub struct ToolCall {
    /// The tool call ID from the provider
    pub id: String,
    /// The tool name (must exist in the registry)
    pub name: String,
    /// Parsed tool input JSON
    pub input: serde_json::Value,
}

/// The final outcome of one agentic turn.
pub struct AgentOutcome {
    /// Final assistant text (empty if the turn ended on a tool result)
    pub final_text: String,
    /// Updated conversation history (includes tool_use / tool_result messages)
    pub messages: Vec<Message>,
}

/// Run one full agentic turn: stream, execute tools, loop until `end_turn`.
///
/// Live UI feedback is emitted through `tx` (StreamChunk for text, AppendMessage
/// for tool lines, SecurePromptRequest / ConfirmationRequest for user input).
pub async fn run_agentic_loop(
    provider: &dyn LlmProvider,
    registry: Arc<ToolRegistry>,
    messages: Vec<Message>,
    tx: UnboundedSender<Event>,
) -> anyhow::Result<AgentOutcome> {
    let mut messages = messages;
    if !messages.iter().any(|m| m.role == "system") {
        messages.insert(0, Message::system(DIAD_SYSTEM_PROMPT));
    }
    let mut final_text = String::new();
    let mut rounds = 0;

    loop {
        if rounds >= MAX_ITERATIONS {
            let _ = tx.send(Event::AppendMessage(
                "[system] Reached maximum tool iterations — stopping.".into(),
            ));
            break;
        }
        rounds += 1;

        // ---- 1. Send conversation + tool definitions to the provider ----
        let tool_defs: Vec<ToolDefinition> = registry
            .list_tools()
            .iter()
            .map(|t| ToolDefinition {
                name: t.name.clone(),
                description: t.description.clone(),
                parameters: t.parameters.clone(),
            })
            .collect();

        trim_context(&mut messages, MAX_CONTEXT_MESSAGES);
        let stream = match provider.stream_chat(messages.clone(), tool_defs).await {
            Ok(s) => s,
            Err(e) => {
                let _ = tx.send(Event::Error(format!("API error: {e}")));
                break;
            }
        };

        // ---- 2. Collect one assistant turn ----
        let mut text = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut current: Option<(String, String, String)> = None; // (id, name, json_buf)
        let mut stream_error: Option<String> = None;

        {
            use futures::StreamExt;
            let mut stream = stream;
            while let Some(event) = stream.next().await {
                match event {
                    Ok(StreamEvent::TextDelta(t)) => {
                        text.push_str(&t);
                        let _ = tx.send(Event::StreamChunk(t));
                    }
                    Ok(StreamEvent::ToolUseStart { id, name }) => {
                        current = Some((id, name, String::new()));
                    }
                    Ok(StreamEvent::ToolInputDelta(json)) => {
                        if let Some((_, _, buf)) = &mut current {
                            buf.push_str(&json);
                        }
                    }
                    Ok(StreamEvent::ToolUseEnd { id: _ }) => {
                        if let Some((id, name, buf)) = current.take() {
                            let input = serde_json::from_str(&buf)
                                .unwrap_or_else(|_| serde_json::json!({}));
                            tool_calls.push(ToolCall { id, name, input });
                        }
                    }
                    Ok(StreamEvent::StreamEnd { .. }) => {}
                    Ok(StreamEvent::Error(e)) => {
                        stream_error = Some(e);
                    }
                    Err(e) => {
                        let _ = tx.send(Event::Error(format!("Stream error: {e}")));
                        break;
                    }
                }
            }
        }

        if let Some(e) = stream_error {
            let _ = tx.send(Event::Error(e));
            break;
        }

        final_text = text;

        // No tool calls → the model finished with a normal answer.
        if tool_calls.is_empty() {
            break;
        }

        // ---- 3. Record assistant tool-use blocks in the conversation -------
        for tc in &tool_calls {
            messages.push(Message::assistant_tool_use(
                &tc.id,
                &tc.name,
                tc.input.clone(),
            ));
            // Notify frontends a tool call was requested (renders a card).
            let _ = tx.send(Event::ToolCallStart {
                call_id: tc.id.clone(),
                name: tc.name.clone(),
                args: tc.input.clone(),
                confirm_required: CONFIRM_TOOLS.contains(&tc.name.as_str()),
            });
        }

        // ---- 4. Execute each tool (with secure prompts / confirmations) ----
        for tc in tool_calls {
            let execution = execute_tool_with_prompts(Arc::clone(&registry), &tc, &tx).await;
            let _ = tx.send(Event::AppendMessage(execution.status_line(&tc.name)));
            let _ = tx.send(Event::ToolCallResult {
                call_id: tc.id.clone(),
                success: execution.success,
                output: execution.to_llm_string(),
            });
            messages.push(Message::tool_result(&tc.id, execution.to_llm_string()));
        }
    }

    let _ = tx.send(Event::StreamDone);
    Ok(AgentOutcome {
        final_text,
        messages,
    })
}

/// The outcome of executing one tool call, after any user prompts.
struct ToolExecution {
    success: bool,
    output: String,
    error: Option<String>,
}

impl ToolExecution {
    fn from_result(result: &ToolResult) -> Self {
        Self {
            success: result.success,
            output: result.output.clone(),
            error: result.error.clone(),
        }
    }

    /// A short UI status line, e.g. `[tool] wifi_list ✓`.
    fn status_line(&self, name: &str) -> String {
        if self.success {
            format!("[tool] {name} ✓")
        } else {
            format!("[tool] {name} ✗")
        }
    }

    /// The text sent back to the LLM as the tool result.
    fn to_llm_string(&self) -> String {
        if self.success {
            if self.output.trim().is_empty() {
                "ok".to_string()
            } else {
                self.output.clone()
            }
        } else {
            self.error.clone().unwrap_or_else(|| "error".into())
        }
    }
}

/// Execute a tool, handling SecurePrompts (masked secrets), confirmations
/// and AI-edit diff approval for `write_file`.
///
/// - `power` always requires an explicit y/N confirmation.
/// - `write_file` proposes a unified diff to the user; it is only applied
///   after explicit approval (Accept/Reject in the GUI, y/N in the TUI).
/// - If the tool returns a `NEED_SECRET_PREFIX` error (Wi-Fi password missing),
///   the user is prompted with a masked input; the value is injected into the
///   tool parameters and the tool is re-executed. The secret never reaches the
///   LLM or the logs.
async fn execute_tool_with_prompts(
    registry: Arc<ToolRegistry>,
    tc: &ToolCall,
    tx: &UnboundedSender<Event>,
) -> ToolExecution {
    let mut params = tc.input.clone();

    // AI-edit flow: `write_file` proposes a diff that must be approved.
    if tc.name == "write_file" {
        if let Some(path) = params["path"].as_str() {
            let old =
                std::fs::read_to_string(shellexpand::tilde(path).as_ref()).unwrap_or_default();
            let new = params["content"].as_str().unwrap_or("");
            if old != new {
                let diff = unified_diff(path, &old, new);
                if !request_edit_approval(tx, path.to_string(), diff).await {
                    return ToolExecution {
                        success: false,
                        output: String::new(),
                        error: Some("Edit rejected by the user.".into()),
                    };
                }
            }
        }
    }

    // Destructive tools (power actions, system updates) always require an
    // explicit user confirmation.
    if CONFIRM_TOOLS.contains(&tc.name.as_str()) {
        let label = match tc.name.as_str() {
            "power" => {
                let action = params["action"].as_str().unwrap_or("").to_string();
                // THE LABEL MUST SHOW THE REAL VERB (RBRO-004 WP-06.6):
                // `shutdown` executes as `systemctl poweroff` (sudoers pins
                // poweroff), so the confirmation must say poweroff too.
                let verb =
                    crate::tools::hardware::device::power_verb(&action).unwrap_or(action.as_str());
                let human = match verb {
                    "poweroff" => "power off",
                    "reboot" => "reboot",
                    _ => "suspend",
                };
                format!("Run `systemctl {verb}`? This will {human} the system.")
            }
            "apply_system_updates" => {
                "Run `pacman -Syu --noconfirm`? This will update all system packages.".to_string()
            }
            "delegate_to_agent" => {
                let agent = params["agent"].as_str().unwrap_or("").to_string();
                format!(
                    "Spawn the local `{agent}` AI CLI to work on this task? \
                     This runs a separate AI tool on your machine."
                )
            }
            "spawn_process" => {
                // Show the EXACT argv that will be executed.
                let command = params["command"].as_str().unwrap_or("").to_string();
                let args: Vec<String> = params["args"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let argv = std::iter::once(command)
                    .chain(args)
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("Run `{argv}`? This spawns a new process on your machine.")
            }
            other => format!("Run `{other}`? This action is destructive."),
        };
        if !request_confirmation(tx, label).await {
            return ToolExecution {
                success: false,
                output: String::new(),
                error: Some("Action cancelled by the user.".into()),
            };
        }
    }

    // Tools spawn processes and walk filesystems: run them on the blocking
    // pool so a slow tool never stalls the daemon's async runtime (on a
    // 1-worker ULTRA_LOW box that would freeze the whole daemon).
    let exec_name = tc.name.clone();
    let exec_params = params.clone();
    let exec_registry = Arc::clone(&registry);
    let mut result = match tokio::task::spawn_blocking(move || {
        exec_registry.execute_tool(&exec_name, exec_params)
    })
    .await
    {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            return ToolExecution {
                success: false,
                output: String::new(),
                error: Some(format!("Tool error: {e}")),
            };
        }
        Err(e) => {
            return ToolExecution {
                success: false,
                output: String::new(),
                error: Some(format!("Tool task panicked: {e}")),
            };
        }
    };

    // Handle secret-required markers (e.g. Wi-Fi password).
    if !result.success {
        if let Some(err) = &result.error {
            if let Some(context) = err.strip_prefix(NEED_SECRET_PREFIX) {
                if let Some((kind, value)) = context.split_once(':') {
                    if kind == "wifi_password" {
                        let ssid = value.to_string();
                        let label = format!("Enter password for {ssid} (hidden):");
                        if let Some(secret) = request_secret(tx, label).await {
                            if !secret.is_empty() {
                                params["password"] = serde_json::Value::String(secret);
                                let retry_name = tc.name.clone();
                                let retry_params = params.clone();
                                let retry_registry = Arc::clone(&registry);
                                result = match tokio::task::spawn_blocking(move || {
                                    retry_registry.execute_tool(&retry_name, retry_params)
                                })
                                .await
                                {
                                    Ok(Ok(r)) => r,
                                    Ok(Err(e)) => ToolResult::err(format!("Tool error: {e}")),
                                    Err(e) => ToolResult::err(format!("Tool error: {e}")),
                                };
                            } else {
                                result = ToolResult::err("Password entry cancelled.");
                            }
                        } else {
                            result = ToolResult::err("Password prompt unavailable.");
                        }
                    }
                }
            }
        }
    }

    ToolExecution::from_result(&result)
}

/// How long a user prompt may go unanswered before the turn treats it as
/// declined. Without this a hung frontend pins the streaming flag forever
/// and every later chat is blocked.
const PROMPT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

/// Request a masked (hidden) secret from the user and await the answer.
///
/// Returns `None` if the prompt channel closed unexpectedly.
async fn request_secret(tx: &UnboundedSender<Event>, label: String) -> Option<String> {
    let (reply_tx, mut reply_rx) = unbounded_channel::<String>();
    let _ = tx.send(Event::SecurePromptRequest {
        label,
        reply: reply_tx,
    });
    match tokio::time::timeout(PROMPT_TIMEOUT, reply_rx.recv()).await {
        Ok(v) => v,
        Err(_) => {
            tracing::warn!("secret prompt timed out after {PROMPT_TIMEOUT:?}");
            None
        }
    }
}

/// Request an explicit yes/no confirmation from the user and await the answer.
///
/// Returns `false` if the prompt channel closed unexpectedly.
async fn request_confirmation(tx: &UnboundedSender<Event>, label: String) -> bool {
    let (reply_tx, mut reply_rx) = unbounded_channel::<bool>();
    let _ = tx.send(Event::ConfirmationRequest {
        label,
        reply: reply_tx,
    });
    match tokio::time::timeout(PROMPT_TIMEOUT, reply_rx.recv()).await {
        Ok(v) => v.unwrap_or_default(),
        Err(_) => {
            tracing::warn!("prompt timed out after {PROMPT_TIMEOUT:?}");
            false
        }
    }
}

/// Request approval for a proposed file edit and await the answer.
///
/// Returns `false` if the prompt channel closed unexpectedly.
async fn request_edit_approval(tx: &UnboundedSender<Event>, path: String, diff: String) -> bool {
    let (reply_tx, mut reply_rx) = unbounded_channel::<bool>();
    let _ = tx.send(Event::EditDiffRequest {
        id: format!("diff-{}", uuid::Uuid::new_v4()),
        path,
        diff,
        reply: reply_tx,
    });
    match tokio::time::timeout(PROMPT_TIMEOUT, reply_rx.recv()).await {
        Ok(v) => v.unwrap_or_default(),
        Err(_) => {
            tracing::warn!("prompt timed out after {PROMPT_TIMEOUT:?}");
            false
        }
    }
}

/// Build a small unified diff between two file contents (line based).
///
/// Produces a `--- / +++ / @@` style diff suitable for rendering in a diff
/// card. Uses a simple longest-common-subsequence over lines.
fn unified_diff(path: &str, old: &str, new: &str) -> String {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let mut out = String::new();
    out.push_str(&format!("--- a/{path}\n+++ b/{path}\n"));
    out.push_str(&format!(
        "@@ -1,{} +1,{} @@\n",
        old_lines.len(),
        new_lines.len()
    ));

    let (n, m) = (old_lines.len(), new_lines.len());

    // LCS is O(n*m) memory. Beyond this the diff card shows a summary
    // instead — an AI edit of a huge file must not allocate gigabytes.
    const MAX_LCS_CELLS: usize = 4_000_000;
    if (n + 1).saturating_mul(m + 1) > MAX_LCS_CELLS {
        out.push_str(&format!(
            "@@ large file: {n} lines -> {m} lines; diff suppressed @@\n"
        ));
        return out;
    }

    let mut table = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[i][j] = if old_lines[i] == new_lines[j] {
                table[i + 1][j + 1] + 1
            } else {
                table[i + 1][j].max(table[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if old_lines[i] == new_lines[j] {
            out.push_str(&format!(" {}\n", old_lines[i]));
            i += 1;
            j += 1;
        } else if table[i + 1][j] >= table[i][j + 1] {
            out.push_str(&format!("-{}\n", old_lines[i]));
            i += 1;
        } else {
            out.push_str(&format!("+{}\n", new_lines[j]));
            j += 1;
        }
    }
    while i < n {
        out.push_str(&format!("-{}\n", old_lines[i]));
        i += 1;
    }
    while j < m {
        out.push_str(&format!("+{}\n", new_lines[j]));
        j += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_context_keeps_recent_and_user_boundary() {
        let mut msgs: Vec<Message> = Vec::new();
        msgs.push(Message::user("first"));
        for i in 0..10 {
            msgs.push(Message::assistant_tool_use(
                format!("id{i}"),
                "t",
                serde_json::json!({}),
            ));
            msgs.push(Message::tool_result(format!("id{i}"), "ok"));
        }
        msgs.push(Message::user("last"));

        trim_context(&mut msgs, 5);
        assert!(msgs.len() <= 5);
        assert_eq!(msgs[0].role, "user", "cut must land on a user message");
        assert_eq!(msgs.last().unwrap().content, "last");
    }

    #[test]
    fn test_trim_context_noop_under_cap() {
        let mut msgs = vec![Message::user("a"), Message::user("b")];
        trim_context(&mut msgs, 200);
        assert_eq!(msgs.len(), 2);
    }

    #[test]
    fn test_unified_diff_suppresses_huge_files() {
        let old: String = (0..2001).map(|i| format!("line {i}\n")).collect();
        let new: String = (0..2001).map(|i| format!("changed {i}\n")).collect();
        let diff = unified_diff("big.txt", &old, &new);
        assert!(
            diff.contains("diff suppressed"),
            "huge diffs must not run LCS"
        );
    }

    #[test]
    fn test_unified_diff_adds_lines() {
        let diff = unified_diff("f.txt", "a\nb\n", "a\nb\nc\n");
        assert!(diff.contains("--- a/f.txt"));
        assert!(diff.contains("+c"));
    }

    #[test]
    fn test_unified_diff_removes_lines() {
        let diff = unified_diff("f.txt", "a\nb\nc\n", "a\n");
        assert!(diff.contains("-b"));
        assert!(diff.contains("-c"));
    }

    #[test]
    fn test_unified_diff_identical() {
        let diff = unified_diff("f.txt", "same\n", "same\n");
        assert!(diff.contains(" same"));
        assert!(!diff.contains("+same"));
    }

    #[test]
    fn test_tool_call_default_input() {
        // Verify that a malformed JSON buffer degrades to an empty object.
        let input = serde_json::from_str::<serde_json::Value>("not json")
            .unwrap_or_else(|_| serde_json::json!({}));
        assert_eq!(input, serde_json::json!({}));
    }

    #[test]
    fn test_tool_execution_status_lines() {
        let ok = ToolExecution {
            success: true,
            output: "done".into(),
            error: None,
        };
        assert_eq!(ok.status_line("wifi_list"), "[tool] wifi_list ✓");
        assert_eq!(ok.to_llm_string(), "done");

        let err = ToolExecution {
            success: false,
            output: String::new(),
            error: Some("boom".into()),
        };
        assert_eq!(err.status_line("wifi_connect"), "[tool] wifi_connect ✗");
        assert_eq!(err.to_llm_string(), "boom");
    }
}
