//! Anthropic (Claude) provider implementation.
//!
//! Communicates with the Anthropic Messages API using Server-Sent Events (SSE).
//! API docs: https://docs.anthropic.com/en/api/messages

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::traits::{LlmProvider, Message, StreamEvent, ToolDefinition};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic provider for Claude models.
pub struct AnthropicProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl AnthropicProvider {
    /// Create a new AnthropicProvider.
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: "claude-sonnet-4-20250514".into(),
            client: super::openai_common::default_http_client(),
        }
    }

    /// Set a custom model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Validate an API key by making a lightweight, non-streaming request.
    pub async fn validate_key(&self) -> anyhow::Result<()> {
        let body = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 1,
            stream: false,
            messages: vec![AnthropicMessage {
                role: "user".into(),
                content: vec![AnthropicContent::Text { text: "Hi".into() }],
            }],
            system: None,
            tools: None,
        };

        let resp = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if status.is_success() {
            Ok(())
        } else {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("HTTP {status}: {body}")
        }
    }
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    stream: bool,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: Vec<AnthropicContent>,
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum AnthropicContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

#[derive(Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: Value,
}

#[derive(Deserialize)]
struct AnthropicSseEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    delta: Option<AnthropicDelta>,
    #[serde(default)]
    content_block: Option<AnthropicContentBlock>,
}

#[derive(Deserialize)]
struct AnthropicDelta {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    partial_json: Option<String>,
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicContentBlock {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(rename = "type", default)]
    block_type: Option<String>,
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolDefinition>,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<StreamEvent>> + Send>>> {
        let (system_content, chat_messages) = Self::split_system_messages(&messages);

        let anthropic_messages: Vec<AnthropicMessage> = chat_messages
            .iter()
            .filter(|m| m.role != "system")
            .map(|m| AnthropicMessage {
                role: Self::map_role(&m.role),
                content: Self::serialize_content(m),
            })
            .collect();

        let anthropic_tools: Option<Vec<AnthropicTool>> = if tools.is_empty() {
            None
        } else {
            Some(
                tools
                    .iter()
                    .map(|t| AnthropicTool {
                        name: t.name.clone(),
                        description: t.description.clone(),
                        input_schema: t.parameters.clone(),
                    })
                    .collect(),
            )
        };

        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            stream: true,
            messages: anthropic_messages,
            system: system_content,
            tools: anthropic_tools,
        };

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("anthropic-beta", "tools-2024-04-04")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Anthropic API error ({}): {}",
                status,
                body
            ));
        }

        let stream = response.bytes_stream();
        let event_stream = futures::stream::unfold(
            (
                stream,
                Vec::<u8>::new(),
                String::new(),
                String::new(),
                None::<String>,
            ),
            |(
                mut stream,
                mut buffer,
                mut current_tool_id,
                mut current_tool_name,
                mut stop_reason,
            )| async move {
                use futures::StreamExt;

                loop {
                    match stream.next().await {
                        Some(Ok(chunk)) => {
                            buffer.extend_from_slice(&chunk);

                            // Process complete SSE events from the byte buffer.
                            // Cutting at b"\n\n" never splits a UTF-8 codepoint
                            // (no 0x0A inside multibyte sequences), so the
                            // per-event decode below is exact.
                            while let Some(pos) = buffer.windows(2).position(|w| w == b"\n\n") {
                                let event_str =
                                    String::from_utf8_lossy(&buffer[..pos]).into_owned();
                                buffer.drain(..pos + 2);

                                for line in event_str.lines() {
                                    if let Some(data) = line.strip_prefix("data: ") {
                                        if data == "[DONE]" {
                                            return Some((
                                                Ok(StreamEvent::StreamEnd {
                                                    stop_reason: stop_reason.clone(),
                                                }),
                                                (
                                                    stream,
                                                    buffer,
                                                    current_tool_id,
                                                    current_tool_name,
                                                    stop_reason,
                                                ),
                                            ));
                                        }

                                        match serde_json::from_str::<AnthropicSseEvent>(data) {
                                            Ok(event) => match event.event_type.as_str() {
                                                "content_block_start" => {
                                                    if let Some(ref block) = event.content_block {
                                                        if block.block_type.as_deref()
                                                            == Some("tool_use")
                                                        {
                                                            let id = block
                                                                .id
                                                                .clone()
                                                                .unwrap_or_default();
                                                            let name = block
                                                                .name
                                                                .clone()
                                                                .unwrap_or_default();
                                                            current_tool_id = id.clone();
                                                            current_tool_name = name.clone();
                                                            return Some((
                                                                Ok(StreamEvent::ToolUseStart {
                                                                    id,
                                                                    name,
                                                                }),
                                                                (
                                                                    stream,
                                                                    buffer,
                                                                    current_tool_id,
                                                                    current_tool_name,
                                                                    stop_reason,
                                                                ),
                                                            ));
                                                        }
                                                    }
                                                }
                                                "content_block_delta" => {
                                                    if let Some(ref delta) = event.delta {
                                                        if let Some(ref text) = delta.text {
                                                            if !text.is_empty() {
                                                                return Some((
                                                                    Ok(StreamEvent::TextDelta(
                                                                        text.clone(),
                                                                    )),
                                                                    (
                                                                        stream,
                                                                        buffer,
                                                                        current_tool_id,
                                                                        current_tool_name,
                                                                        stop_reason,
                                                                    ),
                                                                ));
                                                            }
                                                        }
                                                        if let Some(ref json) = delta.partial_json {
                                                            return Some((
                                                                Ok(StreamEvent::ToolInputDelta(
                                                                    json.clone(),
                                                                )),
                                                                (
                                                                    stream,
                                                                    buffer,
                                                                    current_tool_id,
                                                                    current_tool_name,
                                                                    stop_reason,
                                                                ),
                                                            ));
                                                        }
                                                    }
                                                }
                                                "content_block_stop" => {
                                                    if !current_tool_id.is_empty() {
                                                        let id =
                                                            std::mem::take(&mut current_tool_id);
                                                        let _ =
                                                            std::mem::take(&mut current_tool_name);
                                                        return Some((
                                                            Ok(StreamEvent::ToolUseEnd { id }),
                                                            (
                                                                stream,
                                                                buffer,
                                                                current_tool_id,
                                                                current_tool_name,
                                                                stop_reason,
                                                            ),
                                                        ));
                                                    }
                                                }
                                                "message_delta" => {
                                                    if let Some(ref delta) = event.delta {
                                                        stop_reason = delta.stop_reason.clone();
                                                    }
                                                }
                                                "message_stop" => {
                                                    return Some((
                                                        Ok(StreamEvent::StreamEnd {
                                                            stop_reason: stop_reason.clone(),
                                                        }),
                                                        (
                                                            stream,
                                                            buffer,
                                                            current_tool_id,
                                                            current_tool_name,
                                                            stop_reason,
                                                        ),
                                                    ));
                                                }
                                                "error" => {
                                                    return Some((
                                                        Ok(StreamEvent::Error(format!(
                                                            "Anthropic error: {data}"
                                                        ))),
                                                        (
                                                            stream,
                                                            buffer,
                                                            current_tool_id,
                                                            current_tool_name,
                                                            stop_reason,
                                                        ),
                                                    ));
                                                }
                                                _ => {}
                                            },
                                            Err(e) => {
                                                tracing::debug!(
                                                    "Failed to parse SSE event: {} — data: {}",
                                                    e,
                                                    data
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Some(Err(e)) => {
                            return Some((
                                Ok(StreamEvent::Error(format!("Stream error: {e}"))),
                                (
                                    stream,
                                    buffer,
                                    current_tool_id,
                                    current_tool_name,
                                    stop_reason,
                                ),
                            ));
                        }
                        None => {
                            return None; // Stream exhausted
                        }
                    }
                }
            },
        );

        Ok(Box::pin(event_stream))
    }

    fn provider_name(&self) -> String {
        format!("Anthropic ({})", self.model)
    }

    fn requires_auth(&self) -> bool {
        true
    }
}

impl AnthropicProvider {
    /// Split system messages from the conversation.
    fn split_system_messages(messages: &[Message]) -> (Option<String>, Vec<Message>) {
        let system_content: Vec<String> = messages
            .iter()
            .filter(|m| m.role == "system")
            .map(|m| m.content.clone())
            .collect();

        let system = if system_content.is_empty() {
            None
        } else {
            Some(system_content.join("\n\n"))
        };

        let chat_messages = messages
            .iter()
            .filter(|m| m.role != "system")
            .cloned()
            .collect();

        (system, chat_messages)
    }

    /// Map internal roles to Anthropic role names.
    fn map_role(role: &str) -> String {
        match role {
            "assistant" => "assistant".into(),
            "tool" => "user".into(), // Anthropic expects tool results as user messages
            _ => "user".into(),
        }
    }

    /// Serialize a message into Anthropic content blocks.
    ///
    /// - Assistant messages with `tool_input` become `tool_use` blocks.
    /// - Tool-result messages (role "tool") with a `tool_use_id` become
    ///   `tool_result` blocks.
    /// - Everything else becomes a plain text block.
    fn serialize_content(m: &Message) -> Vec<AnthropicContent> {
        match m.role.as_str() {
            "tool" => {
                if let Some(id) = &m.tool_use_id {
                    vec![AnthropicContent::ToolResult {
                        tool_use_id: id.clone(),
                        content: m.content.clone(),
                    }]
                } else {
                    // No tool_use_id — degrade to plain user text
                    vec![AnthropicContent::Text {
                        text: m.content.clone(),
                    }]
                }
            }
            "assistant" => {
                if let (Some(id), Some(name), Some(input)) =
                    (&m.tool_use_id, &m.tool_use_name, &m.tool_input)
                {
                    vec![AnthropicContent::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    }]
                } else if !m.content.is_empty() {
                    vec![AnthropicContent::Text {
                        text: m.content.clone(),
                    }]
                } else {
                    // Empty assistant message with no tool call — omit content
                    vec![]
                }
            }
            _ => vec![AnthropicContent::Text {
                text: m.content.clone(),
            }],
        }
    }
}
