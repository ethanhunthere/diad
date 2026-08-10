//! Integration tests for the provider factory and provider surface.

use std::sync::Arc;

use diad_agent::auth::vault::Vault;
use diad_agent::config::settings::DiadConfig;
use diad_agent::providers::ProviderFactory;

#[test]
fn test_provider_factory_unknown_provider() {
    let vault = Arc::new(Vault::new().expect("Vault should initialize"));
    let config = Arc::new(DiadConfig::default());
    let factory = ProviderFactory::new(vault, config);

    let provider = factory.create("nonexistent");
    assert!(provider.is_none());
}
