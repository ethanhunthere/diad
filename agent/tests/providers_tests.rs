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

#[test]
fn test_all_16_providers_creation_and_models() {
    let vault = Arc::new(Vault::new().expect("Vault should initialize"));
    let config = Arc::new(DiadConfig::default());

    // Pre-populate mock tokens for all providers that require keys
    let all_providers = [
        "anthropic",
        "openai",
        "gemini",
        "qwen",
        "kimi",
        "deepseek",
        "groq",
        "mistral",
        "openrouter",
        "xai",
        "together",
        "fireworks",
        "cohere",
        "perplexity",
        "cerebras",
        "ollama",
    ];

    for p in all_providers {
        if p != "ollama" {
            vault
                .store_token(p, &format!("mock-key-for-{p}"))
                .expect("token stored");
        }
    }

    let factory = ProviderFactory::new(vault, config);

    for provider_name in all_providers {
        assert!(
            factory.has_key(provider_name),
            "Factory must recognize {provider_name} key"
        );

        let models = factory.models_for(provider_name);
        assert!(
            !models.is_empty(),
            "Provider {provider_name} must offer models"
        );

        // Test default creation
        let default_prov = factory.create(provider_name);
        assert!(
            default_prov.is_some(),
            "Default creation failed for {provider_name}"
        );
        let default_prov = default_prov.unwrap();
        assert!(default_prov
            .provider_name()
            .to_lowercase()
            .contains(provider_name));

        // Test instantiation of every single model listed for this provider
        for model in &models {
            let instance = factory.create_with_model(provider_name, Some(model));
            assert!(
                instance.is_some(),
                "Failed to create {provider_name} with model {model}"
            );
            let inst = instance.unwrap();
            assert!(inst.provider_name().contains(model));
        }
    }
}

#[test]
fn test_reasoning_and_tool_rules_for_models() {
    use diad_agent::providers::openai_common::{is_reasoning_model, model_supports_tools};

    // Reasoning models should be identified
    assert!(is_reasoning_model("o1"));
    assert!(is_reasoning_model("o3"));
    assert!(is_reasoning_model("o3-mini"));
    assert!(is_reasoning_model("deepseek-reasoner"));
    assert!(is_reasoning_model("deepseek-r1"));

    // Tool calling filter check
    assert!(!model_supports_tools("deepseek-r1-distill-llama-70b"));
    assert!(!model_supports_tools("deepseek-reasoner"));
    assert!(!model_supports_tools("gemma2-9b-it"));
    assert!(model_supports_tools("gpt-5"));
    assert!(model_supports_tools("claude-opus-4.8"));
    assert!(model_supports_tools("claude-opus-4.7"));
    assert!(model_supports_tools("claude-opus-4.6"));
    assert!(model_supports_tools("gemini-3.7-flash"));
    assert!(model_supports_tools("qwen3-max"));
}

#[test]
fn test_groq_and_model_switching() {
    let vault = Arc::new(Vault::in_memory());
    vault.store_token("groq", "gsk-mock-key").unwrap();

    let mut cfg = DiadConfig::default();
    cfg.default_provider = "groq".into();
    cfg.default_model = Some("gpt-oss-120b".into());
    let config = Arc::new(cfg);

    let factory = ProviderFactory::new(vault, config);
    assert!(factory.has_key("groq"));

    let groq_models = factory.models_for("groq");
    assert!(groq_models.contains(&"gpt-oss-120b".to_string()));
    assert!(groq_models.contains(&"llama-3.3-70b-versatile".to_string()));

    let groq_instance = factory.create_with_model("groq", Some("gpt-oss-120b"));
    assert!(groq_instance.is_some());
    let inst = groq_instance.unwrap();
    assert!(inst.provider_name().contains("gpt-oss-120b"));
}

#[test]
fn test_openai_common_request_building_and_chunk_parsing() {
    use diad_agent::providers::openai_common::{build_chat_request, OpenAiSseState};
    use diad_agent::providers::traits::{Message, StreamEvent, ToolDefinition};

    let messages = vec![
        Message::system("You are an assistant"),
        Message::user("List files in directory"),
    ];

    let tools = vec![ToolDefinition {
        name: "file_list".into(),
        description: "List directory files".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"}
            },
            "required": ["path"]
        }),
    }];

    let req = build_chat_request("gpt-4o", &messages, &tools, 4096, true);
    assert_eq!(req.model, "gpt-4o");
    assert!(req.stream);
    assert_eq!(req.messages.len(), 2);
    assert!(req.tools.is_some());
    assert_eq!(req.tools.unwrap().len(), 1);

    // Test text chunk parsing via OpenAiSseState
    let mut state = OpenAiSseState::default();
    let text_sse = b"data: {\"choices\":[{\"delta\":{\"content\":\"Hello world\"},\"finish_reason\":null}]}\n\n";
    let events = state.feed(text_sse);
    assert_eq!(events.len(), 1);
    match &events[0] {
        Ok(StreamEvent::TextDelta(t)) => assert_eq!(t, "Hello world"),
        _ => panic!("Expected TextDelta event"),
    }

    // Test tool call chunk parsing
    let tool_sse1 = b"data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_123\",\"type\":\"function\",\"function\":{\"name\":\"file_list\",\"arguments\":\"\"}}]},\"finish_reason\":null}]}\n\n";
    let events1 = state.feed(tool_sse1);
    assert_eq!(events1.len(), 2);
    match &events1[0] {
        Ok(StreamEvent::ToolUseStart { id, name }) => {
            assert_eq!(id, "call_123");
            assert_eq!(name, "file_list");
        }
        _ => panic!("Expected ToolUseStart event"),
    }
    match &events1[1] {
        Ok(StreamEvent::ToolInputDelta(args)) => {
            assert_eq!(args, "");
        }
        _ => panic!("Expected ToolInputDelta event"),
    }

    let tool_sse2 = b"data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"path\\\":\\\"~\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n";
    let events2 = state.feed(tool_sse2);
    assert_eq!(events2.len(), 2);
    match &events2[0] {
        Ok(StreamEvent::ToolInputDelta(args)) => {
            assert_eq!(args, "{\"path\":\"~\"}");
        }
        _ => panic!("Expected ToolInputDelta event"),
    }
    match &events2[1] {
        Ok(StreamEvent::ToolUseEnd { id }) => {
            assert_eq!(id, "call_123");
        }
        _ => panic!("Expected ToolUseEnd event"),
    }

    let done_sse = b"data: [DONE]\n\n";
    let events_done = state.feed(done_sse);
    assert_eq!(events_done.len(), 1);
    match &events_done[0] {
        Ok(StreamEvent::StreamEnd { .. }) => {}
        _ => panic!("Expected StreamEnd event"),
    }
}
