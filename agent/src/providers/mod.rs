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
        name: "xai",
        base_url: "https://api.x.ai/v1",
        model: "grok-2-latest",
        needs_key: true,
    },
    ProviderPreset {
        name: "together",
        base_url: "https://api.together.xyz/v1",
        model: "meta-llama/Llama-3.3-70B-Instruct-Turbo",
        needs_key: true,
    },
    ProviderPreset {
        name: "fireworks",
        base_url: "https://api.fireworks.ai/inference/v1",
        model: "accounts/fireworks/models/llama-v3p3-70b-instruct",
        needs_key: true,
    },
    ProviderPreset {
        name: "cohere",
        base_url: "https://api.cohere.com/v1",
        model: "command-r-plus-08-2024",
        needs_key: true,
    },
    ProviderPreset {
        name: "perplexity",
        base_url: "https://api.perplexity.ai",
        model: "sonar-pro",
        needs_key: true,
    },
    ProviderPreset {
        name: "cerebras",
        base_url: "https://api.cerebras.ai/v1",
        model: "llama-3.3-70b",
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
    "claude-3-7-sonnet-20250219",
    "claude-3-5-sonnet-20241022",
    "claude-3-5-haiku-20241022",
    "claude-3-opus-20240229",
    "claude-sonnet-4-20250514",
    "claude-opus-4-20250514",
    "claude-haiku-4-20250514",
    "claude-opus-4-8",
    "claude-opus-5",
    "claude-sonnet-5",
    "claude-haiku-5",
    "claude-fable-5",
    "claude-3-sonnet-20240229",
    "claude-3-haiku-20240307",
    "claude-2.1",
    "claude-2.0",
];
const OPENAI_MODELS: &[&str] = &[
    "gpt-5",
    "gpt-5-codex",
    "codex-mini-latest",
    "o4-mini",
    "gpt-4o",
    "gpt-4o-mini",
    "o1",
    "o1-mini",
    "o3-mini",
    "o3",
    "gpt-4.5-preview",
    "gpt-5-pro",
    "gpt-4-turbo",
    "gpt-4",
    "gpt-3.5-turbo",
];
const GEMINI_MODELS: &[&str] = &[
    "gemini-2.5-pro",
    "gemini-2.5-flash",
    "gemini-2.5-flash-lite",
    "gemini-2.0-flash",
    "gemini-2.0-flash-lite",
    "gemini-2.0-pro-exp",
    "gemini-2.0-flash-thinking-exp",
    "gemini-1.5-pro",
    "gemini-1.5-flash",
    "gemini-1.5-flash-8b",
    "gemini-3.0-pro",
    "gemini-3.0-flash",
];
const QWEN_MODELS: &[&str] = &[
    "qwen-max",
    "qwen-plus",
    "qwen-turbo",
    "qwen-2.5-max",
    "qwen-2.5-72b-instruct",
    "qwen-2.5-32b-instruct",
    "qwen-2.5-14b-instruct",
    "qwen-2.5-7b-instruct",
    "qwen-2.5-coder-32b-instruct",
    "qwen-2.5-coder-14b-instruct",
    "qwen-2.5-coder-7b-instruct",
    "qwen-2.5-math-72b-instruct",
    "qwq-32b-preview",
    "qvq-72b-preview",
    "qwen-3-max",
    "qwen-3-coder",
];
const DEEPSEEK_MODELS: &[&str] = &[
    "deepseek-chat",
    "deepseek-reasoner",
    "deepseek-coder",
    "deepseek-r1-zero",
    "deepseek-v4",
    "deepseek-r2",
];
const KIMI_MODELS: &[&str] = &[
    "kimi-k2-0905",
    "kimi-k1.5",
    "kimi-latest",
    "moonshot-v1-8k",
    "moonshot-v1-32k",
    "moonshot-v1-128k",
    "moonshot-v1-auto",
];
const XAI_MODELS: &[&str] = &[
    "grok-2-latest",
    "grok-2-1212",
    "grok-2-vision-1212",
    "grok-beta",
    "grok-vision-beta",
    "grok-3",
    "grok-3-mini",
];
const GROQ_MODELS: &[&str] = &[
    "llama-3.3-70b-versatile",
    "llama-3.3-70b-specdec",
    "llama-3.1-8b-instant",
    "llama-3.1-70b-versatile",
    "deepseek-r1-distill-llama-70b",
    "deepseek-r1-distill-qwen-32b",
    "mixtral-8x7b-32768",
    "gemma2-9b-it",
    "qwen-2.5-32b",
    "qwen-2.5-coder-32b",
];
const MISTRAL_MODELS: &[&str] = &[
    "mistral-large-latest",
    "mistral-large-2411",
    "mistral-small-latest",
    "mistral-small-2409",
    "codestral-latest",
    "codestral-2501",
    "pixtral-large-latest",
    "pixtral-12b",
    "ministral-8b-latest",
    "ministral-3b-latest",
    "open-mistral-nemo",
];
const OPENROUTER_MODELS: &[&str] = &[
    "openrouter/auto",
    "anthropic/claude-3.7-sonnet",
    "anthropic/claude-3.7-sonnet:thinking",
    "anthropic/claude-3.5-sonnet",
    "anthropic/claude-3.5-haiku",
    "anthropic/claude-3-opus",
    "anthropic/claude-sonnet-4",
    "anthropic/claude-opus-5",
    "anthropic/claude-fable-5",
    "openai/o3-mini",
    "openai/o1",
    "openai/gpt-4o",
    "openai/gpt-4.5-preview",
    "deepseek/deepseek-r1",
    "deepseek/deepseek-chat",
    "meta-llama/llama-3.3-70b-instruct",
    "google/gemini-2.0-flash-001",
    "google/gemini-2.5-pro",
    "qwen/qwen-2.5-coder-32b-instruct",
    "mistralai/mistral-large-2411",
];
const TOGETHER_MODELS: &[&str] = &[
    "meta-llama/Llama-3.3-70B-Instruct-Turbo",
    "meta-llama/Meta-Llama-3.1-405B-Instruct-Turbo",
    "deepseek-ai/DeepSeek-R1",
    "deepseek-ai/DeepSeek-V3",
    "Qwen/Qwen2.5-Coder-32B-Instruct",
    "Qwen/Qwen2.5-72B-Instruct-Turbo",
    "mistralai/Mixtral-8x22B-Instruct-v0.1",
];
const FIREWORKS_MODELS: &[&str] = &[
    "accounts/fireworks/models/deepseek-r1",
    "accounts/fireworks/models/deepseek-v3",
    "accounts/fireworks/models/llama-v3p3-70b-instruct",
    "accounts/fireworks/models/qwen2p5-coder-32b-instruct",
];
const COHERE_MODELS: &[&str] = &[
    "command-r-plus-08-2024",
    "command-r-08-2024",
    "command-r-plus",
    "command-r",
    "command-light",
];
const PERPLEXITY_MODELS: &[&str] = &[
    "sonar-pro",
    "sonar",
    "sonar-reasoning",
    "sonar-reasoning-pro",
    "r1-1776",
];
const CEREBRAS_MODELS: &[&str] = &[
    "llama-3.3-70b",
    "llama3.1-70b",
    "llama3.1-8b",
];
const OLLAMA_MODELS: &[&str] = &[
    "qwen2.5-coder:32b",
    "qwen2.5-coder:14b",
    "qwen2.5-coder:7b",
    "qwen2.5:72b",
    "qwen2.5:32b",
    "qwen2.5:14b",
    "qwen2.5:7b",
    "deepseek-r1:70b",
    "deepseek-r1:32b",
    "deepseek-r1:14b",
    "deepseek-r1:8b",
    "deepseek-r1:1.5b",
    "llama3.3:70b",
    "llama3.1:8b",
    "mistral-nemo:12b",
    "codestral:22b",
    "phi4:14b",
    "gemma2:27b",
    "gemma2:9b",
];

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
            "deepseek" => DEEPSEEK_MODELS.iter().map(|s| s.to_string()).collect(),
            "kimi" => KIMI_MODELS.iter().map(|s| s.to_string()).collect(),
            "xai" => XAI_MODELS.iter().map(|s| s.to_string()).collect(),
            "groq" => GROQ_MODELS.iter().map(|s| s.to_string()).collect(),
            "mistral" => MISTRAL_MODELS.iter().map(|s| s.to_string()).collect(),
            "openrouter" => OPENROUTER_MODELS.iter().map(|s| s.to_string()).collect(),
            "together" => TOGETHER_MODELS.iter().map(|s| s.to_string()).collect(),
            "fireworks" => FIREWORKS_MODELS.iter().map(|s| s.to_string()).collect(),
            "cohere" => COHERE_MODELS.iter().map(|s| s.to_string()).collect(),
            "perplexity" => PERPLEXITY_MODELS.iter().map(|s| s.to_string()).collect(),
            "cerebras" => CEREBRAS_MODELS.iter().map(|s| s.to_string()).collect(),
            "ollama" => OLLAMA_MODELS.iter().map(|s| s.to_string()).collect(),
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
        OPENAI_COMPAT_PRESETS.iter().any(|p| p.name == n) || self.config.providers.contains_key(&n)
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
                    tracing::info!("No lightweight validation for provider '{n}' — accepting key.");
                    Ok(())
                }
            }
        }
    }

    /// (base_url, model, auth_header) for an OpenAI-compatible provider.
    fn openai_compat_params(&self, name: &str) -> Option<(String, String, Option<String>)> {
        if let Some(preset) = OPENAI_COMPAT_PRESETS.iter().find(|p| p.name == name) {
            return Some((preset.base_url.to_string(), preset.model.to_string(), None));
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
        assert!(names.contains(&"kimi"));
        assert!(names.contains(&"deepseek"));
        assert!(names.contains(&"groq"));
        assert!(names.contains(&"mistral"));
        assert!(names.contains(&"openrouter"));
        assert!(names.contains(&"xai"));
        assert!(names.contains(&"together"));
        assert!(names.contains(&"fireworks"));
        assert!(names.contains(&"cohere"));
        assert!(names.contains(&"perplexity"));
        assert!(names.contains(&"cerebras"));
        assert!(names.contains(&"ollama"));

        let kimi = OPENAI_COMPAT_PRESETS
            .iter()
            .find(|p| p.name == "kimi")
            .unwrap();
        assert_eq!(kimi.base_url, "https://api.moonshot.ai/v1");
        assert_eq!(kimi.model, "kimi-k2-0905");
        assert!(kimi.needs_key);

        let deepseek = OPENAI_COMPAT_PRESETS
            .iter()
            .find(|p| p.name == "deepseek")
            .unwrap();
        assert_eq!(deepseek.base_url, "https://api.deepseek.com");
        assert_eq!(deepseek.model, "deepseek-chat");
        assert!(deepseek.needs_key);

        let groq = OPENAI_COMPAT_PRESETS
            .iter()
            .find(|p| p.name == "groq")
            .unwrap();
        assert_eq!(groq.base_url, "https://api.groq.com/openai/v1");
        assert_eq!(groq.model, "llama-3.3-70b-versatile");

        let mistral = OPENAI_COMPAT_PRESETS
            .iter()
            .find(|p| p.name == "mistral")
            .unwrap();
        assert_eq!(mistral.base_url, "https://api.mistral.ai/v1");
        assert_eq!(mistral.model, "mistral-large-latest");

        let openrouter = OPENAI_COMPAT_PRESETS
            .iter()
            .find(|p| p.name == "openrouter")
            .unwrap();
        assert_eq!(openrouter.base_url, "https://openrouter.ai/api/v1");
        assert_eq!(openrouter.model, "anthropic/claude-sonnet-4");

        let xai = OPENAI_COMPAT_PRESETS
            .iter()
            .find(|p| p.name == "xai")
            .unwrap();
        assert_eq!(xai.base_url, "https://api.x.ai/v1");
        assert_eq!(xai.model, "grok-2-latest");
        assert!(xai.needs_key);

        let ollama = OPENAI_COMPAT_PRESETS
            .iter()
            .find(|p| p.name == "ollama")
            .unwrap();
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
            OPENAI_MODELS
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(factory.default_model("openai"), "gpt-5");
    }

    /// The GUI contract: the OpenAI CODEX model family and every preset must
    /// be present with the exact names the picker renders.
    #[test]
    fn test_openai_coding_models_contract() {
        // The OpenAI CODEX models the GUI offers.
        for codex_m in ["gpt-5", "gpt-5-codex", "codex-mini-latest", "o4-mini"] {
            assert!(
                OPENAI_MODELS.contains(&codex_m),
                "OPENAI_MODELS must contain {codex_m}"
            );
        }

        let factory = test_factory();
        for model in OPENAI_MODELS {
            assert!(
                factory.models_for("openai").iter().any(|m| m == model),
                "openai must offer {model}"
            );
        }
        assert_eq!(factory.default_model("openai"), "gpt-5");

        // Every preset must be registered, offer a default model, and list
        // its default model.
        assert_eq!(OPENAI_COMPAT_PRESETS.len(), 12);
        for preset in OPENAI_COMPAT_PRESETS {
            assert!(
                !preset.base_url.is_empty(),
                "{} needs a base_url",
                preset.name
            );
            assert!(
                !preset.model.is_empty(),
                "{} needs a default model",
                preset.name
            );
            assert!(
                factory
                    .models_for(preset.name)
                    .contains(&preset.model.to_string()),
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
        assert!(names.contains(&"gemini"));
        assert!(names.contains(&"openai"));
        assert!(names.contains(&"qwen"));
        assert!(names.contains(&"kimi"));
        assert!(names.contains(&"deepseek"));
        assert!(names.contains(&"groq"));
        assert!(names.contains(&"mistral"));
        assert!(names.contains(&"openrouter"));
        assert!(names.contains(&"xai"));
        assert!(names.contains(&"together"));
        assert!(names.contains(&"fireworks"));
        assert!(names.contains(&"cohere"));
        assert!(names.contains(&"perplexity"));
        assert!(names.contains(&"cerebras"));
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
