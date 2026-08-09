//! Google Gemini provider implementation.
//!
//! Communicates with the Google Gemini API.
//! API docs: https://ai.google.dev/api/rest/v1/models/streamGenerateContent

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};

use super::traits::{LlmProvider, Message, StreamEvent, ToolDefinition};

const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

/// Google Gemini provider.
pub struct GeminiProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl GeminiProvider {
    /// Create a new GeminiProvider.
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: "gemini-2.5-flash".into(),
            client: reqwest::Client::new(),
        }
    }

    /// Set a custom model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    fn api_url(&self) -> String {
        format!(
            "{}:streamGenerateContent?alt=sse&key={}",
            self.model, self.api_key
        )
    }
}

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<GeminiTool>>,
    generation_config: GeminiGenerationConfig,
}

#[derive(Serialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum GeminiPart {
    Text { text: String },
}

#[derive(Serialize)]
struct GeminiTool {
    function_declarations: Vec<GeminiFunctionDeclaration>,
}

#[derive(Serialize)]
struct GeminiFunctionDeclaration {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Serialize)]
struct GeminiGenerationConfig {
    temperature: f32,
    top_p: f32,
    max_output_tokens: u32,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiContentResponse>,
}

#[derive(Deserialize)]
struct GeminiContentResponse {
    parts: Option<Vec<GeminiPartResponse>>,
}

#[derive(Deserialize)]
struct GeminiPartResponse {
    #[serde(default)]
    text: Option<String>,
    #[serde(default, rename = "functionCall")]
    function_call: Option<GeminiFunctionCallResponse>,
}

#[derive(Deserialize)]
struct GeminiFunctionCallResponse {
    name: String,
    args: Option<serde_json::Value>,
}

#[async_trait]
impl LlmProvider for GeminiProvider {
    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolDefinition>,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<StreamEvent>> + Send>>> {
        let url = format!("{}/{}", GEMINI_API_BASE, self.api_url());

        let (system_instruction, contents) = Self::build_contents(&messages);

        let gemini_tools = if tools.is_empty() {
            None
        } else {
            Some(vec![GeminiTool {
                function_declarations: tools
                    .iter()
                    .map(|t| GeminiFunctionDeclaration {
                        name: t.name.clone(),
                        description: t.description.clone(),
                        parameters: t.parameters.clone(),
                    })
                    .collect(),
            }])
        };

        let request = GeminiRequest {
            contents,
            system_instruction,
            tools: gemini_tools,
            generation_config: GeminiGenerationConfig {
                temperature: 0.7,
                top_p: 0.95,
                max_output_tokens: 4096,
            },
        };

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Gemini API error ({}): {}",
                status,
                body
            ));
        }

        let stream = response.bytes_stream();
        let event_stream = Self::parse_sse_stream(stream);

        Ok(Box::pin(event_stream))
    }

    fn provider_name(&self) -> String {
        format!("Google Gemini ({})", self.model)
    }

    fn requires_auth(&self) -> bool {
        true
    }
}

impl GeminiProvider {
    fn build_contents(messages: &[Message]) -> (Option<GeminiContent>, Vec<GeminiContent>) {
        let system_messages: Vec<&Message> = messages.iter().filter(|m| m.role == "system").collect();
        let system_instruction = if system_messages.is_empty() {
            None
        } else {
            let text = system_messages
                .iter()
                .map(|m| m.content.as_str())
                .collect::<Vec<_>>()
                .join("\n\n");
            Some(GeminiContent {
                role: "user".into(),
                parts: vec![GeminiPart::Text { text }],
            })
        };

        let contents: Vec<GeminiContent> = messages
            .iter()
            .filter(|m| m.role != "system")
            .map(|m| GeminiContent {
                role: Self::map_role(&m.role),
                parts: vec![GeminiPart::Text {
                    text: m.content.clone(),
                }],
            })
            .collect();

        (system_instruction, contents)
    }

    fn map_role(role: &str) -> String {
        match role {
            "assistant" => "model".into(),
            "tool" => "function".into(),
            _ => "user".into(),
        }
    }

    fn parse_sse_stream(
        stream: impl Stream<Item = reqwest::Result<bytes::Bytes>> + Send + 'static,
    ) -> impl Stream<Item = anyhow::Result<StreamEvent>> + Send {
        use futures::StreamExt;

        stream.flat_map(|chunk_result| {
            let events: Vec<anyhow::Result<StreamEvent>> = match chunk_result {
                Ok(chunk) => {
                    let text = String::from_utf8_lossy(&chunk).to_string();
                    let mut results = Vec::new();

                    for line in text.lines() {
                        if let Some(data) = line.strip_prefix("data: ") {
                            if data == "[DONE]" {
                                results.push(Ok(StreamEvent::StreamEnd {
                                    stop_reason: Some("STOP".into()),
                                }));
                                continue;
                            }
                            match serde_json::from_str::<GeminiResponse>(data) {
                                Ok(response) => {
                                    if let Some(candidates) = &response.candidates {
                                        for candidate in candidates {
                                            if let Some(content) = &candidate.content {
                                                if let Some(parts) = &content.parts {
                                                    for part in parts {
                                                        if let Some(ref text) = part.text {
                                                            if !text.is_empty() {
                                                                results.push(Ok(
                                                                    StreamEvent::TextDelta(
                                                                        text.clone(),
                                                                    ),
                                                                ));
                                                            }
                                                        }
                                                        if let Some(ref fc) = part.function_call {
                                                            results.push(Ok(
                                                                StreamEvent::ToolUseStart {
                                                                    id: fc.name.clone(),
                                                                    name: fc.name.clone(),
                                                                },
                                                            ));
                                                            if let Some(ref args) = fc.args {
                                                                results.push(Ok(
                                                                    StreamEvent::ToolInputDelta(
                                                                        args.to_string(),
                                                                    ),
                                                                ));
                                                            }
                                                            results.push(Ok(
                                                                StreamEvent::ToolUseEnd {
                                                                    id: fc.name.clone(),
                                                                },
                                                            ));
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    // Some Gemini SSE lines may be non-JSON (e.g., keepalive)
                                    if !data.is_empty() && !data.starts_with('[') {
                                        // Silently skip non-JSON lines
                                    } else {
                                        tracing::debug!(
                                            "Failed to parse Gemini SSE: {} — data: {}",
                                            e,
                                            data
                                        );
                                    }
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
