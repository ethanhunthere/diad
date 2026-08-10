//! LLM provider adapters.
//!
//! All providers implement the shared [`traits::LlmProvider`] streaming
//! interface. Anthropic, Gemini and Qwen speak their native protocols;
//! OpenAI and everything else (DeepSeek, Kimi, Groq, Mistral, OpenRouter,
//! Ollama, custom) shares the OpenAI-compatible code path in `openai_compat`.

pub mod anthropic;
pub mod gemini;
pub mod openai;
pub mod openai_common;
pub mod openai_compat;
pub mod qwen;
pub mod traits;

use std::sync::Arc;

use crate::auth::vault::Vault;
use crate::config::settings::DiadConfig;
use traits::LlmProvider;

use anthropic::AnthropicProvider;
use gemini::GeminiProvider;
use openai::OpenAiProvider;
use openai_compat::OpenAiCompatProvider;
use qwen::QwenProvider;

// ---------------------------------------------------------------------------
// Built-in OpenAI-compatible presets
// ---------------------------------------------------------------------------

/// A built-in OpenAI-compatible provider preset.
#[derive(Debug, Clone, Copy)]
pub struct ProviderPreset {
    /// Provider name (also the vault slot name for its API key).
    pub name: &'static str,
    /// Base URL — `/chat/completions` is appended at request time.
    pub base_url: &'static str,
    /// Default model.
    pub model: &'static str,
    /// Whether an API key must be stored in the vault for this provider
    /// (Ollama is local and keyless).
    pub needs_key: bool,
}

/// Every built-in OpenAI-compatible provider that can be selected from the
/// picker. Each one's API key is stored in the vault under its own name.
pub const OPENAI_COMPAT_PRESETS: &[ProviderPreset] = &[
    ProviderPreset {
        name: "kimi",
        base_url: "https://api.moonshot.ai/v1",
        model: "kimi-k2-0905",
        needs_key: true,
    },
    ProviderPreset {
        name: "deepseek",
        base_url: "https://api.deepseek.com",
        model: "deepseek-chat",
        needs_key: true,
    },
    ProviderPreset {
        name: "groq",
        base_url: "https://api.groq.com/openai/v1",
        model: "llama-3.3-70b-versatile",
        needs_key: true,
    },
    ProviderPreset {
        name: "mistral",
        base_url: "https://api.mistral.ai/v1",
        model: "mistral-large-latest",
        needs_key: true,
    },
    ProviderPreset {
        name: "openrouter",
        base_url: "https://openrouter.ai/api/v1",
        model: "anthropic/claude-sonnet-4",
        needs_key: true,
    },
    ProviderPreset {
        name: "ollama",
        base_url: "http://localhost:11434/v1",
        model: "qwen2.5:7b",
        needs_key: false,
    },
];

// ---------------------------------------------------------------------------
// Model lists per built-in provider
// ---------------------------------------------------------------------------

const ANTHROPIC_MODELS: &[&str] = &[
    "claude-sonnet-4-20250514",
    "claude-opus-4-20250514",
    "claude-haiku-4-20250514",
];
const OPENAI_MODELS: &[&str] = &["gpt-5", "gpt-5-codex", "codex-mini-latest", "o4-mini"];
const GEMINI_MODELS: &[&str] = &["gemini-2.5-flash", "gemini-2.5-pro", "gemini-2.5-flash-lite"];
const QWEN_MODELS: &[&str] = &["qwen-max", "qwen-plus", "qwen-turbo"];

// ---------------------------------------------------------------------------
// Provider factory
// ---------------------------------------------------------------------------

/// One entry in the provider picker (F6 / `/model`).
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderEntry {
    /// Provider name.
    pub name: String,
    /// Whether this provider needs a stored API key.
    pub needs_key: bool,
    /// Whether a key is present (or the provider is keyless).
    pub has_key: bool,
}

/// Factory for creating LLM provider instances.
///
/// Reads API keys from the secure vault (slot = provider name) and
/// instantiates the appropriate provider implementation — built-in
/// providers (anthropic, gemini, openai, qwen), OpenAI-compatible presets
/// (kimi, deepseek, groq, mistral, openrouter, ollama), and arbitrary
/// user-defined providers from `[providers.<name>]` config sections.
pub struct ProviderFactory {
    vault: Arc<Vault>,
    config: Arc<DiadConfig>,
}

impl ProviderFactory {
    /// Create a new ProviderFactory.
    pub fn new(vault: Arc<Vault>, config: Arc<DiadConfig>) -> Self {
        Self { vault, config }
    }

    /// The API key stored for a provider, if any.
    pub fn get_key(&self, name: &str) -> Option<String> {
        self.vault.get_token(name)
    }

    /// Create a provider by name using its default model.
    pub fn create(&self, name: &str) -> Option<Box<dyn LlmProvider>> {
        self.create_with_model(name, None)
    }

    /// Create a provider by name, optionally overriding the active model.
    ///
    /// Returns `None` if the provider is unknown or its required API key is
    /// missing from the vault.
    pub fn create_with_model(
        &self,
        name: &str,
        model: Option<&str>,
    ) -> Option<Box<dyn LlmProvider>> {
        let n = name.to_lowercase();

        match n.as_str() {
            "anthropic" => {
                let api_key = self.vault.get_token("anthropic")?;
                let m = model.unwrap_or(ANTHROPIC_MODELS[0]);
                Some(Box::new(AnthropicProvider::new(api_key).with_model(m)))
            }
            "gemini" => {
                let api_key = self.vault.get_token("gemini")?;
                let m = model.unwrap_or(GEMINI_MODELS[0]);
                Some(Box::new(GeminiProvider::new(api_key).with_model(m)))
            }
            "openai" => {
                let api_key = self.vault.get_token("openai")?;
                let m = model.unwrap_or(OPENAI_MODELS[0]);
                Some(Box::new(OpenAiProvider::new(api_key).with_model(m)))
            }
            "qwen" => {
                let api_key = self.vault.get_token("qwen")?;
                let m = model.unwrap_or(QWEN_MODELS[0]);
                Some(Box::new(QwenProvider::new(api_key).with_model(m)))
            }
            _ => self.create_openai_compat(&n, model),
        }
    }

    /// Build an OpenAI-compatible provider from a preset or user config.
    fn create_openai_compat(
        &self,
        name: &str,
        model: Option<&str>,
    ) -> Option<Box<dyn LlmProvider>> {
        // Built-in presets
        if let Some(preset) = OPENAI_COMPAT_PRESETS.iter().find(|p| p.name == name) {
            let m = model.unwrap_or(preset.model);
            let mut provider = OpenAiCompatProvider::new(name, preset.base_url, m);
            if preset.needs_key {
                let api_key = self.vault.get_token(name)?;
                provider = provider.with_api_key(api_key);
            }
            return Some(Box::new(provider));
        }

        // User-defined providers from diad.conf: [providers.<name>]
        let user = self.config.providers.get(name)?;
        if user.provider_type != "openai_compat" {
            tracing::warn!(
                "Unsupported provider type '{}' for {name} (only 'openai_compat' is supported)",
                user.provider_type
            );
            return None;
        }
        let base_url = user.base_url.clone()?;
        if base_url.trim().is_empty() {
            tracing::warn!("Provider '{name}' has an empty base_url in config");
            return None;
        }
        let m = model
            .map(str::to_string)
            .unwrap_or_else(|| user.model.clone().unwrap_or_else(|| "default".into()));

        let mut provider = OpenAiCompatProvider::new(name, base_url, m);
        if let Some(header) = &user.auth_header {
            provider = provider.with_auth_header(header.clone());
        }
        if user.auth_header.is_none() {
            let api_key = self.vault.get_token(name)?;
            provider = provider.with_api_key(api_key);
        }
        Some(Box::new(provider))
    }

    /// All selectable provider names: built-ins, presets, then config-defined.
    pub fn all_provider_names(&self) -> Vec<String> {
        let mut names: Vec<String> = vec![
            "anthropic".into(),
            "gemini".into(),
            "openai".into(),
            "qwen".into(),
        ];
        for preset in OPENAI_COMPAT_PRESETS {
            names.push(preset.name.to_string());
        }
        for key in self.config.providers.keys() {
            if !names.iter().any(|n| n == key) {
                names.push(key.clone());
            }
        }
        names
    }

    /// Whether this provider requires a stored API key.
    pub fn needs_key(&self, name: &str) -> bool {
        let n = name.to_lowercase();
        match n.as_str() {
            "anthropic" | "gemini" | "openai" | "qwen" => true,
            _ => {
                if let Some(preset) = OPENAI_COMPAT_PRESETS.iter().find(|p| p.name == n) {
                    preset.needs_key
                } else if let Some(user) = self.config.providers.get(&n) {
                    user.auth_header.is_none()
                } else {
                    true
                }
            }
        }
    }

    /// Whether this provider is ready to use (key present, or keyless).
    pub fn has_key(&self, name: &str) -> bool {
        !self.needs_key(name) || self.vault.get_token(name).is_some()
    }

    /// Entries for the provider picker, with connection status.
    pub fn provider_entries(&self) -> Vec<ProviderEntry> {
        self.all_provider_names()
            .into_iter()
            .map(|name| ProviderEntry {
                needs_key: self.needs_key(&name),
                has_key: self.has_key(&name),
                name,
            })
            .collect()
    }

    /// The selectable models for a provider (used by the model picker).
    pub fn models_for(&self, name: &str) -> Vec<String> {
        let n = name.to_lowercase();
        match n.as_str() {
            "anthropic" => ANTHROPIC_MODELS.iter().map(|s| s.to_string()).collect(),
            "openai" => OPENAI_MODELS.iter().map(|s| s.to_string()).collect(),
            "gemini" => GEMINI_MODELS.iter().map(|s| s.to_string()).collect(),
            "qwen" => QWEN_MODELS.iter().map(|s| s.to_string()).collect(),
            _ => {
                if let Some(preset) = OPENAI_COMPAT_PRESETS.iter().find(|p| p.name == n) {
                    vec![preset.model.to_string()]
                } else if let Some(user) = self.config.providers.get(&n) {
                    vec![user.model.clone().unwrap_or_else(|| "default".into())]
                } else {
                    vec!["default".into()]
                }
            }
        }
    }

    /// The default model for a provider (first entry of its model list).
    pub fn default_model(&self, name: &str) -> String {
        self.models_for(name)
            .into_iter()
            .next()
            .unwrap_or_else(|| "default".into())
    }

    /// Whether a provider name maps to an OpenAI-compatible endpoint
    /// (preset or user-defined).
    pub fn is_openai_compat(&self, name: &str) -> bool {
        let n = name.to_lowercase();
        OPENAI_COMPAT_PRESETS.iter().any(|p| p.name == n)
            || self.config.providers.contains_key(&n)
    }

    /// Validate an API key for a provider. Returns `Ok(())` for providers
    /// without a lightweight validation endpoint (gemini, qwen).
    pub async fn validate(&self, name: &str, key: &str) -> anyhow::Result<()> {
        let n = name.to_lowercase();
        match n.as_str() {
            "anthropic" => AnthropicProvider::new(key.to_string()).validate_key().await,
            "openai" => OpenAiProvider::new(key.to_string()).validate_key().await,
            _ => {
                if self.is_openai_compat(&n) {
                    let (base_url, model, auth_header) =
                        self.openai_compat_params(&n).ok_or_else(|| {
                            anyhow::anyhow!("Provider '{n}' has no endpoint configuration")
                        })?;
                    let mut provider = OpenAiCompatProvider::new(&n, base_url, model)
                        .with_api_key(key.to_string());
                    if let Some(header) = auth_header {
                        provider = provider.with_auth_header(header);
                    }
                    provider.validate_key().await
                } else {
                    tracing::info!(
                        "No lightweight validation for provider '{n}' — accepting key."
                    );
                    Ok(())
                }
            }
        }
    }

    /// (base_url, model, auth_header) for an OpenAI-compatible provider.
    fn openai_compat_params(&self, name: &str) -> Option<(String, String, Option<String>)> {
        if let Some(preset) = OPENAI_COMPAT_PRESETS.iter().find(|p| p.name == name) {
            return Some((
                preset.base_url.to_string(),
                preset.model.to_string(),
                None,
            ));
        }
        let user = self.config.providers.get(name)?;
        let base_url = user.base_url.clone()?;
        if base_url.trim().is_empty() {
            return None;
        }
        let model = user.model.clone().unwrap_or_else(|| "default".into());
        Some((base_url, model, user.auth_header.clone()))
    }

    /// List all providers that are ready to use.
    pub fn available_providers(&self) -> Vec<String> {
        self.all_provider_names()
            .into_iter()
            .filter(|name| self.has_key(name))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_factory() -> ProviderFactory {
        let vault = Arc::new(Vault::new().expect("vault construction never fails"));
        let config = Arc::new(DiadConfig::default());
        ProviderFactory::new(vault, config)
    }

    #[test]
    fn test_preset_registry_completeness() {
        let names: Vec<&str> = OPENAI_COMPAT_PRESETS.iter().map(|p| p.name).collect();
        assert_eq!(
            names,
            ["kimi", "deepseek", "groq", "mistral", "openrouter", "ollama"]
        );

        let kimi = OPENAI_COMPAT_PRESETS.iter().find(|p| p.name == "kimi").unwrap();
        assert_eq!(kimi.base_url, "https://api.moonshot.ai/v1");
        assert_eq!(kimi.model, "kimi-k2-0905");
        assert!(kimi.needs_key);

        let deepseek = OPENAI_COMPAT_PRESETS.iter().find(|p| p.name == "deepseek").unwrap();
        assert_eq!(deepseek.base_url, "https://api.deepseek.com");
        assert_eq!(deepseek.model, "deepseek-chat");
        assert!(deepseek.needs_key);

        let groq = OPENAI_COMPAT_PRESETS.iter().find(|p| p.name == "groq").unwrap();
        assert_eq!(groq.base_url, "https://api.groq.com/openai/v1");
        assert_eq!(groq.model, "llama-3.3-70b-versatile");

        let mistral = OPENAI_COMPAT_PRESETS.iter().find(|p| p.name == "mistral").unwrap();
        assert_eq!(mistral.base_url, "https://api.mistral.ai/v1");
        assert_eq!(mistral.model, "mistral-large-latest");

        let openrouter = OPENAI_COMPAT_PRESETS.iter().find(|p| p.name == "openrouter").unwrap();
        assert_eq!(openrouter.base_url, "https://openrouter.ai/api/v1");
        assert_eq!(openrouter.model, "anthropic/claude-sonnet-4");

        let ollama = OPENAI_COMPAT_PRESETS.iter().find(|p| p.name == "ollama").unwrap();
        assert_eq!(ollama.base_url, "http://localhost:11434/v1");
        assert_eq!(ollama.model, "qwen2.5:7b");
        assert!(!ollama.needs_key);
    }

    #[test]
    fn test_ollama_needs_no_key_and_creates() {
        let factory = test_factory();
        assert!(!factory.needs_key("ollama"));
        assert!(factory.has_key("ollama"));
        assert!(factory.create("ollama").is_some());
    }

    #[test]
    fn test_openai_model_list() {
        let factory = test_factory();
        assert_eq!(
            factory.models_for("openai"),
            ["gpt-5", "gpt-5-codex", "codex-mini-latest", "o4-mini"]
        );
        assert_eq!(factory.default_model("openai"), "gpt-5");
    }

    /// The GUI contract: the OpenAI CODEX model family and every preset must
    /// be present with the exact names the picker renders.
    #[test]
    fn test_openai_coding_models_contract() {
        // The four OpenAI CODEX models the GUI offers.
        assert_eq!(
            OPENAI_MODELS,
            ["gpt-5", "gpt-5-codex", "codex-mini-latest", "o4-mini"]
        );

        let factory = test_factory();
        for model in OPENAI_MODELS {
            assert!(
                factory.models_for("openai").iter().any(|m| m == model),
                "openai must offer {model}"
            );
        }
        assert_eq!(factory.default_model("openai"), "gpt-5");

        // Every preset must be registered, offer a default model, and list
        // exactly the models the picker expects.
        assert_eq!(OPENAI_COMPAT_PRESETS.len(), 6);
        for preset in OPENAI_COMPAT_PRESETS {
            assert!(!preset.base_url.is_empty(), "{} needs a base_url", preset.name);
            assert!(!preset.model.is_empty(), "{} needs a default model", preset.name);
            assert_eq!(
                factory.models_for(preset.name),
                [preset.model],
                "{} must advertise its default model",
                preset.name
            );
        }

        // Keyed presets refuse to activate without a stored key; ollama (the
        // only keyless preset) must activate immediately.
        for preset in OPENAI_COMPAT_PRESETS {
            if preset.needs_key {
                assert!(
                    factory.create(preset.name).is_none(),
                    "{} must require a key",
                    preset.name
                );
            } else {
                assert!(
                    factory.create(preset.name).is_some(),
                    "{} must work without a key",
                    preset.name
                );
            }
        }
    }

    #[test]
    fn test_provider_entries_include_presets() {
        let factory = test_factory();
        let entries = factory.provider_entries();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"anthropic"));
        assert!(names.contains(&"kimi"));
        assert!(names.contains(&"deepseek"));
        assert!(names.contains(&"groq"));
        assert!(names.contains(&"mistral"));
        assert!(names.contains(&"openrouter"));
        assert!(names.contains(&"ollama"));

        let ollama = entries.iter().find(|e| e.name == "ollama").unwrap();
        assert!(!ollama.needs_key);
        assert!(ollama.has_key);
    }

    #[test]
    fn test_custom_provider_from_config() {
        let mut config = DiadConfig::default();
        config.providers.insert(
            "myprovider".into(),
            crate::config::settings::ProviderConfig {
                provider_type: "openai_compat".into(),
                base_url: Some("https://api.example.com/v1".into()),
                model: Some("my-model".into()),
                auth_header: Some("Bearer static-key".into()),
            },
        );
        let factory = ProviderFactory::new(Arc::new(Vault::new().unwrap()), Arc::new(config));

        assert!(!factory.needs_key("myprovider"));
        assert!(factory.has_key("myprovider"));
        assert_eq!(factory.models_for("myprovider"), ["my-model"]);
        assert!(factory.is_openai_compat("myprovider"));
        assert!(factory.create("myprovider").is_some());
    }
}
