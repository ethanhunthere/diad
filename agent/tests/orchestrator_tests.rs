//! Integration tests for the orchestrator: intent routing, sessions and
//! message model.

use diad_agent::orchestrator::router::{Intent, IntentRouter};
use diad_agent::orchestrator::session::Session;
use diad_agent::providers::traits::Message;

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

#[test]
fn test_message_constructors() {
    let user_msg = Message::user("Hello");
    let sys_msg = Message::system("You are a helpful assistant.");
    let asst_msg = Message::assistant("Hi!");

    assert_eq!(user_msg.role, "user");
    assert_eq!(sys_msg.role, "system");
    assert_eq!(asst_msg.role, "assistant");
}
