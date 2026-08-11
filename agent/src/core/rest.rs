//! DIAD AGENT REST API (`/api/v1/*`).
//!
//! A thin, synchronous REST complement to the WebSocket protocol. Where the
//! WS channel is bidirectional and stream-oriented (tokens, tool prompts,
//! terminal output), REST is request/response: fire a request, get a JSON
//! document back.
//!
//! Convention notes:
//! - Every response is JSON; errors use `{"error": true, "message": ...}`
//!   with the appropriate HTTP status code.
//! - File endpoints enforce the same sandbox allowlist as the WS tools
//!   (`DaemonState.registry.is_path_allowed`).
//! - Destructive actions (git commit, file write outside the workspace)
//!   never run blindly; they return the plan and require an explicit
//!   `confirm: true` flag in the request body.
//! - REST chat is non-interactive: tool confirmation prompts are
//!   auto-rejected and secret prompts auto-cancelled, so the loop always
//!   terminates. Use the WebSocket channel for interactive flows.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::mpsc::channel;

use super::daemon::{handle_chat_send, next_id, Conn, DaemonState, PromptBroker};
use super::protocol::{FileEntryDto, ServerMsg, PROTOCOL_VERSION};

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub text: String,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub id: String,
    pub text: String,
    pub done: bool,
    pub timed_out: bool,
    pub messages: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProviderSwitchRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct FileReadQuery {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct FileWriteRequest {
    pub path: String,
    pub content: String,
    #[serde(default)]
    pub confirm: bool,
}

#[derive(Debug, Deserialize)]
pub struct TerminalInputRequest {
    pub data: String,
}

#[derive(Debug, Deserialize)]
pub struct TerminalResizeRequest {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Deserialize)]
pub struct GitStatusQuery {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct GitDiffQuery {
    pub path: String,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub staged: bool,
}

#[derive(Debug, Deserialize)]
pub struct GitLogQuery {
    pub path: String,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct GitCommitRequest {
    pub path: String,
    pub message: String,
    #[serde(default)]
    pub confirm: bool,
}

#[derive(Debug, Deserialize)]
pub struct GitBranchQuery {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct DebugStartRequest {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub working_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DebugSessionRequest {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub working_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DebugBreakpointRequest {
    pub session: String,
    pub file: String,
    pub line: u32,
}

#[derive(Debug, Deserialize)]
pub struct DebugDeleteBreakpointRequest {
    pub session: String,
    pub number: u32,
}

#[derive(Debug, Deserialize)]
pub struct DebugSessionId {
    pub session: String,
}

#[derive(Debug, Deserialize)]
pub struct ProjectListQuery {
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct RecallQuery {
    pub q: String,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct SessionExportQuery {
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct SessionDeleteRequest {
    pub id: String,
    #[serde(default)]
    pub confirm: bool,
}

#[derive(Debug, Deserialize)]
pub struct SessionImportRequest {
    /// A serialized `Session` (as produced by `/sessions/export-json`).
    pub session: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct NoteReadQuery {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct NoteWriteRequest {
    pub name: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct NoteDeleteRequest {
    pub name: String,
    #[serde(default)]
    pub confirm: bool,
}

#[derive(Debug, Deserialize)]
pub struct AnnounceRequest {
    pub message: String,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// The `/api/v1/*` router. Mounted by the daemon under `/api/v1`.
pub fn api_router() -> Router<Arc<DaemonState>> {
    Router::new()
        // Meta
        .route("/version", get(api_version))
        .route("/status", get(api_status))
        .route("/stats", get(api_stats))
        .route("/config", get(api_config))
        // PROMPT-2 S4.3 — GUI-fallback reason: the supervisor writes
        // ~/.diad/.gui-fallback-reason when the GUI failed twice at boot;
        // the next GUI start reads it (GET) and clears it (DELETE) after
        // surfacing the toast.
        .route(
            "/gui/fallback",
            get(api_gui_fallback).delete(api_gui_fallback_clear),
        )
        // RBRO-004 WP-04/WP-05 — app launcher for the icon pages and TUI.
        // POST /launch opens an app via the diad-open shim; GET
        // /launch/status tells the pages what to render and when the
        // launchpad may auto-close.
        .route("/launch", post(api_launch))
        .route("/launch/status", get(api_launch_status))
        // Chat
        .route("/chat", post(api_chat))
        // Providers
        .route("/providers", get(api_providers))
        .route("/provider/switch", post(api_provider_switch))
        .route("/providers/models", get(api_models))
        // Files
        .route("/files/list", get(api_file_list))
        .route("/files/read", get(api_file_read))
        .route("/files/write", post(api_file_write))
        // Terminal
        .route("/terminal/new", post(api_terminal_new))
        .route("/terminal/input", post(api_terminal_input))
        .route("/terminal/resize", post(api_terminal_resize))
        // Git
        .route("/git/status", get(api_git_status))
        .route("/git/diff", get(api_git_diff))
        .route("/git/log", get(api_git_log))
        .route("/git/branch", get(api_git_branch))
        .route("/git/commit", post(api_git_commit))
        // Debug (GDB MI-2)
        .route("/debug/start", post(api_debug_start))
        .route("/debug/session", post(api_debug_session))
        .route("/debug/breakpoint", post(api_debug_breakpoint))
        .route(
            "/debug/delete-breakpoint",
            post(api_debug_delete_breakpoint),
        )
        .route("/debug/run", post(api_debug_run))
        .route("/debug/continue", post(api_debug_continue))
        .route("/debug/step", post(api_debug_step))
        .route("/debug/next", post(api_debug_next))
        .route("/debug/finish", post(api_debug_finish))
        .route("/debug/frames", post(api_debug_frames))
        .route("/debug/state", post(api_debug_state))
        .route("/debug/stop", post(api_debug_stop))
        .route("/debug/sessions", get(api_debug_sessions))
        // Projects
        .route("/projects/list", get(api_projects_list))
        // AI intelligence (usage, memory, sessions)
        .route("/ai/usage", get(api_ai_usage))
        .route("/ai/recall", get(api_ai_recall))
        .route("/sessions/list", get(api_sessions_list))
        .route("/sessions/export", get(api_sessions_export))
        .route("/sessions/export-json", get(api_sessions_export_json))
        .route("/sessions/import", post(api_sessions_import))
        .route("/sessions/delete", post(api_sessions_delete))
        // Collaboration (shared notes, presence, announce)
        .route("/notes/list", get(api_notes_list))
        .route("/notes/read", get(api_notes_read))
        .route("/notes/write", post(api_notes_write))
        .route("/notes/delete", post(api_notes_delete))
        .route("/collab/presence", get(api_collab_presence))
        .route("/collab/announce", post(api_collab_announce))
        // System
        .route("/system/stats", get(api_system_stats))
        // Tool inventory
        .route("/tools", get(api_tools))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// JSON body with a consistent error shape.
fn error_response(status: StatusCode, message: impl Into<String>) -> Response {
    (
        status,
        Json(json!({ "error": true, "message": message.into() })),
    )
        .into_response()
}

/// The sandboxed, tilde-expanded absolute path for a request, or an error
/// response if the path is outside the allowlist.
///
/// The error is boxed: `axum::http::Response` is >128 bytes, and returning it
/// unboxed from a small `Result` would force a large stack copy on every call.
fn allowed_path(state: &DaemonState, raw: &str) -> Result<std::path::PathBuf, Box<Response>> {
    if !state.registry.is_path_allowed(raw) {
        return Err(Box::new(error_response(
            StatusCode::FORBIDDEN,
            format!("path not allowed by sandbox: {raw}"),
        )));
    }
    Ok(std::path::PathBuf::from(shellexpand::tilde(raw).as_ref()))
}

/// Run a blocking closure off the async runtime (filesystem work, tools).
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, Response> {
    tokio::task::spawn_blocking(f).await.map_err(|e| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("task error: {e}"),
        )
    })
}

// ---------------------------------------------------------------------------
// Meta
// ---------------------------------------------------------------------------

async fn api_version() -> Json<Value> {
    Json(json!({
        "name": "diad-agent",
        "version": env!("CARGO_PKG_VERSION"),
        "protocol": PROTOCOL_VERSION,
    }))
}

/// Current daemon configuration, with every credential masked. The frontend
/// uses this to reflect theme / streaming / confirmation preferences.
async fn api_config(State(state): State<Arc<DaemonState>>) -> Json<Value> {
    let cfg = state.config.read().unwrap().clone();
    // NEVER expose auth headers / API keys — mask provider credentials.
    let providers: Vec<Value> = cfg
        .providers
        .iter()
        .map(|(name, p)| {
            json!({
                "name": name,
                "type": p.provider_type,
                "model": p.model,
                "configured": p.auth_header.is_some(),
            })
        })
        .collect();
    Json(json!({
        "default_provider": cfg.default_provider,
        "allowed_paths": cfg.allowed_paths,
        "log_level": cfg.log_level,
        "theme": cfg.theme,
        "stream_tokens": cfg.stream_tokens,
        "confirm_destructive": cfg.confirm_destructive,
        "keybindings": cfg.keybindings,
        "max_history": cfg.max_history,
        "auto_save_interval": cfg.auto_save_interval,
        "storage": cfg.storage,
        "providers": providers,
    }))
}

async fn api_status(State(state): State<Arc<DaemonState>>) -> Json<Value> {
    Json(json!({
        "provider": state.provider.read().unwrap().clone(),
        "model": state.model.read().unwrap().clone(),
        "connected": state.connected.load(std::sync::atomic::Ordering::SeqCst),
        "streaming": state.streaming.load(std::sync::atomic::Ordering::SeqCst),
        "daemon_version": env!("CARGO_PKG_VERSION"),
        "perf_tier": state.perf_tier,
        "idle_stage": state.idle.get().label(),
    }))
}

// PROMPT-2 S4.3 — GUI fallback reason. The supervisor writes a one-line
// reason to ~/.diad/.gui-fallback-reason when the GUI failed twice at boot
// and the TUI engaged; the GUI surfaces it as a toast exactly once (GET
// returns the reason, DELETE removes the file).
fn gui_fallback_path() -> std::path::PathBuf {
    std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
        .join(".diad")
        .join(".gui-fallback-reason")
}

async fn api_gui_fallback() -> Response {
    let reason = std::fs::read_to_string(gui_fallback_path()).ok();
    Json(json!({ "reason": reason })).into_response()
}

async fn api_gui_fallback_clear() -> Response {
    let _ = std::fs::remove_file(gui_fallback_path());
    Json(json!({ "ok": true })).into_response()
}

// ---------------------------------------------------------------------------
// RBRO-004 WP-04/WP-05 — app launcher (icon pages + TUI parity)
// ---------------------------------------------------------------------------

/// `POST /api/v1/launch` — open an app through the `diad-open` shim.
///
/// Body: `{"app": "browser" | "telegram" | "slack"}`. The daemon spawns
/// `diad-open <app>` detached so it never blocks on the browser; the sway
/// assign/for_window rules route the window to the right workspace. With
/// `DIAD_OPEN_DRYRUN=1` nothing is spawned (test hook — the sandbox has no
/// sway/browser).
async fn api_launch(Json(req): Json<Value>) -> Response {
    let app = match req.get("app").and_then(Value::as_str) {
        Some(a) if matches!(a, "browser" | "telegram" | "slack") => a.to_string(),
        _ => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "unknown app: expected browser|telegram|slack",
            );
        }
    };
    if std::env::var_os("DIAD_OPEN_DRYRUN").is_some() {
        return Json(json!({ "ok": true, "app": app, "dry_run": true })).into_response();
    }
    match std::process::Command::new("diad-open").arg(&app).spawn() {
        Ok(_) => Json(json!({ "ok": true, "app": app })).into_response(),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to spawn diad-open {app}: {e}"),
        ),
    }
}

/// `GET /api/v1/launch/status` — what the icon pages render.
///
/// Returns the default browser label ("brave" when the flatpak is present,
/// else "chromium" — chromium always ships on the ISO), which chat apps are
/// installed, and which are currently open (parsed from the sway tree). The
/// launchpad auto-closes once both chat apps are open (WP-05).
async fn api_launch_status() -> Response {
    let (browser, installed, open) = blocking(move || {
        let apps = flatpak_app_ids();
        let browser = if apps.iter().any(|a| a == "com.brave.Browser") {
            "brave"
        } else {
            "chromium"
        };
        let installed = json!({
            "telegram": apps.iter().any(|a| a == "org.telegram.desktop"),
            "slack": apps.iter().any(|a| a == "com.slack.Slack"),
        });
        (browser.to_string(), installed, open_chat_apps())
    })
    .await
    .unwrap_or_else(|_| {
        (
            "chromium".to_string(),
            json!({ "telegram": false, "slack": false }),
            Vec::new(),
        )
    });
    Json(json!({ "browser": browser, "installed": installed, "open": open })).into_response()
}

/// Installed flatpak application IDs (fast local DB read — never network).
fn flatpak_app_ids() -> Vec<String> {
    let out = std::process::Command::new("flatpak")
        .args(["list", "--app", "--columns=application"])
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

/// Chat apps currently open, read from the sway tree. Safe when swaymsg is
/// missing or IPC is dead: returns an empty list, so the launchpad stays
/// open instead of closing on a false positive.
fn open_chat_apps() -> Vec<String> {
    let out = std::process::Command::new("swaymsg")
        .args(["-t", "get_tree"])
        .output();
    match out {
        Ok(o) if o.status.success() => parse_open_from_bytes(&o.stdout),
        _ => Vec::new(),
    }
}

/// Extract open chat apps from a sway `get_tree` document.
fn parse_open_from_bytes(bytes: &[u8]) -> Vec<String> {
    let Ok(tree) = serde_json::from_slice::<Value>(bytes) else {
        return Vec::new();
    };
    let mut open = Vec::new();
    let mut stack = vec![tree];
    while let Some(node) = stack.pop() {
        if let Some(children) = node.get("nodes").and_then(Value::as_array) {
            stack.extend(children.iter().cloned());
        }
        if let Some(children) = node.get("floating_nodes").and_then(Value::as_array) {
            stack.extend(children.iter().cloned());
        }
        let app_id = node.get("app_id").and_then(Value::as_str).unwrap_or("");
        let name = node.get("name").and_then(Value::as_str).unwrap_or("");
        if (app_id == "org.telegram.desktop" || (app_id == "chromium" && name.contains("Telegram")))
            && !open.iter().any(|a| a == "telegram")
        {
            open.push("telegram".to_string());
        }
        if (app_id == "com.slack.Slack" || (app_id == "chromium" && name.contains("Slack")))
            && !open.iter().any(|a| a == "slack")
        {
            open.push("slack".to_string());
        }
    }
    open
}

async fn api_stats(State(state): State<Arc<DaemonState>>) -> Json<Value> {
    Json(json!({
        "tokens": state.tokens.load(std::sync::atomic::Ordering::SeqCst),
        "latency_ms": state.latency_ms.load(std::sync::atomic::Ordering::SeqCst),
        "uptime_seconds": state.uptime_seconds(),
        "idle_stage": state.idle.get().label(),
        "rss_mb": crate::core::perf::read_self_rss_kb() / 1024,
        "perf_tier": state.perf_tier,
    }))
}

// ---------------------------------------------------------------------------
// Chat
// ---------------------------------------------------------------------------

async fn api_chat(State(state): State<Arc<DaemonState>>, Json(req): Json<ChatRequest>) -> Response {
    if req.text.trim().is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "chat: text must not be empty");
    }
    if state.streaming.load(std::sync::atomic::Ordering::SeqCst) > 0 {
        return error_response(
            StatusCode::CONFLICT,
            "chat: another request is already streaming",
        );
    }

    let id = req.id.unwrap_or_else(|| next_id("rest-chat"));

    // A dedicated conn: REST chat is non-interactive, so prompts are
    // auto-resolved (confirm → reject, secret → cancel) inside the drain loop.
    let (tx, mut rx) = channel::<ServerMsg>(4096);
    let conn = Conn {
        tx,
        prompts: PromptBroker::default(),
    };

    handle_chat_send(&state, &conn, id.clone(), req.text).await;

    // Drain until StreamDone for our id (or a generous timeout).
    let mut text = String::new();
    let mut messages = Vec::new();
    let mut timed_out = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(300);

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let msg = match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(msg)) => msg,
            Ok(None) => break, // channel closed
            Err(_) => {
                timed_out = true;
                break;
            }
        };
        match msg {
            ServerMsg::Token { id: mid, text: t } if mid == id => text.push_str(&t),
            ServerMsg::System { message } => messages.push(message),
            ServerMsg::StreamDone { id: mid } if mid == id => break,
            // Non-interactive REST: auto-resolve prompts so the loop finishes.
            ServerMsg::ConfirmRequest { id: cid, label } => {
                conn.prompts.resolve_confirm(&cid, false);
                messages.push(format!("[auto-rejected] {label}"));
            }
            ServerMsg::SecretRequest { id: sid, label } => {
                conn.prompts.resolve_secret(&sid, String::new());
                messages.push(format!("[auto-cancelled] {label}"));
            }
            ServerMsg::EditDiff { id: did, path, .. } => {
                conn.prompts.resolve_diff(&did, false);
                messages.push(format!("[auto-rejected edit] {path}"));
            }
            ServerMsg::Error { message, .. } => messages.push(format!("[error] {message}")),
            _ => {}
        }
    }

    (
        StatusCode::OK,
        Json(ChatResponse {
            id,
            text,
            done: !timed_out,
            timed_out,
            messages,
        }),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Providers
// ---------------------------------------------------------------------------

async fn api_providers(State(state): State<Arc<DaemonState>>) -> Json<Value> {
    Json(json!({
        "providers": state.provider_entries_dto(),
        "current": state.provider.read().unwrap().clone(),
        "model": state.model.read().unwrap().clone(),
        "connected": state.connected.load(std::sync::atomic::Ordering::SeqCst),
    }))
}

async fn api_provider_switch(
    State(state): State<Arc<DaemonState>>,
    Json(req): Json<ProviderSwitchRequest>,
) -> Response {
    let name = req.name;
    let known = state
        .factory
        .provider_entries()
        .iter()
        .any(|e| e.name == name);
    if !known {
        return error_response(StatusCode::NOT_FOUND, format!("unknown provider: {name}"));
    }
    let has_key = state.factory.has_key(&name);
    *state.provider.write().unwrap() = name.clone();
    state
        .connected
        .store(has_key, std::sync::atomic::Ordering::SeqCst);
    state.broadcast_status();
    let models = state.factory.models_for(&name);
    let current = state.model.read().unwrap().clone();
    Json(json!({
        "provider": name,
        "connected": has_key,
        "model": current,
        "models": models,
    }))
    .into_response()
}

async fn api_models(
    State(state): State<Arc<DaemonState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let provider = params
        .get("provider")
        .cloned()
        .unwrap_or_else(|| state.provider.read().unwrap().clone());
    let models = state.factory.models_for(&provider);
    let current = state.model.read().unwrap().clone();
    Json(json!({
        "provider": provider,
        "models": models,
        "current": current,
    }))
    .into_response()
}

// ---------------------------------------------------------------------------
// Files
// ---------------------------------------------------------------------------

async fn api_file_list(
    State(state): State<Arc<DaemonState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let path = params.get("path").cloned().unwrap_or_else(|| ".".into());
    let path2 = path.clone();
    let path_buf = match allowed_path(&state, &path) {
        Ok(p) => p,
        Err(resp) => return *resp,
    };

    match blocking(move || {
        if !path_buf.exists() {
            return Err("directory not found");
        }
        if !path_buf.is_dir() {
            return Err("not a directory");
        }
        Ok(list_entries(&path2))
    })
    .await
    {
        Ok(Ok(entries)) => Json(json!({ "path": path, "entries": entries })).into_response(),
        Ok(Err(msg)) => error_response(StatusCode::NOT_FOUND, msg),
        Err(resp) => resp,
    }
}

/// Non-emoji, git-aware directory listing (matches the GUI's file tree).
fn list_entries(dir: &str) -> Vec<FileEntryDto> {
    super::daemon::list_dir_entries(dir)
}

async fn api_file_read(
    State(state): State<Arc<DaemonState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let path = match params.get("path") {
        Some(p) => p.clone(),
        None => return error_response(StatusCode::BAD_REQUEST, "files/read: missing path"),
    };
    let path_buf = match allowed_path(&state, &path) {
        Ok(p) => p,
        Err(resp) => return *resp,
    };

    match blocking(move || std::fs::read_to_string(&path_buf)).await {
        Ok(Ok(content)) => {
            let lines = content.lines().count();
            let bytes = content.len();
            Json(json!({
                "path": path,
                "content": content,
                "lines": lines,
                "bytes": bytes,
            }))
            .into_response()
        }
        Ok(Err(e)) => error_response(StatusCode::NOT_FOUND, format!("read {path}: {e}")),
        Err(resp) => resp,
    }
}

async fn api_file_write(
    State(state): State<Arc<DaemonState>>,
    Json(req): Json<FileWriteRequest>,
) -> Response {
    let path_buf = match allowed_path(&state, &req.path) {
        Ok(p) => p,
        Err(resp) => return *resp,
    };

    // Writing outside the sandbox root requires an explicit confirm flag.
    let is_outside = !path_buf.starts_with(std::env::temp_dir())
        && state.registry.allowed_paths().iter().all(|a| {
            let a = std::path::PathBuf::from(shellexpand::tilde(a).as_ref());
            !path_buf.starts_with(&a)
        });
    if is_outside && !req.confirm {
        return error_response(
            StatusCode::PRECONDITION_REQUIRED,
            "path is outside the sandbox; pass confirm: true to proceed",
        );
    }

    let content = req.content.clone();
    match blocking(move || {
        if let Some(parent) = path_buf.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path_buf, content)?;
        Ok::<_, std::io::Error>(path_buf.metadata().map(|m| m.len()).unwrap_or(0))
    })
    .await
    {
        Ok(Ok(bytes)) => {
            Json(json!({ "path": req.path, "written": true, "bytes": bytes })).into_response()
        }
        Ok(Err(e)) => error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("write: {e}")),
        Err(resp) => resp,
    }
}

// ---------------------------------------------------------------------------
// Terminal
// ---------------------------------------------------------------------------

async fn api_terminal_new(State(state): State<Arc<DaemonState>>) -> Response {
    state.pty.shutdown();
    Json(json!({ "ok": true, "terminal": "reset" })).into_response()
}

async fn api_terminal_input(
    State(state): State<Arc<DaemonState>>,
    Json(req): Json<TerminalInputRequest>,
) -> Response {
    let (tx, mut rx) = channel::<ServerMsg>(4096);
    // Forward any PTY output to all connected frontends.
    let state2 = Arc::clone(&state);
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let ServerMsg::TermOutput { data } = msg {
                state2.broadcast(ServerMsg::TermOutput { data });
            }
        }
    });

    match state.pty.input(&req.data, tx) {
        Ok(()) => Json(json!({ "ok": true, "written": req.data.len() })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("pty error: {e}")),
    }
}

async fn api_terminal_resize(
    State(state): State<Arc<DaemonState>>,
    Json(req): Json<TerminalResizeRequest>,
) -> Response {
    state.pty.resize(req.cols, req.rows);
    Json(json!({ "ok": true, "cols": req.cols, "rows": req.rows })).into_response()
}

// ---------------------------------------------------------------------------
// Git
// ---------------------------------------------------------------------------

async fn api_git_status(
    State(state): State<Arc<DaemonState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let path = match params.get("path") {
        Some(p) => p.clone(),
        None => return error_response(StatusCode::BAD_REQUEST, "git/status: missing path"),
    };
    let path_buf = match allowed_path(&state, &path) {
        Ok(p) => p,
        Err(resp) => return *resp,
    };

    match blocking(move || {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(&path_buf)
            .arg("status")
            .arg("--porcelain")
            .arg("--branch")
            .output()?;
        Ok::<_, std::io::Error>(out)
    })
    .await
    {
        Ok(Ok(out)) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout).to_string();
            let lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
            let branch = lines
                .iter()
                .find(|l| l.starts_with("## "))
                .map(|l| l[3..].split_whitespace().next().unwrap_or("").to_string())
                .unwrap_or_default();
            Json(json!({
                "path": path,
                "branch": branch,
                "dirty": lines.iter().any(|l| !l.starts_with("## ")),
                "lines": lines,
            }))
            .into_response()
        }
        Ok(Ok(_)) => error_response(StatusCode::UNPROCESSABLE_ENTITY, "git status failed"),
        Ok(Err(e)) => error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("git: {e}")),
        Err(resp) => resp,
    }
}

async fn api_git_commit(
    State(state): State<Arc<DaemonState>>,
    Json(req): Json<GitCommitRequest>,
) -> Response {
    if req.message.trim().is_empty() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "git/commit: message must not be empty",
        );
    }
    let path_buf = match allowed_path(&state, &req.path) {
        Ok(p) => p,
        Err(resp) => return *resp,
    };
    if !req.confirm {
        return error_response(
            StatusCode::PRECONDITION_REQUIRED,
            "git/commit is destructive; pass confirm: true to proceed",
        );
    }

    let message = req.message.clone();
    match blocking(move || {
        // Stage everything in the repo.
        let add = std::process::Command::new("git")
            .arg("-C")
            .arg(&path_buf)
            .arg("add")
            .arg("-A")
            .output()?;
        if !add.status.success() {
            return Ok::<_, std::io::Error>((
                false,
                String::from_utf8_lossy(&add.stderr).to_string(),
            ));
        }
        let commit = std::process::Command::new("git")
            .arg("-C")
            .arg(&path_buf)
            .arg("commit")
            .arg("-m")
            .arg(&message)
            .output()?;
        let out = String::from_utf8_lossy(&commit.stdout).to_string();
        let err = String::from_utf8_lossy(&commit.stderr).to_string();
        Ok((commit.status.success(), format!("{out}{err}")))
    })
    .await
    {
        Ok(Ok((true, out))) => Json(json!({ "committed": true, "output": out })).into_response(),
        Ok(Ok((false, out))) => error_response(StatusCode::UNPROCESSABLE_ENTITY, out),
        Ok(Err(e)) => error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("git: {e}")),
        Err(resp) => resp,
    }
}

/// Resolve the git work tree containing `path` (walk up to the repo root).
fn git_work_tree(path_buf: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut dir = path_buf.to_path_buf();
    if dir.is_file() {
        dir = dir.parent()?.to_path_buf();
    }
    loop {
        if dir.join(".git").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

async fn api_git_diff(
    State(state): State<Arc<DaemonState>>,
    Query(params): Query<GitDiffQuery>,
) -> Response {
    let path_buf = match allowed_path(&state, &params.path) {
        Ok(p) => p,
        Err(resp) => return *resp,
    };
    let root = match git_work_tree(&path_buf) {
        Some(r) => r,
        None => return error_response(StatusCode::UNPROCESSABLE_ENTITY, "not a git repository"),
    };
    let staged = params.staged;
    let file = params.file.clone();
    let root_for_cmd = root.clone();
    match blocking(move || {
        let mut cmd = std::process::Command::new("git");
        cmd.arg("-C").arg(&root_for_cmd).arg("diff");
        if staged {
            cmd.arg("--cached");
        }
        if let Some(f) = &file {
            cmd.arg("--").arg(f);
        }
        let out = cmd.output()?;
        // `--stat` is a second subprocess; run it here too so no git spawn
        // ever touches the async runtime (F-18).
        let mut stat_cmd = std::process::Command::new("git");
        stat_cmd
            .arg("-C")
            .arg(&root_for_cmd)
            .arg("diff")
            .arg("--stat");
        if staged {
            stat_cmd.arg("--cached");
        }
        if let Some(f) = &file {
            stat_cmd.arg("--").arg(f);
        }
        let stat_out = stat_cmd.output();
        Ok::<_, std::io::Error>((out, stat_out))
    })
    .await
    {
        Ok(Ok((out, stat_out))) if out.status.success() => {
            let diff = String::from_utf8_lossy(&out.stdout).to_string();
            let stat = stat_out
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
            Json(json!({
                "root": root.display().to_string(),
                "diff": diff,
                "stat": stat,
                "staged": staged,
            }))
            .into_response()
        }
        Ok(Ok(_)) => error_response(StatusCode::UNPROCESSABLE_ENTITY, "git diff failed"),
        Ok(Err(e)) => error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("git: {e}")),
        Err(resp) => resp,
    }
}

async fn api_git_log(
    State(state): State<Arc<DaemonState>>,
    Query(params): Query<GitLogQuery>,
) -> Response {
    let path_buf = match allowed_path(&state, &params.path) {
        Ok(p) => p,
        Err(resp) => return *resp,
    };
    let root = match git_work_tree(&path_buf) {
        Some(r) => r,
        None => return error_response(StatusCode::UNPROCESSABLE_ENTITY, "not a git repository"),
    };
    let limit = params.limit.unwrap_or(20).clamp(1, 100);
    let root_for_cmd = root.clone();
    match blocking(move || {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(&root_for_cmd)
            .arg("log")
            .arg(format!("--max-count={limit}"))
            .arg("--pretty=format:%h%x1f%H%x1f%an%x1f%ad%x1f%s")
            .arg("--date=short")
            .output()?;
        Ok::<_, std::io::Error>(out)
    })
    .await
    {
        Ok(Ok(out)) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout).to_string();
            let commits: Vec<Value> = text
                .lines()
                .filter_map(|l| {
                    let mut it = l.split('\u{1f}');
                    let short = it.next()?.to_string();
                    let full = it.next().unwrap_or("").to_string();
                    let author = it.next().unwrap_or("").to_string();
                    let date = it.next().unwrap_or("").to_string();
                    let subject = it.next().unwrap_or("").to_string();
                    Some(json!({
                        "short": short, "full": full, "author": author,
                        "date": date, "subject": subject,
                    }))
                })
                .collect();
            Json(json!({ "root": root.display().to_string(), "commits": commits })).into_response()
        }
        Ok(Ok(_)) => error_response(StatusCode::UNPROCESSABLE_ENTITY, "git log failed"),
        Ok(Err(e)) => error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("git: {e}")),
        Err(resp) => resp,
    }
}

async fn api_git_branch(
    State(state): State<Arc<DaemonState>>,
    Query(params): Query<GitBranchQuery>,
) -> Response {
    let path_buf = match allowed_path(&state, &params.path) {
        Ok(p) => p,
        Err(resp) => return *resp,
    };
    let root = match git_work_tree(&path_buf) {
        Some(r) => r,
        None => return error_response(StatusCode::UNPROCESSABLE_ENTITY, "not a git repository"),
    };
    let root_for_cmd = root.clone();
    match blocking(move || {
        // Note: `git branch --format` does NOT interpret %x escapes, so use a
        // literal unit-separator byte in the format string.
        let fmt = "%(refname:short)\u{1f}%(HEAD)\u{1f}%(upstream:short)".to_string();
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(&root_for_cmd)
            .arg("branch")
            .arg(format!("--format={fmt}"))
            .output()?;
        Ok::<_, std::io::Error>(out)
    })
    .await
    {
        Ok(Ok(out)) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout).to_string();
            let branches: Vec<Value> = text
                .lines()
                .filter_map(|l| {
                    let mut it = l.split('\u{1f}');
                    let name = it.next()?.to_string();
                    let head = it.next().unwrap_or("") == "*";
                    let upstream = it.next().unwrap_or("").to_string();
                    Some(json!({ "name": name, "current": head, "upstream": upstream }))
                })
                .collect();
            let current = branches.iter().find(|b| b["current"] == true);
            Json(json!({
                "root": root.display().to_string(),
                "current": current.map(|b| b["name"].as_str().unwrap_or("").to_string()).unwrap_or_default(),
                "branches": branches,
            }))
            .into_response()
        }
        Ok(Ok(_)) => error_response(StatusCode::UNPROCESSABLE_ENTITY, "git branch failed"),
        Ok(Err(e)) => error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("git: {e}")),
        Err(resp) => resp,
    }
}

// ---------------------------------------------------------------------------
// Debug
// ---------------------------------------------------------------------------

async fn api_debug_start(
    State(_state): State<Arc<DaemonState>>,
    Json(req): Json<DebugStartRequest>,
) -> Response {
    if req.program.trim().is_empty() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "debug/start: program must not be empty",
        );
    }
    // Reuse the process sandbox: argv-array spawn, no shell, banned argv0.
    let program = req.program.clone();
    let args = req.args.clone();
    let cwd = req.working_dir.clone();
    match blocking(move || {
        let mut cmd = std::process::Command::new(&program);
        cmd.args(&args);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        cmd.output()
    })
    .await
    {
        Ok(Ok(out)) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let session = next_id("debug");
            Json(json!({
                "session": session,
                "program": req.program,
                "exit_code": out.status.code(),
                "success": out.status.success(),
                "stdout": stdout,
                "stderr": stderr,
            }))
            .into_response()
        }
        Ok(Err(e)) => error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("debug: {e}")),
        Err(resp) => resp,
    }
}

/// Start a new GDB session (MI-2).
async fn api_debug_session(
    State(state): State<Arc<DaemonState>>,
    Json(req): Json<DebugSessionRequest>,
) -> Response {
    let program = req.program.trim().to_string();
    if program.is_empty() || program.len() > 512 {
        return error_response(
            StatusCode::BAD_REQUEST,
            "debug/session: program must not be empty",
        );
    }
    if program.starts_with('-') {
        return error_response(
            StatusCode::BAD_REQUEST,
            "debug/session: invalid program name",
        );
    }
    let args = req.args.clone();
    let cwd = req.working_dir.clone();
    let program_for_cmd = program.clone();
    match blocking(move || state.debug.start(&program_for_cmd, &args, cwd.as_deref())).await {
        Ok(Ok((id, session))) => {
            let _ = session.snapshot();
            Json(json!({ "session": id, "program": program, "ok": true })).into_response()
        }
        Ok(Err(e)) => error_response(StatusCode::INTERNAL_SERVER_ERROR, e),
        Err(resp) => resp,
    }
}

/// Set a breakpoint `file:line` in a session.
async fn api_debug_breakpoint(
    State(state): State<Arc<DaemonState>>,
    Json(req): Json<DebugBreakpointRequest>,
) -> Response {
    if req.line == 0 || req.line > 100_000 || req.file.trim().is_empty() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "debug/breakpoint: invalid file or line",
        );
    }
    let Some(session) = state.debug.get(&req.session) else {
        return error_response(StatusCode::NOT_FOUND, "debug/breakpoint: unknown session");
    };
    match blocking(move || session.set_breakpoint(req.file.trim(), req.line)).await {
        Ok(Ok(bp)) => Json(json!({ "breakpoint": {
            "number": bp.number, "file": bp.file, "line": bp.line, "enabled": bp.enabled,
        } }))
        .into_response(),
        Ok(Err(e)) => error_response(StatusCode::UNPROCESSABLE_ENTITY, e),
        Err(resp) => resp,
    }
}

/// Delete a breakpoint by MI number.
async fn api_debug_delete_breakpoint(
    State(state): State<Arc<DaemonState>>,
    Json(req): Json<DebugDeleteBreakpointRequest>,
) -> Response {
    let Some(session) = state.debug.get(&req.session) else {
        return error_response(
            StatusCode::NOT_FOUND,
            "debug/delete-breakpoint: unknown session",
        );
    };
    match blocking(move || session.delete_breakpoint(req.number)).await {
        Ok(Ok(())) => Json(json!({ "deleted": true })).into_response(),
        Ok(Err(e)) => error_response(StatusCode::UNPROCESSABLE_ENTITY, e),
        Err(resp) => resp,
    }
}

macro_rules! debug_action {
    ($name:ident, $method:ident, $label:literal) => {
        async fn $name(
            State(state): State<Arc<DaemonState>>,
            Json(req): Json<DebugSessionId>,
        ) -> Response {
            let Some(session) = state.debug.get(&req.session) else {
                return error_response(StatusCode::NOT_FOUND, concat!($label, ": unknown session"));
            };
            match blocking(move || session.$method()).await {
                Ok(Ok(())) => Json(json!({ "ok": true, "session": req.session })).into_response(),
                Ok(Err(e)) => error_response(StatusCode::UNPROCESSABLE_ENTITY, e),
                Err(resp) => resp,
            }
        }
    };
}

debug_action!(api_debug_run, run, "debug/run");
debug_action!(api_debug_continue, continue_exec, "debug/continue");
debug_action!(api_debug_step, step, "debug/step");
debug_action!(api_debug_next, next, "debug/next");
debug_action!(api_debug_finish, finish, "debug/finish");

/// Refresh the stack + variables (call while stopped).
async fn api_debug_frames(
    State(state): State<Arc<DaemonState>>,
    Json(req): Json<DebugSessionId>,
) -> Response {
    let Some(session) = state.debug.get(&req.session) else {
        return error_response(StatusCode::NOT_FOUND, "debug/frames: unknown session");
    };
    let session_for_cmd = Arc::clone(&session);
    match blocking(move || session_for_cmd.refresh_frames()).await {
        Ok(Ok(())) => {
            let snap = session.snapshot();
            Json(json!({
                "stack": snap.stack,
                "variables": snap.variables,
                "reason": snap.reason,
            }))
            .into_response()
        }
        Ok(Err(e)) => error_response(StatusCode::UNPROCESSABLE_ENTITY, e),
        Err(resp) => resp,
    }
}

/// Current snapshot: state, breakpoints, stack, variables, console tail.
async fn api_debug_state(
    State(state): State<Arc<DaemonState>>,
    Json(req): Json<DebugSessionId>,
) -> Response {
    let Some(session) = state.debug.get(&req.session) else {
        return error_response(StatusCode::NOT_FOUND, "debug/state: unknown session");
    };
    let snap = session.snapshot();
    Json(json!({
        "session": req.session,
        "program": session.program,
        "running": snap.running,
        "exited": snap.exited,
        "exit_code": snap.exit_code,
        "reason": snap.reason,
        "breakpoints": snap.breakpoints,
        "stack": snap.stack,
        "variables": snap.variables,
        "console": snap.console,
    }))
    .into_response()
}

/// Kill a session (and the inferior).
async fn api_debug_stop(
    State(state): State<Arc<DaemonState>>,
    Json(req): Json<DebugSessionId>,
) -> Response {
    if state.debug.stop(&req.session) {
        Json(json!({ "stopped": true, "session": req.session })).into_response()
    } else {
        error_response(StatusCode::NOT_FOUND, "debug/stop: unknown session")
    }
}

/// List live sessions.
async fn api_debug_sessions(State(state): State<Arc<DaemonState>>) -> Json<Value> {
    let sessions: Vec<Value> = state
        .debug
        .list()
        .into_iter()
        .map(|(id, program)| json!({ "session": id, "program": program }))
        .collect();
    Json(json!({ "sessions": sessions }))
}

/// Scan a directory for projects (git repos, package manifests, etc.).
async fn api_projects_list(
    State(state): State<Arc<DaemonState>>,
    Query(params): Query<ProjectListQuery>,
) -> Response {
    let root_raw = params.root.unwrap_or_else(|| "~".to_string());
    let root = match allowed_path(&state, &root_raw) {
        Ok(p) => p,
        Err(resp) => return *resp,
    };
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let root_display = root.display().to_string();
    let root_for_cmd = root.clone();
    match blocking(move || {
        let mut projects = Vec::new();
        let entries = match std::fs::read_dir(&root_for_cmd) {
            Ok(e) => e,
            Err(_) => return projects,
        };
        for entry in entries.flatten() {
            if projects.len() >= limit {
                break;
            }
            let path = entry.path();
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            // Recognise project markers.
            let markers = [
                ".git",
                "package.json",
                "Cargo.toml",
                "go.mod",
                "pyproject.toml",
                "requirements.txt",
                "Makefile",
                "CMakeLists.txt",
                "pom.xml",
                "build.gradle",
                "mix.exs",
                "Gemfile",
                "composer.json",
            ];
            let kind = markers
                .iter()
                .find(|m| path.join(m).exists())
                .map(|m| match *m {
                    ".git" => "git",
                    "package.json" => "node",
                    "Cargo.toml" => "rust",
                    "go.mod" => "go",
                    "pyproject.toml" | "requirements.txt" => "python",
                    "Makefile" | "CMakeLists.txt" => "c",
                    "pom.xml" | "build.gradle" => "java",
                    "mix.exs" => "elixir",
                    "Gemfile" => "ruby",
                    "composer.json" => "php",
                    _ => "other",
                })
                .unwrap_or("other");
            let modified = meta.modified().ok().map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0)
            });
            projects.push(json!({
                "name": name,
                "path": path.display().to_string(),
                "kind": kind,
                "modified": modified,
            }));
        }
        projects.sort_by(|a, b| {
            b["modified"]
                .as_i64()
                .unwrap_or(0)
                .cmp(&a["modified"].as_i64().unwrap_or(0))
        });
        projects
    })
    .await
    {
        Ok(projects) => Json(json!({ "root": root_display, "projects": projects })).into_response(),
        Err(resp) => resp,
    }
}

// ---------------------------------------------------------------------------
// AI intelligence (usage, memory recall, session export)
// ---------------------------------------------------------------------------

async fn api_ai_usage(State(state): State<Arc<DaemonState>>) -> Json<Value> {
    let summary = match state.usage.lock() {
        Ok(usage) => usage.summary(),
        Err(_) => super::usage::UsageSummary::default(),
    };
    Json(json!(summary))
}

async fn api_ai_recall(
    State(state): State<Arc<DaemonState>>,
    Query(q): Query<RecallQuery>,
) -> Response {
    let limit = q.limit.unwrap_or(6);
    let hits = match state.sessions.lock() {
        Ok(manager) => super::usage::recall(&manager, &q.q, limit),
        Err(_) => Vec::new(),
    };
    Json(json!({ "query": q.q, "hits": hits })).into_response()
}

async fn api_sessions_list(State(state): State<Arc<DaemonState>>) -> Response {
    let sessions = match state.sessions.lock() {
        Ok(manager) => match manager.list_sessions() {
            Ok(list) => list,
            Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        },
        Err(_) => {
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "sessions lock poisoned")
        }
    };
    Json(json!({
        "sessions": sessions.iter().map(|s| json!({
            "id": s.id,
            "title": s.title,
            "provider": s.active_provider,
            "model": s.active_model,
            "messages": s.messages.len(),
            "updated_at": s.updated_at.to_rfc3339(),
        })).collect::<Vec<_>>()
    }))
    .into_response()
}

async fn api_sessions_export(
    State(state): State<Arc<DaemonState>>,
    Query(q): Query<SessionExportQuery>,
) -> Response {
    let session = match state.sessions.lock() {
        Ok(manager) => match manager.load_session(&q.id) {
            Ok(s) => s,
            Err(e) => {
                return error_response(StatusCode::NOT_FOUND, format!("session not found: {e}"))
            }
        },
        Err(_) => {
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "sessions lock poisoned")
        }
    };
    let markdown = super::usage::session_to_markdown(&session);
    Json(json!({
        "id": session.id,
        "title": session.title,
        "markdown": markdown,
    }))
    .into_response()
}

async fn api_sessions_delete(
    State(state): State<Arc<DaemonState>>,
    Json(req): Json<SessionDeleteRequest>,
) -> Response {
    if !req.confirm {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "session deletion is destructive — set confirm: true",
        );
    }
    match state.sessions.lock() {
        Ok(manager) => match manager.delete_session(&req.id) {
            Ok(()) => Json(json!({ "deleted": true, "id": req.id })).into_response(),
            Err(e) => error_response(StatusCode::NOT_FOUND, format!("session not found: {e}")),
        },
        Err(_) => error_response(StatusCode::INTERNAL_SERVER_ERROR, "sessions lock poisoned"),
    }
}

/// Export a session as raw JSON (round-trippable through `/sessions/import`).
async fn api_sessions_export_json(
    State(state): State<Arc<DaemonState>>,
    Query(q): Query<SessionExportQuery>,
) -> Response {
    let session = match state.sessions.lock() {
        Ok(manager) => match manager.load_session(&q.id) {
            Ok(s) => s,
            Err(e) => {
                return error_response(StatusCode::NOT_FOUND, format!("session not found: {e}"))
            }
        },
        Err(_) => {
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "sessions lock poisoned")
        }
    };
    Json(json!({
        "format": "diad-session",
        "version": 1,
        "session": session,
    }))
    .into_response()
}

/// Import a session shared from another device. A fresh id is minted so an
/// import can never overwrite an existing session. Title defaults to a
/// timestamped label when missing.
async fn api_sessions_import(
    State(state): State<Arc<DaemonState>>,
    Json(req): Json<SessionImportRequest>,
) -> Response {
    let session = &req.session;
    if !session.is_object() {
        return error_response(StatusCode::BAD_REQUEST, "session must be a JSON object");
    }
    let title = session
        .get("title")
        .and_then(|t| t.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            format!(
                "Imported session {}",
                chrono::Utc::now().format("%Y-%m-%d %H:%M")
            )
        });
    let provider = session
        .get("active_provider")
        .and_then(|p| p.as_str())
        .unwrap_or("anthropic")
        .to_string();

    // Rebuild a fresh Session from the shared JSON, discarding the remote id
    // and timestamps so the local copy is a clean, independent session.
    let mut fresh = crate::orchestrator::session::Session::new(&title, &provider);
    if let Some(msgs) = session.get("messages").and_then(|m| m.as_array()) {
        for m in msgs {
            let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let content = m.get("content").and_then(|c| c.as_str()).unwrap_or("");
            if role == "user" || role == "assistant" || role == "system" || role == "tool" {
                fresh.add_message(role, content);
            }
        }
    }

    match state.sessions.lock() {
        Ok(manager) => match manager.save_session(&fresh) {
            Ok(()) => {
                state.broadcast(ServerMsg::System {
                    message: format!("Session «{}» imported", fresh.title),
                });
                Json(json!({
                    "id": fresh.id,
                    "title": fresh.title,
                    "messages": fresh.messages.len(),
                }))
                .into_response()
            }
            Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        },
        Err(_) => error_response(StatusCode::INTERNAL_SERVER_ERROR, "sessions lock poisoned"),
    }
}

// ---------------------------------------------------------------------------
// Collaboration (shared notes, presence, announce)
// ---------------------------------------------------------------------------

async fn api_notes_list(State(state): State<Arc<DaemonState>>) -> Json<Value> {
    Json(json!({ "notes": state.notes.list() }))
}

async fn api_notes_read(
    State(state): State<Arc<DaemonState>>,
    Query(q): Query<NoteReadQuery>,
) -> Response {
    match state.notes.read(&q.name) {
        Some(note) => Json(json!({
            "name": note.name,
            "content": note.content,
            "updated_at": note.updated_at.to_rfc3339(),
        }))
        .into_response(),
        None => error_response(StatusCode::NOT_FOUND, "note not found"),
    }
}

async fn api_notes_write(
    State(state): State<Arc<DaemonState>>,
    Json(req): Json<NoteWriteRequest>,
) -> Response {
    if super::notes::NotesStore::sanitize_name(&req.name).is_none() {
        return error_response(StatusCode::BAD_REQUEST, "invalid note name");
    }
    let notes = Arc::clone(&state.notes);
    match blocking(move || notes.write(&req.name, &req.content)).await {
        Ok(Ok(meta)) => {
            state.broadcast(ServerMsg::System {
                message: format!("Note «{}» saved", meta.name),
            });
            Json(json!({
                "name": meta.name,
                "size": meta.size,
                "updated_at": meta.updated_at.to_rfc3339(),
            }))
            .into_response()
        }
        Ok(Err(e)) => error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        Err(resp) => resp,
    }
}

async fn api_notes_delete(
    State(state): State<Arc<DaemonState>>,
    Json(req): Json<NoteDeleteRequest>,
) -> Response {
    if !req.confirm {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "note deletion is destructive — set confirm: true",
        );
    }
    if state.notes.delete(&req.name) {
        state.broadcast(ServerMsg::System {
            message: format!("Note «{}» deleted", req.name),
        });
        Json(json!({ "deleted": true, "name": req.name })).into_response()
    } else {
        error_response(StatusCode::NOT_FOUND, "note not found")
    }
}

/// Presence snapshot: how many frontends are connected, how many sessions
/// exist, which local AI agents are detected, and how many notes are shared.
async fn api_collab_presence(State(state): State<Arc<DaemonState>>) -> Response {
    let frontends = state.client_count();
    let sessions = match state.sessions.lock() {
        Ok(manager) => manager.list_sessions().map(|l| l.len()).unwrap_or(0),
        Err(_) => 0,
    };
    let agents = crate::tools::external::detect_agents();
    let notes = state.notes.list().len();
    Json(json!({
        "frontends": frontends,
        "sessions": sessions,
        "notes": notes,
        "agents": agents,
    }))
    .into_response()
}

/// Broadcast a message to every connected frontend (renders as a [system]
/// line in each chat pane). Returns the number of frontends reached.
async fn api_collab_announce(
    State(state): State<Arc<DaemonState>>,
    Json(req): Json<AnnounceRequest>,
) -> Response {
    let message = req.message.trim().to_string();
    if message.is_empty() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "announce message must not be empty",
        );
    }
    let frontends = state.client_count();
    state.broadcast(ServerMsg::System {
        message: format!("📣 {message}"),
    });
    Json(json!({ "sent": frontends, "message": message })).into_response()
}

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

async fn api_system_stats(State(state): State<Arc<DaemonState>>) -> Json<Value> {
    // Quick, non-privileged metrics.
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let load = std::fs::read_to_string("/proc/loadavg")
        .map(|s| {
            s.split_whitespace()
                .take(3)
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mem = std::fs::read_to_string("/proc/meminfo")
        .map(|s| {
            let mut map = HashMap::new();
            for line in s.lines() {
                let mut it = line.split_whitespace();
                if let (Some(k), Some(v)) = (it.next(), it.next()) {
                    map.insert(k.trim_end_matches(':').to_string(), v.to_string());
                }
            }
            map
        })
        .unwrap_or_default();
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "~".into());

    Json(json!({
        "cpus": cpus,
        "loadavg": load,
        "memory": mem,
        "cwd": cwd,
        "tokens": state.tokens.load(std::sync::atomic::Ordering::SeqCst),
        "latency_ms": state.latency_ms.load(std::sync::atomic::Ordering::SeqCst),
        "uptime_seconds": state.uptime_seconds(),
        "provider": state.provider.read().unwrap().clone(),
        "model": state.model.read().unwrap().clone(),
        "connected": state.connected.load(std::sync::atomic::Ordering::SeqCst),
    }))
}

// ---------------------------------------------------------------------------
// Tool inventory
// ---------------------------------------------------------------------------

async fn api_tools(State(state): State<Arc<DaemonState>>) -> Json<Value> {
    let tools = state.registry.list_tools();
    Json(json!({
        "count": tools.len(),
        "tools": tools.iter().map(|t| json!({
            "name": t.name,
            "description": t.description,
            "parameters": t.parameters,
        })).collect::<Vec<_>>(),
    }))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::vault::Vault;
    use crate::config::settings::DiadConfig;
    use crate::providers::ProviderFactory;
    use tower::util::ServiceExt;

    fn test_state() -> Arc<DaemonState> {
        let vault = Arc::new(Vault::new().expect("vault"));
        let config = Arc::new(DiadConfig::default());
        let factory = Arc::new(ProviderFactory::new(
            Arc::clone(&vault),
            Arc::clone(&config),
        ));
        let registry = Arc::new(crate::tools::create_default_registry());
        Arc::new(DaemonState::new(config, vault, factory, registry))
    }

    #[tokio::test]
    async fn test_version_endpoint() {
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/version")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["name"], "diad-agent");
        assert_eq!(v["protocol"], PROTOCOL_VERSION);
    }

    #[tokio::test]
    async fn test_status_endpoint() {
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/status")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["provider"], "anthropic");
        assert!(v["connected"].is_boolean());
    }

    #[tokio::test]
    async fn test_providers_endpoint() {
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/providers")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        let providers = v["providers"].as_array().expect("providers array");
        assert!(providers.len() >= 10);
        let names: Vec<&str> = providers
            .iter()
            .filter_map(|p| p["name"].as_str())
            .collect();
        for expected in ["anthropic", "gemini", "openai", "kimi", "deepseek"] {
            assert!(names.contains(&expected), "missing {expected}");
        }
    }

    #[tokio::test]
    async fn test_provider_switch_unknown() {
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/provider/switch")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&json!({ "name": "nope" })).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_config_endpoint_masks_secrets() {
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/config")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();

        // Public config surface is present.
        assert_eq!(v["default_provider"], "anthropic");
        assert!(v["allowed_paths"].is_array());
        assert_eq!(v["stream_tokens"], json!(true));
        assert_eq!(v["confirm_destructive"], json!(true));
        assert!(v["keybindings"].is_object());
        assert_eq!(v["max_history"], json!(100));
        assert_eq!(v["auto_save_interval"], json!(0));
        assert!(v["storage"].is_object());

        // Provider credentials must NEVER be exposed.
        let providers = v["providers"].as_array().expect("providers array");
        for p in providers {
            assert!(p.get("auth_header").is_none(), "auth_header leaked: {p}");
            assert!(p.get("api_key").is_none(), "api_key leaked: {p}");
            assert!(p.get("key").is_none(), "key leaked: {p}");
            assert!(p["name"].is_string());
            assert!(p["type"].is_string());
            assert!(p["configured"].is_boolean());
        }
        // No secret material anywhere in the serialized config.
        let serialized = serde_json::to_string(&v).unwrap().to_lowercase();
        for needle in ["sk-ant", "api_key", "auth_header", "bearer "] {
            assert!(
                !serialized.contains(needle),
                "secret pattern {needle:?} leaked"
            );
        }
    }

    #[tokio::test]
    async fn test_provider_switch_known() {
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/provider/switch")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&json!({ "name": "openai" })).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["provider"], "openai");
    }

    #[tokio::test]
    async fn test_chat_empty_rejected() {
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/chat")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&json!({ "text": "   " })).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_file_list_forbidden_outside_sandbox() {
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/files/list?path=/etc/shadow")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // /etc/shadow is outside the default sandbox allowlist.
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_file_list_home() {
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/files/list?path=~")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert!(v["entries"].is_array());
    }

    #[tokio::test]
    async fn test_terminal_new() {
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/terminal/new")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_tools_inventory() {
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/tools")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert!(v["count"].as_u64().unwrap() > 50);
    }

    #[tokio::test]
    async fn test_system_stats() {
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/system/stats")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert!(v["cpus"].as_u64().unwrap() >= 1);
    }

    // -----------------------------------------------------------------------
    // Phase 4: Git diff/log/branch + debug sessions + projects
    // -----------------------------------------------------------------------

    /// Create a throwaway git repo with one committed file, then dirty it.
    fn make_git_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path();
        let run = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(p)
                .args(args)
                .output()
                .expect("git runs");
            assert!(
                out.status.success(),
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&out.stderr)
            );
        };
        run(&["init", "-q"]);
        run(&["config", "user.email", "diad-test@local"]);
        run(&["config", "user.name", "DIAD Test"]);
        std::fs::write(p.join("alpha.txt"), "hello\n").unwrap();
        run(&["add", "alpha.txt"]);
        run(&["commit", "-q", "-m", "init commit"]);
        // Dirty it.
        std::fs::write(p.join("alpha.txt"), "hello world\n").unwrap();
        dir
    }

    #[tokio::test]
    async fn test_git_status_diff_log_branch() {
        let dir = make_git_repo();
        let path = dir.path().display().to_string();
        let app = api_router().with_state(test_state());

        // status
        let resp = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/git/status?path={path}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["dirty"], true);
        assert!(!v["branch"].as_str().unwrap_or("").is_empty());

        // diff
        let resp = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/git/diff?path={path}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        let diff = v["diff"].as_str().unwrap();
        assert!(diff.contains("+hello world"), "diff missing change: {diff}");
        assert!(diff.contains("alpha.txt"), "diff missing file: {diff}");
        // F-18: the --stat subprocess must run inside the same blocking()
        // closure as the diff — never on the async runtime. Asserting the
        // field is populated exercises that exact spawn path end-to-end.
        let stat = v["stat"].as_str().unwrap_or("");
        assert!(
            stat.contains("alpha.txt") && stat.contains("1 file changed"),
            "diff --stat missing expected summary: {stat:?}"
        );

        // log
        let resp = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/git/log?path={path}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        let commits = v["commits"].as_array().unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0]["subject"], "init commit");
        assert!(!commits[0]["short"].as_str().unwrap_or("").is_empty());

        // branch
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/git/branch?path={path}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert!(!v["current"].as_str().unwrap_or("").is_empty());
        assert!(!v["branches"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_git_diff_staged_works() {
        // F-18 regression: the staged-diff branch (`git diff --cached`) runs
        // both subprocesses off the async runtime; stage a change and verify
        // both `diff` and `stat` come back populated.
        let dir = make_git_repo();
        let path = dir.path().display().to_string();
        let app = api_router().with_state(test_state());
        let _ = std::process::Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(["add", "alpha.txt"])
            .output()
            .expect("git add runs");
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/git/diff?path={path}&staged=true"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["staged"], true);
        assert!(
            v["diff"].as_str().unwrap_or("").contains("+hello world"),
            "staged diff missing change"
        );
        assert!(
            v["stat"].as_str().unwrap_or("").contains("alpha.txt"),
            "staged diff --stat missing file"
        );
    }

    #[tokio::test]
    async fn test_git_not_a_repo() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().display().to_string();
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/git/status?path={path}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn test_git_sandbox_denied() {
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/git/status?path=/etc")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_debug_state_unknown_session() {
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/debug/state")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&json!({ "session": "dbg-999" })).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_debug_sessions_empty() {
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/debug/sessions")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["sessions"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_debug_start_invalid_program() {
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/debug/session")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&json!({ "program": "   " })).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_projects_list() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("myproj")).unwrap();
        std::fs::write(
            dir.path().join("myproj/Cargo.toml"),
            "[package]\nname = \"x\"\n",
        )
        .unwrap();
        let root = dir.path().display().to_string();
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/projects/list?root={root}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        let projects = v["projects"].as_array().unwrap();
        let mine = projects.iter().find(|p| p["name"] == "myproj");
        assert!(mine.is_some(), "project not found: {projects:?}");
        assert_eq!(mine.unwrap()["kind"], "rust");
    }

    // -----------------------------------------------------------------------
    // Phase 5: AI intelligence (usage, recall, session export/delete)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_ai_usage_empty() {
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/ai/usage")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["total_requests"], 0);
        assert_eq!(v["est_cost_usd"], 0.0);
        assert!(v["by_provider"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_sessions_list_empty_and_export_missing() {
        let app = api_router().with_state(test_state());
        let resp = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/sessions/list")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        // Sessions live in the shared ~/.diad store and tests run in parallel,
        // so other tests may legitimately add sessions. Assert shape, not count.
        assert!(v["sessions"].as_array().is_some());

        // Export of a non-existent session → 404
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/sessions/export?id=does-not-exist")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_sessions_delete_requires_confirm() {
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/sessions/delete")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&json!({ "id": "x", "confirm": false })).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn test_ai_recall_requires_query() {
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/ai/recall?q=")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert!(v["hits"].as_array().unwrap().is_empty());
    }

    // -----------------------------------------------------------------------
    // Phase 6: Collaboration (notes, presence, announce, session import)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_notes_roundtrip() {
        let app = api_router().with_state(test_state());

        // list starts empty
        let resp = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/notes/list")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // write
        let resp = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/notes/write")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&json!({ "name": "agenda", "content": "# hi" }))
                            .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["name"], "agenda");
        assert!(v["size"].as_u64().unwrap() > 0);

        // read
        let resp = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/notes/read?name=agenda")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["content"], "# hi");

        // list now has one note
        let resp = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/notes/list")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["notes"].as_array().unwrap().len(), 1);
        assert_eq!(v["notes"][0]["name"], "agenda");

        // delete without confirm → 422
        let resp = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/notes/delete")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&json!({ "name": "agenda", "confirm": false })).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

        // delete with confirm
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/notes/delete")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&json!({ "name": "agenda", "confirm": true })).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_notes_traversal_rejected() {
        let app = api_router().with_state(test_state());
        let resp = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/notes/write")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&json!({ "name": "../evil", "content": "x" })).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // read of a traversal name → 404 (sanitized to nothing)
        let resp = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/notes/read?name=../../etc/passwd")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_collab_presence_and_announce() {
        let app = api_router().with_state(test_state());

        // presence (no clients connected in tests → 0 frontends)
        let resp = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/collab/presence")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert!(v["frontends"].is_number());
        assert!(v["sessions"].is_number());
        assert!(v["agents"].is_array());

        // announce empty → 400
        let resp = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/collab/announce")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&json!({ "message": "   " })).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // announce ok
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/collab/announce")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&json!({ "message": "meeting at 3" })).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["message"], "meeting at 3");
        assert_eq!(v["sent"], 0);
    }

    #[tokio::test]
    async fn test_sessions_import_export_roundtrip() {
        let app = api_router().with_state(test_state());

        // Build a minimal shared session document (as exported by another
        // DIAD machine) and import it.
        let shared = json!({
            "format": "diad-session",
            "version": 1,
            "session": {
                "id": "remote-id-123",
                "title": "Shared planning",
                "active_provider": "gemini",
                "active_model": "gemini-2.5-flash",
                "messages": [
                    { "role": "user", "content": "plan the release" },
                    { "role": "assistant", "content": "Here is the plan." }
                ]
            }
        });

        let resp = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/sessions/import")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(serde_json::to_vec(&shared).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["title"], "Shared planning");
        assert_eq!(v["messages"], 2);
        let new_id = v["id"].as_str().unwrap().to_string();
        assert_ne!(new_id, "remote-id-123", "import must mint a fresh id");

        // Export as JSON → round-trips the same messages
        let resp = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/sessions/export-json?id={new_id}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["format"], "diad-session");
        assert_eq!(v["session"]["messages"].as_array().unwrap().len(), 2);
        assert_eq!(v["session"]["title"], "Shared planning");

        // Import with garbage body → 400
        let resp = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/sessions/import")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&json!({ "session": [1, 2, 3] })).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // List reflects the import (may share the store with parallel tests,
        // so only require that OUR session is present).
        let resp = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/sessions/list")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        let sessions = v["sessions"].as_array().unwrap();
        assert!(
            sessions.iter().any(|s| s["title"] == "Shared planning"),
            "imported session must be listed"
        );

        // Clean up so we never leave persisted state behind for other tests.
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/sessions/delete")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&json!({ "id": new_id, "confirm": true })).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_sessions_export_json_missing() {
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/sessions/export-json?id=nope")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    // RBRO-004 WP-04 — /api/v1/launch + /launch/status.

    #[tokio::test]
    async fn test_launch_dry_run_ok() {
        // DIAD_OPEN_DRYRUN=1 is the test hook: nothing is spawned, the
        // endpoint acknowledges and returns the app name.
        unsafe { std::env::set_var("DIAD_OPEN_DRYRUN", "1") };
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/launch")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&json!({ "app": "browser" })).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["app"], "browser");
        assert_eq!(v["dry_run"], true);
        std::env::remove_var("DIAD_OPEN_DRYRUN");
    }

    #[tokio::test]
    async fn test_launch_unknown_app_400() {
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/launch")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&json!({ "app": "minecraft" })).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_launch_status_shape() {
        // No sway / no flatpak in the test sandbox: the endpoint must still
        // answer with a well-formed document (browser falls back to
        // "chromium", installed false, open empty).
        let app = api_router().with_state(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/launch/status")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert!(v["browser"].is_string());
        assert!(v["installed"]["telegram"].is_boolean());
        assert!(v["installed"]["slack"].is_boolean());
        assert!(v["open"].is_array());
    }

    #[tokio::test]
    async fn test_open_chat_apps_parses_sway_tree() {
        // Sway tree snippet: telegram open (flatpak), slack closed, a
        // chromium "Slack" web window counts as slack.
        let tree = json!({
            "nodes": [{
                "app_id": null,
                "name": "root",
                "nodes": [
                    { "app_id": "org.telegram.desktop", "name": "Telegram", "nodes": [] },
                    { "app_id": "chromium", "name": "Slack", "nodes": [], "floating_nodes": [] },
                    { "app_id": "chromium", "name": "DIAD Launchpad", "nodes": [] }
                ],
                "floating_nodes": []
            }]
        });
        // Feed the tree through the same predicate by rendering it back to
        // JSON bytes and calling the parser directly.
        let raw = serde_json::to_vec(&tree).unwrap();
        let mut open = parse_open_from_bytes(&raw);
        open.sort();
        assert_eq!(open, vec!["slack", "telegram"]);
    }

    // RBRO-004 WP-04 — static icon pages must NEVER fall back to the SPA:
    // /apps and /apps.html serve apps.html (real title, text/html), the SPA
    // assets keep their content types, and unknown paths fall back to the
    // SPA (client-side routing) as text/html.

    #[tokio::test]
    async fn test_apps_page_serves_standalone_html() {
        let app = crate::core::server::router(test_state());
        let resp = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/apps")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = resp.status();
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        assert_eq!(status, StatusCode::OK);
        assert!(
            ct.starts_with("text/html"),
            "/apps must be text/html, got {ct}"
        );
        let text = String::from_utf8_lossy(&body).to_string();
        assert!(
            text.contains("DIAD Apps"),
            "/apps must contain the DIAD Apps title, got: {text}"
        );
        assert!(
            !text.contains("DIAD AGENT"),
            "/apps must not serve the SPA (DIAD AGENT), got: {text}"
        );
    }

    #[tokio::test]
    async fn test_launch_page_serves_standalone_html() {
        let app = crate::core::server::router(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/launch")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = resp.status();
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        assert_eq!(status, StatusCode::OK);
        assert!(
            ct.starts_with("text/html"),
            "/launch must be text/html, got {ct}"
        );
        let text = String::from_utf8_lossy(&body).to_string();
        assert!(
            text.contains("DIAD Launchpad"),
            "/launch must contain the DIAD Launchpad title, got: {text}"
        );
        assert!(
            !text.contains("DIAD AGENT"),
            "/launch must not serve the SPA (DIAD AGENT), got: {text}"
        );
    }

    #[tokio::test]
    async fn test_launch_html_path_serves_standalone_html() {
        let app = crate::core::server::router(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/launch.html")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = resp.status();
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        assert_eq!(status, StatusCode::OK);
        assert!(
            ct.starts_with("text/html"),
            "/launch.html must be text/html, got {ct}"
        );
        let text = String::from_utf8_lossy(&body).to_string();
        assert!(
            text.contains("DIAD Launchpad"),
            "/launch.html must serve the standalone page, got: {text}"
        );
    }

    #[tokio::test]
    async fn test_apps_html_path_serves_standalone_html() {
        let app = crate::core::server::router(test_state());
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/apps.html")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = resp.status();
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        assert_eq!(status, StatusCode::OK);
        assert!(
            ct.starts_with("text/html"),
            "/apps.html must be text/html, got {ct}"
        );
        let text = String::from_utf8_lossy(&body).to_string();
        assert!(
            text.contains("DIAD Apps"),
            "/apps.html must serve the standalone page, got: {text}"
        );
    }

    #[tokio::test]
    async fn test_spa_assets_keep_content_type() {
        let app = crate::core::server::router(test_state());
        // The SPA's hashed JS bundle must keep its real content type and
        // not be swallowed by an octet-stream fallback.
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/assets/index-BV-xNFif.js")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        assert!(
            ct.contains("javascript"),
            "/assets/index-BV-xNFif.js must be javascript, got {ct}"
        );
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        assert!(
            body.len() > 1000,
            "JS bundle must be the real bundle, got {} bytes",
            body.len()
        );
    }

    #[tokio::test]
    async fn test_unknown_static_path_falls_back_to_spa() {
        let app = crate::core::server::router(test_state());
        // Unknown paths fall back to the SPA (client-side routing) and must
        // be served as text/html — never octet-stream.
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/no-such-file.xyz")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        assert!(
            ct.starts_with("text/html"),
            "SPA fallback must be text/html, got {ct}"
        );
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let text = String::from_utf8_lossy(&body).to_string();
        assert!(
            text.contains("DIAD AGENT"),
            "SPA fallback must serve the DIAD AGENT app, got: {text}"
        );
    }
}
