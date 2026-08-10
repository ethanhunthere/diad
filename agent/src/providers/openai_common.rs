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

/// Byte-safe SSE line assembler (spec: UTF-8 multibyte chars and SSE lines
/// split across chunk boundaries must never corrupt or drop data).
///
/// SSE framing guarantees no `\n` inside a multibyte character, so keeping
/// everything after the last newline in the buffer makes line-level UTF-8
/// decoding exact — no lossy replacement characters from chunk splits.
#[derive(Default)]
pub struct SseDecoder {
    buf: Vec<u8>,
}

impl SseDecoder {
    /// Append a raw chunk; return every now-complete line (`\r` stripped).
    pub fn take_lines(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(chunk);
        let Some(last_nl) = self.buf.iter().rposition(|&b| b == b'\n') else {
            return Vec::new();
        };
        let complete: Vec<u8> = self.buf.drain(..=last_nl).collect();
        String::from_utf8_lossy(&complete)
            .lines()
            .map(|l| l.trim_end_matches('\r').to_string())
            .collect()
    }

    /// Bytes still waiting for the next chunk (diagnostics/tests).
    pub fn pending(&self) -> &[u8] {
        &self.buf
    }
}

/// Stateful OpenAI-style SSE event builder: decoder + tool-call state that
/// survives across chunk boundaries (id, name and args deltas routinely
/// arrive in different TCP chunks).
#[derive(Default)]
pub struct OpenAiSseState {
    decoder: SseDecoder,
    tool_id: String,
    tool_name: String,
}

impl OpenAiSseState {
    /// Feed one raw chunk; get every event it completes.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<anyhow::Result<StreamEvent>> {
        let mut out = Vec::new();
        for line in self.decoder.take_lines(chunk) {
            let Some(data) = line.strip_prefix("data: ") else {
                continue; // keep-alive comments, event: lines, blanks
            };
            if data == "[DONE]" {
                out.push(Ok(StreamEvent::StreamEnd {
                    stop_reason: Some("stop".into()),
                }));
                continue;
            }
            match serde_json::from_str::<ChatChunk>(data) {
                Ok(chunk) => {
                    for choice in &chunk.choices {
                        if let Some(ref content) = choice.delta.content {
                            if !content.is_empty() {
                                out.push(Ok(StreamEvent::TextDelta(content.clone())));
                            }
                        }
                        if let Some(ref tool_calls) = choice.delta.tool_calls {
                            for tc in tool_calls {
                                if let Some(ref id) = tc.id {
                                    self.tool_id = id.clone();
                                }
                                if let Some(ref function) = tc.function {
                                    if let Some(ref name) = function.name {
                                        self.tool_name = name.clone();
                                        out.push(Ok(StreamEvent::ToolUseStart {
                                            id: self.tool_id.clone(),
                                            name: self.tool_name.clone(),
                                        }));
                                    }
                                    if let Some(ref args) = function.arguments {
                                        out.push(Ok(StreamEvent::ToolInputDelta(args.clone())));
                                    }
                                }
                            }
                        }
                        if choice.finish_reason.as_deref() == Some("tool_calls") {
                            out.push(Ok(StreamEvent::ToolUseEnd {
                                id: self.tool_id.clone(),
                            }));
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("Failed to parse OpenAI-compatible SSE: {e} — data: {data}");
                }
            }
        }
        out
    }
}

/// Parse an SSE byte stream from an OpenAI-compatible chat completions API.
///
/// Emits [`StreamEvent`]s: text deltas, tool-call start/input/end, and
/// `StreamEnd` on `data: [DONE]`. Malformed lines are logged and skipped;
/// split UTF-8 / split lines are reassembled by [`OpenAiSseState`].
pub fn parse_sse_stream(
    stream: impl Stream<Item = reqwest::Result<bytes::Bytes>> + Send + 'static,
) -> impl Stream<Item = anyhow::Result<StreamEvent>> + Send {
    use futures::StreamExt;

    let stream = Box::pin(stream);
    let inner = futures::stream::unfold(
        (stream, OpenAiSseState::default()),
        |(mut stream, mut state)| async move {
            loop {
                match stream.next().await {
                    Some(Ok(chunk)) => {
                        let events = state.feed(&chunk);
                        if !events.is_empty() {
                            return Some((futures::stream::iter(events), (stream, state)));
                        }
                        // Chunk produced no complete events — read on.
                    }
                    Some(Err(e)) => {
                        return Some((
                            futures::stream::iter(vec![Err(anyhow::anyhow!("Stream error: {e}"))]),
                            (stream, state),
                        ));
                    }
                    None => return None,
                }
            }
        },
    );
    Box::pin(inner).flatten()
}

/// Shared HTTP client for every provider: connect timeout so a blackholed
/// endpoint cannot hang the agent forever (10 s), and a hard 5-minute cap
/// on a single chat turn.
pub fn default_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
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

    #[tokio::test]
    async fn test_parse_sse_utf8_split_byte_by_byte() {
        // Worst case: the stream arrives ONE BYTE at a time. A 4-byte emoji
        // and a 2-byte é are split across "chunks" and must survive intact.
        let payload = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"héllo🌍\"},\"finish_reason\":null}]}\n\n",
            "data: [DONE]\n\n",
        );
        let chunks: Vec<Result<bytes::Bytes, reqwest::Error>> = payload
            .as_bytes()
            .iter()
            .map(|b| Ok(bytes::Bytes::from(vec![*b])))
            .collect();
        let mut events = parse_sse_stream(futures::stream::iter(chunks));

        let mut text = String::new();
        let mut done = false;
        while let Some(ev) = events.next().await {
            match ev.expect("byte-by-byte stream must not error") {
                StreamEvent::TextDelta(t) => text.push_str(&t),
                StreamEvent::StreamEnd { .. } => done = true,
                _ => {}
            }
        }
        assert_eq!(text, "héllo🌍", "multibyte chars must survive chunk splits");
        assert!(done);
    }

    #[tokio::test]
    async fn test_parse_sse_data_line_split_across_chunks() {
        // One logical SSE line delivered in two chunks must not be dropped.
        let half1 = b"data: {\"choices\":[{\"delta\"".to_vec();
        let half2 =
            b":{\"content\":\"split\"},\"finish_reason\":null}]}\n\ndata: [DONE]\n\n".to_vec();
        let chunks: Vec<Result<bytes::Bytes, reqwest::Error>> =
            vec![Ok(bytes::Bytes::from(half1)), Ok(bytes::Bytes::from(half2))];
        let mut events = parse_sse_stream(futures::stream::iter(chunks));

        let mut text = String::new();
        let mut done = false;
        while let Some(ev) = events.next().await {
            match ev.expect("split-line stream must not error") {
                StreamEvent::TextDelta(t) => text.push_str(&t),
                StreamEvent::StreamEnd { .. } => done = true,
                _ => {}
            }
        }
        assert_eq!(text, "split", "a line split across chunks must still parse");
        assert!(done);
    }

    #[tokio::test]
    async fn test_parse_sse_tool_state_survives_chunk_boundaries() {
        // Tool id, name, args and finish arrive in FOUR separate chunks;
        // ToolUseEnd must still carry the id seen chunks earlier.
        let c1 = b"data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_42\",\"function\":{\"name\":\"read_file\"}}]},\"finish_reason\":null}]}\n\n".to_vec();
        let c2 = b"data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"path\\\":\"}}]},\"finish_reason\":null}]}\n\n".to_vec();
        let c3 = b"data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"/tmp\\\"}\"}}]},\"finish_reason\":null}]}\n\n".to_vec();
        let c4 = b"data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\ndata: [DONE]\n\n".to_vec();
        let chunks: Vec<Result<bytes::Bytes, reqwest::Error>> = vec![
            Ok(bytes::Bytes::from(c1)),
            Ok(bytes::Bytes::from(c2)),
            Ok(bytes::Bytes::from(c3)),
            Ok(bytes::Bytes::from(c4)),
        ];
        let mut events = parse_sse_stream(futures::stream::iter(chunks));

        let mut start: Option<(String, String)> = None;
        let mut args = String::new();
        let mut end_id: Option<String> = None;
        while let Some(ev) = events.next().await {
            match ev.expect("tool-state stream must not error") {
                StreamEvent::ToolUseStart { id, name } => start = Some((id, name)),
                StreamEvent::ToolInputDelta(a) => args.push_str(&a),
                StreamEvent::ToolUseEnd { id } => end_id = Some(id),
                _ => {}
            }
        }
        assert_eq!(
            start,
            Some(("call_42".into(), "read_file".into())),
            "tool start must carry id+name"
        );
        assert_eq!(
            args, "{\"path\":\"/tmp\"}",
            "args must reassemble across chunks"
        );
        assert_eq!(
            end_id.as_deref(),
            Some("call_42"),
            "end must keep the id across chunks"
        );
    }

    #[test]
    fn test_sse_decoder_holds_partial_utf8_and_lines() {
        let mut d = SseDecoder::default();
        // "a\n" + first two bytes of € (0xE2 0x82) — no newline yet for the rest.
        assert_eq!(d.take_lines(b"a\n\xe2\x82"), vec!["a".to_string()]);
        assert_eq!(d.pending(), b"\xe2\x82");
        // Complete the € and the line.
        assert_eq!(d.take_lines(b"\xac x\n"), vec!["€ x".to_string()]);
        assert!(d.pending().is_empty());
    }

    #[test]
    fn test_default_http_client_builds() {
        let _ = default_http_client();
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
