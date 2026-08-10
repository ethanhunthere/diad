//! DIAD AGENT daemon — HTTP + WebSocket transport.
//!
//! Serves the embedded GUI (`frontend/dist` via rust-embed) and
//! accepts WebSocket connections from both frontends, routing
//! each client message to the brain in `daemon`. Binds loopback.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::State;
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
struct Assets;

// ---------------------------------------------------------------------------
// WebSocket handler
// ---------------------------------------------------------------------------

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<DaemonState>>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<DaemonState>) {
    let (mut sender, mut receiver) = socket.split();

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
        streaming: state.streaming.load(Ordering::SeqCst),
        daemon_version: env!("CARGO_PKG_VERSION").into(),
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

async fn static_handler(State(_state): State<Arc<DaemonState>>, path: String) -> Response {
    let clean = path.trim_start_matches('/');
    let asset = Assets::get(clean).or_else(|| Assets::get("index.html"));
    match asset {
        Some(file) => {
            // Hashed build assets are content-addressed: cache for a year
            // (spec 6.6). Everything else revalidates.
            let cache = if clean.starts_with("assets/") {
                "public, max-age=31536000, immutable"
            } else {
                "public, max-age=300"
            };
            (
                [
                    ("Content-Type", content_type(clean)),
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

/// Build the axum router.
pub fn router(state: Arc<DaemonState>) -> Router {
    Router::new()
        .route("/", get(index_html))
        .route("/health", get(|| async { "ok" }))
        .route("/ws", get(ws_handler))
        .nest("/api/v1", super::rest::api_router())
        .route("/{*path}", get(static_handler))
        .with_state(state)
}

/// Bind loopback and serve forever. Returns when the listener stops.
pub async fn serve(state: Arc<DaemonState>, port: u16) -> anyhow::Result<()> {
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
