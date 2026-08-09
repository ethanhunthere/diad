//! DIAD AGENT tools — the capability surface of the agent.
//!
//! Every tool the LLM (or the user, offline) can invoke lives here,
//! grouped by domain:
//!
//! - `filesystem` / `files_universal` — file operations, sandboxed
//! - `hardware` / `hardware_universal` — wifi, audio, display, power
//! - `system` tools live in `system`, `process`, `exec`, `update`,
//!   `security`, `help`, `power`
//! - `network` / `external` — connectivity + delegation to external AI CLIs
//! - `registry` — the container that binds every tool to its handler
//!
//! Handlers are `Arc<dyn Fn>` closures registered at startup in a single
//! `ToolRegistry` (see `registry::create_default_registry`).

pub mod display;
pub mod exec;
pub mod external;
pub mod files_universal;
pub mod filesystem;
pub mod hardware;
pub mod hardware_universal;
pub mod help;
pub mod input;
pub mod network;
pub mod power;
pub mod process;
pub mod security;
pub mod system;
pub mod update;

/// Test utilities shared across tool modules.
#[cfg(test)]
pub mod testutil {
    /// Serializes tests that mutate the process-wide `HOME` env var.
    /// Poison-tolerant: a panic in one test never breaks the others.
    pub static HOME_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
}

use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// Definition of a tool available to LLMs and users.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Unique tool name (e.g., "read_file")
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// JSON Schema for the tool's parameters
    pub parameters: serde_json::Value,
}

/// The result of executing a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Whether the tool executed successfully
    pub success: bool,
    /// The output of the tool (stdout, file contents, etc.)
    pub output: String,
    /// Error message if success is false
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ToolResult {
    /// Create a successful result.
    pub fn ok(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            error: None,
        }
    }

    /// Create an error result.
    pub fn err(error: impl Into<String>) -> Self {
        Self {
            success: false,
            output: String::new(),
            error: Some(error.into()),
        }
    }
}

/// Type alias for a tool handler function.
pub type ToolHandler = Arc<dyn Fn(serde_json::Value) -> anyhow::Result<ToolResult> + Send + Sync>;

pub fn valid_identifier(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && !s.starts_with('-')
        && s.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '@' | '.' | '_' | '+' | ':' | '-')
        })
}


pub mod registry;

// Re-export the registry API so callers keep the stable `tools::` paths.
pub use registry::{create_default_registry, ToolRegistry};
pub(crate) use registry::registered_tools_snapshot;
