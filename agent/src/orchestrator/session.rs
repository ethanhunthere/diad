//! Session management — persistence and lifecycle of conversation sessions.

use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single conversation session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier
    pub id: String,
    /// Human-readable title (derived from first message)
    pub title: String,
    /// All messages in the conversation
    pub messages: Vec<SessionMessage>,
    /// The LLM provider used for this session
    pub active_provider: String,
    /// The model used for this session's provider (e.g. "claude-sonnet-4-20250514")
    #[serde(default)]
    pub active_model: String,
    /// When the session was created
    pub created_at: DateTime<Utc>,
    /// When the session was last modified
    pub updated_at: DateTime<Utc>,
}

/// A single message within a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    /// Role: "user", "assistant", "system", or "tool"
    pub role: String,
    /// Message content
    pub content: String,
    /// Timestamp of the message
    pub timestamp: DateTime<Utc>,
}

impl Session {
    /// Create a new session.
    pub fn new(title: impl Into<String>, provider: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            title: title.into(),
            messages: Vec::new(),
            active_provider: provider.into(),
            active_model: String::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a message to the session.
    pub fn add_message(&mut self, role: impl Into<String>, content: impl Into<String>) {
        self.messages.push(SessionMessage {
            role: role.into(),
            content: content.into(),
            timestamp: Utc::now(),
        });
        self.updated_at = Utc::now();
    }
}

/// Manages loading, saving, and listing sessions from disk.
pub struct SessionManager {
    /// Directory where sessions are stored
    sessions_dir: PathBuf,
}

impl SessionManager {
    /// Create a new SessionManager.
    ///
    /// The sessions directory is `~/.diad/sessions/`.
    pub fn new() -> anyhow::Result<Self> {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let sessions_dir = PathBuf::from(home).join(".diad").join("sessions");

        fs::create_dir_all(&sessions_dir)?;

        Ok(Self { sessions_dir })
    }

    /// Create a SessionManager rooted at an explicit directory.
    pub fn new_with_dir(sessions_dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&sessions_dir);
        Self { sessions_dir }
    }

    /// Load a session by its ID.
    pub fn load_session(&self, session_id: &str) -> anyhow::Result<Session> {
        let path = self.sessions_dir.join(format!("{session_id}.json"));
        let content = fs::read_to_string(&path)?;
        let session: Session = serde_json::from_str(&content)?;
        Ok(session)
    }

    /// Save a session to disk.
    pub fn save_session(&self, session: &Session) -> anyhow::Result<()> {
        let path = self.sessions_dir.join(format!("{}.json", session.id));
        let content = serde_json::to_string_pretty(session)?;
        fs::write(&path, content)?;
        tracing::info!("Session {} saved to {}", session.id, path.display());
        Ok(())
    }

    /// List all saved sessions.
    pub fn list_sessions(&self) -> anyhow::Result<Vec<Session>> {
        let mut sessions = Vec::new();

        for entry in fs::read_dir(&self.sessions_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "json").unwrap_or(false) {
                match fs::read_to_string(&path) {
                    Ok(content) => match serde_json::from_str::<Session>(&content) {
                        Ok(session) => sessions.push(session),
                        Err(e) => {
                            tracing::warn!("Failed to parse session file {}: {e}", path.display());
                        }
                    },
                    Err(e) => {
                        tracing::warn!("Failed to read session file {}: {e}", path.display());
                    }
                }
            }
        }

        // Sort by most recently updated first
        sessions.sort_by_key(|s| std::cmp::Reverse(s.updated_at));

        Ok(sessions)
    }

    /// Delete a session by its ID.
    pub fn delete_session(&self, session_id: &str) -> anyhow::Result<()> {
        let path = self.sessions_dir.join(format!("{session_id}.json"));
        if path.exists() {
            fs::remove_file(&path)?;
            tracing::info!("Session {session_id} deleted.");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let session = Session::new("Test Session", "anthropic");
        assert_eq!(session.title, "Test Session");
        assert_eq!(session.active_provider, "anthropic");
        assert!(session.messages.is_empty());
        assert!(!session.id.is_empty());
    }

    #[test]
    fn test_add_message() {
        let mut session = Session::new("Test", "openai");
        session.add_message("user", "Hello");
        session.add_message("assistant", "Hi there!");

        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[0].role, "user");
        assert_eq!(session.messages[1].role, "assistant");
    }
}
