//! DIAD AGENT tools — the capability surface of the agent.
//!
//! Every tool the LLM (or the user, offline) can invoke lives here, grouped
//! by domain:
//!
//! - `files` — sandboxed file operations + universal file surface
//! - `hardware` — wifi, audio, display, input, universal hardware surface
//! - `network` — HTTP, ping, delegation to external AI CLIs
//! - `system` — OS info, packages, services, processes, power, updates,
//!   security, and the agent's help surface
//! - `registry` — the container that binds every tool to its handler
//!
//! Handlers are `Arc<dyn Fn>` closures registered at startup in a single
//! `ToolRegistry` (see `registry::create_default_registry`).

pub mod files;
pub mod hardware;
pub mod network;
pub mod registry;
pub mod system;

// Facade: stable `tools::` paths for the rest of the crate.
pub use network::external;
pub use system::help;

/// Test utilities shared across tool modules.
#[cfg(test)]
pub mod testutil {
    /// Serializes tests that mutate the process-wide `HOME` env var.
    /// Poison-tolerant: a panic in one test never breaks the others.
    pub static HOME_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// RAII guard that points `HOME` at a fresh temp dir for the duration of
    /// a test and restores the original value on drop (or removes it if it
    /// was unset). Holds `HOME_LOCK` so parallel tests never observe a
    /// foreign `HOME`.
    pub struct TestHome {
        _lock: std::sync::MutexGuard<'static, ()>,
        original: Option<std::ffi::OsString>,
        _dir: tempfile::TempDir,
    }

    impl TestHome {
        /// Point `HOME` at a fresh temp dir for this test.
        pub fn new() -> Self {
            let lock = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let original = std::env::var_os("HOME");
            let dir = tempfile::tempdir().expect("tempdir for test HOME");
            std::env::set_var("HOME", dir.path());
            Self { _lock: lock, original, _dir: dir }
        }

        /// The temp dir currently backing `HOME` (also useful for writing
        /// fixtures the code under test expects under `$HOME`).
        pub fn path(&self) -> &std::path::Path {
            self._dir.path()
        }
    }

    impl Default for TestHome {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Drop for TestHome {
        fn drop(&mut self) {
            match &self.original {
                Some(home) => std::env::set_var("HOME", home),
                None => std::env::remove_var("HOME"),
            }
        }
    }
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



pub use registry::{create_default_registry, ToolRegistry};
pub(crate) use registry::registered_tools_snapshot;
