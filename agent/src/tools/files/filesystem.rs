//! Filesystem tools — safe file operations sandboxed to allowed paths.
//!
//! Every operation is gated by `sandboxed_path()`:
//! 1. Raw `..` components are rejected outright.
//! 2. The path is canonicalized (symlinks resolved) before any operation, so
//!    a symlink pointing outside the sandbox is caught.
//! 3. The canonical path must start with a configured `allowed_paths` entry;
//!    otherwise the operation fails with `path outside sandbox`.
//! 4. Because the allowlist gates every write, paths under `/etc`, `/usr` and
//!    `/boot` are only writable when explicitly listed in `allowed_paths`
//!    (the default allowlist is `~`, `/tmp`, `/etc/diad`).

use std::fs;
use std::path::{Component, Path, PathBuf};

use super::ToolResult;

/// Error message for paths rejected by the sandbox.
const OUTSIDE_SANDBOX: &str = "path outside sandbox";

/// Canonicalize an input path for sandbox checking.
///
/// For paths that exist, `fs::canonicalize` resolves symlinks, so a symlink
/// pointing outside the sandbox resolves to its real target and is caught by
/// the allowlist check. For paths that do not exist yet (e.g. a file about to
/// be written), the nearest existing ancestor is canonicalized and the
/// remaining components are appended.
fn canonicalize_for_sandbox(input: &str) -> PathBuf {
    let expanded = shellexpand::tilde(input).to_string();
    let path = Path::new(&expanded);

    if let Ok(canon) = fs::canonicalize(path) {
        return canon;
    }

    // New file/dir: walk up to the nearest existing ancestor.
    let mut pending: Vec<std::ffi::OsString> = Vec::new();
    let mut current = path;
    loop {
        if let Ok(canon) = fs::canonicalize(current) {
            let mut result = canon;
            for comp in pending.iter().rev() {
                result.push(comp);
            }
            return result;
        }
        match current.parent() {
            Some(parent) => {
                if let Some(name) = current.file_name() {
                    pending.push(name.to_os_string());
                }
                current = parent;
            }
            None => return path.to_path_buf(),
        }
    }
}

/// Reject raw `..` components outright (no parent-directory escapes, even if
/// the resolved path would land inside an allowed directory).
fn has_raw_dotdot(input: &str) -> bool {
    Path::new(&shellexpand::tilde(input).to_string())
        .components()
        .any(|c| matches!(c, Component::ParentDir))
}

/// Check that `candidate` is inside one of the allowed paths (component-aware
/// prefix match on canonical paths). An empty allowlist means unrestricted.
fn is_within_allowed(candidate: &Path, allowed: &[String]) -> bool {
    if allowed.is_empty() {
        return true; // No restrictions configured.
    }
    allowed.iter().any(|a| {
        let allowed_canon = canonicalize_for_sandbox(a);
        candidate.starts_with(&allowed_canon)
    })
}

/// Resolve and sandbox-check an input path.
///
/// Returns the canonical path if it is inside the allowed paths; otherwise an
/// error whose message is exactly `path outside sandbox`.
fn sandboxed_path(input: &str, allowed: &[String]) -> anyhow::Result<PathBuf> {
    if input.is_empty() {
        return Err(anyhow::anyhow!("path is required"));
    }
    if has_raw_dotdot(input) {
        return Err(anyhow::anyhow!(OUTSIDE_SANDBOX));
    }
    let canonical = canonicalize_for_sandbox(input);
    if !is_within_allowed(&canonical, allowed) {
        return Err(anyhow::anyhow!(OUTSIDE_SANDBOX));
    }
    Ok(canonical)
}

/// Read the contents of a file.
pub fn read_file(path: &str, allowed: &[String]) -> anyhow::Result<ToolResult> {
    let file_path = match sandboxed_path(path, allowed) {
        Ok(p) => p,
        Err(e) => return Ok(ToolResult::err(e.to_string())),
    };

    if !file_path.exists() {
        return Ok(ToolResult::err(format!("File not found: {path}")));
    }

    if !file_path.is_file() {
        return Ok(ToolResult::err(format!("Not a file: {path}")));
    }

    // Refuse huge files BEFORE loading them (spec 3.15): the output was
    // already truncated to 100 lines, but the read itself used to pull the
    // whole file into memory first — a 1 GB log would OOM the daemon.
    const MAX_READ_BYTES: u64 = 10 * 1024 * 1024;
    if let Ok(meta) = fs::metadata(&file_path) {
        if meta.len() > MAX_READ_BYTES {
            return Ok(ToolResult::err(format!(
                "File too large to read: {} bytes (limit {}). Use grep/search instead.",
                meta.len(),
                MAX_READ_BYTES
            )));
        }
    }

    match fs::read_to_string(&file_path) {
        Ok(contents) => {
            let preview = if contents.lines().count() > 100 {
                let truncated: String = contents.lines().take(100).collect::<Vec<_>>().join("\n");
                format!(
                    "{truncated}\n\n... (truncated, {} total lines)",
                    contents.lines().count()
                )
            } else {
                contents
            };
            Ok(ToolResult::ok(preview))
        }
        Err(e) => Ok(ToolResult::err(format!("Failed to read {path}: {e}"))),
    }
}

/// Write content to a file, creating parent directories if needed.
pub fn write_file(path: &str, content: &str, allowed: &[String]) -> anyhow::Result<ToolResult> {
    let file_path = match sandboxed_path(path, allowed) {
        Ok(p) => p,
        Err(e) => return Ok(ToolResult::err(e.to_string())),
    };

    // Create parent directories
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| anyhow::anyhow!("Failed to create parent directories: {e}"))?;
    }

    match fs::write(&file_path, content) {
        Ok(()) => Ok(ToolResult::ok(format!(
            "Written {} bytes to {path}",
            content.len()
        ))),
        Err(e) => Ok(ToolResult::err(format!("Failed to write {path}: {e}"))),
    }
}

/// List the contents of a directory.
pub fn list_dir(path: &str, allowed: &[String]) -> anyhow::Result<ToolResult> {
    let dir_path = match sandboxed_path(path, allowed) {
        Ok(p) => p,
        Err(e) => return Ok(ToolResult::err(e.to_string())),
    };

    if !dir_path.exists() {
        return Ok(ToolResult::err(format!("Directory not found: {path}")));
    }

    if !dir_path.is_dir() {
        return Ok(ToolResult::err(format!("Not a directory: {path}")));
    }

    let mut entries = Vec::new();
    match fs::read_dir(&dir_path) {
        Ok(read_dir) => {
            for entry in read_dir {
                match entry {
                    Ok(entry) => {
                        let file_type = entry.file_type().ok();
                        let is_dir = file_type.as_ref().map(|ft| ft.is_dir()).unwrap_or(false);
                        let is_file = file_type.as_ref().map(|ft| ft.is_file()).unwrap_or(false);
                        let prefix = if is_dir {
                            "📁"
                        } else if is_executable(&entry.path()) {
                            "⚙️"
                        } else {
                            "📄"
                        };
                        let name = entry.file_name().to_string_lossy().to_string();

                        let size = if is_file {
                            match entry.metadata() {
                                Ok(m) => format!(" ({})", human_size(m.len())),
                                Err(_) => String::new(),
                            }
                        } else {
                            String::new()
                        };

                        entries.push(format!("{prefix} {name}{size}"));
                    }
                    Err(e) => {
                        entries.push(format!("??? <error: {e}>"));
                    }
                }
            }
        }
        Err(e) => return Ok(ToolResult::err(format!("Failed to read directory: {e}"))),
    }

    entries.sort();
    Ok(ToolResult::ok(entries.join("\n")))
}

/// Search for files matching a glob pattern.
pub fn search_files(pattern: &str, path: &str, allowed: &[String]) -> anyhow::Result<ToolResult> {
    let search_path = match sandboxed_path(path, allowed) {
        Ok(p) => p,
        Err(e) => return Ok(ToolResult::err(e.to_string())),
    };

    if !search_path.exists() {
        return Ok(ToolResult::err(format!("Path not found: {path}")));
    }

    let mut matches = Vec::new();
    let glob_pattern = glob::Pattern::new(pattern)?;

    fn walk(dir: &Path, pattern: &glob::Pattern, matches: &mut Vec<String>, depth: u32) {
        if depth > 5 {
            return; // Limit recursion depth
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if pattern.matches(&name) {
                    matches.push(entry.path().display().to_string());
                }
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    walk(&entry.path(), pattern, matches, depth + 1);
                }
            }
        }
    }

    walk(&search_path, &glob_pattern, &mut matches, 0);

    if matches.is_empty() {
        Ok(ToolResult::ok(format!(
            "No files matching '{pattern}' found in {path}"
        )))
    } else {
        Ok(ToolResult::ok(matches.join("\n")))
    }
}

/// Change file permissions.
pub fn chmod_file(path: &str, mode: &str, allowed: &[String]) -> anyhow::Result<ToolResult> {
    use std::os::unix::fs::PermissionsExt;

    let file_path = match sandboxed_path(path, allowed) {
        Ok(p) => p,
        Err(e) => return Ok(ToolResult::err(e.to_string())),
    };

    if !file_path.exists() {
        return Ok(ToolResult::err(format!("File not found: {path}")));
    }

    let mode_val = u32::from_str_radix(mode, 8)
        .map_err(|_| anyhow::anyhow!("Invalid mode: {mode}. Use octal (e.g., 755)"))?;

    let mut perms = fs::metadata(&file_path)?.permissions();
    perms.set_mode(mode_val);
    fs::set_permissions(&file_path, perms)?;

    Ok(ToolResult::ok(format!(
        "Changed permissions of {path} to {mode}"
    )))
}

/// Check if a file is executable.
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Format a byte size as a human-readable string.
fn human_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    format!("{:.1} {}", size, UNITS[unit_idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Default-style allowlist for sandbox tests: a temp dir + /tmp.
    fn allowed_with(base: &Path) -> Vec<String> {
        vec![base.display().to_string(), "/tmp".to_string()]
    }

    #[test]
    fn test_read_write_within_sandbox() {
        let dir = tempfile::tempdir().unwrap();
        let allowed = allowed_with(dir.path());
        let target = dir.path().join("sub/file.txt");

        // Write into a NEW subdirectory of the sandbox (parent doesn't exist yet).
        let result = write_file(target.to_str().unwrap(), "hello", &allowed).unwrap();
        assert!(result.success, "{:?}", result.error);

        let result = read_file(target.to_str().unwrap(), &allowed).unwrap();
        assert!(result.success);
        assert_eq!(result.output, "hello");
    }

    #[test]
    fn test_dotdot_escape_denied() {
        let dir = tempfile::tempdir().unwrap();
        let allowed = allowed_with(dir.path());

        // `../` escape: even if the resolved path would be inside /tmp, raw
        // `..` components are rejected outright.
        let result = read_file("../etc/passwd", &allowed).unwrap();
        assert!(!result.success);
        assert_eq!(result.error.unwrap(), OUTSIDE_SANDBOX);

        // Absolute path with an embedded `..` — denied.
        let evil = format!("{}/../../etc/passwd", dir.path().display());
        let result = read_file(&evil, &allowed).unwrap();
        assert!(!result.success);
        assert_eq!(result.error.unwrap(), OUTSIDE_SANDBOX);
    }

    #[test]
    fn test_absolute_path_outside_allowlist_denied() {
        let dir = tempfile::tempdir().unwrap();
        let allowed = allowed_with(dir.path());

        // /etc/passwd is outside the allowlist (default allowlist is
        // ~, /tmp, /etc/diad — never /etc wholesale).
        let result = read_file("/etc/passwd", &allowed).unwrap();
        assert!(!result.success);
        assert_eq!(result.error.unwrap(), OUTSIDE_SANDBOX);

        // Writes under /usr are denied unless explicitly allowed.
        let result = write_file("/usr/share/evil.txt", "x", &allowed).unwrap();
        assert!(!result.success);
        assert_eq!(result.error.unwrap(), OUTSIDE_SANDBOX);
    }

    #[test]
    fn test_symlink_outside_denied() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let allowed = allowed_with(dir.path());

        // Symlink INSIDE the sandbox pointing OUTSIDE it (e.g. at /etc/passwd).
        let link = dir.path().join("escape");
        symlink("/etc/passwd", &link).unwrap();

        let result = read_file(link.to_str().unwrap(), &allowed).unwrap();
        assert!(!result.success);
        assert_eq!(result.error.unwrap(), OUTSIDE_SANDBOX);

        // chmod through the symlink must also be denied.
        let result = chmod_file(link.to_str().unwrap(), "777", &allowed).unwrap();
        assert!(!result.success);
        assert_eq!(result.error.unwrap(), OUTSIDE_SANDBOX);
    }

    #[test]
    fn test_list_and_search_are_sandboxed() {
        let dir = tempfile::tempdir().unwrap();
        let allowed = allowed_with(dir.path());
        let inner = dir.path().join("probe");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(inner.join("data.txt"), "x").unwrap();

        // In-sandbox listing works.
        let result = list_dir(dir.path().to_str().unwrap(), &allowed).unwrap();
        assert!(result.success);
        assert!(result.output.contains("probe"));

        // Out-of-sandbox listing denied.
        let result = list_dir("/etc", &allowed).unwrap();
        assert!(!result.success);
        assert_eq!(result.error.unwrap(), OUTSIDE_SANDBOX);

        // In-sandbox search works.
        let result = search_files("*.txt", dir.path().to_str().unwrap(), &allowed).unwrap();
        assert!(result.success);
        assert!(result.output.contains("data.txt"));

        // Out-of-sandbox search denied.
        let result = search_files("*.conf", "/etc", &allowed).unwrap();
        assert!(!result.success);
        assert_eq!(result.error.unwrap(), OUTSIDE_SANDBOX);
    }

    #[test]
    fn test_allowed_path_must_match_component_boundary() {
        // Allowed: /tmp/<sandbox> only. A sibling like /tmp/<sandbox>-evil is
        // a DIFFERENT directory and must not match (component-boundary).
        let sandbox = tempfile::tempdir().unwrap();
        let allowed = vec![sandbox.path().display().to_string()];

        let sibling = tempfile::Builder::new()
            .prefix("component-boundary-sibling")
            .tempdir_in(std::env::temp_dir())
            .unwrap();
        let result = read_file(sibling.path().join("x").to_str().unwrap(), &allowed).unwrap();
        assert!(!result.success);
        assert_eq!(result.error.unwrap(), OUTSIDE_SANDBOX);

        // A prefix-match-only implementation would let /tmp/component-boundary-*
        // through; component-aware starts_with must not.
        let near = format!("{}x", sandbox.path().display());
        let result = read_file(&near, &allowed).unwrap();
        assert!(!result.success);
        assert_eq!(result.error.unwrap(), OUTSIDE_SANDBOX);
    }

    #[test]
    fn test_empty_allowlist_allows_everything() {
        // An empty allowlist means "no restrictions" (registry default guard).
        let result = read_file("/etc/hostname", &[]).unwrap();
        assert!(result.success);
    }
}
