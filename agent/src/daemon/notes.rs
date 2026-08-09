//! Shared notes store — the collaboration scratchpad.
//!
//! Notes live in `~/.diad/notes/` as `.md` files and are shared across ALL
//! sessions and frontends. They are the cross-session collaboration surface:
//! any connected UI can read, write, or delete a note, and writes/deletes
//! broadcast a `System` message so every connected frontend sees the change
//! live (multi-window / multi-device presence is handled by the daemon's
//! broadcast channel).
//!
//! Names are strictly sanitized to a single filename component (no path
//! separators, no `..`, no leading dot, no control characters) so a note can
//! never escape the notes directory.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Metadata for one note (without its content).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteMeta {
    pub name: String,
    pub updated_at: DateTime<Utc>,
    pub size: u64,
}

/// A note with content (read operation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub name: String,
    pub content: String,
    pub updated_at: DateTime<Utc>,
}

/// Maximum note name length (keeps URLs and listings sane).
pub const MAX_NOTE_NAME: usize = 80;
/// Maximum note size we will read into memory (256 KB — notes are snippets).
pub const MAX_NOTE_SIZE: u64 = 256 * 1024;

/// Persistent notes store rooted at `~/.diad/notes/`.
#[derive(Debug, Clone)]
pub struct NotesStore {
    dir: PathBuf,
}

impl Default for NotesStore {
    fn default() -> Self {
        Self::new()
    }
}

impl NotesStore {
    /// Create the store at `~/.diad/notes/` (creates the directory).
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let dir = PathBuf::from(home).join(".diad").join("notes");
        let _ = fs::create_dir_all(&dir);
        Self { dir }
    }

    /// Create the store rooted at an explicit directory (testable).
    pub fn new_with_dir(dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&dir);
        Self { dir }
    }

    /// Root directory of the store.
    pub fn root(&self) -> &Path {
        &self.dir
    }

    /// Normalize a raw note name to a safe single filename component.
    ///
    /// Returns `None` when the name is unusable: empty, too long, contains
    /// path separators or `..`, starts with a dot (hidden files), or contains
    /// control characters. An `.md` extension is appended by the caller only
    /// for storage; the public name never includes it.
    pub fn sanitize_name(raw: &str) -> Option<String> {
        let name = raw.trim();
        if name.is_empty() || name.len() > MAX_NOTE_NAME {
            return None;
        }
        if name == "." || name == ".." {
            return None;
        }
        if name.starts_with('.') {
            return None;
        }
        if name.contains('/') || name.contains('\\') || name.contains('\0') {
            return None;
        }
        if name.chars().any(|c| c.is_control()) {
            return None;
        }
        Some(name.to_string())
    }

    fn path_for(&self, name: &str) -> Option<PathBuf> {
        let safe = Self::sanitize_name(name)?;
        Some(self.dir.join(format!("{safe}.md")))
    }

    /// List all notes, newest first.
    pub fn list(&self) -> Vec<NoteMeta> {
        let mut notes = Vec::new();
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return notes;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "md").unwrap_or(false) {
                let Ok(meta) = fs::metadata(&path) else { continue };
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
                    .to_string();
                if name.is_empty() {
                    continue;
                }
                notes.push(NoteMeta {
                    name,
                    updated_at: meta.modified().ok().map(DateTime::<Utc>::from).unwrap_or_else(Utc::now),
                    size: meta.len(),
                });
            }
        }
        notes.sort_by_key(|n| std::cmp::Reverse(n.updated_at));
        notes
    }

    /// Read a note's content. `None` if the name is invalid or the note does
    /// not exist or exceeds `MAX_NOTE_SIZE`.
    pub fn read(&self, name: &str) -> Option<Note> {
        let path = self.path_for(name)?;
        let meta = fs::metadata(&path).ok()?;
        if meta.len() > MAX_NOTE_SIZE {
            return None;
        }
        let content = fs::read_to_string(&path).ok()?;
        let name = Self::sanitize_name(name)?;
        Some(Note {
            name,
            content,
            updated_at: DateTime::<Utc>::from(meta.modified().ok()?),
        })
    }

    /// Write (create or overwrite) a note. Returns its metadata.
    pub fn write(&self, name: &str, content: &str) -> anyhow::Result<NoteMeta> {
        let safe = Self::sanitize_name(name)
            .ok_or_else(|| anyhow::anyhow!("invalid note name"))?;
        let path = self.dir.join(format!("{safe}.md"));
        fs::write(&path, content)?;
        let meta = fs::metadata(&path)?;
        Ok(NoteMeta {
            name: safe,
            updated_at: DateTime::<Utc>::from(meta.modified().map_err(anyhow::Error::from)?),
            size: meta.len(),
        })
    }

    /// Delete a note. Returns `true` if it existed and was removed.
    pub fn delete(&self, name: &str) -> bool {
        let Some(path) = self.path_for(name) else {
            return false;
        };
        match fs::remove_file(&path) {
            Ok(()) => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_name_accepts_plain() {
        assert_eq!(NotesStore::sanitize_name("roadmap").as_deref(), Some("roadmap"));
        assert_eq!(
            NotesStore::sanitize_name("  ideas.md  ").as_deref(),
            Some("ideas.md")
        );
        assert_eq!(NotesStore::sanitize_name("a-b_c.d").as_deref(), Some("a-b_c.d"));
    }

    #[test]
    fn test_sanitize_name_rejects_traversal() {
        assert!(NotesStore::sanitize_name("../evil").is_none());
        assert!(NotesStore::sanitize_name("a/b").is_none());
        assert!(NotesStore::sanitize_name("a\\b").is_none());
        assert!(NotesStore::sanitize_name("..").is_none());
        assert!(NotesStore::sanitize_name(".").is_none());
        assert!(NotesStore::sanitize_name("").is_none());
        assert!(NotesStore::sanitize_name("   ").is_none());
        assert!(NotesStore::sanitize_name(".hidden").is_none());
        assert!(NotesStore::sanitize_name("a\0b").is_none());
        assert!(NotesStore::sanitize_name("a\nb").is_none());
        assert!(NotesStore::sanitize_name(&"x".repeat(81)).is_none());
    }

    #[test]
    fn test_write_read_list_delete_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = NotesStore::new_with_dir(dir.path().to_path_buf());

        let meta = store.write("agenda", "# Standup\n- fix wifi\n").unwrap();
        assert_eq!(meta.name, "agenda");
        assert_eq!(meta.size, 21);

        let note = store.read("agenda").unwrap();
        assert_eq!(note.name, "agenda");
        assert!(note.content.contains("wifi"));

        let notes = store.list();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].name, "agenda");

        assert!(store.delete("agenda"));
        assert!(store.read("agenda").is_none());
        assert!(!store.delete("agenda"));
    }

    #[test]
    fn test_write_rejects_bad_name() {
        let dir = tempfile::tempdir().unwrap();
        let store = NotesStore::new_with_dir(dir.path().to_path_buf());
        assert!(store.write("../escape", "x").is_err());
        assert!(store.write("", "x").is_err());
        assert!(store.list().is_empty());
    }

    #[test]
    fn test_read_missing_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let store = NotesStore::new_with_dir(dir.path().to_path_buf());
        assert!(store.read("nope").is_none());
        assert!(store.read("../nope").is_none());
    }

    #[test]
    fn test_overwrite_updates_content() {
        let dir = tempfile::tempdir().unwrap();
        let store = NotesStore::new_with_dir(dir.path().to_path_buf());
        store.write("note", "v1").unwrap();
        store.write("note", "v2-longer").unwrap();
        let note = store.read("note").unwrap();
        assert_eq!(note.content, "v2-longer");
        assert_eq!(store.list().len(), 1);
    }
}
