//! Shared OpenAI-compatible Chat Completions types + SSE parsing.
//!
//! Every OpenAI-compatible endpoint (OpenAI itself, Moonshot Kimi,
//! DeepSeek, Groq, Mistral, OpenRouter, Ollama, and arbitrary user-defined
//! endpoints) speaks the same wire protocol:
//!   POST {base}/chat/completions  with a JSON body
//!   SSE stream of `data: {json}` chunks ending with `data: [DONE]`
//!
//! This module centralizes the request building, chunk types, and stream
//! parsing so `openai.rs` (native OpenAI) and `openai_compat.rs` (everything
//! else) share one implementation.

use futures::Stream;
use serde::{Deserialize, Serialize};

use super::traits::{Message, StreamEvent, ToolDefinition};

/// An OpenAI-compatible chat completion request.
#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub stream: bool,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ChatTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ChatToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ChatFunctionCall,
}

#[derive(Debug, Serialize)]
pub struct ChatFunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Serialize)]
pub struct ChatTool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ChatFunctionDef,
}

#[derive(Debug, Serialize)]
pub struct ChatFunctionDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct ChatChunk {
    pub choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
pub struct ChatChoice {
    pub delta: ChatDelta,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ChatDelta {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<ChatToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
pub struct ChatToolCallDelta {
    #[serde(default)]
    pub index: usize,
    #[serde(default)]
    pub id: Option<String>,
    pub function: Option<ChatFunctionDelta>,
}

#[derive(Debug, Deserialize)]
pub struct ChatFunctionDelta {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arguments: Option<String>,
}

/// Build a serializable chat completion request from the DIAD message model.
///
/// `tools` are omitted entirely when empty (some OpenAI-compatible endpoints
/// reject an empty `tools: []`), matching the original providers' behavior.
pub fn build_chat_request(
    model: &str,
    messages: &[Message],
    tools: &[ToolDefinition],
    max_tokens: u32,
    stream: bool,
) -> ChatRequest {
    let chat_messages: Vec<ChatMessage> = messages
        .iter()
        .map(|m| ChatMessage {
            role: m.role.clone(),
            content: m.content.clone(),
            tool_calls: None,
            tool_call_id: None,
        })
        .collect();

    let chat_tools: Option<Vec<ChatTool>> = if tools.is_empty() {
        None
    } else {
        Some(
            tools
                .iter()
                .map(|t| ChatTool {
                    tool_type: "function".into(),
                    function: ChatFunctionDef {
                        name: t.name.clone(),
                        description: t.description.clone(),
                        parameters: t.parameters.clone(),
                    },
                })
                .collect(),
        )
    };

    ChatRequest {
        model: model.to_string(),
        stream,
        messages: chat_messages,
        tools: chat_tools,
        tool_choice: None,
        temperature: Some(0.7),
        max_tokens: Some(max_tokens),
    }
}

/// Parse an SSE byte stream from an OpenAI-compatible chat completions API.
///
/// Emits [`StreamEvent`]s: text deltas, tool-call start/input/end, and
/// `StreamEnd` on `data: [DONE]`. Malformed chunks are logged and skipped.
pub fn parse_sse_stream(
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

                        match serde_json::from_str::<ChatChunk>(data) {
                            Ok(chunk) => {
                                for choice in &chunk.choices {
                                    // Text content deltas
                                    if let Some(ref content) = choice.delta.content {
                                        if !content.is_empty() {
                                            results.push(Ok(StreamEvent::TextDelta(
                                                content.clone(),
                                            )));
                                        }
                                    }

                                    // Tool-call deltas (function name + streaming JSON args)
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

                                    // finish_reason == "tool_calls" ends the tool block
                                    if let Some(ref reason) = choice.finish_reason {
                                        if reason == "tool_calls" {
                                            results.push(Ok(StreamEvent::ToolUseEnd {
                                                id: current_tool_id.clone(),
                                            }));
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::debug!(
                                    "Failed to parse OpenAI-compatible SSE: {} — data: {}",
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

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    const PAYLOAD: &str = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"Hello \"},\"finish_reason\":null}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"world\"},\"finish_reason\":null}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"read_file\",\"arguments\":\"{\\\"path\\\":\\\"/tmp\\\"}\"}}]},\"finish_reason\":null}]}\n\n",
        "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );

    #[tokio::test]
    async fn test_parse_sse_text_and_tool_calls() {
        let stream = futures::stream::iter(vec![Ok::<_, reqwest::Error>(
            bytes::Bytes::from_static(PAYLOAD.as_bytes()),
        )]);
        let mut events = parse_sse_stream(stream);

        let mut got: Vec<StreamEvent> = Vec::new();
        while let Some(ev) = events.next().await {
            got.push(ev.expect("no stream errors in test payload"));
        }

        assert!(
            got.iter()
                .any(|e| matches!(e, StreamEvent::TextDelta(t) if t == "Hello ")),
            "missing first text delta"
        );
        assert!(
            got.iter()
                .any(|e| matches!(e, StreamEvent::TextDelta(t) if t == "world")),
            "missing second text delta"
        );
        assert!(
            got.iter().any(|e| matches!(
                e,
                StreamEvent::ToolUseStart { id, name } if id == "call_1" && name == "read_file"
            )),
            "missing tool-use start"
        );
        assert!(
            got.iter().any(|e| matches!(
                e,
                StreamEvent::ToolInputDelta(j) if j == "{\"path\":\"/tmp\"}"
            )),
            "missing tool input delta"
        );
        assert!(
            got.iter()
                .any(|e| matches!(e, StreamEvent::ToolUseEnd { id } if id == "call_1")),
            "missing tool-use end"
        );
        assert!(
            matches!(got.last(), Some(StreamEvent::StreamEnd { .. })),
            "stream must end with StreamEnd after [DONE]"
        );
    }

    #[tokio::test]
    async fn test_parse_sse_skips_malformed_chunks() {
        let payload = concat!(
            "data: not-json\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"},\"finish_reason\":null}]}\n\n",
            "data: [DONE]\n\n",
        );
        let stream = futures::stream::iter(vec![Ok::<_, reqwest::Error>(
            bytes::Bytes::from_static(payload.as_bytes()),
        )]);
        let mut events = parse_sse_stream(stream);

        let mut got: Vec<StreamEvent> = Vec::new();
        while let Some(ev) = events.next().await {
            got.push(ev.expect("no stream errors in test payload"));
        }

        assert!(
            got.iter()
                .any(|e| matches!(e, StreamEvent::TextDelta(t) if t == "ok")),
            "valid chunk must still parse after a malformed one"
        );
        assert!(matches!(got.last(), Some(StreamEvent::StreamEnd { .. })));
    }

    #[test]
    fn test_build_chat_request_omits_tools_when_empty() {
        let req = build_chat_request("gpt-5", &[Message::user("hi")], &[], 4096, true);
        assert!(req.tools.is_none());
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, "user");
        assert!(req.stream);
        assert_eq!(req.max_tokens, Some(4096));
    }
}
