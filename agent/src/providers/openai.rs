//! OpenAI provider implementation.
//!
//! Communicates with the OpenAI Chat Completions API using SSE streaming.
//! API docs: https://platform.openai.com/docs/api-reference/chat
//!
//! The request building + SSE parsing live in `openai_common` so every
//! OpenAI-compatible provider (Kimi, DeepSeek, Groq, Mistral, OpenRouter,
//! Ollama, custom endpoints) shares one implementation.

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;

use super::openai_common;
use super::traits::{LlmProvider, Message, StreamEvent, ToolDefinition};

const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";

/// OpenAI provider for GPT models.
pub struct OpenAiProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl OpenAiProvider {
    /// Create a new OpenAiProvider.
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: "gpt-5".into(),
            client: super::openai_common::default_http_client(),
        }
    }

    /// Set a custom model (e.g. "gpt-5-codex", "codex-mini-latest", "o4-mini").
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Lightweight, non-streaming key validation (`max_tokens: 1`).
    pub async fn validate_key(&self) -> anyhow::Result<()> {
        let request =
            openai_common::build_chat_request(&self.model, &[Message::user("Hi")], &[], 1, false);

        let resp = self
            .client
            .post(OPENAI_API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
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

#[async_trait]
impl LlmProvider for OpenAiProvider {
    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolDefinition>,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<StreamEvent>> + Send>>> {
        let request = openai_common::build_chat_request(&self.model, &messages, &tools, 4096, true);

        let response = self
            .client
            .post(OPENAI_API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("OpenAI API error ({}): {}", status, body));
        }

        let stream = response.bytes_stream();
        let event_stream = openai_common::parse_sse_stream(stream);

        Ok(Box::pin(event_stream))
    }

    fn provider_name(&self) -> String {
        format!("OpenAI ({})", self.model)
    }

    fn requires_auth(&self) -> bool {
        true
    }
}
