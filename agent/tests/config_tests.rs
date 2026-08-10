//! Integration tests for configuration loading and round-tripping.

use diad_agent::config::settings::DiadConfig;

#[test]
fn test_config_loads_defaults() {
    let config = DiadConfig::default();

    assert_eq!(config.default_provider, "anthropic");
    assert_eq!(config.log_level, "info");
    assert_eq!(config.theme, "dark");
    assert!(config.stream_tokens);
    assert!(config.confirm_destructive);
    assert_eq!(config.max_history, 100);
    assert_eq!(config.auto_save_interval, 0);
    assert!(!config.allowed_paths.is_empty());
}

#[test]
fn test_config_toml_roundtrip() {
    let original = DiadConfig::default();
    let toml_str = toml::to_string_pretty(&original).expect("Serialize should succeed");
    let parsed: DiadConfig = toml::from_str(&toml_str).expect("Deserialize should succeed");

    assert_eq!(parsed.default_provider, original.default_provider);
    assert_eq!(parsed.log_level, original.log_level);
    assert_eq!(parsed.theme, original.theme);
}
