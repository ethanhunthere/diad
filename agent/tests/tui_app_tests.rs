//! Integration tests for the TUI application shell.
//!
//! Verifies initialization, input handling, mode switching and the
//! provider/model picker flows of the ratatui app.

use diad_agent::config::settings::DiadConfig;
use diad_agent::tui::app::{App, AppMode};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

// ---------------------------------------------------------------------------
// Initialization & input
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Provider & model pickers
// ---------------------------------------------------------------------------

#[test]
fn test_app_f6_opens_provider_picker() {
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
