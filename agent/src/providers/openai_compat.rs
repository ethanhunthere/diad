//! Universal OpenAI-compatible provider.
//!
//! Speaks the OpenAI Chat Completions protocol against ANY compatible
//! endpoint: Moonshot Kimi, DeepSeek, Groq, Mistral, OpenRouter, Ollama,
//! and arbitrary user-defined endpoints from `[providers.<name>]` sections
//! in diad.conf.
//!
//! The provider is fully config-driven — name, base URL, model, optional
//! API key (stored per-provider in the vault) and an optional static
//! `Authorization` header value. Ollama-style local endpoints need no key.

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;

use super::openai_common;
use super::traits::{LlmProvider, Message, StreamEvent, ToolDefinition};

/// A config-driven OpenAI-compatible provider.
pub struct OpenAiCompatProvider {
    /// Provider display name (e.g. "kimi", "myprovider").
    pub name: String,
    /// Base URL — the `/chat/completions` suffix is appended automatically
    /// (e.g. `https://api.moonshot.ai/v1`).
    pub base_url: String,
    /// Model name (e.g. "kimi-k2-0905", "deepseek-chat").
    pub model: String,
    /// Optional API key, read from the vault slot named after this provider.
    pub api_key: Option<String>,
    /// Optional static `Authorization` header value
    /// (e.g. `"Bearer sk-..."` for providers with pre-shared keys).
    pub auth_header: Option<String>,
    client: reqwest::Client,
}

impl OpenAiCompatProvider {
    /// Create a new provider for the given name/base URL/model.
    pub fn new(
        name: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            base_url: base_url.into(),
            model: model.into(),
            api_key: None,
            auth_header: None,
            client: super::openai_common::default_http_client(),
        }
    }

    /// Set the API key (normally injected from the vault by the factory).
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set a static `Authorization` header value.
    pub fn with_auth_header(mut self, auth_header: impl Into<String>) -> Self {
        self.auth_header = Some(auth_header.into());
        self
    }

    /// Override the model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// The full chat completions URL.
    fn chat_url(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }

    /// Lightweight, non-streaming key validation (`max_tokens: 1`).
    pub async fn validate_key(&self) -> anyhow::Result<()> {
        let request =
            openai_common::build_chat_request(&self.model, &[Message::user("Hi")], &[], 1, false);

        let mut req = self
            .client
            .post(self.chat_url())
            .header("Content-Type", "application/json");

        if let Some(key) = &self.api_key {
            req = req.header("Authorization", format!("Bearer {key}"));
        }
        if let Some(header) = &self.auth_header {
            req = req.header("Authorization", header);
        }

        let resp = req.json(&request).send().await?;

        let status = resp.status();
        if status.is_success() {
            Ok(())
        } else {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("HTTP {status}: {body}")
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatProvider {
    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolDefinition>,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<StreamEvent>> + Send>>> {
        let request = openai_common::build_chat_request(&self.model, &messages, &tools, 4096, true);

        let mut req = self
            .client
            .post(self.chat_url())
            .header("Content-Type", "application/json");

        if let Some(key) = &self.api_key {
            req = req.header("Authorization", format!("Bearer {key}"));
        }
        if let Some(header) = &self.auth_header {
            req = req.header("Authorization", header);
        }

        let response = req.json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "{} API error ({}): {}",
                self.name,
                status,
                body
            ));
        }

        let stream = response.bytes_stream();
        let event_stream = openai_common::parse_sse_stream(stream);

        Ok(Box::pin(event_stream))
    }

    fn provider_name(&self) -> String {
        format!("{} ({})", self.name, self.model)
    }

    fn requires_auth(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_url_appends_endpoint() {
        let p = OpenAiCompatProvider::new("kimi", "https://api.moonshot.ai/v1", "kimi-k2-0905");
        assert_eq!(p.chat_url(), "https://api.moonshot.ai/v1/chat/completions");
    }

    #[test]
    fn test_chat_url_trims_trailing_slash() {
        let p = OpenAiCompatProvider::new("ollama", "http://localhost:11434/v1/", "qwen2.5:7b");
        assert_eq!(p.chat_url(), "http://localhost:11434/v1/chat/completions");
    }

    #[test]
    fn test_provider_name_includes_model() {
        let p = OpenAiCompatProvider::new("deepseek", "https://api.deepseek.com", "deepseek-chat");
        assert_eq!(p.provider_name(), "deepseek (deepseek-chat)");
    }

    #[test]
    fn test_with_model_overrides() {
        let p = OpenAiCompatProvider::new("openrouter", "https://openrouter.ai/api/v1", "a")
            .with_model("anthropic/claude-sonnet-4");
        assert_eq!(p.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn test_keyless_provider_does_not_require_auth() {
        let p = OpenAiCompatProvider::new("ollama", "http://localhost:11434/v1", "qwen2.5:7b");
        assert!(!p.requires_auth());
    }

    /// End-to-end: `stream_chat` against a local mock SSE endpoint must parse
    /// text deltas, tool-call deltas, and the terminating `[DONE]` into the
    /// same [`StreamEvent`] sequence the rest of DIAD consumes.
    #[tokio::test]
    async fn test_stream_chat_parses_sse_from_mock_server() {
        use futures::StreamExt;
        use tokio::io::AsyncWriteExt;
        use tokio::net::TcpListener as AsyncListener;

        // A mock OpenAI-compatible chat/completions endpoint speaking SSE.
        const SSE_BODY: &str = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hello \"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"from the mock\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_9\",\"function\":{\"name\":\"power\",\"arguments\":\"{\\\"action\\\":\\\"reboot\\\"}\"}}]},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: [DONE]\n\n",
        );

        let listener = AsyncListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let serve = tokio::spawn(async move {
            // Accept one connection and answer it with the SSE payload.
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 8192];
            // Read the request head (non-blocking read until headers arrive).
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                match sock.try_read(&mut buf) {
                    Ok(n) if n > 0 => break,
                    Ok(_) => {
                        if std::time::Instant::now() > deadline {
                            break;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if std::time::Instant::now() > deadline {
                            break;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                    }
                    Err(e) => panic!("mock server read error: {e}"),
                }
            }
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                SSE_BODY.len(),
                SSE_BODY
            );
            sock.write_all(headers.as_bytes()).await.unwrap();
            // Keep the socket open briefly so the client finishes reading.
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        });

        let provider = OpenAiCompatProvider::new("mock", format!("http://{addr}/v1"), "mock-model");

        let mut stream = provider
            .stream_chat(vec![Message::user("ping")], vec![])
            .await
            .expect("stream_chat must succeed against the mock endpoint");

        let mut texts: Vec<String> = Vec::new();
        let mut tool_calls: Vec<(String, String)> = Vec::new();
        let mut done = false;

        while let Some(ev) = stream.next().await {
            let ev = ev.expect("no stream errors from mock payload");
            match ev {
                StreamEvent::TextDelta(t) => texts.push(t),
                StreamEvent::ToolUseStart { id, name } => tool_calls.push((id, name)),
                StreamEvent::ToolInputDelta(args) => assert_eq!(args, r#"{"action":"reboot"}"#),
                StreamEvent::ToolUseEnd { id } => assert_eq!(id, "call_9"),
                StreamEvent::StreamEnd { stop_reason } => {
                    done = true;
                    assert_eq!(stop_reason.as_deref(), Some("stop"));
                }
                StreamEvent::Error(e) => panic!("unexpected stream error: {e}"),
            }
        }

        serve.await.unwrap();

        assert_eq!(texts, ["Hello ", "from the mock"]);
        assert_eq!(tool_calls, [("call_9".to_string(), "power".to_string())]);
        assert!(done, "stream must end with StreamEnd");
    }

    /// A non-2xx endpoint must surface as an error, not a silent stream.
    #[tokio::test]
    async fn test_stream_chat_surfaces_http_error() {
        use tokio::io::AsyncWriteExt;
        use tokio::net::TcpListener as AsyncListener;

        let listener = AsyncListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let serve = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let body = r#"{"error":{"message":"bad key"}}"#;
            let headers = format!(
                "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            sock.write_all(headers.as_bytes()).await.unwrap();
        });

        let provider = OpenAiCompatProvider::new("mock", format!("http://{addr}/v1"), "m");

        let result = provider
            .stream_chat(vec![Message::user("hi")], vec![])
            .await;

        serve.await.unwrap();
        let err = match result {
            Ok(_) => panic!("non-2xx must fail stream_chat"),
            Err(e) => e,
        };
        assert!(
            err.to_string().contains("401"),
            "error should mention the HTTP status: {err}"
        );
    }
}
