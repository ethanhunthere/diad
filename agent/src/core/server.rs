//! DIAD AGENT daemon — HTTP + WebSocket transport.
//!
//! Serves the embedded GUI (`frontend/dist` via rust-embed) and
//! accepts WebSocket connections from both frontends, routing
//! each client message to the brain in `daemon`. Binds loopback.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use rust_embed::RustEmbed;
use tokio::sync::mpsc::unbounded_channel;

use super::daemon::{handle_client_msg, Conn, DaemonState, PromptBroker};
use super::protocol::{Envelope, PROTOCOL_VERSION, ServerMsg};

// ---------------------------------------------------------------------------
// Embedded GUI bundle
// ---------------------------------------------------------------------------

#[derive(RustEmbed)]
#[folder = "../frontend/dist"]
struct Assets;

// ---------------------------------------------------------------------------
// WebSocket handler
// ---------------------------------------------------------------------------

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<DaemonState>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<DaemonState>) {
    let (mut sender, mut receiver) = socket.split();

    // This task drains ServerMsg → WS text frames.
    let (conn_tx, mut conn_rx) = unbounded_channel::<ServerMsg>();
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
    let _ = conn_tx.send(greeting);
    state.broadcast_status();

    // Inbound loop.
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            WsMessage::Text(text) => {
                let env: Envelope = match serde_json::from_str(&text) {
                    Ok(e) => e,
                    Err(err) => {
                        let _ = conn_tx.send(ServerMsg::Error {
                            id: None,
                            message: format!("protocol error: {err}"),
                        });
                        continue;
                    }
                };
                if env.v != PROTOCOL_VERSION {
                    let _ = conn_tx.send(ServerMsg::Error {
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
        [("Content-Type", "text/html; charset=utf-8")],
        Assets::get("index.html")
            .map(|f| f.data.into_owned())
            .unwrap_or_else(|| "<h1>DIAD AGENT - frontend not built</h1>".as_bytes().to_vec()),
    )
}

async fn static_handler(State(_state): State<Arc<DaemonState>>, path: String) -> Response {
    let clean = path.trim_start_matches('/');
    let asset = Assets::get(clean).or_else(|| Assets::get("index.html"));
    match asset {
        Some(file) => (
            [("Content-Type", content_type(clean))],
            file.data.into_owned(),
        )
            .into_response(),
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

    // Background auto-save: persist the in-memory session every
    // `config.auto_save_interval` seconds so a crash never loses more than
    // that window of conversation. Runs for the daemon's lifetime.
    {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            loop {
                let interval = {
                    let cfg = state.config.read().unwrap();
                    // 0 (or absurdly small) disables auto-save.
                    cfg.auto_save_interval.max(1)
                };
                tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
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

    axum::serve(listener, router(state)).await?;
    Ok(())
}
