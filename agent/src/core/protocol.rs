//! Versioned WebSocket protocol between the DIAD AGENT daemon and its
//! frontends (React GUI and ratatui TUI client).
//!
//! Every message carries `"v": 1` (protocol version) and a `"type"` tag.
//! Long-running requests (chat.send, file.read, …) carry an `id` that the
//! daemon echoes in its response/stream-end messages so clients can match
//! replies to requests.

use serde::{Deserialize, Serialize};

/// Current protocol version.
pub const PROTOCOL_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Client → daemon
// ---------------------------------------------------------------------------

/// A message sent from a frontend to the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    /// Send a chat message (routed to the agent loop or the offline router).
    ChatSend {
        /// Request id (echoed in token/stream.done).
        id: String,
        /// The raw user text.
        text: String,
    },
    /// Approve a tool call (response to `tool_use`).
    ToolApprove {
        /// The tool use request id.
        id: String,
    },
    /// Reject a tool call (response to `tool_use`).
    ToolReject {
        /// The tool use request id.
        id: String,
    },
    /// Approve a proposed file edit (response to `edit_diff`).
    EditApprove {
        /// The edit request id.
        id: String,
    },
    /// Reject a proposed file edit (response to `edit_diff`).
    EditReject {
        /// The edit request id.
        id: String,
    },
    /// Ask the daemon to open the TUI client in a foot terminal
    /// (GUI Ctrl+Shift+X).
    SpawnTui,
    /// Submit a masked secret (response to `secret.request`).
    SecretSubmit {
        /// The secret request id.
        id: String,
        /// The secret value — only delivered to the awaiting tool call,
        /// never logged, stored, or sent to any LLM.
        value: String,
    },
    /// Cancel a masked secret prompt.
    SecretCancel {
        /// The secret request id.
        id: String,
    },
    /// Read a file.
    FileRead { id: String, path: String },
    /// Write a file.
    FileWrite {
        id: String,
        path: String,
        content: String,
    },
    /// List a directory (single level, git-aware).
    FileList { id: String, path: String },
    /// Keyboard/data input for the daemon-spawned PTY shell.
    TermInput { data: String },
    /// Resize the daemon-spawned PTY.
    TermResize { cols: u16, rows: u16 },
    /// Switch the active provider (opens model picker flow in GUI).
    ProviderSwitch { name: String },
    /// Switch the active model for the current provider.
    ModelSwitch { model: String },
    /// Store an API key for a provider after validation.
    ProviderConnect {
        id: String,
        name: String,
        key: String,
    },
    /// Delegate a task to an installed external AI coding CLI.
    AgentDelegate {
        id: String,
        agent: String,
        task: String,
    },
    /// Set a config value (e.g. `theme`, `default_provider`).
    ConfigSet {
        key: String,
        value: serde_json::Value,
    },
    /// Request the full provider list (with connection status).
    ProvidersList { id: String },
    /// Request the daemon stats snapshot.
    StatsGet { id: String },
    /// Request the current model list for a provider.
    ModelsList { id: String, provider: String },
    /// Request the list of detected external AI coding CLIs.
    AgentsList { id: String },
}

// ---------------------------------------------------------------------------
// Daemon → client
// ---------------------------------------------------------------------------

/// One entry in the provider list sent to frontends.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderEntryDto {
    pub name: String,
    pub needs_key: bool,
    pub has_key: bool,
}

/// One entry in a file listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntryDto {
    pub name: String,
    pub kind: String, // "file" | "dir" | "symlink"
    pub size: u64,
    pub git: Option<String>, // "M", "A", "D", "??", "✓" …
}

/// A message sent from the daemon to a frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    /// Streaming text delta for a chat turn.
    Token { id: String, text: String },
    /// The model requested a tool call; frontend must confirm/reject.
    ToolUse {
        /// Request id (echo of chat.send id + call suffix).
        id: String,
        call_id: String,
        name: String,
        args: serde_json::Value,
        confirm_required: bool,
    },
    /// Result of a tool execution.
    ToolResult {
        id: String,
        call_id: String,
        success: bool,
        output: String,
        status_line: String,
    },
    /// A file edit proposed by the model — frontend shows Accept/Reject.
    EditDiff {
        /// The edit request id (echo of the daemon-generated id).
        id: String,
        path: String,
        diff: String,
    },
    /// Daemon status snapshot (broadcast on change).
    Status {
        provider: String,
        model: String,
        connected: bool,
        streaming: bool,
        daemon_version: String,
        /// THE LAW F-15: live config snapshot so surfaces can reflect the
        /// current values on open (theme, confirm_destructive, stream_tokens,
        /// max_history, auto_save_interval).
        #[serde(default)]
        config: serde_json::Value,
    },
    /// Provider list response.
    Providers {
        id: String,
        entries: Vec<ProviderEntryDto>,
    },
    /// Model list response.
    Models {
        id: String,
        provider: String,
        models: Vec<String>,
        current: String,
    },
    /// Stats snapshot.
    Stats {
        id: String,
        tokens: u64,
        latency_ms: u64,
        battery: String,
        wifi: String,
        cwd: String,
        /// Idle engine label: ACTIVE / IDLE1 / IDLE2 (spec 5.4).
        idle_stage: String,
        /// Daemon resident set size in MB (spec 5.4).
        rss_mb: u64,
        /// Perf tier word from /etc/diad/perf-tier (spec 5.4).
        perf_tier: String,
    },
    /// PTY output.
    TermOutput { data: String },
    /// Request a masked (hidden) input from the user.
    SecretRequest { id: String, label: String },
    /// Request an explicit yes/no confirmation.
    ConfirmRequest { id: String, label: String },
    /// A streaming turn finished.
    StreamDone { id: String },
    /// A plain status/event line (rendered as [system]).
    System { message: String },
    /// An error (optionally tied to a request id).
    Error { id: Option<String>, message: String },
    /// List of detected external AI coding CLIs.
    Agents { id: String, agents: Vec<String> },
    /// Greeting sent right after the WebSocket connects.
    Connected {
        v: u32,
        provider: String,
        model: String,
        connected: bool,
        streaming: bool,
        daemon_version: String,
        /// THE LAW F-15: live config snapshot (same shape as Status.config).
        #[serde(default)]
        config: serde_json::Value,
    },
    /// File read response (file.read).
    FileContent {
        id: String,
        path: String,
        content: String,
    },
    /// Directory listing response (file.list).
    Files {
        id: String,
        path: String,
        entries: Vec<FileEntryDto>,
    },
    /// File write acknowledgement (file.write).
    FileWritten { id: String, path: String },
}

impl ServerMsg {
    /// Wrap in the protocol version envelope when needed by the transport.
    pub fn version(&self) -> u32 {
        PROTOCOL_VERSION
    }
}

/// Validate a client message's version field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub v: u32,
    #[serde(flatten)]
    pub msg: ClientMsg,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_chat_send_serde_roundtrip() {
        let msg = ClientMsg::ChatSend {
            id: "req-1".into(),
            text: "hello".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ClientMsg = serde_json::from_str(&json).unwrap();
        match back {
            ClientMsg::ChatSend { id, text } => {
                assert_eq!(id, "req-1");
                assert_eq!(text, "hello");
            }
            other => panic!("wrong variant: {other:?}"),
        }
        assert!(json.contains("\"type\":\"chat_send\""));
    }

    #[test]
    fn test_client_secret_submit_serde() {
        let msg = ClientMsg::SecretSubmit {
            id: "s-9".into(),
            value: "hunter2".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ClientMsg = serde_json::from_str(&json).unwrap();
        match back {
            ClientMsg::SecretSubmit { id, value } => {
                assert_eq!(id, "s-9");
                assert_eq!(value, "hunter2");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_server_token_serde_roundtrip() {
        let msg = ServerMsg::Token {
            id: "req-1".into(),
            text: "Hello ".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ServerMsg = serde_json::from_str(&json).unwrap();
        match back {
            ServerMsg::Token { id, text } => {
                assert_eq!(id, "req-1");
                assert_eq!(text, "Hello ");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_server_tool_use_serde() {
        let msg = ServerMsg::ToolUse {
            id: "req-1".into(),
            call_id: "call_1".into(),
            name: "power".into(),
            args: serde_json::json!({"action": "reboot"}),
            confirm_required: true,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ServerMsg = serde_json::from_str(&json).unwrap();
        match back {
            ServerMsg::ToolUse {
                id,
                call_id,
                name,
                args,
                confirm_required,
            } => {
                assert_eq!(id, "req-1");
                assert_eq!(call_id, "call_1");
                assert_eq!(name, "power");
                assert_eq!(args["action"], "reboot");
                assert!(confirm_required);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_envelope_serde() {
        let env = Envelope {
            v: 1,
            msg: ClientMsg::ProvidersList { id: "r".into() },
        };
        let json = serde_json::to_string(&env).unwrap();
        let back: Envelope = serde_json::from_str(&json).unwrap();
        assert_eq!(back.v, 1);
        assert!(matches!(back.msg, ClientMsg::ProvidersList { .. }));
    }

    #[test]
    fn test_status_serde() {
        let msg = ServerMsg::Status {
            provider: "kimi".into(),
            model: "kimi-k2-0905".into(),
            connected: true,
            streaming: false,
            daemon_version: env!("CARGO_PKG_VERSION").into(),
            config: serde_json::json!({ "stream_tokens": true }),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ServerMsg = serde_json::from_str(&json).unwrap();
        match back {
            ServerMsg::Status {
                provider,
                model,
                connected,
                ..
            } => {
                assert_eq!(provider, "kimi");
                assert_eq!(model, "kimi-k2-0905");
                assert!(connected);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_provider_entry_dto_serde() {
        let dto = ProviderEntryDto {
            name: "ollama".into(),
            needs_key: false,
            has_key: true,
        };
        let json = serde_json::to_string(&dto).unwrap();
        let back: ProviderEntryDto = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "ollama");
        assert!(!back.needs_key);
        assert!(back.has_key);
    }

    /// The wire contract for every client → daemon message: each variant
    /// serializes with its snake_case `"type"` tag and round-trips losslessly.
    /// Guards the frontend (protocol.ts) contract against drift.
    #[test]
    fn test_all_client_msg_variants_roundtrip() {
        let msgs: Vec<(ClientMsg, &str)> = vec![
            (
                ClientMsg::ChatSend {
                    id: "r1".into(),
                    text: "hi".into(),
                },
                "chat_send",
            ),
            (ClientMsg::ToolApprove { id: "r1".into() }, "tool_approve"),
            (ClientMsg::ToolReject { id: "r1".into() }, "tool_reject"),
            (ClientMsg::EditApprove { id: "r1".into() }, "edit_approve"),
            (ClientMsg::EditReject { id: "r1".into() }, "edit_reject"),
            (ClientMsg::SpawnTui, "spawn_tui"),
            (
                ClientMsg::SecretSubmit {
                    id: "r1".into(),
                    value: "s".into(),
                },
                "secret_submit",
            ),
            (ClientMsg::SecretCancel { id: "r1".into() }, "secret_cancel"),
            (
                ClientMsg::FileRead {
                    id: "r1".into(),
                    path: "/etc/hostname".into(),
                },
                "file_read",
            ),
            (
                ClientMsg::FileWrite {
                    id: "r1".into(),
                    path: "/tmp/a".into(),
                    content: "x".into(),
                },
                "file_write",
            ),
            (
                ClientMsg::FileList {
                    id: "r1".into(),
                    path: "/tmp".into(),
                },
                "file_list",
            ),
            (
                ClientMsg::TermInput {
                    data: "ls\n".into(),
                },
                "term_input",
            ),
            (
                ClientMsg::TermResize {
                    cols: 120,
                    rows: 40,
                },
                "term_resize",
            ),
            (
                ClientMsg::ProviderSwitch {
                    name: "kimi".into(),
                },
                "provider_switch",
            ),
            (
                ClientMsg::ModelSwitch {
                    model: "kimi-k2-0905".into(),
                },
                "model_switch",
            ),
            (
                ClientMsg::ProviderConnect {
                    id: "r1".into(),
                    name: "kimi".into(),
                    key: "sk-x".into(),
                },
                "provider_connect",
            ),
            (
                ClientMsg::AgentDelegate {
                    id: "r1".into(),
                    agent: "claude".into(),
                    task: "refactor".into(),
                },
                "agent_delegate",
            ),
            (
                ClientMsg::ConfigSet {
                    key: "theme".into(),
                    value: serde_json::json!("#C87F4A"),
                },
                "config_set",
            ),
            (
                ClientMsg::ProvidersList { id: "r1".into() },
                "providers_list",
            ),
            (ClientMsg::StatsGet { id: "r1".into() }, "stats_get"),
            (
                ClientMsg::ModelsList {
                    id: "r1".into(),
                    provider: "openai".into(),
                },
                "models_list",
            ),
            (ClientMsg::AgentsList { id: "r1".into() }, "agents_list"),
        ];

        assert_eq!(msgs.len(), 22, "every ClientMsg variant must be covered");

        for (msg, expected_tag) in msgs {
            let json = serde_json::to_string(&msg).unwrap();
            let value: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(
                value["type"], expected_tag,
                "ClientMsg tag mismatch for {expected_tag}: {json}"
            );

            let back: ClientMsg = serde_json::from_str(&json).unwrap();
            assert_eq!(
                std::mem::discriminant(&msg),
                std::mem::discriminant(&back),
                "ClientMsg roundtrip variant mismatch for {expected_tag}"
            );
        }
    }

    /// The wire contract for every daemon → client message: each variant
    /// serializes with its snake_case `"type"` tag and round-trips losslessly.
    #[test]
    fn test_all_server_msg_variants_roundtrip() {
        let entry = ProviderEntryDto {
            name: "ollama".into(),
            needs_key: false,
            has_key: true,
        };
        let file = FileEntryDto {
            name: "main.rs".into(),
            kind: "file".into(),
            size: 42,
            git: Some("M".into()),
        };

        let msgs: Vec<(ServerMsg, &str)> = vec![
            (
                ServerMsg::Token {
                    id: "r1".into(),
                    text: "Hello".into(),
                },
                "token",
            ),
            (
                ServerMsg::ToolUse {
                    id: "r1".into(),
                    call_id: "call_1".into(),
                    name: "power".into(),
                    args: serde_json::json!({"action": "reboot"}),
                    confirm_required: true,
                },
                "tool_use",
            ),
            (
                ServerMsg::ToolResult {
                    id: "r1".into(),
                    call_id: "call_1".into(),
                    success: true,
                    output: "ok".into(),
                    status_line: "[ok]".into(),
                },
                "tool_result",
            ),
            (
                ServerMsg::EditDiff {
                    id: "r1".into(),
                    path: "a.rs".into(),
                    diff: "-x\n+y".into(),
                },
                "edit_diff",
            ),
            (
                ServerMsg::Status {
                    provider: "kimi".into(),
                    model: "kimi-k2-0905".into(),
                    connected: true,
                    streaming: false,
                    daemon_version: "0.1.0".into(),
                    config: serde_json::json!({}),
                },
                "status",
            ),
            (
                ServerMsg::Providers {
                    id: "r1".into(),
                    entries: vec![entry.clone()],
                },
                "providers",
            ),
            (
                ServerMsg::Models {
                    id: "r1".into(),
                    provider: "openai".into(),
                    models: vec!["gpt-5".into()],
                    current: "gpt-5".into(),
                },
                "models",
            ),
            (
                ServerMsg::Stats {
                    id: "r1".into(),
                    tokens: 1234,
                    latency_ms: 567,
                    battery: "100%".into(),
                    wifi: "diad".into(),
                    cwd: "/home".into(),
                    idle_stage: "ACTIVE".into(),
                    rss_mb: 42,
                    perf_tier: "MID".into(),
                },
                "stats",
            ),
            (
                ServerMsg::TermOutput {
                    data: "ls\n".into(),
                },
                "term_output",
            ),
            (
                ServerMsg::SecretRequest {
                    id: "r1".into(),
                    label: "API key".into(),
                },
                "secret_request",
            ),
            (
                ServerMsg::ConfirmRequest {
                    id: "r1".into(),
                    label: "Reboot?".into(),
                },
                "confirm_request",
            ),
            (ServerMsg::StreamDone { id: "r1".into() }, "stream_done"),
            (
                ServerMsg::System {
                    message: "daemon ready".into(),
                },
                "system",
            ),
            (
                ServerMsg::Error {
                    id: Some("r1".into()),
                    message: "boom".into(),
                },
                "error",
            ),
            (
                ServerMsg::Agents {
                    id: "r1".into(),
                    agents: vec!["claude".into()],
                },
                "agents",
            ),
            (
                ServerMsg::Connected {
                    v: 1,
                    provider: "kimi".into(),
                    model: "kimi-k2-0905".into(),
                    connected: true,
                    streaming: false,
                    daemon_version: "0.1.0".into(),
                    config: serde_json::json!({}),
                },
                "connected",
            ),
            (
                ServerMsg::FileContent {
                    id: "r1".into(),
                    path: "/etc/hostname".into(),
                    content: "diad\n".into(),
                },
                "file_content",
            ),
            (
                ServerMsg::Files {
                    id: "r1".into(),
                    path: "/tmp".into(),
                    entries: vec![file.clone()],
                },
                "files",
            ),
            (
                ServerMsg::FileWritten {
                    id: "r1".into(),
                    path: "/tmp/a".into(),
                },
                "file_written",
            ),
        ];

        assert_eq!(msgs.len(), 19, "every ServerMsg variant must be covered");

        for (msg, expected_tag) in msgs {
            let json = serde_json::to_string(&msg).unwrap();
            let value: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(
                value["type"], expected_tag,
                "ServerMsg tag mismatch for {expected_tag}: {json}"
            );

            let back: ServerMsg = serde_json::from_str(&json).unwrap();
            assert_eq!(
                std::mem::discriminant(&msg),
                std::mem::discriminant(&back),
                "ServerMsg roundtrip variant mismatch for {expected_tag}"
            );
        }
    }

    /// The version envelope must be honored by both directions of the wire.
    #[test]
    fn test_envelope_version_contract() {
        let env = Envelope {
            v: 1,
            msg: ClientMsg::StatsGet { id: "r".into() },
        };
        let json = serde_json::to_string(&env).unwrap();
        let back: Envelope = serde_json::from_str(&json).unwrap();
        assert_eq!(back.v, PROTOCOL_VERSION);
        assert!(matches!(back.msg, ClientMsg::StatsGet { .. }));
        // Version + flattened message live at the top level (no nesting).
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["v"], 1);
        assert_eq!(value["type"], "stats_get");
    }
}
