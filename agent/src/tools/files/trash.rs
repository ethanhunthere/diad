//! Native freedesktop trash — trash, list, restore, empty.
//!
//! Implements the freedesktop.org Trash specification (spec v1.0) using
//! Rust `std` only — no external binaries, no shell. Files are moved into
//! `$XDG_DATA_HOME/Trash/files/` (falling back to `~/.local/share/Trash`)
//! and a matching `.trashinfo` sidecar records the original absolute path
//! and an RFC 3339 UTC deletion timestamp, so `trash_restore` can put a
//! file back exactly where it came from — including after a reboot.
//!
//! SECURITY CONTRACT (inherited from `files_universal.rs`):
//! - `trash_move` accepts only validated absolute paths (`valid_path`):
//!   no `..`, no `~`, no shell metacharacters.
//! - `trash_restore` validates the entry name (plain file name) before
//!   touching the filesystem and refuses to overwrite an existing file.
//! - `trash_empty` is DESTRUCTIVE and is registered in the orchestrator's
//!   `CONFIRM_TOOLS` list so the user explicitly approves every call.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::files_universal::valid_path;
use super::ToolResult;

/// `$XDG_DATA_HOME/Trash` (default `~/.local/share/Trash`).
fn trash_root() -> PathBuf {
    let xdg = std::env::var_os("XDG_DATA_HOME")
        .map(|v| v.to_string_lossy().to_string())
        .filter(|v| !v.is_empty());
    match xdg {
        Some(base) => PathBuf::from(base).join("Trash"),
        None => {
            let home = std::env::var_os("HOME")
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|| ".".to_string());
            PathBuf::from(home).join(".local/share/Trash")
        }
    }
}

fn files_dir() -> PathBuf {
    trash_root().join("files")
}

fn info_dir() -> PathBuf {
    trash_root().join("info")
}

/// A single entry in the trash.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TrashEntry {
    pub name: String,
    pub original_path: String,
    pub deleted_at: String,
    pub size: u64,
}

/// Create the trash directory layout if missing.
fn ensure_layout() -> std::io::Result<()> {
    std::fs::create_dir_all(files_dir())?;
    std::fs::create_dir_all(info_dir())
}

/// RFC 3339 UTC timestamp for `.trashinfo` `DeletionDate`.
fn now_rfc3339_utc() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // %Y-%m-%dT%H:%M:%SZ — no local timezone, deterministic.
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (y, m, d) = civil_from_days(days as i64);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Convert a days-since-1970-01-01 count to a (year, month, day) civil date
/// (Howard Hinnant's algorithm, public domain).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Pick a unique name inside `files/` on collision: `name`, `name.1`, …
fn unique_trash_name(name: &str) -> String {
    if !files_dir().join(name).exists() {
        return name.to_string();
    }
    let mut i = 1u32;
    loop {
        let candidate = format!("{name}.{i}");
        if !files_dir().join(&candidate).exists() {
            return candidate;
        }
        i += 1;
    }
}

/// Move a file (or empty directory) into the trash.
///
/// Mirrors `delete_download`'s argv discipline: only validated absolute
/// paths, `Command`-free, pure `std::fs`.
pub fn trash_move(path: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(path) {
        return Ok(ToolResult::err("trash: invalid path"));
    }
    let src = Path::new(path);
    if !src.exists() {
        return Ok(ToolResult::err(format!("trash: file not found: {path}")));
    }
    if src.is_dir() {
        // Scope: files only (a recursive directory trash needs an explicit
        // "recursive" flag to stay honest — not part of this parity win).
        return Ok(ToolResult::err(format!(
            "trash: '{path}' is a directory; trash files only"
        )));
    }
    let name = match src.file_name().and_then(|n| n.to_str()) {
        Some(n) if !n.is_empty() && !n.contains('/') => n,
        _ => return Ok(ToolResult::err("trash: invalid file name")),
    };
    if let Err(e) = ensure_layout() {
        return Ok(ToolResult::err(format!("trash: cannot create layout: {e}")));
    }
    let size = src.metadata().map(|m| m.len()).unwrap_or(0);
    let dest_name = unique_trash_name(name);
    let dest = files_dir().join(&dest_name);
    match std::fs::rename(src, &dest) {
        Ok(()) => {}
        Err(e) => {
            return Ok(ToolResult::err(format!("trash: move failed: {e}")));
        }
    }
    // Write the .trashinfo sidecar (escaped per the spec: backslash and
    // newlines in the path are percent-encoded).
    let original = path.replace('\\', "%5C").replace('\n', "%0A");
    let info = format!(
        "[Trash Info]\nPath={original}\nDeletionDate={}\n",
        now_rfc3339_utc()
    );
    if let Err(e) = std::fs::write(info_dir().join(format!("{dest_name}.trashinfo")), info) {
        let _ = std::fs::rename(&dest, src); // roll back on sidecar failure
        return Ok(ToolResult::err(format!("trash: sidecar write failed: {e}")));
    }
    Ok(ToolResult::ok(format!(
        "moved {name} to trash ({size} bytes)"
    )))
}

/// List the current trash contents (name, original path, deletion date).
pub fn trash_list() -> anyhow::Result<ToolResult> {
    let info = match std::fs::read_dir(info_dir()) {
        Ok(d) => d,
        Err(_) => return Ok(ToolResult::ok("trash is empty".to_string())),
    };
    let mut entries: Vec<TrashEntry> = Vec::new();
    for entry in info.flatten() {
        let info_path = entry.path();
        let name = match info_path.file_stem().and_then(|n| n.to_str()) {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => continue,
        };
        let body = match std::fs::read_to_string(&info_path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let (original, deleted_at) = parse_trashinfo(&body);
        let size = files_dir()
            .join(&name)
            .metadata()
            .map(|m| m.len())
            .unwrap_or(0);
        entries.push(TrashEntry {
            name,
            original_path: original.unwrap_or_default(),
            deleted_at: deleted_at.unwrap_or_default(),
            size,
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    if entries.is_empty() {
        return Ok(ToolResult::ok("trash is empty".to_string()));
    }
    let lines = entries
        .iter()
        .map(|e| {
            format!(
                "{} | {} | {} | {}",
                e.name,
                e.deleted_at,
                e.size,
                if e.original_path.is_empty() {
                    "(unknown)".to_string()
                } else {
                    e.original_path.clone()
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(ToolResult::ok(lines))
}

fn parse_trashinfo(body: &str) -> (Option<String>, Option<String>) {
    let mut original = None;
    let mut deleted = None;
    for line in body.lines() {
        if let Some(v) = line.strip_prefix("Path=") {
            original = Some(v.replace("%5C", "\\").replace("%0A", "\n"));
        } else if let Some(v) = line.strip_prefix("DeletionDate=") {
            deleted = Some(v.to_string());
        }
    }
    (original, deleted)
}

/// Restore a trashed entry to its original location.
pub fn trash_restore(name: &str) -> anyhow::Result<ToolResult> {
    if name.is_empty()
        || name.starts_with('/')
        || name.contains('/')
        || name.contains("..")
    {
        return Ok(ToolResult::err("trash: invalid entry name"));
    }
    let info = info_dir().join(format!("{name}.trashinfo"));
    let body = match std::fs::read_to_string(&info) {
        Ok(b) => b,
        Err(_) => return Ok(ToolResult::err(format!("trash: entry not found: {name}"))),
    };
    let (original, _) = parse_trashinfo(&body);
    let src = files_dir().join(name);
    if !src.exists() {
        return Ok(ToolResult::err(format!(
            "trash: data missing for entry '{name}'"
        )));
    }
    let dest = match original {
        Some(p) if p.starts_with('/') && !p.contains("..") => PathBuf::from(p),
        _ => {
            return Ok(ToolResult::err(
                "trash: entry has no valid original path (refusing to guess)",
            ))
        }
    };
    if dest.exists() {
        return Ok(ToolResult::err(format!(
            "trash: target already exists: {}",
            dest.display()
        )));
    }
    if let Some(parent) = dest.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return Ok(ToolResult::err(format!("trash: cannot recreate parent: {e}")));
        }
    }
    match std::fs::rename(&src, &dest) {
        Ok(()) => {
            let _ = std::fs::remove_file(&info);
            Ok(ToolResult::ok(format!("restored {name} → {}", dest.display())))
        }
        Err(e) => Ok(ToolResult::err(format!("trash: restore failed: {e}"))),
    }
}

/// Permanently delete every trashed entry (DESTRUCTIVE — confirmed).
pub fn trash_empty() -> anyhow::Result<ToolResult> {
    let mut removed = 0u32;
    let mut errors = 0u32;
    if let Ok(entries) = std::fs::read_dir(files_dir()) {
        for entry in entries.flatten() {
            match std::fs::remove_file(entry.path()) {
                Ok(()) => removed += 1,
                Err(_) => errors += 1,
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir(info_dir()) {
        for entry in entries.flatten() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    if removed == 0 && errors == 0 {
        return Ok(ToolResult::ok("trash is empty".to_string()));
    }
    if errors > 0 {
        return Ok(ToolResult::err(format!(
            "trash: emptied {removed} entries, {errors} failed"
        )));
    }
    Ok(ToolResult::ok(format!("trash emptied ({removed} entries)")))
}

/// Register the four trash tools in the default registry (called from
/// `create_default_registry`; `trash_empty` is confirm-gated upstream in
/// the orchestrator's `CONFIRM_TOOLS`).
pub fn register_trash_tools(registry: &mut crate::tools::ToolRegistry) {
    use std::sync::Arc;
    use serde_json::json;

    macro_rules! reg {
        ($reg:expr, $name:expr, $desc:expr, $schema:expr, $f:expr) => {
            $reg.register_tool(
                $name,
                $desc,
                $schema,
                Arc::new(|p: serde_json::Value| $f(p)),
            );
        };
    }

    reg!(
        registry,
        "trash_move",
        "Move a file to the freedesktop trash (recoverable)",
        json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}),
        |p: serde_json::Value| trash_move(p["path"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "trash_list",
        "List the trash: entry name, original path, deletion date, size",
        json!({}),
        |_| trash_list()
    );
    reg!(
        registry,
        "trash_restore",
        "Restore a trashed entry to its original location",
        json!({"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}),
        |p: serde_json::Value| trash_restore(p["name"].as_str().unwrap_or(""))
    );
    reg!(
        registry,
        "trash_empty",
        "Permanently empty the trash (DESTRUCTIVE, requires confirmation)",
        json!({}),
        |_| trash_empty()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_home(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("diad-trash-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("HOME", &dir);
        std::env::remove_var("XDG_DATA_HOME");
        dir
    }

    #[test]
    fn trash_roundtrip_and_restore() {
        let home = tmp_home("roundtrip");
        let file = home.join("notes.txt");
        std::fs::write(&file, "hello trash\n").unwrap();

        let r = trash_move(file.to_str().unwrap()).unwrap();
        assert!(r.success, "{}", r.error.unwrap_or_default());
        assert!(!file.exists(), "source must be moved away");
        assert!(files_dir().join("notes.txt").exists());

        // Sidecar format per the freedesktop spec.
        let info = std::fs::read_to_string(info_dir().join("notes.txt.trashinfo")).unwrap();
        assert!(info.starts_with("[Trash Info]\n"), "header: {info}");
        assert!(info.contains(&format!("Path={}", file.display())), "path: {info}");
        assert!(
            info.contains("DeletionDate="),
            "RFC3339 timestamp: {info}"
        );

        // trash_list reports the entry.
        let list = trash_list().unwrap();
        assert!(list.success);
        let out = list.output;
        assert!(out.contains("notes.txt"), "list output: {out}");

        // Restore puts it back at the original absolute path.
        let r = trash_restore("notes.txt").unwrap();
        assert!(r.success, "{}", r.error.unwrap_or_default());
        assert!(file.exists());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "hello trash\n");
        assert!(!info_dir().join("notes.txt.trashinfo").exists());
        assert!(!files_dir().join("notes.txt").exists());
        let _ = home;
    }

    #[test]
    fn trash_empty_removes_everything() {
        let home = tmp_home("empty");
        for i in 0..3 {
            let f = home.join(format!("f{i}.txt"));
            std::fs::write(&f, "x").unwrap();
            trash_move(f.to_str().unwrap()).unwrap();
        }
        let r = trash_empty().unwrap();
        assert!(r.success, "{}", r.error.unwrap_or_default());
        assert_eq!(std::fs::read_dir(files_dir()).map(|d| d.count()).unwrap_or(0), 0);
        assert_eq!(std::fs::read_dir(info_dir()).map(|d| d.count()).unwrap_or(0), 0);
        let _ = home;
    }

    #[test]
    fn trash_move_rejects_unsafe_paths() {
        let home = tmp_home("unsafe");
        let _ = home;
        for bad in ["/etc/passwd;rm -rf /", "~/notes.txt", "relative.txt", "/tmp/../etc/x"] {
            let r = trash_move(bad).unwrap();
            assert!(!r.success, "must reject '{bad}'");
        }
    }

    #[test]
    fn trash_restore_refuses_overwrite() {
        let home = tmp_home("overwrite");
        let file = home.join("keep.txt");
        std::fs::write(&file, "original").unwrap();
        trash_move(file.to_str().unwrap()).unwrap();
        // Recreate the original location with a different file.
        std::fs::write(&file, "replacement").unwrap();
        let r = trash_restore("keep.txt").unwrap();
        assert!(!r.success, "must refuse to overwrite");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "replacement");
        let _ = home;
    }
}
