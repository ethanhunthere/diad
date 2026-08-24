//! DIAD AGENT daemon — HTTP + WebSocket transport.
//!
//! Serves the embedded GUI (`frontend/dist` via rust-embed) and
//! accepts WebSocket connections from both frontends, routing
//! each client message to the brain in `daemon`. Binds loopback.

use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock};

use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use rust_embed::RustEmbed;
use tokio::sync::mpsc::channel;

use super::daemon::{handle_client_msg, Conn, DaemonState, PromptBroker};
use super::protocol::{Envelope, ServerMsg, PROTOCOL_VERSION};

// ---------------------------------------------------------------------------
// Embedded GUI bundle
// ---------------------------------------------------------------------------

#[derive(RustEmbed)]
#[folder = "../frontend/dist"]
pub(crate) struct Assets;

// ---------------------------------------------------------------------------
// Security: localhost bearer-token auth for the control plane
// ---------------------------------------------------------------------------

/// Process-wide API token (64 hex chars = 32 random bytes).
static API_TOKEN: OnceLock<String> = OnceLock::new();

/// Generate a 32-byte random token, store it process-wide, and persist it to
/// a 0600 file in the runtime directory (/run/diad/api-token, falling back to
/// XDG_RUNTIME_DIR). Local clients read the file to authenticate.
pub fn init_api_token() -> anyhow::Result<&'static str> {
    if let Some(existing) = API_TOKEN.get() {
        return Ok(existing);
    }
    // 32 random bytes via two UUIDv4s (uuid crate is already a dependency).
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    bytes[16..].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();

    // Prefer the user's runtime dir; fall back to /run/diad (root/systemd).
    let mut dir = None;
    if let Some(xdg) = std::env::var_os("XDG_RUNTIME_DIR") {
        let p = std::path::PathBuf::from(xdg);
        if std::fs::create_dir_all(&p).is_ok() {
            dir = Some(p);
        }
    }
    if dir.is_none() {
        let p = std::path::PathBuf::from("/run/diad");
        if std::fs::create_dir_all(&p).is_ok() {
            dir = Some(p);
        }
    }

    if let Some(dir) = &dir {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
        }
        let path = dir.join("api-token");
        // Write with 0600 so the token is never briefly world-readable.
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&path)?;
            f.write_all(hex.as_bytes())?;
        }
        #[cfg(not(unix))]
        std::fs::write(&path, &hex)?;
        tracing::info!("API token written to {}", path.display());
    } else {
        tracing::warn!("no runtime dir available — API token kept in memory only");
    }

    Ok(API_TOKEN.get_or_init(|| hex))
}

/// Constant-time-ish comparison (length + XOR fold) to avoid trivially
/// leaking the token via timing.
fn token_matches(presented: &str, expected: &str) -> bool {
    if presented.len() != expected.len() {
        return false;
    }
    presented
        .bytes()
        .zip(expected.bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

/// Extract the bearer token from an `Authorization: Bearer <t>` header.
fn bearer_from_headers(headers: &axum::http::HeaderMap) -> Option<String> {
    let v = headers.get(axum::http::header::AUTHORIZATION)?.to_str().ok()?;
    let rest = v.strip_prefix("Bearer ")?;
    Some(rest.trim().to_string())
}

/// Whether this request path is exempt from token auth. Only /health is.
fn auth_exempt(path: &str) -> bool {
    path == "/health"
}

/// Check the token from header or `?token=` query param. WS clients may also
/// authenticate with their first message instead.
fn request_token(headers: &axum::http::HeaderMap, query: Option<&str>) -> Option<String> {
    if let Some(t) = bearer_from_headers(headers) {
        return Some(t);
    }
    query.and_then(|q| {
        q.split('&').find_map(|p| {
            p.split_once('=')
                .filter(|(k, _)| *k == "token")
                .map(|(_, v)| v.to_string())
        })
    })
}

/// Axum middleware enforcing bearer-token auth on everything except /health.
/// Accepts `Authorization: Bearer <token>` or `?token=<token>`.
async fn auth_middleware(
    req: axum::extract::Request,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    if auth_exempt(&path) {
        return next.run(req).await;
    }

    let expected = match API_TOKEN.get() {
        Some(t) => t.as_str(),
        // Token not initialized: fail closed.
        None => return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let query = req.uri().query();
    let ok = match request_token(req.headers(), query) {
        Some(t) => token_matches(&t, expected),
        None => false,
    };

    if ok {
        next.run(req).await
    } else {
        tracing::warn!(path = %path, "rejected request without valid API token");
        axum::http::StatusCode::UNAUTHORIZED.into_response()
    }
}


/// Validates whether an HTTP Origin header comes from an allowed local source.
/// Non-browser clients (TUI, foot, supervisor, curl) send no Origin (None) -> allowed.
/// Browser-originated requests must strictly match loopback port 7860.
pub fn origin_allowed(headers: &axum::http::HeaderMap) -> bool {
    match headers.get(axum::http::header::ORIGIN) {
        None => true, // non-browser clients (TUI, curl, supervisor) send no Origin
        Some(v) => matches!(
            v.to_str().unwrap_or(""),
            "http://127.0.0.1:7860"
                | "http://localhost:7860"
                | "https://127.0.0.1:7860"
                | "https://localhost:7860"
        ),
    }
}

/// Validates the Host header against loopback. Browsers ALWAYS send Host —
/// including DNS-rebinding attacks, where the Host is the attacker's domain.
/// A missing Host (HTTP/1.0 clients, tests) is allowed; a mismatched one is
/// rejected. Blocks rebinding for the whole control plane.
pub fn host_allowed(headers: &axum::http::HeaderMap) -> bool {
    match headers.get(axum::http::header::HOST) {
        None => true,
        Some(v) => matches!(
            v.to_str().unwrap_or(""),
            "127.0.0.1:7860" | "localhost:7860"
        ),
    }
}

// ---------------------------------------------------------------------------
// WebSocket handler
// ---------------------------------------------------------------------------

async fn ws_handler(
    headers: axum::http::HeaderMap,
    axum::extract::RawQuery(query): axum::extract::RawQuery,
    ws: WebSocketUpgrade,
    State(state): State<Arc<DaemonState>>,
) -> Response {
    if !origin_allowed(&headers) || !host_allowed(&headers) {
        tracing::warn!("rejected cross-origin WebSocket connection attempt");
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }

    // WS clients may present the token up-front (Authorization header or
    // ?token= query param); otherwise they MUST send it as their first
    // message before any other traffic is processed.
    let pre_authed = match API_TOKEN.get() {
        Some(expected) => request_token(&headers, query.as_deref())
            .map(|t| token_matches(&t, expected))
            .unwrap_or(false),
        None => false,
    };

    let auth_via_message = !pre_authed && API_TOKEN.get().is_some();
    if API_TOKEN.get().is_none() {
        // Fail closed: no token initialized.
        return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response();
    }

    ws.on_upgrade(move |socket| handle_socket(socket, state, auth_via_message))
}

async fn handle_socket(socket: WebSocket, state: Arc<DaemonState>, auth_pending: bool) {
    let (mut sender, mut receiver) = socket.split();

    // If the client did not authenticate during the upgrade, its FIRST
    // message must be exactly the API token. Anything else closes the
    // connection.
    if auth_pending {
        match tokio::time::timeout(std::time::Duration::from_secs(5), receiver.next()).await {
            Ok(Some(Ok(WsMessage::Text(text)))) => {
                let t = text.trim();
                let presented = t.strip_prefix("token:").unwrap_or(t);
                let ok = token_matches(presented, API_TOKEN.get().map(|s| s.as_str()).unwrap_or(""));
                if !ok {
                    tracing::warn!("ws client failed first-message token auth — closing");
                    return;
                }
                tracing::info!("ws client authenticated via first message");
            }
            _ => {
                tracing::warn!("ws client did not complete token auth — closing");
                return;
            }
        }
    }

    // This task drains ServerMsg → WS text frames. The channel is BOUNDED:
    // a slow or malicious client that stops draining gets frames dropped
    // (try_send in broadcast/forward paths) instead of an unbounded queue
    // in daemon memory (spec 2.1).
    let (conn_tx, mut conn_rx) = channel::<ServerMsg>(4096);
    let writer = tokio::spawn(async move {
        while let Some(msg) = conn_rx.recv().await {
            let text = match serde_json::to_string(&msg) {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("failed to serialize server msg: {e}");
                    continue;
                }
            };
            if sender.send(WsMessage::Text(text.into())).await.is_err() {
                break;
            }
        }
    });

    let conn = Conn {
        tx: conn_tx.clone(),
        prompts: PromptBroker::default(),
    };

    state.register_client(conn_tx.clone());

    // Greeting.
    let greeting = ServerMsg::Connected {
        v: PROTOCOL_VERSION,
        provider: state.provider.read().unwrap().clone(),
        model: state.model.read().unwrap().clone(),
        connected: state.connected.load(Ordering::SeqCst),
        streaming: state.streaming.load(Ordering::SeqCst) > 0,
        daemon_version: env!("CARGO_PKG_VERSION").into(),
        config: crate::core::daemon::config_snapshot(&state.config.read().unwrap()),
    };
    let _ = conn_tx.try_send(greeting);
    state.broadcast_status();

    // Inbound loop.
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            WsMessage::Text(text) => {
                let env: Envelope = match serde_json::from_str(&text) {
                    Ok(e) => e,
                    Err(err) => {
                        let _ = conn_tx.try_send(ServerMsg::Error {
                            id: None,
                            message: format!("protocol error: {err}"),
                        });
                        continue;
                    }
                };
                if env.v != PROTOCOL_VERSION {
                    let _ = conn_tx.try_send(ServerMsg::Error {
                        id: None,
                        message: format!(
                            "protocol version mismatch: client {} != server {}",
                            env.v, PROTOCOL_VERSION
                        ),
                    });
                    continue;
                }
                handle_client_msg(&state, &conn, env.msg).await;
            }
            WsMessage::Close(_) => break,
            _ => {}
        }
    }

    state.unregister_client(&conn_tx);
    writer.abort();
}

fn content_type(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "map" => "application/json",
        _ => "application/octet-stream",
    }
}

async fn index_html() -> impl IntoResponse {
    (
        [
            ("Content-Type", "text/html; charset=utf-8"),
            // Entry point must revalidate so a new daemon build swaps the
            // hashed asset references (spec 6.6).
            ("Cache-Control", "no-cache"),
        ],
        Assets::get("index.html")
            .map(|f| f.data.into_owned())
            .unwrap_or_else(|| {
                "<h1>DIAD AGENT - frontend not built</h1>"
                    .as_bytes()
                    .to_vec()
            }),
    )
}

async fn static_handler(
    State(_state): State<Arc<DaemonState>>,
    Path(path): Path<String>,
) -> Response {
    let clean = path.trim_start_matches('/');
    // /apps and /launch are standalone icon pages (RBRO-004 WP-04/WP-05):
    // resolve them to their own .html files and NEVER fall back to the SPA.
    // Serving the SPA here was the root cause of the workspace floods — the
    // pages must keep their real titles for the sway assign rules to match.
    let (asset_name, asset) = match clean {
        "apps" | "apps.html" => ("apps.html", Assets::get("apps.html")),
        "launch" => ("launch.html", Assets::get("launch.html")),
        _ => match Assets::get(clean) {
            Some(file) => (clean, Some(file)),
            // Unknown path → SPA fallback (client-side routing). Serve it
            // under the index.html name so the content type is correct.
            None => ("index.html", Assets::get("index.html")),
        },
    };
    match asset {
        Some(file) => {
            // Hashed build assets are content-addressed: cache for a year
            // (spec 6.6). Everything else revalidates.
            let cache = if asset_name.starts_with("assets/") {
                "public, max-age=31536000, immutable"
            } else {
                "public, max-age=300"
            };
            (
                [
                    ("Content-Type", content_type(asset_name)),
                    ("Cache-Control", cache),
                ],
                file.data.into_owned(),
            )
                .into_response()
        }
        None => (axum::http::StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

// ---------------------------------------------------------------------------
// Router + serve
// ---------------------------------------------------------------------------

/// Bootstrap endpoint for the embedded GUI: returns the API token so the
/// browser client can authenticate REST + WS calls. Only same-origin GUI
/// contexts can read the response (origin/host validated below; no CORS
/// headers are ever emitted, so cross-origin JS cannot read it).
async fn gui_token(headers: axum::http::HeaderMap) -> Response {
    if !origin_allowed(&headers) || !host_allowed(&headers) {
        tracing::warn!("rejected /gui-token request from disallowed origin/host");
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }
    match API_TOKEN.get() {
        Some(t) => axum::Json(serde_json::json!({ "token": t })).into_response(),
        None => axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

/// Build the axum router.
pub fn router(state: Arc<DaemonState>) -> Router {
    // Protected plane: API + WebSocket (token-authenticated).
    let protected = Router::new()
        .route("/ws", get(ws_handler))
        .nest("/api/v1", super::rest::api_router())
        .layer(axum::middleware::from_fn(auth_middleware));

    // Public plane: embedded GUI + health probe + GUI token bootstrap.
    // /gui-token hands the embedded GUI its bearer token. It is guarded by
    // origin_allowed + host_allowed (same checks as the WS upgrade): a page
    // served from any other origin is refused outright, and cross-origin
    // reads are blocked because no CORS headers are ever emitted. Local
    // non-browser processes can just read the 0600 token file instead.
    Router::new()
        .route("/", get(index_html))
        .route("/health", get(|| async { "ok" }))
        .route("/gui-token", get(gui_token))
        .route("/{*path}", get(static_handler))
        .merge(protected)
        .with_state(state)
}

/// Bind loopback and serve forever. Returns when the listener stops.
pub async fn serve(state: Arc<DaemonState>, port: u16) -> anyhow::Result<()> {
    // Generate + persist the API token before accepting any connections.
    init_api_token()?;

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("DIAD AGENT daemon listening on http://{addr}");

    // Register every periodic task up front (spec 3.5): the idle engine
    // flips these flags on stage transitions; each loop checks its flag.
    let autosave_flag = state.tasks.register("session-autosave", 300);
    let env_flag = state.tasks.register("env-monitor", 15);

    // Background auto-save: persist the in-memory session every
    // `config.auto_save_interval` seconds so a crash never loses more than
    // that window of conversation. Skipped while idle (flag off).
    {
        let state = Arc::clone(&state);
        let flag = Arc::clone(&autosave_flag);
        tokio::spawn(async move {
            loop {
                let interval = {
                    let cfg = state.config.read().unwrap();
                    // 0 (or absurdly small) disables auto-save.
                    cfg.auto_save_interval.max(1)
                };
                tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
                if !flag.load(Ordering::SeqCst) {
                    continue; // idle: no clients, nothing new to save
                }
                if let Ok(session) = state.session.lock() {
                    if !session.messages.is_empty() {
                        if let Ok(manager) = state.sessions.lock() {
                            let _ = manager.save_session(&session);
                        }
                    }
                }
            }
        });
    }

    spawn_idle_monitor(Arc::clone(&state));
    spawn_env_monitor(Arc::clone(&state), env_flag);
    spawn_bluetooth_watchdog(Arc::clone(&state));

    axum::serve(listener, router(state)).await?;
    Ok(())
}

/// Idle engine state machine (spec 3.2/3.3). The only task that keeps
/// ticking while idle — it IS the mechanism that deepens and ends idle
/// stages, so it cannot gate on its own flag. One 5 s tick costs one
/// atomic read + one mutex lock (~µs).
fn spawn_idle_monitor(state: Arc<DaemonState>) {
    use super::idle::{IdleStage, STAGE1_AFTER_SECS, STAGE2_AFTER_SECS};

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            if state.clients_count() > 0 {
                continue; // wake is handled in register_client
            }
            let idle_secs = state.last_disconnect.lock().unwrap().elapsed().as_secs();
            let stage = state.idle.get();

            if idle_secs >= STAGE2_AFTER_SECS && stage < IdleStage::Stage2 {
                // Stage 2: stop every periodic task, drop caches, close PTYs.
                state.tasks.set_all_enabled(false);
                state.registry.clear_cache();
                trim_history(&state, 200);
                state.pty.shutdown();
                state.idle.set(IdleStage::Stage2);
                tracing::info!("diad daemon idle (stage2)");
            } else if idle_secs >= STAGE1_AFTER_SECS && stage == IdleStage::Active {
                // Stage 1: stop periodic tasks + drop volatile caches.
                state.tasks.set_all_enabled(false);
                state.registry.clear_cache();
                trim_history(&state, 200);
                state.idle.set(IdleStage::Stage1);
                tracing::info!("diad daemon idle (stage1)");
            }
        }
    });
}

/// Cap in-memory chat history (spec 8.2): keep only the newest `keep`.
fn trim_history(state: &DaemonState, keep: usize) {
    let mut history = state.history.lock().unwrap();
    if history.len() > keep {
        let excess = history.len() - keep;
        history.drain(..excess);
    }
}

/// Lazy bluetooth (spec 4.3): bluetooth.service starts on first tool use
/// (inside the tools themselves) and stops here after 10 idle minutes.
fn spawn_bluetooth_watchdog(state: Arc<DaemonState>) {
    let flag = state.tasks.register("bluetooth-idle-stop", 60);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            if !flag.load(Ordering::SeqCst) {
                continue; // idle stage: nothing to stop, nobody listening
            }
            if crate::tools::hardware::hardware_universal::bluetooth_idle_secs() < 600 {
                continue;
            }
            if let Ok(res) =
                crate::tools::system::exec::run(&["systemctl", "is-active", "bluetooth"])
            {
                if res.success && res.output.trim() == "active" {
                    let _ = crate::tools::system::exec::run_privileged(&[
                        "systemctl",
                        "stop",
                        "bluetooth",
                    ]);
                    tracing::info!("bluetooth: stopped after 10 min idle");
                }
            }
        }
    });
}

/// Battery + thermal awareness (spec 3.7 / 10.x). Reads sysfs directly —
/// no polling daemons involved, and the loop itself is flag-gated so it
/// costs nothing while idle. On battery the tick interval doubles (halved
/// frequency per spec 3.7).
fn spawn_env_monitor(state: Arc<DaemonState>, flag: Arc<std::sync::atomic::AtomicBool>) {
    use crate::core::perf;

    tokio::spawn(async move {
        loop {
            let on_battery = state.on_battery.load(Ordering::SeqCst);
            let secs = if on_battery { 30 } else { 15 };
            tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
            if !flag.load(Ordering::SeqCst) {
                continue; // idle stage: env monitoring suspended too
            }

            // Battery transition (AC ↔ discharging).
            if let Some((battery, pct)) = perf::read_battery_state() {
                let was = state.on_battery.swap(battery, Ordering::SeqCst);
                state.battery_pct.store(pct, Ordering::SeqCst);
                if was != battery {
                    tracing::info!("power: {}", if battery { "on battery" } else { "on AC" });
                }
            }

            // Thermal throttle transition (with hysteresis).
            if let Some(milli_c) = perf::max_thermal_millic() {
                let throttled = state.thermal_throttled.load(Ordering::SeqCst);
                if !throttled && milli_c >= 90_000 {
                    state.thermal_throttled.store(true, Ordering::SeqCst);
                    tracing::warn!("thermal limit — reducing effects");
                    state.broadcast(ServerMsg::System {
                        message: "[system] thermal limit — reducing effects".into(),
                    });
                } else if throttled && milli_c < 80_000 {
                    state.thermal_throttled.store(false, Ordering::SeqCst);
                    tracing::info!("thermal: back under control");
                }
            }
        }
    });
}
