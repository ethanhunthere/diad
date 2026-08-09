//! Integration tests for the DIAD Agent.
//!
//! These tests verify that core components initialize correctly and
//! that cross-module contracts are upheld.

use diad_agent::config::settings::DiadConfig;
use diad_agent::tools;
use diad_agent::tui::app::{App, AppMode};
use diad_agent::orchestrator::router::{IntentRouter, Intent};
use diad_agent::orchestrator::session::Session;
use diad_agent::providers::traits::Message;

// =============================================================================
// TUI App Tests
// =============================================================================

#[test]
fn test_app_initializes_without_panic() {
    let config = DiadConfig::default();
    let app = App::new("anthropic".into(), config);

    assert_eq!(app.provider, "anthropic");
    assert_eq!(app.mode, AppMode::Chat);
    assert!(app.input_buffer.is_empty());
    assert_eq!(app.cursor_position, 0);
    assert!(!app.streaming);
    assert!(!app.connected);
    // Should have welcome messages
    assert!(!app.messages.is_empty());
}

#[test]
fn test_app_submit_input() {
    let config = DiadConfig::default();
    let mut app = App::new("openai".into(), config);

    app.input_buffer = "Hello, world!".into();
    app.cursor_position = 13;
    app.submit_input();

    assert!(app.input_buffer.is_empty());
    assert_eq!(app.cursor_position, 0);
    assert!(app.streaming);
    // Should have user message and processing message
    assert!(app.messages.iter().any(|m| m.contains("Hello, world!")));
}

#[test]
fn test_app_empty_submit() {
    let config = DiadConfig::default();
    let mut app = App::new("gemini".into(), config);

    let before_count = app.messages.len();
    app.submit_input(); // Empty buffer
    assert_eq!(app.messages.len(), before_count);
    assert!(!app.streaming);
}

#[test]
fn test_app_cursor_movement() {
    let config = DiadConfig::default();
    let mut app = App::new("qwen".into(), config);

    app.input_buffer = "test".into();
    app.cursor_position = 4;

    app.retreat_cursor();
    assert_eq!(app.cursor_position, 3);

    app.retreat_cursor();
    assert_eq!(app.cursor_position, 2);

    app.advance_cursor();
    assert_eq!(app.cursor_position, 3);

    // Can't go past end
    app.advance_cursor();
    app.advance_cursor();
    assert_eq!(app.cursor_position, 4);
}

#[test]
fn test_app_mode_switching() {
    let config = DiadConfig::default();
    let mut app = App::new("anthropic".into(), config);

    assert_eq!(app.mode, AppMode::Chat);

    // Simulate F2 press
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let f2 = KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE);
    app.handle_key_event(f2);
    assert_eq!(app.mode, AppMode::FileTree);

    let f3 = KeyEvent::new(KeyCode::F(3), KeyModifiers::NONE);
    app.handle_key_event(f3);
    assert_eq!(app.mode, AppMode::CodeView);

    let f4 = KeyEvent::new(KeyCode::F(4), KeyModifiers::NONE);
    app.handle_key_event(f4);
    assert_eq!(app.mode, AppMode::SystemStatus);

    let f1 = KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE);
    app.handle_key_event(f1);
    assert_eq!(app.mode, AppMode::Chat);
}

// =============================================================================
// Provider & Model Picker Tests
// =============================================================================

#[test]
fn test_app_f6_opens_provider_picker() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = DiadConfig::default();
    let mut app = App::new("openai".into(), config);
    app.provider_entries = vec![
        diad_agent::providers::ProviderEntry {
            name: "openai".into(),
            needs_key: true,
            has_key: true,
        },
        diad_agent::providers::ProviderEntry {
            name: "ollama".into(),
            needs_key: false,
            has_key: true,
        },
    ];

    app.handle_key_event(KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE));
    assert_eq!(app.mode, AppMode::ProviderPicker);
    assert!(app.picker_requested);
}

#[test]
fn test_app_model_picker_switching() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = DiadConfig::default();
    let mut app = App::new("openai".into(), config);
    app.model = "gpt-5".into();
    app.provider_entries = vec![
        diad_agent::providers::ProviderEntry {
            name: "openai".into(),
            needs_key: true,
            has_key: true,
        },
        diad_agent::providers::ProviderEntry {
            name: "ollama".into(),
            needs_key: false,
            has_key: true,
        },
    ];

    // Open the picker (F6), move down to ollama, select it.
    app.handle_key_event(KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE));
    assert_eq!(app.mode, AppMode::ProviderPicker);
    app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.picker_index, 1);
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.provider_selected.as_deref(), Some("ollama"));

    // The event loop switches the provider and opens the model picker.
    app.provider = "ollama".into();
    app.mode = AppMode::ModelPicker;
    app.model_list = vec!["qwen2.5:7b".into()];
    app.model_index = 0;

    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.model, "qwen2.5:7b");
    assert_eq!(app.mode, AppMode::Chat);
}

#[test]
fn test_app_model_picker_navigation() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let config = DiadConfig::default();
    let mut app = App::new("openai".into(), config);
    app.mode = AppMode::ModelPicker;
    app.model_list = vec![
        "gpt-5".into(),
        "gpt-5-codex".into(),
        "codex-mini-latest".into(),
        "o4-mini".into(),
    ];
    app.model_index = 0;

    // Down to index 2
    app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.model_index, 2);

    // 'k' moves up
    app.handle_key_event(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
    assert_eq!(app.model_index, 1);

    // Enter applies the selected model
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.model, "gpt-5-codex");
    assert_eq!(app.mode, AppMode::Chat);
}

#[test]
fn test_app_model_slash_command_opens_picker() {
    let config = DiadConfig::default();
    let mut app = App::new("anthropic".into(), config);
    app.input_buffer = "/model".into();
    app.cursor_position = 6;
    app.submit_input();

    assert_eq!(app.mode, AppMode::ProviderPicker);
    assert!(app.picker_requested);
    assert!(app.input_buffer.is_empty());
}

#[test]
fn test_app_connect_named_provider() {
    let config = DiadConfig::default();
    let mut app = App::new("anthropic".into(), config);
    app.input_buffer = "/connect kimi".into();
    app.cursor_position = 13;
    app.submit_input();

    assert_eq!(app.mode, AppMode::Connect);
    assert_eq!(app.connect_target, "kimi");

    // Bare /connect targets the current provider.
    let mut app2 = App::new("qwen".into(), DiadConfig::default());
    app2.input_buffer = "/connect".into();
    app2.cursor_position = 8;
    app2.submit_input();
    assert_eq!(app2.mode, AppMode::Connect);
    assert_eq!(app2.connect_target, "qwen");
}

// =============================================================================
// Config Tests
// =============================================================================

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

// =============================================================================
// Tool Registry Tests
// =============================================================================

#[test]
fn test_tool_registry_registers_all_tools() {
    let registry = tools::create_default_registry();
    let names = registry.list_tool_names();

    // Check all expected tools are present
    assert!(names.contains(&"read_file".to_string()));
    assert!(names.contains(&"write_file".to_string()));
    assert!(names.contains(&"list_dir".to_string()));
    assert!(names.contains(&"search_files".to_string()));
    assert!(names.contains(&"chmod_file".to_string()));
    assert!(names.contains(&"spawn_process".to_string()));
    assert!(names.contains(&"list_processes".to_string()));
    assert!(names.contains(&"kill_process".to_string()));
    assert!(names.contains(&"http_get".to_string()));
    assert!(names.contains(&"http_post".to_string()));
    assert!(names.contains(&"ping_host".to_string()));
    assert!(names.contains(&"get_system_info".to_string()));
    assert!(names.contains(&"install_package".to_string()));
    assert!(names.contains(&"remove_package".to_string()));
    assert!(names.contains(&"manage_service".to_string()));

    // All tools should have definitions
    let definitions = registry.list_tools();
    assert_eq!(definitions.len(), names.len());
}

#[test]
fn test_tool_execute_unknown_tool() {
    let registry = tools::create_default_registry();
    let result = registry.execute_tool("nonexistent_tool", serde_json::json!({}));

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Unknown tool"));
}

#[test]
fn test_tool_list_dir_cwd() {
    let registry = tools::create_default_registry();
    let result = registry.execute_tool("list_dir", serde_json::json!({"path": "."}));

    assert!(result.is_ok());
    let tool_result = result.unwrap();
    assert!(tool_result.success, "list_dir should succeed on current dir");
}

#[test]
fn test_tool_get_system_info() {
    let registry = tools::create_default_registry();
    let result = registry.execute_tool("get_system_info", serde_json::json!({}));

    assert!(result.is_ok());
    let tool_result = result.unwrap();
    assert!(tool_result.success);
    assert!(!tool_result.output.is_empty());
}

// =============================================================================
// Provider Factory Tests
// =============================================================================

#[test]
fn test_provider_factory_unknown_provider() {
    use diad_agent::providers::ProviderFactory;
    use diad_agent::auth::vault::Vault;
    use std::sync::Arc;

    let vault = Arc::new(Vault::new().expect("Vault should initialize"));
    let config = Arc::new(DiadConfig::default());
    let factory = ProviderFactory::new(vault, config);

    let provider = factory.create("nonexistent");
    assert!(provider.is_none());
}

// =============================================================================
// Intent Router Tests
// =============================================================================

#[test]
fn test_intent_router_classification() {
    assert_eq!(IntentRouter::classify_input("Hello"), Intent::Chat);
    assert_eq!(IntentRouter::classify_input("list files"), Intent::ToolCall);
    assert_eq!(
        IntentRouter::classify_input("create file test.rs"),
        Intent::EditCommand
    );
    assert_eq!(
        IntentRouter::classify_input("system info"),
        Intent::SystemQuery
    );
}

// =============================================================================
// Session Tests
// =============================================================================

#[test]
fn test_session_creation() {
    let session = Session::new("Integration Test", "anthropic");

    assert_eq!(session.title, "Integration Test");
    assert_eq!(session.active_provider, "anthropic");
    assert!(session.messages.is_empty());
    assert!(!session.id.is_empty());
    assert!(session.created_at <= chrono::Utc::now());
}

#[test]
fn test_session_add_messages() {
    let mut session = Session::new("Test", "openai");

    session.add_message("user", "Hello, Claude!");
    session.add_message("assistant", "Hello! How can I help you today?");

    assert_eq!(session.messages.len(), 2);
    assert_eq!(session.messages[0].role, "user");
    assert_eq!(session.messages[1].role, "assistant");
    assert_eq!(session.messages[0].content, "Hello, Claude!");
}

// =============================================================================
// Message Tests
// =============================================================================

#[test]
fn test_message_constructors() {
    let user_msg = Message::user("Hello");
    let sys_msg = Message::system("You are a helpful assistant.");
    let asst_msg = Message::assistant("Hi!");

    assert_eq!(user_msg.role, "user");
    assert_eq!(sys_msg.role, "system");
    assert_eq!(asst_msg.role, "assistant");
}
