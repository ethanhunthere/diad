//! Common traits and types for LLM providers.

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};

/// A chat message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role: "system", "user", "assistant", or "tool"
    pub role: String,
    /// Message content text
    pub content: String,
    /// For assistant tool-use blocks: the tool call ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    /// For assistant tool-use blocks: the tool name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_name: Option<String>,
    /// For assistant tool-use blocks: the parsed tool input JSON
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_input: Option<serde_json::Value>,
}

impl Message {
    /// Create a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
            tool_use_id: None,
            tool_use_name: None,
            tool_input: None,
        }
    }

    /// Create a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
            tool_use_id: None,
            tool_use_name: None,
            tool_input: None,
        }
    }

    /// Create an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
            tool_use_id: None,
            tool_use_name: None,
            tool_input: None,
        }
    }

    /// Create an assistant message containing a tool-use block
    /// (as returned by the LLM during an agentic tool call).
    pub fn assistant_tool_use(
        id: impl Into<String>,
        name: impl Into<String>,
        input: serde_json::Value,
    ) -> Self {
        Self {
            role: "assistant".into(),
            content: String::new(),
            tool_use_id: Some(id.into()),
            tool_use_name: Some(name.into()),
            tool_input: Some(input),
        }
    }

    /// Create a tool result message (the outcome of executing a tool call).
    pub fn tool_result(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".into(),
            content: content.into(),
            tool_use_id: Some(id.into()),
            tool_use_name: None,
            tool_input: None,
        }
    }
}

/// Definition of a tool that the LLM can call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Unique tool name
    pub name: String,
    /// Human-readable description of what the tool does
    pub description: String,
    /// JSON Schema describing the tool's parameters
    pub parameters: serde_json::Value,
}

/// Events emitted during a streaming response.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// A text delta from the LLM
    TextDelta(String),
    /// The start of a tool use block
    ToolUseStart { id: String, name: String },
    /// A chunk of tool input JSON
    ToolInputDelta(String),
    /// The end of a tool use block
    ToolUseEnd { id: String },
    /// The stream has finished (stop reason may be included)
    StreamEnd { stop_reason: Option<String> },
    /// An error occurred during streaming
    Error(String),
}

/// The core trait that all LLM providers must implement.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Stream a chat completion from the LLM.
    ///
    /// Takes a list of messages and optional tool definitions,
    /// returns a stream of events (text deltas, tool calls, etc.).
    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolDefinition>,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<StreamEvent>> + Send>>>;

    /// The display name of this provider.
    fn provider_name(&self) -> String;

    /// Whether this provider requires OAuth authentication.
    fn requires_auth(&self) -> bool;
}
