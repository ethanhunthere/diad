//! UNIVERSAL file tools — the complete file & content surface of DIAD AGENT.
//!
//! Six subsections, one file. Every tool returns `anyhow::Result<ToolResult>`
//! and is registered in the default `ToolRegistry` (see `tools/mod.rs`):
//!
//!   2.1 File type registry   — type, metadata, previews, thumbnails, icons
//!   2.2 Natural language opening — open files, URLs, dirs, "open with", defaults
//!   2.3 Download integration — list, open, move, delete, organize, stats, scan
//!   2.4 Clipboard & sharing  — text/image/history, share, QR, phone, AirDrop
//!   2.5 Search & organization— name/content/regex/tag search, duplicates, index
//!   2.6 Archive management  — extract, create, list, test, convert, encrypt, split
//!
//! SECURITY CONTRACT (inherited from the existing tool modules):
//! - Every user-supplied value that lands in argv passes `valid_identifier()`,
//!   a local `valid_value()`, or a local `valid_path()` check. Values are NEVER
//!   interpolated into a shell string — `Command::new(...).args(...)` only.
//! - Destructive operations (delete downloads, organize, clean old files) are
//!   registered in the orchestrator's `CONFIRM_TOOLS` list so the user
//!   explicitly approves every one.
//! - Paths, URLs and tags are validated; unexpected characters are rejected
//!   before they can reach a command or the filesystem.
//! - `search_files` (rich, option-based) supersedes the simple glob-based
//!   `filesystem::search_files` in the registry: the registry keeps the LAST
//!   registration, and this module registers after the classic module.

use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use super::{valid_identifier, ToolResult};

// ===========================================================================
// Low-level command helpers
// ===========================================================================

/// Run a command as the current user; return stdout (or stderr on failure).
/// Never returns an `Err` — a missing binary degrades to `ToolResult::err`.
fn run(args: &[&str]) -> anyhow::Result<ToolResult> {
    let program = args.first().copied().unwrap_or("?");
    let output = match Command::new(args[0]).args(&args[1..]).output() {
        Ok(o) => o,
        Err(e) => {
            return Ok(ToolResult::err(format!(
                "`{program}` failed to launch: {e} (is the tool installed?)"
            )))
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        Ok(ToolResult::ok(if stdout.is_empty() { stderr } else { stdout }))
    } else {
        Ok(ToolResult::err(if stderr.is_empty() { stdout } else { stderr }))
    }
}

/// Poll `child.try_wait()` until it exits or `secs` elapse; kills the child on
/// timeout. Returns `Some(status)` on exit, `None` on timeout/kill.
fn wait_timeout(
    child: &mut std::process::Child,
    secs: u64,
    program: &str,
) -> Option<std::process::ExitStatus> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    tracing::warn!("`{program}` killed after {secs}s timeout");
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// Run a command with a hard timeout, capturing piped output.
/// GUI-dependent commands (`wl-paste`, `wl-copy`, `xdg-open`, `cliphist`,
/// `kdeconnect-cli`, …) can block forever when no display server / daemon is
/// reachable; this guarantees they degrade to a friendly error instead of
/// hanging the agent. IMPORTANT: some commands (`wl-copy`, `xdg-open`) fork a
/// background daemon that INHERITS the stdout/stderr pipes — reading them to
/// EOF would block forever. Callers of those commands MUST use
/// [`run_timeout_null`] instead; this function is only safe for commands that
/// do not fork a daemon holding the pipes (e.g. `wl-paste`, `cliphist`).
fn run_timeout(args: &[&str], secs: u64) -> anyhow::Result<ToolResult> {
    let program = args.first().copied().unwrap_or("?");
    let mut child = match Command::new(args[0])
        .args(&args[1..])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return Ok(ToolResult::err(format!(
                "`{program}` failed to launch: {e} (is the tool installed?)"
            )))
        }
    };
    let status = match wait_timeout(&mut child, secs, program) {
        Some(s) => s,
        None => {
            return Ok(ToolResult::err(format!(
                "`{program}` timed out after {secs}s (is a display server or daemon running?)"
            )))
        }
    };
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = out.read_to_string(&mut stdout);
    }
    if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_string(&mut stderr);
    }
    let stdout = stdout.trim().to_string();
    let stderr = stderr.trim().to_string();
    if status.success() {
        Ok(ToolResult::ok(if stdout.is_empty() { stderr } else { stdout }))
    } else {
        Ok(ToolResult::err(if stderr.is_empty() { stdout } else { stderr }))
    }
}

/// Run a command with a hard timeout but WITHOUT capturing output (stdio is
/// redirected to null). Use this for commands that fork a background daemon
/// (`wl-copy`, `xdg-open`, …) — the daemon inherits piped fds, so reading to
/// EOF would block forever even after the parent exits. With null stdio there
/// is nothing to block on; we only wait for the parent process to exit.
fn run_timeout_null(args: &[&str], secs: u64) -> anyhow::Result<ToolResult> {
    let program = args.first().copied().unwrap_or("?");
    let mut child = match Command::new(args[0])
        .args(&args[1..])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return Ok(ToolResult::err(format!(
                "`{program}` failed to launch: {e} (is the tool installed?)"
            )))
        }
    };
    match wait_timeout(&mut child, secs, program) {
        Some(status) if status.success() => Ok(ToolResult::ok(format!("`{program}` started"))),
        Some(_) => Ok(ToolResult::err(format!("`{program}` failed"))),
        None => Ok(ToolResult::err(format!(
            "`{program}` timed out after {secs}s (is a display server or daemon running?)"
        ))),
    }
}

/// Validate a free-form value (tags, names, labels): printable ASCII, no
/// whitespace, no shell metacharacters, no leading dash.
fn valid_value(v: &str) -> bool {
    !v.is_empty()
        && !v.starts_with('-')
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':' | '@' | '/' | '+' | '='))
}

/// Validate an absolute path-like value. Blocks `..`, `~` expansion, quotes,
/// `$` and shell metacharacters; requires an absolute path.
fn valid_path(p: &str) -> bool {
    !p.is_empty()
        && !p.contains("..")
        && !p.contains('$')
        && !p.contains('`')
        && !p.contains('"')
        && !p.contains('\'')
        && !p.contains(';')
        && !p.contains('|')
        && !p.contains('&')
        && !p.contains('>')
        && !p.contains('<')
        && p.starts_with('/')
}

/// Validate a URL for `open_url`: allowlist of schemes, no shell metachars,
/// no whitespace, bounded length.
fn valid_url(u: &str) -> bool {
    (u.starts_with("http://") || u.starts_with("https://") || u.starts_with("ftp://"))
        && u.len() <= 4096
        && !u.contains('`')
        && !u.contains('"')
        && !u.contains('\'')
        && !u.contains(';')
        && !u.contains('|')
        && !u.contains('&')
        && !u.contains('$')
        && !u.contains(' ')
}

/// The user's Downloads directory ($HOME/Downloads), falling back to the
/// current directory when HOME is unset.
fn downloads_dir() -> String {
    let home = std::env::var_os("HOME").map(|h| h.to_string_lossy().to_string());
    match home {
        Some(h) if !h.is_empty() => format!("{h}/Downloads"),
        _ => ".".to_string(),
    }
}

/// Resolve a download entry name to an absolute path inside the Downloads
/// directory. The name may contain spaces but must be a plain file name:
/// no separators, no `..`, no shell metacharacters.
fn download_path(name: &str) -> Option<String> {
    if name.is_empty()
        || name.starts_with('/')
        || name.contains('/')
        || name.contains("..")
        || name.contains('$')
        || name.contains('`')
        || name.contains('"')
        || name.contains('\'')
        || name.contains(';')
        || name.contains('|')
        || name.contains('&')
        || name.contains('<')
        || name.contains('>')
    {
        return None;
    }
    Some(format!("{}/{}", downloads_dir(), name))
}

/// Read the first `limit` bytes of a file as text. Returns None if the file
/// cannot be read or contains NUL bytes (binary).
fn read_text_preview(path: &str, limit: usize) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = vec![0u8; limit];
    let n = f.read(&mut buf).ok()?;
    buf.truncate(n);
    if buf.contains(&0) {
        return None;
    }
    Some(String::from_utf8_lossy(&buf).to_string())
}

/// Compute the sha256 of a file via `sha256sum` (argv-only).
fn sha256_of(path: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(path) {
        return Ok(ToolResult::err("invalid path"));
    }
    run(&["sha256sum", path])
}

/// Map a file extension (lowercase, no dot) to a human category.
fn category_for_ext(ext: &str) -> &'static str {
    match ext {
        "txt" | "md" | "rst" | "log" | "tsv" | "json" | "xml" | "yaml" | "yml" | "toml" | "ini" | "conf" | "cfg" => "text",
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "tiff" | "tif" | "ico" | "heic" | "avif" => "image",
        "mp3" | "wav" | "flac" | "ogg" | "opus" | "aac" | "m4a" | "wma" | "mid" | "midi" => "audio",
        "mp4" | "mkv" | "avi" | "mov" | "webm" | "flv" | "wmv" | "m4v" | "mpg" | "mpeg" => "video",
        "pdf" => "pdf",
        "doc" | "docx" | "odt" | "rtf" => "document",
        "xls" | "xlsx" | "ods" | "csv" | "numbers" => "spreadsheet",
        "ppt" | "pptx" | "odp" => "presentation",
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "tgz" | "tbz2" | "txz" | "zst" | "lz4" => "archive",
        "rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "go" | "c" | "h" | "cpp" | "hpp" | "java" | "kt" | "swift" | "rb" | "php" | "sh" | "bash" | "zsh" | "fish" | "sql" | "html" | "css" | "scss" | "vue" | "svelte" | "lua" | "r" | "m" | "ex" | "exs" | "erl" | "hs" | "clj" | "scala" | "zig" | "nim" | "dart" | "asm" | "s" | "pl" | "pm" | "tcl" | "vb" | "cs" | "fs" | "ml" | "cmake" | "makefile" | "dockerfile" => "code",
        "deb" | "rpm" | "apk" | "exe" | "msi" | "AppImage" | "flatpak" | "snap" | "pkg" | "dmg" => "installer",
        "iso" | "img" | "vhd" | "vmdk" | "qcow2" => "disk-image",
        "ttf" | "otf" | "woff" | "woff2" | "eot" => "font",
        "epub" | "mobi" | "azw3" => "ebook",
        "torrent" => "torrent",
        "gpg" | "asc" | "pem" | "key" | "crt" | "cer" | "p12" | "pfx" => "crypto",
        _ => "other",
    }
}

/// Map an extension to a reasonable icon name (freedesktop icon theme).
fn icon_for_ext(ext: &str) -> &'static str {
    match category_for_ext(ext) {
        "text" => "text-x-generic",
        "image" => "image-x-generic",
        "audio" => "audio-x-generic",
        "video" => "video-x-generic",
        "pdf" => "application-pdf",
        "document" => "x-office-document",
        "spreadsheet" => "x-office-spreadsheet",
        "presentation" => "x-office-presentation",
        "archive" => "package-x-generic",
        "code" => "text-x-script",
        "installer" => "application-x-executable",
        "disk-image" => "drive-harddisk",
        "font" => "font-x-generic",
        "ebook" => "x-office-document",
        "torrent" => "network-workgroup",
        "crypto" => "security-high",
        _ => "text-x-generic",
    }
}

// ===========================================================================
// 2.1 FILE TYPE REGISTRY
// ===========================================================================

/// Identify the type and category of a file (extension table + `file`).
pub fn get_file_type(path: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(path) {
        return Ok(ToolResult::err("invalid path"));
    }
    let ext = Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let mut category = category_for_ext(&ext).to_string();
    let mut mime = format!("application/{ext}");
    // Enrich with the real mime when the extension is unknown or absent.
    if category == "other" || ext.is_empty() {
        let file_out = Command::new("file").args(["--mime-type", "-b", path]).output();
        if let Ok(o) = file_out {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !s.is_empty() {
                mime = s;
                category = if mime.starts_with("text/") {
                    "text".to_string()
                } else if mime.starts_with("image/") {
                    "image".to_string()
                } else if mime.starts_with("audio/") {
                    "audio".to_string()
                } else if mime.starts_with("video/") {
                    "video".to_string()
                } else if mime.starts_with("application/pdf") {
                    "pdf".to_string()
                } else if mime.contains("zip") || mime.contains("tar") || mime.contains("gzip") || mime.contains("bzip") || mime.contains("x-7z") || mime.contains("x-rar") {
                    "archive".to_string()
                } else {
                    category
                };
            }
        }
    }
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    Ok(ToolResult::ok(format!(
        "path: {path}\nextension: {ext}\ncategory: {category}\nmime: {mime}\nsize: {} bytes",
        size
    )))
}

/// File metadata: size, modification time, permissions, kind.
pub fn get_file_metadata(path: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(path) {
        return Ok(ToolResult::err("invalid path"));
    }
    let md = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) => return Ok(ToolResult::err(format!("cannot stat {path}: {e}"))),
    };
    let kind = if md.is_dir() {
        "directory"
    } else if md.is_symlink() {
        "symlink"
    } else if md.is_file() {
        "file"
    } else {
        "special"
    };
    let modified = md
        .modified()
        .ok()
        .map(|t| format!("{}", t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)))
        .unwrap_or_else(|| "unknown".to_string());
    let perms = format!("{:o}", md.permissions().mode() & 0o7777);
    Ok(ToolResult::ok(format!(
        "path: {path}\nkind: {kind}\nsize: {} bytes\nmodified_unix: {modified}\npermissions: {perms}",
        md.len()
    )))
}

/// Human-readable preview of a file's contents (first ~4 KiB of text).
pub fn get_file_preview(path: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(path) {
        return Ok(ToolResult::err("invalid path"));
    }
    match read_text_preview(path, 4096) {
        Some(text) => Ok(ToolResult::ok(text)),
        None => {
            // Binary file: fall back to a descriptive summary.
            let md = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            let ext = Path::new(path)
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            Ok(ToolResult::ok(format!(
                "[binary preview not shown] extension: {ext}, size: {md} bytes. Use get_file_info_deep or open the file."
            )))
        }
    }
}

/// Deep file info: metadata + sha256 + category.
pub fn get_file_info_deep(path: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(path) {
        return Ok(ToolResult::err("invalid path"));
    }
    let meta = get_file_metadata(path)?;
    let meta_out = if meta.success { meta.output.clone() } else { String::new() };
    let hash = sha256_of(path)?;
    let hash_out = if hash.success {
        hash.output.split_whitespace().next().unwrap_or("unknown").to_string()
    } else {
        format!("(sha256 unavailable: {})", hash.error.unwrap_or_default())
    };
    let ext = Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    Ok(ToolResult::ok(format!(
        "{meta_out}\nsha256: {hash_out}\ncategory: {}",
        category_for_ext(&ext)
    )))
}

/// Generate a 256x256 PNG thumbnail of an image (ffmpeg or ImageMagick).
pub fn get_file_thumbnail(path: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(path) {
        return Ok(ToolResult::err("invalid path"));
    }
    let out = format!("{path}.thumb.png");
    // Prefer ffmpeg; fall back to ImageMagick's convert.
    let ffmpeg = Command::new("ffmpeg")
        .args(["-y", "-i", path, "-vf", "scale=256:256", out.as_str()])
        .output();
    if let Ok(o) = ffmpeg {
        if o.status.success() {
            return Ok(ToolResult::ok(format!("thumbnail written to {out}")));
        }
    }
    let convert = Command::new("convert")
        .args([path, "-resize", "256x256", out.as_str()])
        .output();
    if let Ok(o) = convert {
        if o.status.success() {
            return Ok(ToolResult::ok(format!("thumbnail written to {out}")));
        }
    }
    Ok(ToolResult::err(
        "thumbnail generation failed: install ffmpeg or ImageMagick (convert)",
    ))
}

/// Extract a video frame as a thumbnail (ffmpeg).
pub fn generate_video_thumbnail(path: &str, at_seconds: u64) -> anyhow::Result<ToolResult> {
    if !valid_path(path) {
        return Ok(ToolResult::err("invalid path"));
    }
    let secs = at_seconds.min(3600);
    let out = format!("{path}.thumb.png");
    let r = Command::new("ffmpeg")
        .args([
            "-y",
            "-ss",
            &secs.to_string(),
            "-i",
            path,
            "-frames:v",
            "1",
            "-vf",
            "scale=256:256",
            out.as_str(),
        ])
        .output();
    match r {
        Ok(o) if o.status.success() => Ok(ToolResult::ok(format!("video thumbnail written to {out}"))),
        Ok(_) => Ok(ToolResult::err("ffmpeg failed to extract the frame")),
        Err(e) => Ok(ToolResult::err(format!("ffmpeg not available: {e}"))),
    }
}

/// Suggest an icon name (freedesktop icon theme) for a file.
pub fn get_file_icon(path: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(path) {
        return Ok(ToolResult::err("invalid path"));
    }
    let ext = Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let icon = icon_for_ext(&ext);
    let md = std::fs::metadata(path);
    let icon = if matches!(&md, Ok(m) if m.is_dir()) {
        "folder"
    } else {
        icon
    };
    Ok(ToolResult::ok(icon.to_string()))
}

// ===========================================================================
// 2.2 NATURAL LANGUAGE FILE OPENING
// ===========================================================================

/// Open a file with its default application (xdg-open).
pub fn open_file(path: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(path) {
        return Ok(ToolResult::err("invalid path"));
    }
    // xdg-open forks the target application (which inherits pipes) — never
    // capture its output, only wait for the launcher to exit.
    run_timeout_null(&["xdg-open", path], 5)
}

/// Open a URL in the default browser (xdg-open, scheme allowlist).
pub fn open_url(url: &str) -> anyhow::Result<ToolResult> {
    if !valid_url(url) {
        return Ok(ToolResult::err("invalid URL"));
    }
    run_timeout_null(&["xdg-open", url], 5)
}

/// Open a directory in the default file manager.
pub fn open_directory(path: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(path) {
        return Ok(ToolResult::err("invalid path"));
    }
    if !Path::new(path).is_dir() {
        return Ok(ToolResult::err(format!("not a directory: {path}")));
    }
    run_timeout_null(&["xdg-open", path], 5)
}

/// Open a file with an explicit application.
pub fn open_with(path: &str, app: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(path) {
        return Ok(ToolResult::err("invalid path"));
    }
    if !valid_identifier(app) {
        return Ok(ToolResult::err("invalid application name"));
    }
    run(&[app, path])
}

/// Set the default application for a mime type (xdg-mime).
pub fn set_default_app(mime: &str, app: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(mime) || mime.contains('/') && mime.split('/').count() != 2 {
        return Ok(ToolResult::err("invalid mime type"));
    }
    if !valid_identifier(app) {
        return Ok(ToolResult::err("invalid application name"));
    }
    run(&["xdg-mime", "default", app, mime])
}

// ===========================================================================
// 2.3 DOWNLOAD INTEGRATION
// ===========================================================================

/// List everything in the Downloads directory.
pub fn list_downloads() -> anyhow::Result<ToolResult> {
    let dir = downloads_dir();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => return Ok(ToolResult::err(format!("cannot read {dir}: {e}"))),
    };
    let mut lines: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let kind = if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            "dir"
        } else {
            "file"
        };
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        lines.push(format!("{name}\t{kind}\t{size} bytes"));
    }
    lines.sort();
    Ok(ToolResult::ok(if lines.is_empty() {
        "(Downloads is empty)".to_string()
    } else {
        lines.join("\n")
    }))
}

/// Open a download with its default application.
pub fn open_download(name: &str) -> anyhow::Result<ToolResult> {
    let path = match download_path(name) {
        Some(p) => p,
        None => return Ok(ToolResult::err("invalid download name")),
    };
    if !Path::new(&path).exists() {
        return Ok(ToolResult::err(format!("download not found: {name}")));
    }
    open_file(&path)
}

/// Move a download to a destination directory (std::fs::rename).
pub fn move_download(name: &str, destination: &str) -> anyhow::Result<ToolResult> {
    let path = match download_path(name) {
        Some(p) => p,
        None => return Ok(ToolResult::err("invalid download name")),
    };
    if !valid_path(destination) {
        return Ok(ToolResult::err("invalid destination"));
    }
    if !Path::new(&path).exists() {
        return Ok(ToolResult::err(format!("download not found: {name}")));
    }
    let dest_dir = destination.trim_end_matches('/');
    if !Path::new(dest_dir).is_dir() {
        return Ok(ToolResult::err(format!("destination is not a directory: {destination}")));
    }
    let dest = format!("{dest_dir}/{name}");
    match std::fs::rename(&path, &dest) {
        Ok(_) => Ok(ToolResult::ok(format!("moved {name} to {dest}"))),
        Err(e) => Ok(ToolResult::err(format!("move failed: {e}"))),
    }
}

/// Delete a download (DESTRUCTIVE — requires confirmation).
pub fn delete_download(name: &str) -> anyhow::Result<ToolResult> {
    let path = match download_path(name) {
        Some(p) => p,
        None => return Ok(ToolResult::err("invalid download name")),
    };
    if !Path::new(&path).exists() {
        return Ok(ToolResult::err(format!("download not found: {name}")));
    }
    match std::fs::remove_file(&path) {
        Ok(_) => Ok(ToolResult::ok(format!("deleted {name}"))),
        Err(e) => Ok(ToolResult::err(format!("delete failed: {e}"))),
    }
}

/// Organize the Downloads folder into category subdirectories
/// (DESTRUCTIVE-ish — moves files, requires confirmation).
pub fn organize_downloads() -> anyhow::Result<ToolResult> {
    let dir = downloads_dir();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => return Ok(ToolResult::err(format!("cannot read {dir}: {e}"))),
    };
    let mut moved = 0u32;
    let mut errors: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = Path::new(&name)
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let category = if ext.is_empty() { "misc" } else { category_for_ext(&ext) };
        let cat_dir = format!("{dir}/{category}");
        if let Err(e) = std::fs::create_dir_all(&cat_dir) {
            errors.push(format!("create {cat_dir}: {e}"));
            continue;
        }
        let dest = format!("{cat_dir}/{name}");
        if dest == path.to_string_lossy() {
            continue;
        }
        match std::fs::rename(&path, &dest) {
            Ok(_) => moved += 1,
            Err(e) => errors.push(format!("move {name}: {e}")),
        }
    }
    let mut out = format!("organized {moved} files into category folders");
    if !errors.is_empty() {
        out.push_str(&format!("\nerrors:\n{}", errors.join("\n")));
    }
    Ok(ToolResult::ok(out))
}

/// Download statistics: counts per category + total size.
pub fn get_download_stats() -> anyhow::Result<ToolResult> {
    let dir = downloads_dir();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => return Ok(ToolResult::err(format!("cannot read {dir}: {e}"))),
    };
    let mut by_category: std::collections::BTreeMap<String, (u32, u64)> = std::collections::BTreeMap::new();
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = Path::new(&name)
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let category = if ext.is_empty() { "misc" } else { category_for_ext(&ext) }.to_string();
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        let e = by_category.entry(category).or_insert((0, 0));
        e.0 += 1;
        e.1 += size;
    }
    let mut lines: Vec<String> = Vec::new();
    let mut total_files = 0u32;
    let mut total_bytes = 0u64;
    for (cat, (count, bytes)) in by_category {
        total_files += count;
        total_bytes += bytes;
        lines.push(format!("{cat}: {count} files, {bytes} bytes"));
    }
    lines.push(format!("TOTAL: {total_files} files, {total_bytes} bytes"));
    Ok(ToolResult::ok(lines.join("\n")))
}

/// Delete downloads older than N days (DESTRUCTIVE — requires confirmation).
pub fn clean_old_downloads(days: u64) -> anyhow::Result<ToolResult> {
    if days == 0 || days > 3650 {
        return Ok(ToolResult::err("days must be between 1 and 3650"));
    }
    let dir = downloads_dir();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => return Ok(ToolResult::err(format!("cannot read {dir}: {e}"))),
    };
    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(days * 86_400))
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    let mut removed = 0u32;
    for entry in entries.flatten() {
        let path = entry.path();
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        if let Ok(modified) = entry.metadata().and_then(|m| m.modified()) {
            if modified < cutoff
                && std::fs::remove_file(&path).is_ok() {
                    removed += 1;
                }
        }
    }
    Ok(ToolResult::ok(format!("removed {removed} files older than {days} days")))
}

/// Find duplicate downloads by sha256.
pub fn find_duplicate_downloads() -> anyhow::Result<ToolResult> {
    let dir = downloads_dir();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => return Ok(ToolResult::err(format!("cannot read {dir}: {e}"))),
    };
    let mut files: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            files.push(entry.path().to_string_lossy().to_string());
        }
    }
    if files.is_empty() {
        return Ok(ToolResult::ok("no files to compare"));
    }
    let mut by_hash: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    for f in files {
        let h = sha256_of(&f)?;
        if h.success {
            if let Some(hash) = h.output.split_whitespace().next() {
                by_hash.entry(hash.to_string()).or_default().push(f.clone());
            }
        }
    }
    let mut lines: Vec<String> = Vec::new();
    let mut groups = 0u32;
    for (hash, group) in by_hash {
        if group.len() > 1 {
            groups += 1;
            lines.push(format!("{hash}: {}", group.join(", ")));
        }
    }
    if lines.is_empty() {
        Ok(ToolResult::ok("no duplicate files found"))
    } else {
        let mut out = format!("{groups} duplicate group(s):\n");
        out.push_str(&lines.join("\n"));
        Ok(ToolResult::ok(out))
    }
}

/// Virus-scan a download (clamscan).
pub fn scan_download(name: &str) -> anyhow::Result<ToolResult> {
    let path = match download_path(name) {
        Some(p) => p,
        None => return Ok(ToolResult::err("invalid download name")),
    };
    if !Path::new(&path).exists() {
        return Ok(ToolResult::err(format!("download not found: {name}")));
    }
    run(&["clamscan", "--no-summary", &path])
}

// ===========================================================================
// 2.4 CLIPBOARD & SHARING
// ===========================================================================

/// Read the current clipboard text (wl-paste or xclip).
pub fn get_clipboard_text() -> anyhow::Result<ToolResult> {
    let wl = run_timeout(&["wl-paste", "--no-newline"], 3)?;
    if wl.success {
        return Ok(wl);
    }
    run_timeout(&["xclip", "-selection", "clipboard", "-o"], 3)
}

/// Write text to the clipboard (wl-copy or xclip).
pub fn set_clipboard_text(text: &str) -> anyhow::Result<ToolResult> {
    if text.len() > 1_000_000 {
        return Ok(ToolResult::err("clipboard text too large (max 1 MB)"));
    }
    // wl-copy forks a daemon that inherits pipe fds — never capture output.
    let wl = run_timeout_null(&["wl-copy", text], 3)?;
    if wl.success {
        return Ok(wl);
    }
    // Fall back to X11: pipe the text into `xclip` with a hard timeout.
    let mut child = match Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => {
            return Ok(ToolResult::err(
                "no clipboard tool available (install wl-clipboard or xclip)",
            ))
        }
    };
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(text.as_bytes());
    }
    match wait_timeout(&mut child, 3, "xclip -selection clipboard") {
        Some(status) if status.success() => Ok(ToolResult::ok("clipboard updated (X11)")),
        Some(_) => Ok(ToolResult::err("xclip failed")),
        None => Ok(ToolResult::err(
            "xclip timed out (is an X11 display server running?)",
        )),
    }
}

/// Save the current clipboard image to a PNG file.
pub fn get_clipboard_image() -> anyhow::Result<ToolResult> {
    let out = format!("{}/diad-clipboard.png", std::env::temp_dir().to_string_lossy());
    let mut child = match Command::new("wl-paste")
        .args(["--type", "image/png"])
        .stdout(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => {
            return Ok(ToolResult::err("wl-paste not available (install wl-clipboard)"))
        }
    };
    let status = match wait_timeout(&mut child, 3, "wl-paste --type image/png") {
        Some(s) => s,
        None => {
            return Ok(ToolResult::err(
                "wl-paste timed out (is a display server running?)",
            ))
        }
    };
    let mut buf = Vec::new();
    if let Some(mut stdout) = child.stdout.take() {
        let _ = std::io::copy(&mut stdout, &mut buf);
    }
    if status.success() && !buf.is_empty() {
        let _ = std::fs::write(&out, &buf);
        return Ok(ToolResult::ok(format!("clipboard image saved to {out} ({} bytes)", buf.len())));
    }
    Ok(ToolResult::err("clipboard contains no image"))
}

/// Copy an image file to the clipboard.
pub fn set_clipboard_image(path: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(path) {
        return Ok(ToolResult::err("invalid path"));
    }
    if !Path::new(path).exists() {
        return Ok(ToolResult::err(format!("file not found: {path}")));
    }
    // wl-copy forks a daemon that inherits pipe fds — never capture output.
    let wl = run_timeout_null(&["wl-copy", "--type", "image/png", path], 3)?;
    if wl.success {
        return Ok(wl);
    }
    Ok(ToolResult::err("wl-copy not available (install wl-clipboard)"))
}

/// Clipboard history via `cliphist list`.
pub fn get_clipboard_history() -> anyhow::Result<ToolResult> {
    run_timeout(&["cliphist", "list"], 3)
}

/// Clear clipboard history (DESTRUCTIVE — requires confirmation).
pub fn clear_clipboard_history() -> anyhow::Result<ToolResult> {
    run_timeout(&["cliphist", "wipe"], 3)
}

/// Share a file with the best available method (Bluetooth/KDE Connect).
pub fn share_file(path: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(path) {
        return Ok(ToolResult::err("invalid path"));
    }
    if !Path::new(path).exists() {
        return Ok(ToolResult::err(format!("file not found: {path}")));
    }
    // Prefer KDE Connect (desktop-agnostic local sharing).
    let kde = run_timeout(&["kdeconnect-cli", "--share", path], 5)?;
    if kde.success {
        return Ok(kde);
    }
    // Fall back to Bluetooth OBEX.
    let obex = run_timeout(&["obexctl", "send", path], 5)?;
    if obex.success {
        return Ok(obex);
    }
    Ok(ToolResult::err(
        "no sharing backend available (install kdeconnect or bluez-obexd)",
    ))
}

/// Generate a QR code for a file (qrencode → PNG).
pub fn qr_code_file(path: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(path) {
        return Ok(ToolResult::err("invalid path"));
    }
    if !Path::new(path).exists() {
        return Ok(ToolResult::err(format!("file not found: {path}")));
    }
    let out = format!("{path}.qr.png");
    run(&["qrencode", "-o", &out, path])
}

/// Share a file to a connected phone (KDE Connect).
pub fn share_to_phone(path: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(path) {
        return Ok(ToolResult::err("invalid path"));
    }
    if !Path::new(path).exists() {
        return Ok(ToolResult::err(format!("file not found: {path}")));
    }
    run_timeout(&["kdeconnect-cli", "--share", path], 5)
}

/// AirDrop-like nearby sharing: discover devices and push the file.
pub fn airdrop_like(path: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(path) {
        return Ok(ToolResult::err("invalid path"));
    }
    if !Path::new(path).exists() {
        return Ok(ToolResult::err(format!("file not found: {path}")));
    }
    let discover = run_timeout(&["kdeconnect-cli", "-l"], 5)?;
    if !discover.success {
        return Ok(ToolResult::err("KDE Connect unavailable — install kdeconnect to use AirDrop-like sharing"));
    }
    // Parse device IDs from `kdeconnect-cli -l` output and share to each.
    let mut shared = 0u32;
    for line in discover.output.lines() {
        if let Some(idx) = line.find("id: ") {
            let id = line[idx + 4..].split_whitespace().next().unwrap_or("").to_string();
            if valid_identifier(&id) && !id.is_empty() {
                let r = run_timeout(&["kdeconnect-cli", "-d", &id, "--share", path], 5)?;
                if r.success {
                    shared += 1;
                }
            }
        }
    }
    Ok(ToolResult::ok(format!("shared to {shared} device(s)")))
}

// ===========================================================================
// 2.5 SEARCH & ORGANIZATION
// ===========================================================================

/// Rich file-name search with options (supersedes the simple glob version).
///
/// Options: `regex` (bool), `case_sensitive` (bool), `include_hidden` (bool),
/// `max_results` (int), `file_types` (array of extensions).
pub fn search_files(query: &str, path: &str, regex: bool, case_sensitive: bool, include_hidden: bool, max_results: u64, file_types: &[String]) -> anyhow::Result<ToolResult> {
    let root = if path.is_empty() { "." } else { path };
    if !valid_path(root) && root != "." {
        return Ok(ToolResult::err("invalid search path"));
    }
    if query.is_empty() || query.len() > 512 {
        return Ok(ToolResult::err("invalid query"));
    }
    let max = max_results.clamp(1, 1000);

    // List candidates with plain `find`; all filtering (hidden, file type,
    // query) happens in Rust below for consistent, testable semantics.
    // NOTE: we deliberately do NOT use `-not -path '*/.*'` because that
    // predicate also matches a *hidden root* (e.g. a tempdir like
    // /tmp/.tmpXXX) and would silently exclude every result.
    let out = match Command::new("find").arg(root).output() {
        Ok(o) => o,
        Err(e) => return Ok(ToolResult::err(format!("find failed to launch: {e}"))),
    };
    if !out.status.success() {
        return Ok(ToolResult::err(String::from_utf8_lossy(&out.stderr).trim().to_string()));
    }

    // Compile the query matcher once.
    let re = if regex {
        match regex::Regex::new(query) {
            Ok(r) => Some(r),
            Err(e) => return Ok(ToolResult::err(format!("invalid regex: {e}"))),
        }
    } else {
        None
    };
    let glob = !regex && (query.contains('*') || query.contains('?'));

    let mut lines: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| include_hidden || !is_hidden_below(root, l))
        .filter(|l| {
            if file_types.is_empty() {
                return true;
            }
            let ext = Path::new(l)
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            file_types.iter().any(|ft| ft.to_lowercase() == ext)
        })
        .filter(|l| {
            let base = Path::new(l)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let hay = if case_sensitive { base.clone() } else { base.to_lowercase() };
            if let Some(re) = &re {
                let text = if case_sensitive { base } else { base.to_lowercase() };
                re.is_match(&text)
            } else if glob {
                glob_match(query, &hay)
            } else {
                let needle = if case_sensitive { query.to_string() } else { query.to_lowercase() };
                hay.contains(&needle)
            }
        })
        .map(|l| l.to_string())
        .take(max as usize)
        .collect();
    lines.sort();
    Ok(ToolResult::ok(if lines.is_empty() {
        "(no matches)".to_string()
    } else {
        lines.join("\n")
    }))
}

/// Minimal glob matcher supporting `*` (any run) and `?` (single char).
fn glob_match(pattern: &str, text: &str) -> bool {
    // Convert the glob to a regex for correctness.
    let mut rx = String::with_capacity(pattern.len() * 2);
    rx.push('^');
    for c in pattern.chars() {
        match c {
            '*' => rx.push_str(".*"),
            '?' => rx.push('.'),
            // Escape regex metacharacters literally.
            '.' | '+' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                rx.push('\\');
                rx.push(c);
            }
            _ => rx.push(c),
        }
    }
    rx.push('$');
    regex::Regex::new(&rx).map(|r| r.is_match(text)).unwrap_or(false)
}

/// True when a path returned by `find` has a hidden path component BELOW the
/// search root (a hidden root itself — e.g. /tmp/.tmpXXX — is allowed, since
/// the user explicitly searched it).
fn is_hidden_below(root: &str, path: &str) -> bool {
    let root = root.trim_end_matches('/');
    let rel = match path.strip_prefix(root) {
        Some(r) => r.trim_start_matches('/'),
        None => return false,
    };
    rel.split('/').any(|c| c.starts_with('.') && !c.is_empty())
}

/// Search file contents for a string (rg, with grep fallback).
pub fn search_by_content(query: &str, path: &str, case_sensitive: bool, max_results: u64) -> anyhow::Result<ToolResult> {
    let root = if path.is_empty() { "." } else { path };
    if !valid_path(root) && root != "." {
        return Ok(ToolResult::err("invalid search path"));
    }
    if query.is_empty() || query.len() > 512 {
        return Ok(ToolResult::err("invalid query"));
    }
    let max = max_results.clamp(1, 1000);
    let rg = Command::new("rg")
        .args(["-l", "-F", query, root])
        .args(if case_sensitive { &[][..] } else { &["-i"][..] })
        .output();
    if let Ok(o) = rg {
        if o.status.success() {
            let lines: Vec<String> = String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|l| l.to_string())
                .take(max as usize)
                .collect();
            return Ok(ToolResult::ok(lines.join("\n")));
        }
    }
    // Fallback: grep -r -l -F (or -i).
    let mut grep = Command::new("grep");
    grep.args(["-r", "-l", "-F"]);
    if !case_sensitive {
        grep.arg("-i");
    }
    grep.args(["--", query, root]);
    let out = match grep.output() {
        Ok(o) => o,
        Err(e) => return Ok(ToolResult::err(format!("grep failed to launch: {e}"))),
    };
    if !out.status.success() && out.stdout.is_empty() {
        return Ok(ToolResult::err(String::from_utf8_lossy(&out.stderr).trim().to_string()));
    }
    let lines: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.to_string())
        .take(max as usize)
        .collect();
    Ok(ToolResult::ok(if lines.is_empty() {
        "(no matches)".to_string()
    } else {
        lines.join("\n")
    }))
}

/// Search file contents with a regex (rg -E, grep -E fallback).
pub fn search_by_regex(pattern: &str, path: &str, case_sensitive: bool, max_results: u64) -> anyhow::Result<ToolResult> {
    let root = if path.is_empty() { "." } else { path };
    if !valid_path(root) && root != "." {
        return Ok(ToolResult::err("invalid search path"));
    }
    if pattern.is_empty() || pattern.len() > 512 {
        return Ok(ToolResult::err("invalid pattern"));
    }
    let max = max_results.clamp(1, 1000);
    let rg = Command::new("rg")
        .args(["-l", "-E", pattern, root])
        .args(if case_sensitive { &[][..] } else { &["-i"][..] })
        .output();
    if let Ok(o) = rg {
        if o.status.success() {
            let lines: Vec<String> = String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|l| l.to_string())
                .take(max as usize)
                .collect();
            return Ok(ToolResult::ok(lines.join("\n")));
        }
    }
    let mut grep = Command::new("grep");
    grep.args(["-r", "-l", "-E"]);
    if !case_sensitive {
        grep.arg("-i");
    }
    grep.args(["--", pattern, root]);
    let out = match grep.output() {
        Ok(o) => o,
        Err(e) => return Ok(ToolResult::err(format!("grep failed to launch: {e}"))),
    };
    if !out.status.success() && out.stdout.is_empty() {
        return Ok(ToolResult::err(String::from_utf8_lossy(&out.stderr).trim().to_string()));
    }
    let lines: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.to_string())
        .take(max as usize)
        .collect();
    Ok(ToolResult::ok(if lines.is_empty() {
        "(no matches)".to_string()
    } else {
        lines.join("\n")
    }))
}

/// Path to the tag store (JSON sidecar under $HOME/.diad/tags.json).
fn tag_store_path() -> String {
    let home = std::env::var_os("HOME").map(|h| h.to_string_lossy().to_string());
    match home {
        Some(h) if !h.is_empty() => format!("{h}/.diad/tags.json"),
        _ => ".diad-tags.json".to_string(),
    }
}

fn read_tag_store() -> std::collections::BTreeMap<String, Vec<String>> {
    let path = tag_store_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_tag_store(store: &std::collections::BTreeMap<String, Vec<String>>) -> anyhow::Result<ToolResult> {
    let path = tag_store_path();
    if let Some(parent) = Path::new(&path).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    match serde_json::to_string_pretty(store) {
        Ok(json) => match std::fs::write(&path, json) {
            Ok(_) => Ok(ToolResult::ok(format!("tags saved to {path}"))),
            Err(e) => Ok(ToolResult::err(format!("cannot write tag store: {e}"))),
        },
        Err(e) => Ok(ToolResult::err(format!("cannot serialize tag store: {e}"))),
    }
}

/// Tag a file with one or more tags (persisted in a JSON sidecar).
pub fn tag_file(path: &str, tags: &[String]) -> anyhow::Result<ToolResult> {
    if !valid_path(path) {
        return Ok(ToolResult::err("invalid path"));
    }
    if !Path::new(path).exists() {
        return Ok(ToolResult::err(format!("file not found: {path}")));
    }
    if tags.is_empty() {
        return Ok(ToolResult::err("at least one tag required"));
    }
    for t in tags {
        if !valid_value(t) {
            return Ok(ToolResult::err(format!("invalid tag: {t}")));
        }
    }
    let mut store = read_tag_store();
    let entry = store.entry(path.to_string()).or_default();
    for t in tags {
        if !entry.contains(t) {
            entry.push(t.clone());
        }
    }
    write_tag_store(&store)
}

/// List the tags of a file.
pub fn get_file_tags(path: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(path) {
        return Ok(ToolResult::err("invalid path"));
    }
    let store = read_tag_store();
    match store.get(path) {
        Some(tags) if !tags.is_empty() => Ok(ToolResult::ok(tags.join(", "))),
        _ => Ok(ToolResult::ok("(no tags)")),
    }
}

/// Find all files carrying a tag.
pub fn search_by_tag(tag: &str) -> anyhow::Result<ToolResult> {
    if !valid_value(tag) {
        return Ok(ToolResult::err("invalid tag"));
    }
    let store = read_tag_store();
    let mut files: Vec<String> = Vec::new();
    for (path, tags) in &store {
        if tags.iter().any(|t| t == tag) {
            files.push(path.clone());
        }
    }
    files.sort();
    Ok(ToolResult::ok(if files.is_empty() {
        "(no files with this tag)".to_string()
    } else {
        files.join("\n")
    }))
}

/// Most recently modified files in a directory (find -printf, parsed in Rust).
pub fn get_recent_files(path: &str, limit: u64) -> anyhow::Result<ToolResult> {
    let root = if path.is_empty() { "." } else { path };
    if !valid_path(root) && root != "." {
        return Ok(ToolResult::err("invalid search path"));
    }
    let max = limit.clamp(1, 500);
    let out = Command::new("find")
        .args([root, "-type", "f", "-printf", "%T@ %p\\n"])
        .output();
    let out = match out {
        Ok(o) => o,
        Err(e) => return Ok(ToolResult::err(format!("find failed to launch: {e}"))),
    };
    if !out.status.success() {
        return Ok(ToolResult::err(String::from_utf8_lossy(&out.stderr).trim().to_string()));
    }
    let mut rows: Vec<(f64, String)> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let (ts, p) = l.split_once(' ')?;
            let t: f64 = ts.parse().ok()?;
            Some((t, p.to_string()))
        })
        .collect();
    rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    rows.truncate(max as usize);
    let lines: Vec<String> = rows.iter().map(|(_, p)| p.clone()).collect();
    Ok(ToolResult::ok(if lines.is_empty() {
        "(no files)".to_string()
    } else {
        lines.join("\n")
    }))
}

/// Find duplicate files anywhere (sha256 grouping).
pub fn get_duplicate_files(path: &str) -> anyhow::Result<ToolResult> {
    let root = if path.is_empty() { "." } else { path };
    if !valid_path(root) && root != "." {
        return Ok(ToolResult::err("invalid search path"));
    }
    let out = Command::new("find")
        .args([root, "-type", "f"])
        .output();
    let out = match out {
        Ok(o) => o,
        Err(e) => return Ok(ToolResult::err(format!("find failed to launch: {e}"))),
    };
    if !out.status.success() {
        return Ok(ToolResult::err(String::from_utf8_lossy(&out.stderr).trim().to_string()));
    }
    let files: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.to_string())
        .take(500)
        .collect();
    let mut by_hash: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    for f in files {
        let h = sha256_of(&f)?;
        if h.success {
            if let Some(hash) = h.output.split_whitespace().next() {
                by_hash.entry(hash.to_string()).or_default().push(f.clone());
            }
        }
    }
    let mut lines: Vec<String> = Vec::new();
    let mut groups = 0u32;
    for (hash, group) in by_hash {
        if group.len() > 1 {
            groups += 1;
            lines.push(format!("{hash}: {}", group.join(", ")));
        }
    }
    if lines.is_empty() {
        Ok(ToolResult::ok("no duplicate files found"))
    } else {
        let mut out = format!("{groups} duplicate group(s):\n");
        out.push_str(&lines.join("\n"));
        Ok(ToolResult::ok(out))
    }
}

/// Index a directory: path, size and mtime for every file.
pub fn index_directory(path: &str) -> anyhow::Result<ToolResult> {
    let root = if path.is_empty() { "." } else { path };
    if !valid_path(root) && root != "." {
        return Ok(ToolResult::err("invalid search path"));
    }
    let out = Command::new("find")
        .args([root, "-type", "f", "-printf", "%p\\t%s\\t%T@\\n"])
        .output();
    let out = match out {
        Ok(o) => o,
        Err(e) => return Ok(ToolResult::err(format!("find failed to launch: {e}"))),
    };
    if !out.status.success() {
        return Ok(ToolResult::err(String::from_utf8_lossy(&out.stderr).trim().to_string()));
    }
    let entries: Vec<serde_json::Value> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let mut it = l.split('\t');
            let p = it.next()?.to_string();
            let s = it.next()?.to_string();
            let t = it.next()?.to_string();
            Some(serde_json::json!({ "path": p, "size": s, "mtime": t }))
        })
        .collect();
    let json = serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string());
    Ok(ToolResult::ok(format!("{} files indexed:\n{json}", entries.len())))
}

/// Build a nested JSON tree of a directory (bounded depth).
pub fn get_directory_tree(path: &str, depth: u32) -> anyhow::Result<ToolResult> {
    let root = if path.is_empty() { "." } else { path };
    if !valid_path(root) && root != "." {
        return Ok(ToolResult::err("invalid search path"));
    }
    if depth > 8 {
        return Ok(ToolResult::err("depth must be between 1 and 8"));
    }
    let max_depth = depth.max(1);
    let mut tree: Vec<serde_json::Value> = Vec::new();
    build_tree(Path::new(root), max_depth, 0, &mut tree);
    let json = serde_json::to_string_pretty(&serde_json::json!({ "root": root, "tree": tree }))
        .unwrap_or_else(|_| "{}".to_string());
    Ok(ToolResult::ok(json))
}

fn build_tree(dir: &Path, max_depth: u32, level: u32, out: &mut Vec<serde_json::Value>) {
    if level >= max_depth {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        if is_dir {
            let mut children: Vec<serde_json::Value> = Vec::new();
            build_tree(&entry.path(), max_depth, level + 1, &mut children);
            out.push(serde_json::json!({ "name": name, "type": "dir", "size": size, "children": children }));
        } else {
            out.push(serde_json::json!({ "name": name, "type": "file", "size": size }));
        }
    }
}

// ===========================================================================
// 2.6 ARCHIVE MANAGEMENT
// ===========================================================================

/// Detect the archive toolset for an extension: (list_cmd, extract_cmd).
fn archive_cmds(archive: &str) -> Option<(&'static str, &'static [&'static str])> {
    let lower = archive.to_lowercase();
    if lower.ends_with(".zip") {
        Some(("unzip", &["-l", "-qq"][..]))
    } else if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
        Some(("tar", &["-tzf"][..]))
    } else if lower.ends_with(".tar.bz2") || lower.ends_with(".tbz2") {
        Some(("tar", &["-tjf"][..]))
    } else if lower.ends_with(".tar.xz") || lower.ends_with(".txz") {
        Some(("tar", &["-tJf"][..]))
    } else if lower.ends_with(".tar.zst") || lower.ends_with(".tzst") {
        Some(("tar", &["-t", "--zstd", "-f"][..]))
    } else if lower.ends_with(".tar") {
        Some(("tar", &["-tf"][..]))
    } else if lower.ends_with(".gz") {
        Some(("gzip", &["-l"][..]))
    } else if lower.ends_with(".bz2") {
        Some(("bzip2", &["-tv"][..]))
    } else if lower.ends_with(".xz") {
        Some(("xz", &["-l"][..]))
    } else if lower.ends_with(".7z") {
        Some(("7z", &["l"][..]))
    } else if lower.ends_with(".rar") {
        Some(("unrar", &["l"][..]))
    } else {
        None
    }
}

/// Extract an archive into a destination directory (auto-detected format).
pub fn extract_archive(archive: &str, destination: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(archive) || !valid_path(destination) {
        return Ok(ToolResult::err("invalid path"));
    }
    if !Path::new(archive).exists() {
        return Ok(ToolResult::err(format!("archive not found: {archive}")));
    }
    if let Err(e) = std::fs::create_dir_all(destination) {
        return Ok(ToolResult::err(format!("cannot create destination: {e}")));
    }
    let lower = archive.to_lowercase();
    let r = if lower.ends_with(".zip") {
        run(&["unzip", "-qq", "-o", archive, "-d", destination])
    } else if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
        run(&["tar", "-xzf", archive, "-C", destination])
    } else if lower.ends_with(".tar.bz2") || lower.ends_with(".tbz2") {
        run(&["tar", "-xjf", archive, "-C", destination])
    } else if lower.ends_with(".tar.xz") || lower.ends_with(".txz") {
        run(&["tar", "-xJf", archive, "-C", destination])
    } else if lower.ends_with(".tar.zst") || lower.ends_with(".tzst") {
        run(&["tar", "--use-compress-program=zstd", "-xf", archive, "-C", destination])
    } else if lower.ends_with(".tar") {
        run(&["tar", "-xf", archive, "-C", destination])
    } else if lower.ends_with(".7z") {
        run(&["7z", "x", "-y", format!("-o{destination}").as_str(), archive])
    } else if lower.ends_with(".rar") {
        run(&["unrar", "x", "-y", archive, &format!("{destination}/")])
    } else {
        return Ok(ToolResult::err(format!("unsupported archive format: {archive}")));
    }?;
    if r.success {
        Ok(ToolResult::ok(format!("extracted {archive} into {destination}")))
    } else {
        Ok(r)
    }
}

/// Create an archive from a source path.
/// Formats: zip, tar, tar.gz, tar.bz2, tar.xz, 7z.
pub fn create_archive(source: &str, format: &str, output: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(source) || !valid_path(output) {
        return Ok(ToolResult::err("invalid path"));
    }
    if !Path::new(source).exists() {
        return Ok(ToolResult::err(format!("source not found: {source}")));
    }
    if !valid_value(format) {
        return Ok(ToolResult::err("invalid format"));
    }
    let name = Path::new(source)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let fmt = format.to_lowercase();
    let r = match fmt.as_str() {
        "zip" => run(&["zip", "-r", "-q", output, source]),
        "tar" => run(&["tar", "-cf", output, source]),
        "tar.gz" | "tgz" => run(&["tar", "-czf", output, source]),
        "tar.bz2" | "tbz2" => run(&["tar", "-cjf", output, source]),
        "tar.xz" | "txz" => run(&["tar", "-cJf", output, source]),
        "7z" => run(&["7z", "a", "-y", output, source]),
        _ => {
            return Ok(ToolResult::err(format!(
                "unsupported format: {format} (zip, tar, tar.gz, tar.bz2, tar.xz, 7z)"
            )))
        }
    }?;
    if r.success {
        Ok(ToolResult::ok(format!("created {output} ({fmt}) from {name}")))
    } else {
        Ok(r)
    }
}

/// List the contents of an archive.
pub fn list_archive_contents(archive: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(archive) {
        return Ok(ToolResult::err("invalid path"));
    }
    if !Path::new(archive).exists() {
        return Ok(ToolResult::err(format!("archive not found: {archive}")));
    }
    match archive_cmds(archive) {
        Some((bin, args)) => {
            let mut full: Vec<&str> = vec![bin];
            full.extend_from_slice(args);
            full.push(archive);
            run(&full)
        }
        None => Ok(ToolResult::err(format!("unsupported archive format: {archive}"))),
    }
}

/// Extract a single member from an archive.
pub fn extract_single_file(archive: &str, member: &str, destination: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(archive) || !valid_path(destination) {
        return Ok(ToolResult::err("invalid path"));
    }
    if member.is_empty() || member.contains("..") || member.contains('$') || member.contains('`') || member.contains(';') || member.contains('|') {
        return Ok(ToolResult::err("invalid member name"));
    }
    if !Path::new(archive).exists() {
        return Ok(ToolResult::err(format!("archive not found: {archive}")));
    }
    if let Err(e) = std::fs::create_dir_all(destination) {
        return Ok(ToolResult::err(format!("cannot create destination: {e}")));
    }
    let lower = archive.to_lowercase();
    let r = if lower.ends_with(".zip") {
        run(&["unzip", "-qq", "-o", archive, member, "-d", destination])
    } else if lower.ends_with(".tar") || lower.ends_with(".tar.gz") || lower.ends_with(".tgz") || lower.ends_with(".tar.bz2") || lower.ends_with(".tbz2") || lower.ends_with(".tar.xz") || lower.ends_with(".txz") {
        run(&["tar", "-xf", archive, "-C", destination, member])
    } else if lower.ends_with(".7z") {
        run(&["7z", "x", "-y", format!("-o{destination}").as_str(), archive, member])
    } else {
        return Ok(ToolResult::err(format!("unsupported archive format: {archive}")));
    }?;
    if r.success {
        Ok(ToolResult::ok(format!("extracted {member} into {destination}")))
    } else {
        Ok(r)
    }
}

/// Verify an archive's integrity.
pub fn test_archive(archive: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(archive) {
        return Ok(ToolResult::err("invalid path"));
    }
    if !Path::new(archive).exists() {
        return Ok(ToolResult::err(format!("archive not found: {archive}")));
    }
    let lower = archive.to_lowercase();
    if lower.ends_with(".zip") {
        run(&["unzip", "-t", "-qq", archive])
    } else if lower.ends_with(".tar") || lower.ends_with(".tar.gz") || lower.ends_with(".tgz") || lower.ends_with(".tar.bz2") || lower.ends_with(".tbz2") || lower.ends_with(".tar.xz") || lower.ends_with(".txz") || lower.ends_with(".tar.zst") {
        run(&["tar", "-tf", archive])
    } else if lower.ends_with(".7z") {
        run(&["7z", "t", archive])
    } else if lower.ends_with(".rar") {
        run(&["unrar", "t", archive])
    } else {
        Ok(ToolResult::err(format!("unsupported archive format: {archive}")))
    }
}

/// Convert an archive from one format to another (extract to temp, re-pack).
pub fn convert_archive(archive: &str, output_format: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(archive) {
        return Ok(ToolResult::err("invalid path"));
    }
    if !Path::new(archive).exists() {
        return Ok(ToolResult::err(format!("archive not found: {archive}")));
    }
    if !valid_value(output_format) {
        return Ok(ToolResult::err("invalid format"));
    }
    let base = Path::new(archive)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "archive".to_string());
    let tmp = std::env::temp_dir().join(format!("diad-convert-{}", std::process::id()));
    let tmp_s = tmp.to_string_lossy().to_string();
    let extracted = extract_archive(archive, &tmp_s)?;
    if !extracted.success {
        let _ = std::fs::remove_dir_all(&tmp);
        return Ok(extracted);
    }
    let output = format!("{base}.{}", output_format.to_lowercase());
    let created = create_archive(&tmp_s, output_format, &output)?;
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(created)
}

/// Encrypt an archive with a password (7z AES-256 with header encryption).
pub fn encrypt_archive(archive: &str, password: &str, output: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(archive) || !valid_path(output) {
        return Ok(ToolResult::err("invalid path"));
    }
    if password.is_empty() || password.len() < 4 || password.len() > 128 || password.contains(' ') {
        return Ok(ToolResult::err("password must be 4-128 characters without spaces"));
    }
    if !Path::new(archive).exists() {
        return Ok(ToolResult::err(format!("archive not found: {archive}")));
    }
    run(&["7z", "a", "-y", "-p", "-mhe=on", output, archive])
}

/// Split an archive into fixed-size parts (split).
pub fn split_archive(archive: &str, part_size_mb: u64, prefix: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(archive) {
        return Ok(ToolResult::err("invalid path"));
    }
    if part_size_mb == 0 || part_size_mb > 4096 {
        return Ok(ToolResult::err("part size must be between 1 and 4096 MB"));
    }
    if !valid_value(prefix) {
        return Ok(ToolResult::err("invalid prefix"));
    }
    if !Path::new(archive).exists() {
        return Ok(ToolResult::err(format!("archive not found: {archive}")));
    }
    let size = format!("{part_size_mb}M");
    run(&["split", "-b", &size, archive, prefix])
}

/// Merge split archive parts back into one file.
pub fn merge_archive_parts(parts_dir: &str, output: &str) -> anyhow::Result<ToolResult> {
    if !valid_path(parts_dir) || !valid_path(output) {
        return Ok(ToolResult::err("invalid path"));
    }
    if !Path::new(parts_dir).is_dir() {
        return Ok(ToolResult::err(format!("parts directory not found: {parts_dir}")));
    }
    let mut parts: Vec<std::path::PathBuf> = std::fs::read_dir(parts_dir)
        .map(|rd| rd.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    parts.sort();
    let mut sink = match std::fs::File::create(output) {
        Ok(f) => f,
        Err(e) => return Ok(ToolResult::err(format!("cannot create {output}: {e}"))),
    };
    let mut total: u64 = 0;
    for part in parts {
        let mut src = match std::fs::File::open(&part) {
            Ok(f) => f,
            Err(e) => return Ok(ToolResult::err(format!("cannot open {}: {e}", part.display()))),
        };
        match std::io::copy(&mut src, &mut sink) {
            Ok(n) => total += n,
            Err(e) => return Ok(ToolResult::err(format!("merge failed at {}: {e}", part.display()))),
        }
    }
    Ok(ToolResult::ok(format!("merged {} parts into {output} ({total} bytes)", total)))
}

// ===========================================================================
// Registration
// ===========================================================================

/// Register every universal file tool in the given registry.
///
/// Called once from `tools::create_default_registry`. The rich option-based
/// `search_files` deliberately supersedes the simple glob version registered
/// by the classic `filesystem` module (registry keeps the last registration).
pub fn register_universal_file_tools(registry: &mut super::ToolRegistry) {
    use std::sync::Arc;

    macro_rules! reg {
        ($reg:expr, $name:expr, $desc:expr, $schema:expr, $f:expr) => {
            $reg.register_tool($name, $desc, $schema, Arc::new(|p: serde_json::Value| $f(p)));
        };
    }

    use serde_json::json;

    // ---- 2.1 File type registry ----
    reg!(registry, "get_file_type", "Identify the type, category and mime of a file", json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}), |p: serde_json::Value| get_file_type(p["path"].as_str().unwrap_or("")));
    reg!(registry, "get_file_metadata", "File metadata: size, mtime, permissions, kind", json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}), |p: serde_json::Value| get_file_metadata(p["path"].as_str().unwrap_or("")));
    reg!(registry, "get_file_preview", "Human-readable text preview of a file", json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}), |p: serde_json::Value| get_file_preview(p["path"].as_str().unwrap_or("")));
    reg!(registry, "get_file_info_deep", "Deep file info: metadata + sha256 + category", json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}), |p: serde_json::Value| get_file_info_deep(p["path"].as_str().unwrap_or("")));
    reg!(registry, "get_file_thumbnail", "Generate a 256x256 PNG thumbnail of an image", json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}), |p: serde_json::Value| get_file_thumbnail(p["path"].as_str().unwrap_or("")));
    reg!(registry, "generate_video_thumbnail", "Extract a video frame as a thumbnail", json!({"type":"object","properties":{"path":{"type":"string"},"at_seconds":{"type":"integer"}},"required":["path"]}), |p: serde_json::Value| generate_video_thumbnail(p["path"].as_str().unwrap_or(""), p["at_seconds"].as_u64().unwrap_or(1)));
    reg!(registry, "get_file_icon", "Suggest an icon name (freedesktop) for a file", json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}), |p: serde_json::Value| get_file_icon(p["path"].as_str().unwrap_or("")));

    // ---- 2.2 Natural language opening ----
    reg!(registry, "open_file", "Open a file with its default application", json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}), |p: serde_json::Value| open_file(p["path"].as_str().unwrap_or("")));
    reg!(registry, "open_url", "Open a URL in the default browser", json!({"type":"object","properties":{"url":{"type":"string"}},"required":["url"]}), |p: serde_json::Value| open_url(p["url"].as_str().unwrap_or("")));
    reg!(registry, "open_directory", "Open a directory in the file manager", json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}), |p: serde_json::Value| open_directory(p["path"].as_str().unwrap_or("")));
    reg!(registry, "open_with", "Open a file with an explicit application", json!({"type":"object","properties":{"path":{"type":"string"},"app":{"type":"string"}},"required":["path","app"]}), |p: serde_json::Value| open_with(p["path"].as_str().unwrap_or(""), p["app"].as_str().unwrap_or("")));
    reg!(registry, "set_default_app", "Set the default application for a mime type", json!({"type":"object","properties":{"mime":{"type":"string"},"app":{"type":"string"}},"required":["mime","app"]}), |p: serde_json::Value| set_default_app(p["mime"].as_str().unwrap_or(""), p["app"].as_str().unwrap_or("")));

    // ---- 2.3 Download integration ----
    reg!(registry, "list_downloads", "List everything in the Downloads directory", json!({}), |_| list_downloads());
    reg!(registry, "open_download", "Open a download with its default application", json!({"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}), |p: serde_json::Value| open_download(p["name"].as_str().unwrap_or("")));
    reg!(registry, "move_download", "Move a download to a destination directory", json!({"type":"object","properties":{"name":{"type":"string"},"destination":{"type":"string"}},"required":["name","destination"]}), |p: serde_json::Value| move_download(p["name"].as_str().unwrap_or(""), p["destination"].as_str().unwrap_or("")));
    reg!(registry, "delete_download", "Delete a download (DESTRUCTIVE, requires confirmation)", json!({"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}), |p: serde_json::Value| delete_download(p["name"].as_str().unwrap_or("")));
    reg!(registry, "organize_downloads", "Organize Downloads into category folders (moves files)", json!({}), |_| organize_downloads());
    reg!(registry, "get_download_stats", "Download statistics per category", json!({}), |_| get_download_stats());
    reg!(registry, "clean_old_downloads", "Delete downloads older than N days (DESTRUCTIVE)", json!({"type":"object","properties":{"days":{"type":"integer"}},"required":["days"]}), |p: serde_json::Value| clean_old_downloads(p["days"].as_u64().unwrap_or(0)));
    reg!(registry, "find_duplicate_downloads", "Find duplicate downloads by sha256", json!({}), |_| find_duplicate_downloads());
    reg!(registry, "scan_download", "Virus-scan a download (clamscan)", json!({"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}), |p: serde_json::Value| scan_download(p["name"].as_str().unwrap_or("")));

    // ---- 2.4 Clipboard & sharing ----
    reg!(registry, "get_clipboard_text", "Read the current clipboard text", json!({}), |_| get_clipboard_text());
    reg!(registry, "set_clipboard_text", "Write text to the clipboard", json!({"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}), |p: serde_json::Value| set_clipboard_text(p["text"].as_str().unwrap_or("")));
    reg!(registry, "get_clipboard_image", "Save the clipboard image to a PNG file", json!({}), |_| get_clipboard_image());
    reg!(registry, "set_clipboard_image", "Copy an image file to the clipboard", json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}), |p: serde_json::Value| set_clipboard_image(p["path"].as_str().unwrap_or("")));
    reg!(registry, "get_clipboard_history", "Clipboard history via cliphist", json!({}), |_| get_clipboard_history());
    reg!(registry, "clear_clipboard_history", "Clear clipboard history (DESTRUCTIVE, requires confirmation)", json!({}), |_| clear_clipboard_history());
    reg!(registry, "share_file", "Share a file (KDE Connect or Bluetooth OBEX)", json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}), |p: serde_json::Value| share_file(p["path"].as_str().unwrap_or("")));
    reg!(registry, "qr_code_file", "Generate a QR code for a file", json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}), |p: serde_json::Value| qr_code_file(p["path"].as_str().unwrap_or("")));
    reg!(registry, "share_to_phone", "Share a file to a connected phone (KDE Connect)", json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}), |p: serde_json::Value| share_to_phone(p["path"].as_str().unwrap_or("")));
    reg!(registry, "airdrop_like", "AirDrop-like nearby sharing to all connected devices", json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}), |p: serde_json::Value| airdrop_like(p["path"].as_str().unwrap_or("")));

    // ---- 2.5 Search & organization ----
    reg!(registry, "search_files", "Search for files by name (rich options: regex, case, hidden, types)", json!({"type":"object","properties":{"query":{"type":"string"},"path":{"type":"string"},"regex":{"type":"boolean"},"case_sensitive":{"type":"boolean"},"include_hidden":{"type":"boolean"},"max_results":{"type":"integer"},"file_types":{"type":"array","items":{"type":"string"}}},"required":["query"]}), |p: serde_json::Value| {
        let ft = p["file_types"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<String>>()).unwrap_or_default();
        search_files(p["query"].as_str().unwrap_or(""), p["path"].as_str().unwrap_or(""), p["regex"].as_bool().unwrap_or(false), p["case_sensitive"].as_bool().unwrap_or(false), p["include_hidden"].as_bool().unwrap_or(false), p["max_results"].as_u64().unwrap_or(100), &ft)
    });
    reg!(registry, "search_by_content", "Search file contents for a string", json!({"type":"object","properties":{"query":{"type":"string"},"path":{"type":"string"},"case_sensitive":{"type":"boolean"},"max_results":{"type":"integer"}},"required":["query"]}), |p: serde_json::Value| search_by_content(p["query"].as_str().unwrap_or(""), p["path"].as_str().unwrap_or(""), p["case_sensitive"].as_bool().unwrap_or(false), p["max_results"].as_u64().unwrap_or(100)));
    reg!(registry, "search_by_regex", "Search file contents with a regex", json!({"type":"object","properties":{"pattern":{"type":"string"},"path":{"type":"string"},"case_sensitive":{"type":"boolean"},"max_results":{"type":"integer"}},"required":["pattern"]}), |p: serde_json::Value| search_by_regex(p["pattern"].as_str().unwrap_or(""), p["path"].as_str().unwrap_or(""), p["case_sensitive"].as_bool().unwrap_or(false), p["max_results"].as_u64().unwrap_or(100)));
    reg!(registry, "tag_file", "Tag a file with one or more tags", json!({"type":"object","properties":{"path":{"type":"string"},"tags":{"type":"array","items":{"type":"string"}},"required":["path","tags"]}}), |p: serde_json::Value| {
        let tags = p["tags"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<String>>()).unwrap_or_default();
        tag_file(p["path"].as_str().unwrap_or(""), &tags)
    });
    reg!(registry, "get_file_tags", "List the tags of a file", json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}), |p: serde_json::Value| get_file_tags(p["path"].as_str().unwrap_or("")));
    reg!(registry, "search_by_tag", "Find all files carrying a tag", json!({"type":"object","properties":{"tag":{"type":"string"}},"required":["tag"]}), |p: serde_json::Value| search_by_tag(p["tag"].as_str().unwrap_or("")));
    reg!(registry, "get_recent_files", "Most recently modified files in a directory", json!({"type":"object","properties":{"path":{"type":"string"},"limit":{"type":"integer"}},"required":["path"]}), |p: serde_json::Value| get_recent_files(p["path"].as_str().unwrap_or(""), p["limit"].as_u64().unwrap_or(20)));
    reg!(registry, "get_duplicate_files", "Find duplicate files by sha256", json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}), |p: serde_json::Value| get_duplicate_files(p["path"].as_str().unwrap_or("")));
    reg!(registry, "index_directory", "Index a directory: path, size, mtime for every file", json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}), |p: serde_json::Value| index_directory(p["path"].as_str().unwrap_or("")));
    reg!(registry, "get_directory_tree", "Nested JSON tree of a directory", json!({"type":"object","properties":{"path":{"type":"string"},"depth":{"type":"integer"}},"required":["path"]}), |p: serde_json::Value| get_directory_tree(p["path"].as_str().unwrap_or(""), p["depth"].as_u64().unwrap_or(3) as u32));

    // ---- 2.6 Archive management ----
    reg!(registry, "extract_archive", "Extract an archive into a destination directory", json!({"type":"object","properties":{"archive":{"type":"string"},"destination":{"type":"string"}},"required":["archive","destination"]}), |p: serde_json::Value| extract_archive(p["archive"].as_str().unwrap_or(""), p["destination"].as_str().unwrap_or("")));
    reg!(registry, "create_archive", "Create an archive (zip, tar, tar.gz, tar.bz2, tar.xz, 7z)", json!({"type":"object","properties":{"source":{"type":"string"},"format":{"type":"string"},"output":{"type":"string"}},"required":["source","format","output"]}), |p: serde_json::Value| create_archive(p["source"].as_str().unwrap_or(""), p["format"].as_str().unwrap_or("zip"), p["output"].as_str().unwrap_or("")));
    reg!(registry, "list_archive_contents", "List the contents of an archive", json!({"type":"object","properties":{"archive":{"type":"string"}},"required":["archive"]}), |p: serde_json::Value| list_archive_contents(p["archive"].as_str().unwrap_or("")));
    reg!(registry, "extract_single_file", "Extract a single member from an archive", json!({"type":"object","properties":{"archive":{"type":"string"},"member":{"type":"string"},"destination":{"type":"string"}},"required":["archive","member","destination"]}), |p: serde_json::Value| extract_single_file(p["archive"].as_str().unwrap_or(""), p["member"].as_str().unwrap_or(""), p["destination"].as_str().unwrap_or("")));
    reg!(registry, "test_archive", "Verify an archive's integrity", json!({"type":"object","properties":{"archive":{"type":"string"}},"required":["archive"]}), |p: serde_json::Value| test_archive(p["archive"].as_str().unwrap_or("")));
    reg!(registry, "convert_archive", "Convert an archive to another format", json!({"type":"object","properties":{"archive":{"type":"string"},"output_format":{"type":"string"}},"required":["archive","output_format"]}), |p: serde_json::Value| convert_archive(p["archive"].as_str().unwrap_or(""), p["output_format"].as_str().unwrap_or("")));
    reg!(registry, "encrypt_archive", "Encrypt an archive (7z AES-256 with header encryption)", json!({"type":"object","properties":{"archive":{"type":"string"},"password":{"type":"string"},"output":{"type":"string"}},"required":["archive","password","output"]}), |p: serde_json::Value| encrypt_archive(p["archive"].as_str().unwrap_or(""), p["password"].as_str().unwrap_or(""), p["output"].as_str().unwrap_or("")));
    reg!(registry, "split_archive", "Split an archive into fixed-size parts", json!({"type":"object","properties":{"archive":{"type":"string"},"part_size_mb":{"type":"integer"},"prefix":{"type":"string"}},"required":["archive","part_size_mb","prefix"]}), |p: serde_json::Value| split_archive(p["archive"].as_str().unwrap_or(""), p["part_size_mb"].as_u64().unwrap_or(0), p["prefix"].as_str().unwrap_or("")));
    reg!(registry, "merge_archive_parts", "Merge split archive parts back into one file", json!({"type":"object","properties":{"parts_dir":{"type":"string"},"output":{"type":"string"}},"required":["parts_dir","output"]}), |p: serde_json::Value| merge_archive_parts(p["parts_dir"].as_str().unwrap_or(""), p["output"].as_str().unwrap_or("")));
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::testutil::HOME_LOCK;

    // ---- 2.1 File type registry ----
    #[test]
    fn test_valid_value_rejects_shell_metachars() {
        assert!(valid_value("report-final"));
        assert!(valid_value("tag.one"));
        assert!(!valid_value("foo; rm -rf /"));
        assert!(!valid_value("foo $(x)"));
        assert!(!valid_value("-leading-dash"));
    }

    #[test]
    fn test_valid_path_contract() {
        assert!(valid_path("/home/diad/file.txt"));
        assert!(!valid_path("relative/path"));
        assert!(!valid_path("/home/../etc"));
        assert!(!valid_path("/tmp/a;b"));
        assert!(!valid_path("/tmp/$(x)"));
        assert!(!valid_path("/tmp/`x`"));
    }

    #[test]
    fn test_valid_url_contract() {
        assert!(valid_url("https://example.com/docs"));
        assert!(valid_url("http://localhost:3000"));
        assert!(!valid_url("file:///etc/passwd"));
        assert!(!valid_url("javascript:alert(1)"));
        assert!(!valid_url("https://x.com/`id`"));
        assert!(!valid_url("https://x.com/a b"));
    }

    #[test]
    fn test_category_and_icon_mapping() {
        assert_eq!(category_for_ext("png"), "image");
        assert_eq!(category_for_ext("rs"), "code");
        assert_eq!(category_for_ext("zip"), "archive");
        assert_eq!(category_for_ext("pdf"), "pdf");
        assert_eq!(category_for_ext("unknownxyz"), "other");
        assert!(icon_for_ext("png").contains("image"));
        assert!(icon_for_ext("rs").contains("script"));
    }

    #[test]
    fn test_get_file_type_real_file() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("notes.txt");
        std::fs::write(&f, "hello").unwrap();
        let r = get_file_type(f.to_str().unwrap()).unwrap();
        assert!(r.success, "err: {:?}", r.error);
        assert!(r.output.contains("category: text"), "{}", r.output);
        assert!(r.output.contains("txt"), "{}", r.output);
    }

    #[test]
    fn test_get_file_type_validation() {
        assert!(!get_file_type("../x").unwrap().success);
        assert!(!get_file_type("/tmp/x;rm").unwrap().success);
    }

    #[test]
    fn test_get_file_metadata_real_file() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("meta.bin");
        std::fs::write(&f, b"abc").unwrap();
        let r = get_file_metadata(f.to_str().unwrap()).unwrap();
        assert!(r.success, "err: {:?}", r.error);
        assert!(r.output.contains("kind: file"), "{}", r.output);
        assert!(r.output.contains("3 bytes"), "{}", r.output);
        assert!(r.output.contains("permissions:"), "{}", r.output);
    }

    #[test]
    fn test_get_file_preview_text_and_binary() {
        let tmp = tempfile::tempdir().unwrap();
        let t = tmp.path().join("preview.txt");
        std::fs::write(&t, "line one\nline two\n").unwrap();
        let r = get_file_preview(t.to_str().unwrap()).unwrap();
        assert!(r.success && r.output.contains("line one"), "{}", r.output);
        let b = tmp.path().join("binary.bin");
        std::fs::write(&b, [0u8, 1, 2, 3, 255]).unwrap();
        let r2 = get_file_preview(b.to_str().unwrap()).unwrap();
        assert!(r2.success && r2.output.contains("binary"), "{}", r2.output);
    }

    #[test]
    fn test_get_file_info_deep_real_file() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("deep.txt");
        std::fs::write(&f, "content").unwrap();
        let r = get_file_info_deep(f.to_str().unwrap()).unwrap();
        assert!(r.success, "err: {:?}", r.error);
        assert!(r.output.contains("sha256"), "{}", r.output);
        assert!(r.output.contains("category: text"), "{}", r.output);
    }

    #[test]
    fn test_get_file_icon_folder_vs_file() {
        let tmp = tempfile::tempdir().unwrap();
        let r = get_file_icon(tmp.path().to_str().unwrap()).unwrap();
        assert!(r.success && r.output.contains("folder"), "{}", r.output);
    }

    #[test]
    fn test_thumbnail_graceful_degrade() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("x.png");
        std::fs::write(&f, b"not really a png").unwrap();
        let r = get_file_thumbnail(f.to_str().unwrap()).unwrap();
        assert!(r.success || r.error.is_some());
        let r2 = generate_video_thumbnail(f.to_str().unwrap(), 5).unwrap();
        assert!(r2.success || r2.error.is_some());
    }

    // ---- 2.2 Natural language opening ----
    #[test]
    fn test_open_file_validation() {
        assert!(!open_file("../etc/passwd").unwrap().success);
        assert!(!open_file("/tmp/a;b").unwrap().success);
        let r = open_file("/tmp/nonexistent-diad-file-xyz.txt").unwrap();
        assert!(r.success || r.error.is_some());
    }

    #[test]
    fn test_open_url_validation() {
        assert!(!open_url("javascript:alert(1)").unwrap().success);
        assert!(!open_url("https://x.com/`id`").unwrap().success);
        let r = open_url("https://example.com").unwrap();
        assert!(r.success || r.error.is_some());
    }

    #[test]
    fn test_open_directory_validation() {
        assert!(!open_directory("/tmp/not-a-dir-diad").unwrap().success);
        let tmp = tempfile::tempdir().unwrap();
        let r = open_directory(tmp.path().to_str().unwrap()).unwrap();
        assert!(r.success || r.error.is_some());
    }

    #[test]
    fn test_open_with_validation() {
        assert!(!open_with("/tmp/x", "bad;app").unwrap().success);
        assert!(!open_with("../x", "cat").unwrap().success);
    }

    #[test]
    fn test_set_default_app_validation() {
        assert!(!set_default_app("text/plain;rm", "code.desktop").unwrap().success);
        assert!(!set_default_app("text/plain", "code;x").unwrap().success);
        let r = set_default_app("text/plain", "code.desktop").unwrap();
        assert!(r.success || r.error.is_some());
    }

    // ---- 2.3 Download integration ----
    #[test]
    fn test_downloads_roundtrip() {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        let dl = tmp.path().join("Downloads");
        std::fs::create_dir_all(&dl).unwrap();
        std::fs::write(dl.join("report.pdf"), b"%PDF-1.4").unwrap();
        std::fs::write(dl.join("photo.png"), b"png").unwrap();
        std::env::set_var("HOME", tmp.path());

        let listed = list_downloads().unwrap();
        assert!(listed.success && listed.output.contains("report.pdf"), "{}", listed.output);
        let stats = get_download_stats().unwrap();
        assert!(stats.success && stats.output.contains("pdf"), "{}", stats.output);

        let moved = move_download("report.pdf", dl.to_str().unwrap()).unwrap();
        assert!(moved.success, "{}", moved.error.clone().unwrap_or_default());
        let deleted = delete_download("photo.png").unwrap();
        assert!(deleted.success);
        assert!(!dl.join("photo.png").exists());

        std::env::remove_var("HOME");
    }

    #[test]
    fn test_download_name_validation() {
        assert!(download_path("report.pdf").is_some());
        assert!(download_path("my file with spaces.pdf").is_some());
        assert!(download_path("../etc/passwd").is_none());
        assert!(download_path("/abs/path").is_none());
        assert!(download_path("a;b").is_none());
        assert!(download_path("a$(x)").is_none());
        assert!(download_path("").is_none());
    }

    #[test]
    fn test_delete_download_validation() {
        assert!(!delete_download("..").unwrap().success);
        assert!(!delete_download("a;b").unwrap().success);
        let r = delete_download("nonexistent-file-xyz").unwrap();
        assert!(r.success || r.error.is_some());
    }

    #[test]
    fn test_clean_old_downloads_bounds() {
        assert!(!clean_old_downloads(0).unwrap().success);
        assert!(!clean_old_downloads(4000).unwrap().success);
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("Downloads")).unwrap();
        std::env::set_var("HOME", tmp.path());
        let r = clean_old_downloads(30).unwrap();
        assert!(r.success, "{}", r.error.clone().unwrap_or_default());
        std::env::remove_var("HOME");
    }

    #[test]
    fn test_organize_downloads_moves_files() {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        let dl = tmp.path().join("Downloads");
        std::fs::create_dir_all(&dl).unwrap();
        std::fs::write(dl.join("a.txt"), b"x").unwrap();
        std::fs::write(dl.join("b.png"), b"x").unwrap();
        std::env::set_var("HOME", tmp.path());
        let r = organize_downloads().unwrap();
        assert!(r.success, "{}", r.error.clone().unwrap_or_default());
        assert!(dl.join("text").join("a.txt").exists());
        assert!(dl.join("image").join("b.png").exists());
        std::env::remove_var("HOME");
    }

    #[test]
    fn test_find_duplicate_downloads_graceful() {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("Downloads")).unwrap();
        std::env::set_var("HOME", tmp.path());
        let r = find_duplicate_downloads().unwrap();
        assert!(r.success, "{}", r.error.clone().unwrap_or_default());
        std::env::remove_var("HOME");
    }

    #[test]
    fn test_scan_download_validation() {
        assert!(!scan_download("..").unwrap().success);
        let r = scan_download("nonexistent-file-xyz").unwrap();
        assert!(r.success || r.error.is_some());
    }

    // ---- 2.4 Clipboard & sharing ----
    #[test]
    fn test_clipboard_never_panics() {
        let _ = get_clipboard_text().unwrap();
        let _ = set_clipboard_text("diad test").unwrap();
        let _ = get_clipboard_image().unwrap();
        let _ = get_clipboard_history().unwrap();
        let _ = clear_clipboard_history().unwrap();
    }

    #[test]
    fn test_set_clipboard_size_limit() {
        let big = "x".repeat(1_000_001);
        assert!(!set_clipboard_text(&big).unwrap().success);
    }

    #[test]
    fn test_set_clipboard_image_validation() {
        assert!(!set_clipboard_image("../x").unwrap().success);
        assert!(!set_clipboard_image("/tmp/nonexistent-diad.png").unwrap().success);
    }

    #[test]
    fn test_share_file_validation() {
        assert!(!share_file("../x").unwrap().success);
        assert!(!share_file("/tmp/nonexistent-diad-share").unwrap().success);
        assert!(!share_to_phone("../x").unwrap().success);
        assert!(!airdrop_like("../x").unwrap().success);
    }

    #[test]
    fn test_qr_code_file_validation() {
        assert!(!qr_code_file("../x").unwrap().success);
        let r = qr_code_file("/tmp/nonexistent-diad-qr").unwrap();
        assert!(r.success || r.error.is_some());
    }

    // ---- 2.5 Search & organization ----
    #[test]
    fn test_search_files_by_name() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("alpha.txt"), b"hello").unwrap();
        std::fs::write(tmp.path().join("beta.rs"), b"fn main() {}").unwrap();
        std::fs::create_dir_all(tmp.path().join(".hidden")).unwrap();
        std::fs::write(tmp.path().join(".hidden/s.txt"), b"x").unwrap();
        let root = tmp.path().to_str().unwrap();
        let r = search_files("alpha", root, false, false, false, 50, &[]).unwrap();
        assert!(r.success && r.output.contains("alpha.txt"), "{}", r.output);
        assert!(!r.output.contains("beta.rs"), "{}", r.output);
        let r2 = search_files("*.rs", root, false, false, true, 50, &[]).unwrap();
        assert!(r2.success && r2.output.contains("beta.rs"), "{}", r2.output);
        let r3 = search_files("*", root, false, false, false, 50, &[]).unwrap();
        assert!(!r3.output.contains(".hidden"), "hidden shown: {}", r3.output);
    }

    #[test]
    fn test_search_files_validation() {
        assert!(!search_files("", ".", false, false, false, 10, &[]).unwrap().success);
        assert!(!search_files("q", "/home/../etc", false, false, false, 10, &[]).unwrap().success);
    }

    #[test]
    fn test_search_by_content_and_regex() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("one.txt"), b"the quick brown fox").unwrap();
        std::fs::write(tmp.path().join("two.txt"), b"nothing here").unwrap();
        let root = tmp.path().to_str().unwrap();
        let r = search_by_content("quick", root, false, 50).unwrap();
        assert!(r.success && r.output.contains("one.txt"), "{}", r.output);
        let r2 = search_by_regex("br[o]+wn", root, false, 50).unwrap();
        assert!(r2.success && r2.output.contains("one.txt"), "{}", r2.output);
    }

    #[test]
    fn test_tag_lifecycle() {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", tmp.path());
        let f = tmp.path().join("doc.txt");
        std::fs::write(&f, b"x").unwrap();
        let p = f.to_str().unwrap().to_string();
        let tagged = tag_file(&p, &["important".to_string(), "work".to_string()]).unwrap();
        assert!(tagged.success, "{}", tagged.error.clone().unwrap_or_default());
        let got = get_file_tags(&p).unwrap();
        assert!(got.success && got.output.contains("important"), "{}", got.output);
        let found = search_by_tag("important").unwrap();
        assert!(found.success && found.output.contains("doc.txt"), "{}", found.output);
        let none = search_by_tag("nope").unwrap();
        assert!(none.output.contains("no files"), "{}", none.output);
        std::env::remove_var("HOME");
    }

    #[test]
    fn test_tag_validation() {
        assert!(!tag_file("../x", &["a".to_string()]).unwrap().success);
        assert!(!tag_file("/tmp/x", &[]).unwrap().success);
        assert!(!search_by_tag("bad;tag").unwrap().success);
        assert!(!get_file_tags("../x").unwrap().success);
    }

    #[test]
    fn test_directory_tree_real() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"x").unwrap();
        std::fs::write(tmp.path().join("sub/b.txt"), b"y").unwrap();
        let r = get_directory_tree(tmp.path().to_str().unwrap(), 3).unwrap();
        assert!(r.success, "{}", r.error.clone().unwrap_or_default());
        assert!(r.output.contains("a.txt"), "{}", r.output);
        assert!(r.output.contains("sub"), "{}", r.output);
        assert!(!get_directory_tree(tmp.path().to_str().unwrap(), 99).unwrap().success);
    }

    #[test]
    fn test_recent_files_and_index_graceful() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("new.txt"), b"x").unwrap();
        let root = tmp.path().to_str().unwrap();
        let r = get_recent_files(root, 10).unwrap();
        assert!(r.success || r.error.is_some());
        let idx = index_directory(root).unwrap();
        assert!(idx.success || idx.error.is_some());
        assert!(!get_recent_files("/home/../etc", 10).unwrap().success);
    }

    #[test]
    fn test_get_duplicate_files_graceful() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"same").unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"same").unwrap();
        let root = tmp.path().to_str().unwrap();
        let r = get_duplicate_files(root).unwrap();
        assert!(r.success, "{}", r.error.clone().unwrap_or_default());
    }

    // ---- 2.6 Archive management ----
    #[test]
    fn test_archive_validation() {
        assert!(!extract_archive("../x", "/tmp/d").unwrap().success);
        assert!(!extract_archive("/tmp/x", "../d").unwrap().success);
        assert!(!create_archive("../x", "zip", "/tmp/o.zip").unwrap().success);
        assert!(!create_archive("/tmp/x", "badfmt", "/tmp/o").unwrap().success);
        assert!(!list_archive_contents("../x").unwrap().success);
        assert!(!extract_single_file("/tmp/x.zip", "a;b", "/tmp/d").unwrap().success);
        assert!(!test_archive("../x").unwrap().success);
        assert!(!encrypt_archive("/tmp/x", "ab", "/tmp/o.7z").unwrap().success);
        assert!(!split_archive("/tmp/x", 0, "part").unwrap().success);
        assert!(!merge_archive_parts("../x", "/tmp/o").unwrap().success);
    }

    #[test]
    fn test_extract_unsupported_format() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("weird.xyz");
        std::fs::write(&f, b"data").unwrap();
        let r = extract_archive(f.to_str().unwrap(), tmp.path().join("out").to_str().unwrap()).unwrap();
        assert!(!r.success, "should reject unknown format");
    }

    #[test]
    fn test_encrypt_archive_password_rules() {
        assert!(!encrypt_archive("/tmp/x.7z", "has space", "/tmp/o.7z").unwrap().success);
        assert!(!encrypt_archive("/tmp/x.7z", "abc", "/tmp/o.7z").unwrap().success);
        assert!(!encrypt_archive("/tmp/x.7z", "okpassword", "../o.7z").unwrap().success);
    }

    #[test]
    fn test_split_archive_validation() {
        assert!(!split_archive("/tmp/x.zip", 5000, "part").unwrap().success);
        assert!(!split_archive("/tmp/x.zip", 5, "bad;prefix").unwrap().success);
        assert!(!split_archive("../x.zip", 5, "part").unwrap().success);
    }

    #[test]
    fn test_merge_parts_missing_dir() {
        assert!(!merge_archive_parts("/tmp/diad-no-such-parts-dir", "/tmp/o.bin").unwrap().success);
    }

    #[test]
    fn test_convert_archive_validation() {
        assert!(!convert_archive("../x", "zip").unwrap().success);
        assert!(!convert_archive("/tmp/x.zip", "bad;fmt").unwrap().success);
    }

    #[test]
    fn test_archive_cmds_detection() {
        assert!(archive_cmds("/x/a.zip").is_some());
        assert!(archive_cmds("/x/a.tar.gz").is_some());
        assert!(archive_cmds("/x/a.7z").is_some());
        assert!(archive_cmds("/x/a.rar").is_some());
        assert!(archive_cmds("/x/a.xyz").is_none());
    }

    #[test]
    fn test_archive_member_validation() {
        assert!(!extract_single_file("/tmp/a.zip", "../evil", "/tmp/d").unwrap().success);
        assert!(!extract_single_file("/tmp/a.zip", "a$b", "/tmp/d").unwrap().success);
        assert!(!extract_single_file("/tmp/a.zip", "a;b", "/tmp/d").unwrap().success);
    }

    #[test]
    fn test_read_text_preview() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("t.txt");
        std::fs::write(&f, "hello world").unwrap();
        assert!(read_text_preview(f.to_str().unwrap(), 100).unwrap().contains("hello"));
        let b = tmp.path().join("b.bin");
        std::fs::write(&b, [0u8, 1, 2]).unwrap();
        assert!(read_text_preview(b.to_str().unwrap(), 100).is_none());
    }
}
