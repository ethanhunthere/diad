//! Alibaba Qwen (DashScope) provider implementation.
//!
//! Communicates with the Alibaba Cloud DashScope API for Qwen models.
//! API docs: https://help.aliyun.com/document_detail/2712195.html

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use serde::Serialize;

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
            client: super::openai_common::default_http_client(),
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
        let event_stream = super::openai_common::parse_sse_stream(stream);

        Ok(Box::pin(event_stream))
    }

    fn provider_name(&self) -> String {
        format!("Alibaba Qwen ({})", self.model)
    }

    fn requires_auth(&self) -> bool {
        true
    }
}
