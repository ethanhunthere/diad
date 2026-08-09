//! Application configuration — load, save, and manage DIAD settings.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Main DIAD Agent configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiadConfig {
    /// Default LLM provider (anthropic, gemini, openai, qwen)
    #[serde(default = "default_provider")]
    pub default_provider: String,

    /// Paths that tools are allowed to access
    #[serde(default = "default_allowed_paths")]
    pub allowed_paths: Vec<String>,

    /// Log level (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// UI theme (dark, light)
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Whether to stream tokens from the LLM
    #[serde(default = "default_true")]
    pub stream_tokens: bool,

    /// Whether to require confirmation before destructive actions
    #[serde(default = "default_true")]
    pub confirm_destructive: bool,

    /// Custom keybindings (action -> key combination)
    #[serde(default)]
    pub keybindings: HashMap<String, String>,

    /// Maximum conversation history to keep in memory
    #[serde(default = "default_max_history")]
    pub max_history: usize,

    /// Auto-save session interval in seconds (0 = disabled)
    #[serde(default)]
    pub auto_save_interval: u64,

    /// Persistent storage location (e.g. the DIAD-DATA partition)
    #[serde(default = "default_storage")]
    pub storage: StorageConfig,

    /// User-defined providers, e.g.:
    ///
    /// ```toml
    /// [providers.myprovider]
    /// type = "openai_compat"
    /// base_url = "https://api.example.com/v1"
    /// model = "my-model"
    /// # optional static Authorization header (overrides the vault key):
    /// # auth_header = "Bearer sk-..."
    /// ```
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
}

/// A user-defined provider section from `[providers.<name>]` in diad.conf.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Provider type — only `"openai_compat"` is supported for user-defined
    /// providers (any OpenAI Chat Completions-compatible endpoint).
    #[serde(rename = "type", default = "default_provider_type")]
    pub provider_type: String,
    /// Base URL of the OpenAI-compatible endpoint (e.g. `https://api.x/v1`).
    pub base_url: Option<String>,
    /// Model name to use by default.
    pub model: Option<String>,
    /// Optional static `Authorization` header value. When set, no vault key
    /// is required for this provider.
    pub auth_header: Option<String>,
}

fn default_provider_type() -> String {
    "openai_compat".into()
}

/// Persistent storage configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Directory where persistent user data lives
    /// (e.g. /home/diad/data, backed by the DIAD-DATA partition).
    pub persistent_path: String,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            persistent_path: "/home/diad/data".into(),
        }
    }
}

fn default_storage() -> StorageConfig {
    StorageConfig::default()
}

fn default_provider() -> String {
    "anthropic".into()
}

fn default_allowed_paths() -> Vec<String> {
    vec!["~".into(), "/tmp".into(), "/etc/diad".into()]
}

fn default_log_level() -> String {
    "info".into()
}

fn default_theme() -> String {
    "dark".into()
}

fn default_true() -> bool {
    true
}

fn default_max_history() -> usize {
    100
}

impl Default for DiadConfig {
    fn default() -> Self {
        Self {
            default_provider: default_provider(),
            allowed_paths: default_allowed_paths(),
            log_level: default_log_level(),
            theme: default_theme(),
            stream_tokens: true,
            confirm_destructive: true,
            keybindings: HashMap::new(),
            max_history: 100,
            auto_save_interval: 0,
            storage: StorageConfig::default(),
            providers: HashMap::new(),
        }
    }
}

impl DiadConfig {
    /// Load configuration from the standard search paths.
    ///
    /// Search order:
    /// 1. Path provided via `--config` CLI flag (if any)
    /// 2. `$HOME/.config/diad/config.toml`
    /// 3. `/etc/diad/diad.conf`
    ///
    /// Falls back to defaults if no config file is found.
    pub fn load(cli_path: Option<&str>) -> anyhow::Result<Self> {
        let config_paths = if let Some(path) = cli_path {
            vec![PathBuf::from(path)]
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            vec![
                PathBuf::from(&home).join(".config/diad/config.toml"),
                PathBuf::from("/etc/diad/diad.conf"),
            ]
        };

        let mut config = Self::default();

        for path in &config_paths {
            if path.exists() {
                let content = std::fs::read_to_string(path)?;
                let file_config: DiadConfig = toml::from_str(&content)?;

                // Merge file config over defaults
                if !file_config.default_provider.is_empty() {
                    config.default_provider = file_config.default_provider;
                }
                if !file_config.allowed_paths.is_empty() {
                    config.allowed_paths = file_config.allowed_paths;
                }
                if !file_config.log_level.is_empty() {
                    config.log_level = file_config.log_level;
                }
                if !file_config.theme.is_empty() {
                    config.theme = file_config.theme;
                }
                config.stream_tokens = file_config.stream_tokens;
                config.confirm_destructive = file_config.confirm_destructive;
                if !file_config.keybindings.is_empty() {
                    config.keybindings = file_config.keybindings;
                }
                if file_config.max_history > 0 {
                    config.max_history = file_config.max_history;
                }
                if file_config.auto_save_interval > 0 {
                    config.auto_save_interval = file_config.auto_save_interval;
                }
                if !file_config.storage.persistent_path.is_empty() {
                    config.storage = file_config.storage;
                }
                if !file_config.providers.is_empty() {
                    config.providers = file_config.providers;
                }

                tracing::info!("Loaded config from: {}", path.display());
                break;
            }
        }

        Ok(config)
    }

    /// Save configuration to the user's config directory.
    ///
    /// Writes to `$HOME/.config/diad/config.toml`, creating directories as needed.
    pub fn save(&self, path: Option<&str>) -> anyhow::Result<()> {
        let file_path = if let Some(p) = path {
            PathBuf::from(p)
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            let config_dir = PathBuf::from(&home).join(".config/diad");
            std::fs::create_dir_all(&config_dir)?;
            config_dir.join("config.toml")
        };

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&file_path, content)?;
        tracing::info!("Config saved to: {}", file_path.display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DiadConfig::default();
        assert_eq!(config.default_provider, "anthropic");
        assert_eq!(config.log_level, "info");
        assert_eq!(config.theme, "dark");
        assert!(config.stream_tokens);
        assert!(config.confirm_destructive);
        assert_eq!(config.max_history, 100);
        assert!(!config.allowed_paths.is_empty());
        assert_eq!(config.storage.persistent_path, "/home/diad/data");
    }

    #[test]
    fn test_serialize_deserialize() {
        let config = DiadConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: DiadConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed.default_provider, config.default_provider);
        assert_eq!(parsed.log_level, config.log_level);
        assert_eq!(parsed.theme, config.theme);
        assert_eq!(parsed.storage.persistent_path, "/home/diad/data");
    }

    #[test]
    fn test_custom_config() {
        let toml_str = r#"
default_provider = "openai"
log_level = "debug"
theme = "light"
stream_tokens = false
confirm_destructive = false
max_history = 50
"#;

        let config: DiadConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.default_provider, "openai");
        assert_eq!(config.log_level, "debug");
        assert_eq!(config.theme, "light");
        assert!(!config.stream_tokens);
        assert!(!config.confirm_destructive);
        assert_eq!(config.max_history, 50);
    }

    #[test]
    fn test_custom_provider_sections_parse() {
        let toml_str = r#"
default_provider = "myprovider"

[providers.myprovider]
type = "openai_compat"
base_url = "https://api.example.com/v1"
model = "my-model"
auth_header = "Bearer static-key"

[providers.local]
type = "openai_compat"
base_url = "http://localhost:8080/v1"
model = "tiny-model"
"#;

        let config: DiadConfig = toml::from_str(toml_str).unwrap();

        assert_eq!(config.providers.len(), 2);

        let my = &config.providers["myprovider"];
        assert_eq!(my.provider_type, "openai_compat");
        assert_eq!(my.base_url.as_deref(), Some("https://api.example.com/v1"));
        assert_eq!(my.model.as_deref(), Some("my-model"));
        assert_eq!(my.auth_header.as_deref(), Some("Bearer static-key"));

        let local = &config.providers["local"];
        assert_eq!(local.provider_type, "openai_compat");
        assert_eq!(local.auth_header, None);
    }

    #[test]
    fn test_default_provider_type_is_openai_compat() {
        let toml_str = r#"
[providers.foo]
base_url = "https://api.foo/v1"
"#;
        let config: DiadConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.providers["foo"].provider_type, "openai_compat");
    }
}
