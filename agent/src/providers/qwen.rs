//! Alibaba Qwen (DashScope) provider implementation.
//!
//! Communicates with the Alibaba Cloud DashScope API for Qwen models.
//! API docs: https://help.aliyun.com/document_detail/2712195.html

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};

use super::traits::{LlmProvider, Message, StreamEvent, ToolDefinition};

const QWEN_API_URL: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions";

/// Alibaba Qwen provider.
pub struct QwenProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl QwenProvider {
    /// Create a new QwenProvider.
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: "qwen-max".into(),
            client: reqwest::Client::new(),
        }
    }

    /// Set a custom model (e.g., "qwen-plus", "qwen-turbo", "qwen-max").
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }
}

#[derive(Serialize)]
struct QwenRequest {
    model: String,
    stream: bool,
    messages: Vec<QwenMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<QwenTool>>,
    temperature: f32,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_options: Option<QwenStreamOptions>,
}

#[derive(Serialize)]
struct QwenStreamOptions {
    include_usage: bool,
}

#[derive(Serialize)]
struct QwenMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct QwenTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: QwenFunctionDef,
}

#[derive(Serialize)]
struct QwenFunctionDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Deserialize)]
struct QwenChunk {
    choices: Vec<QwenChoice>,
}

#[derive(Deserialize)]
struct QwenChoice {
    delta: QwenDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize, Default)]
struct QwenDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<QwenToolCallDelta>>,
}

#[derive(Deserialize)]
struct QwenToolCallDelta {
    #[serde(default)]
    index: usize,
    #[serde(default)]
    id: Option<String>,
    function: Option<QwenFunctionDelta>,
}

#[derive(Deserialize)]
struct QwenFunctionDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[async_trait]
impl LlmProvider for QwenProvider {
    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolDefinition>,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<StreamEvent>> + Send>>> {
        let qwen_messages: Vec<QwenMessage> = messages
            .iter()
            .map(|m| QwenMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect();

        let qwen_tools: Option<Vec<QwenTool>> = if tools.is_empty() {
            None
        } else {
            Some(
                tools
                    .iter()
                    .map(|t| QwenTool {
                        tool_type: "function".into(),
                        function: QwenFunctionDef {
                            name: t.name.clone(),
                            description: t.description.clone(),
                            parameters: t.parameters.clone(),
                        },
                    })
                    .collect(),
            )
        };

        let request = QwenRequest {
            model: self.model.clone(),
            stream: true,
            messages: qwen_messages,
            tools: qwen_tools,
            temperature: 0.7,
            max_tokens: 4096,
            stream_options: Some(QwenStreamOptions {
                include_usage: true,
            }),
        };

        let response = self
            .client
            .post(QWEN_API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Qwen API error ({}): {}", status, body));
        }

        let stream = response.bytes_stream();
        let event_stream = Self::parse_sse_stream(stream);

        Ok(Box::pin(event_stream))
    }

    fn provider_name(&self) -> String {
        format!("Alibaba Qwen ({})", self.model)
    }

    fn requires_auth(&self) -> bool {
        true
    }
}

impl QwenProvider {
    fn parse_sse_stream(
        stream: impl Stream<Item = reqwest::Result<bytes::Bytes>> + Send + 'static,
    ) -> impl Stream<Item = anyhow::Result<StreamEvent>> + Send {
        use futures::StreamExt;

        stream.flat_map(|chunk_result| {
            let events: Vec<anyhow::Result<StreamEvent>> = match chunk_result {
                Ok(chunk) => {
                    let text = String::from_utf8_lossy(&chunk).to_string();
                    let mut results = Vec::new();
                    #[allow(unused_assignments)]
                    let mut current_tool_id = String::new();
                    #[allow(unused_assignments)]
                    let mut current_tool_name = String::new();

                    for line in text.lines() {
                        if let Some(data) = line.strip_prefix("data: ") {
                            if data == "[DONE]" {
                                results.push(Ok(StreamEvent::StreamEnd {
                                    stop_reason: Some("stop".into()),
                                }));
                                continue;
                            }

                            match serde_json::from_str::<QwenChunk>(data) {
                                Ok(chunk) => {
                                    for choice in &chunk.choices {
                                        // Handle text content
                                        if let Some(ref content) = choice.delta.content {
                                            if !content.is_empty() {
                                                results.push(Ok(StreamEvent::TextDelta(
                                                    content.clone(),
                                                )));
                                            }
                                        }

                                        // Handle tool calls
                                        if let Some(ref tool_calls) = choice.delta.tool_calls {
                                            for tc in tool_calls {
                                                if let Some(ref id) = tc.id {
                                                    current_tool_id = id.clone();
                                                }
                                                if let Some(ref function) = tc.function {
                                                    if let Some(ref name) = function.name {
                                                        current_tool_name = name.clone();
                                                        results.push(Ok(
                                                            StreamEvent::ToolUseStart {
                                                                id: current_tool_id.clone(),
                                                                name: current_tool_name.clone(),
                                                            },
                                                        ));
                                                    }
                                                    if let Some(ref args) = function.arguments {
                                                        results.push(Ok(
                                                            StreamEvent::ToolInputDelta(
                                                                args.clone(),
                                                            ),
                                                        ));
                                                    }
                                                }
                                            }
                                        }

                                        if let Some(ref reason) = choice.finish_reason {
                                            if reason == "tool_calls" && !current_tool_id.is_empty()
                                            {
                                                results.push(Ok(StreamEvent::ToolUseEnd {
                                                    id: current_tool_id.clone(),
                                                }));
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::debug!(
                                        "Failed to parse Qwen SSE: {} — data: {}",
                                        e,
                                        data
                                    );
                                }
                            }
                        }
                    }

                    results
                }
                Err(e) => {
                    vec![Err(anyhow::anyhow!("Stream error: {e}"))]
                }
            };

            futures::stream::iter(events)
        })
    }
}
