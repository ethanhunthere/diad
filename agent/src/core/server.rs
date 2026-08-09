//! DIAD AGENT daemon — HTTP + WebSocket server.
//!
//! Owns ALL application logic (providers, tools, orchestrator, vault,
//! sessions, offline router, PTY) and exposes it to interchangeable
//! frontends — the React GUI and the ratatui TUI client — over a
//! versioned WebSocket JSON API. The HTTP side serves the built GUI
//! (rust-embed of `frontend/dist`). Binds loopback only.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use rust_embed::RustEmbed;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::auth::vault::Vault;
use crate::config::settings::DiadConfig;
use crate::orchestrator::agent;
use crate::orchestrator::router::{IntentRouter, OfflineIntent};
use crate::orchestrator::session::{Session, SessionManager};
use crate::providers::traits::Message;
use crate::providers::ProviderFactory;
use crate::tools::external;
use crate::tools::hardware;
use crate::tools::ToolRegistry;

use super::debug::DebugManager;
use super::notes::NotesStore;
use super::protocol::{
    ClientMsg, Envelope, FileEntryDto, ProviderEntryDto, PROTOCOL_VERSION, ServerMsg,
};
use super::pty::PtyManager;
use super::usage::UsageTracker;

// ---------------------------------------------------------------------------
// Embedded GUI bundle
// ---------------------------------------------------------------------------

#[derive(RustEmbed)]
#[folder = "../frontend/dist"]
struct Assets;

// ---------------------------------------------------------------------------
// Shared daemon state
// ---------------------------------------------------------------------------

/// Everything the daemon owns. Cloned `Arc`s are passed into handlers.
#[derive(Clone)]
pub struct DaemonState {
    // Connected chat state (one global brain; both frontends share it).
    pub config: Arc<RwLock<DiadConfig>>,
    pub vault: Arc<Vault>,
    pub factory: Arc<ProviderFactory>,
    pub registry: Arc<ToolRegistry>,
    pub pty: Arc<PtyManager>,
    pub sessions: Arc<Mutex<SessionManager>>,
    pub debug: Arc<DebugManager>,
    pub usage: Arc<Mutex<UsageTracker>>,
    /// Shared cross-session notes store (collaboration scratchpad).
    pub notes: Arc<NotesStore>,

    // Active chat state (one global brain; both frontends share it).
    pub provider: Arc<RwLock<String>>,
    pub model: Arc<RwLock<String>>,
    pub connected: Arc<AtomicBool>,
    pub streaming: Arc<AtomicBool>,
    /// User preference: stream tokens live (true) vs deliver the full reply
    /// in one chunk at the end (false). Mirrors `config.stream_tokens` but is
    /// cheap to read on the hot path inside `forward_event`.
    pub stream_tokens: Arc<AtomicBool>,
    pub history: Arc<Mutex<Vec<Message>>>,
    pub session: Arc<Mutex<Session>>,

    // Offline wifi flow: true while we expect the user to type an SSID.
    pub pending_wifi_ssid: Arc<RwLock<bool>>,

    // Stats.
    pub tokens: Arc<AtomicU64>,
    pub latency_ms: Arc<AtomicU64>,

    // Connected frontends (for broadcasts).
    clients: Arc<RwLock<Vec<UnboundedSender<ServerMsg>>>>,
}

impl DaemonState {
    /// Build the daemon state from shared singletons.
    pub fn new(
        config: Arc<DiadConfig>,
        vault: Arc<Vault>,
        factory: Arc<ProviderFactory>,
        registry: Arc<ToolRegistry>,
    ) -> Self {
        let provider = config.default_provider.clone();
        let model = factory.default_model(&provider);
        let connected = factory.has_key(&provider);

        let sessions = Arc::new(Mutex::new(
            SessionManager::new().unwrap_or_else(|_| {
                // Fallback manager pointing at a temp dir so the daemon can
                // always start, even if the sessions dir is unwritable.
                SessionManager::new_with_dir(std::env::temp_dir().join("diad-sessions"))
            }),
        ));
        let session = Arc::new(Mutex::new(Session::new(
            format!("DIAD AGENT — {provider}"),
            &provider,
        )));

        Self {
            config: Arc::new(RwLock::new((*config).clone())),
            vault,
            factory,
            registry,
            pty: Arc::new(PtyManager::new()),
            sessions,
            debug: Arc::new(DebugManager::new()),
            usage: Arc::new(Mutex::new(UsageTracker::new())),
            notes: Arc::new(NotesStore::new()),
            provider: Arc::new(RwLock::new(provider)),
            model: Arc::new(RwLock::new(model)),
            connected: Arc::new(AtomicBool::new(connected)),
            streaming: Arc::new(AtomicBool::new(false)),
            stream_tokens: Arc::new(AtomicBool::new(config.stream_tokens)),
            history: Arc::new(Mutex::new(Vec::new())),
            session,
            pending_wifi_ssid: Arc::new(RwLock::new(false)),
            tokens: Arc::new(AtomicU64::new(0)),
            latency_ms: Arc::new(AtomicU64::new(0)),
            clients: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// The current status snapshot.
    pub fn status_snapshot(&self) -> ServerMsg {
        ServerMsg::Status {
            provider: self.provider.read().unwrap().clone(),
            model: self.model.read().unwrap().clone(),
            connected: self.connected.load(Ordering::SeqCst),
            streaming: self.streaming.load(Ordering::SeqCst),
            daemon_version: env!("CARGO_PKG_VERSION").into(),
        }
    }

    /// Broadcast a message to every connected frontend.
    pub fn broadcast(&self, msg: ServerMsg) {
        let clients = self.clients.read().unwrap();
        for tx in clients.iter() {
            let _ = tx.send(msg.clone());
        }
    }

    /// Number of frontends currently connected (presence).
    pub fn client_count(&self) -> usize {
        self.clients.read().unwrap().len()
    }

    /// Broadcast the current status to all frontends.
    pub fn broadcast_status(&self) {
        self.broadcast(self.status_snapshot());
    }

    fn register_client(&self, tx: UnboundedSender<ServerMsg>) {
        self.clients.write().unwrap().push(tx);
    }

    fn unregister_client(&self, tx: &UnboundedSender<ServerMsg>) {
        self.clients
            .write()
            .unwrap()
            .retain(|c| !c.same_channel(tx));
    }

    /// Async tool execution with a bounded wait (for stats gathering).
    async fn run_tool(&self, name: &str, params: serde_json::Value) -> Option<String> {
        let registry = Arc::clone(&self.registry);
        let name = name.to_string();
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();
        tokio::task::spawn_blocking(move || {
            let r = registry.execute_tool(&name, params);
            let _ = result_tx.send(r);
        });
        match tokio::time::timeout(std::time::Duration::from_secs(3), result_rx).await {
            Ok(Ok(Ok(r))) => Some(r.output),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Per-connection prompt broker
// ---------------------------------------------------------------------------

/// Tracks pending secret/confirmation prompts for ONE frontend connection.
///
/// When the orchestrator asks the user for input (`Event::SecurePromptRequest`
/// / `Event::ConfirmationRequest` / `Event::EditDiffRequest`), the bridge
/// stores the reply channel here keyed by an id, and the frontend answers
/// with `secret.submit` / `tool.approve` / `tool.reject` / `secret.cancel` /
/// `edit.approve` / `edit.reject`.
#[derive(Clone, Default)]
pub struct PromptBroker {
    secrets: Arc<Mutex<HashMap<String, UnboundedSender<String>>>>,
    confirms: Arc<Mutex<HashMap<String, UnboundedSender<bool>>>>,
    diffs: Arc<Mutex<HashMap<String, UnboundedSender<bool>>>>,
}

impl PromptBroker {
    pub fn register_secret(&self, id: String, reply: UnboundedSender<String>) {
        self.secrets.lock().unwrap().insert(id, reply);
    }

    pub fn resolve_secret(&self, id: &str, value: String) -> bool {
        let reply = self.secrets.lock().unwrap().remove(id);
        match reply {
            Some(rx) => {
                let _ = rx.send(value);
                true
            }
            None => false,
        }
    }

    pub fn register_confirm(&self, id: String, reply: UnboundedSender<bool>) {
        self.confirms.lock().unwrap().insert(id, reply);
    }

    pub fn resolve_confirm(&self, id: &str, accepted: bool) -> bool {
        let reply = self.confirms.lock().unwrap().remove(id);
        match reply {
            Some(rx) => {
                let _ = rx.send(accepted);
                true
            }
            None => false,
        }
    }

    pub fn register_diff(&self, id: String, reply: UnboundedSender<bool>) {
        self.diffs.lock().unwrap().insert(id, reply);
    }

    pub fn resolve_diff(&self, id: &str, accepted: bool) -> bool {
        let reply = self.diffs.lock().unwrap().remove(id);
        match reply {
            Some(rx) => {
                let _ = rx.send(accepted);
                true
            }
            None => false,
        }
    }
}

/// A handle to one frontend connection (send + prompt broker).
#[derive(Clone)]
pub struct Conn {
    pub tx: UnboundedSender<ServerMsg>,
    pub prompts: PromptBroker,
}

// ---------------------------------------------------------------------------
// Event bridge: orchestrator Event → WebSocket ServerMsg
// ---------------------------------------------------------------------------

/// Generates unique request ids.
pub(crate) fn next_id(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{n}")
}

/// Convert an orchestrator `Event` into frontend messages through a `Conn`.
///
/// Mirrors what the old monolithic TUI event loop did locally, but over WS.
pub fn forward_event(
    conn: &Conn,
    event: crate::tui::event::Event,
    stream_tokens: Arc<AtomicBool>,
) {
    use crate::tui::event::Event;
    match event {
        Event::StreamChunk(text) => {
            // `stream_tokens=false` (config preference) suppresses per-token
            // delivery; the caller sends the assembled reply in one chunk.
            if stream_tokens.load(Ordering::SeqCst) {
                let _ = conn.tx.send(ServerMsg::Token {
                    id: "chat".into(),
                    text,
                });
            }
        }
        Event::StreamDone => {
            let _ = conn.tx.send(ServerMsg::StreamDone { id: "chat".into() });
        }
        Event::AppendMessage(line) => {
            let _ = conn.tx.send(ServerMsg::System { message: line });
        }
        Event::Error(message) => {
            let _ = conn.tx.send(ServerMsg::Error {
                id: None,
                message,
            });
        }
        Event::ToolResult(output) => {
            let _ = conn.tx.send(ServerMsg::System {
                message: format!("[Tool Result] {output}"),
            });
        }
        Event::SecurePromptRequest { label, reply } => {
            let id = next_id("secret");
            conn.prompts.register_secret(id.clone(), reply);
            let _ = conn.tx.send(ServerMsg::SecretRequest { id, label });
        }
        Event::ConfirmationRequest { label, reply } => {
            let id = next_id("confirm");
            conn.prompts.register_confirm(id.clone(), reply);
            let _ = conn.tx.send(ServerMsg::ConfirmRequest { id, label });
        }
        Event::EditDiffRequest { id, path, diff, reply } => {
            conn.prompts.register_diff(id.clone(), reply);
            let _ = conn.tx.send(ServerMsg::EditDiff { id, path, diff });
        }
        Event::ToolCallStart {
            call_id,
            name,
            args,
            confirm_required,
        } => {
            let _ = conn.tx.send(ServerMsg::ToolUse {
                id: format!("tool:{call_id}"),
                call_id,
                name,
                args,
                confirm_required,
            });
        }
        Event::ToolCallResult {
            call_id,
            success,
            output,
        } => {
            let _ = conn.tx.send(ServerMsg::ToolResult {
                id: format!("tool:{call_id}"),
                call_id,
                success,
                output,
                status_line: if success {
                    "✓".into()
                } else {
                    "✗".into()
                },
            });
        }
        Event::OfflineWifiListed(output) => {
            for line in output.lines() {
                let line = line.trim();
                if !line.is_empty() {
                    let _ = conn.tx.send(ServerMsg::System {
                        message: format!("[local] {line}"),
                    });
                }
            }
            let _ = conn.tx.send(ServerMsg::System {
                message: "[system] Type the SSID you want to connect to, then press Enter.".into(),
            });
        }
        Event::HistoryUpdated(_) => {
            // Handled by the chat handler (state.history) directly.
        }
        Event::ProviderConnected(_) | Event::ProviderResponse(_) => {
            let _ = conn.tx.send(ServerMsg::System {
                message: "[system] provider connected".into(),
            });
        }
        Event::StatusUpdate { .. }
        | Event::ModelsList { .. }
        | Event::ProvidersList(_)
        | Event::Key(_)
        | Event::Tick => {}
    }
}

// ---------------------------------------------------------------------------
// Chat handling (agent loop + offline router) over the WS bridge
// ---------------------------------------------------------------------------

pub(crate) async fn handle_chat_send(state: &DaemonState, conn: &Conn, id: String, text: String) {
    let provider_name = state.provider.read().unwrap().clone();
    let model = state.model.read().unwrap().clone();
    let connected = state.connected.load(Ordering::SeqCst);

    if text.trim().is_empty() {
        return;
    }

    // ---- Offline wifi flow: awaiting an SSID? ----------------
    if *state.pending_wifi_ssid.read().unwrap() {
        *state.pending_wifi_ssid.write().unwrap() = false;
        let ssid = text.trim().to_string();
        let state = state.clone();
        let tx = conn.tx.clone();
        let prompts = conn.prompts.clone();
        let req_id = id.clone();
        tokio::spawn(async move {
            let (reply_tx, mut reply_rx) = unbounded_channel::<String>();
            let sid = next_id("secret");
            prompts.register_secret(sid.clone(), reply_tx);
            let _ = tx.send(ServerMsg::SecretRequest {
                id: sid,
                label: format!("Enter password for {ssid} (hidden):"),
            });
            let password = reply_rx.recv().await.unwrap_or_default();
            if password.is_empty() {
                let _ = tx.send(ServerMsg::System {
                    message: "[local] wifi_connect cancelled — no password entered.".into(),
                });
                let _ = tx.send(ServerMsg::StreamDone { id: req_id });
                return;
            }
            match hardware::wifi_connect(&ssid, Some(&password)) {
                Ok(result) if result.success => {
                    let _ = tx.send(ServerMsg::System {
                        message: format!("[local] connected to {ssid}"),
                    });
                }
                Ok(result) => {
                    let err = result.error.unwrap_or_default();
                    let _ = tx.send(ServerMsg::System {
                        message: format!("[tool] wifi_connect ✗ {err}"),
                    });
                }
                Err(e) => {
                    let _ = tx.send(ServerMsg::Error {
                        id: None,
                        message: format!("wifi_connect error: {e}"),
                    });
                }
            }
            state.streaming.store(false, Ordering::SeqCst);
            state.broadcast_status();
            let _ = tx.send(ServerMsg::StreamDone { id: req_id });
        });
        return;
    }

    // Persist the user turn.
    if let Ok(mut session) = state.session.lock() {
        session.add_message("user", &text);
        if let Ok(manager) = state.sessions.lock() {
            let _ = manager.save_session(&session);
        }
    }

    // Local slash commands never need a provider.
    let trimmed = text.trim();
    if matches!(trimmed, "/help" | "/tools" | "/version") {
        let _ = conn.tx.send(ServerMsg::Token {
            id: id.clone(),
            text: String::new(),
        });
        let result = match trimmed {
            "/help" => crate::tools::help::show_help(),
            "/tools" => crate::tools::help::show_tool_list(),
            _ => crate::tools::help::show_version(),
        };
        match result {
            Ok(r) => {
                let _ = conn.tx.send(ServerMsg::System {
                    message: format!("[local] {}", r.output),
                });
            }
            Err(e) => {
                let _ = conn.tx.send(ServerMsg::Error {
                    id: Some(id.clone()),
                    message: format!("{trimmed} error: {e}"),
                });
            }
        }
        let _ = conn.tx.send(ServerMsg::StreamDone { id });
        return;
    }

    state.streaming.store(true, Ordering::SeqCst);
    state.broadcast_status();

    let mut messages = state.history.lock().unwrap().clone();
    messages.push(Message::user(&text));

    // ---- Session memory: ground the answer in past sessions -------------
    // Recall the most relevant past messages and inject them as context so
    // the model already knows what the user did before. Zero-config memory.
    if let Ok(manager) = state.sessions.lock() {
        let hits = super::usage::recall(&manager, &text, 4);
        if !hits.is_empty() {
            let mut ctx = String::from(
                "Relevant context from your past sessions (grounding memory):\n",
            );
            for h in hits {
                ctx.push_str(&format!(
                    "- [{} · {} · {}] {}\n",
                    h.title, h.role, h.ts.format("%Y-%m-%d %H:%M"), h.snippet
                ));
            }
            messages.insert(
                0,
                Message::system(format!(
                    "{ctx}\nUse this context if it helps answer the user's question; otherwise ignore it."
                )),
            );
        }
    }

    if connected {
        // ---- Agentic loop (with automatic provider failover) ----
        let factory = Arc::clone(&state.factory);
        let registry = Arc::clone(&state.registry);
        let conn = conn.clone();
        let state = state.clone();
        let tx = conn.tx.clone();
        let prompts = conn.prompts.clone();
        let req_id = id.clone();
        let started = std::time::Instant::now();

        tokio::spawn(async move {
            // Failover chain: current provider first, then every other ready
            // provider. On error we transparently retry with the next one.
            let chain = super::usage::failover_chain(&factory, &provider_name);
            let mut last_error: Option<String> = None;
            let mut outcome: Option<anyhow::Result<agent::AgentOutcome>> = None;

            for candidate in &chain {
                let model_for = if candidate == &provider_name {
                    model.clone()
                } else {
                    factory.default_model(candidate)
                };
                let Some(provider) = factory.create_with_model(candidate, Some(&model_for)) else {
                    continue;
                };

                // Bridge the orchestrator's Event channel onto the WS conn.
                let (bridge_tx, mut bridge_rx) = unbounded_channel::<crate::tui::event::Event>();
                let conn_b = Conn {
                    tx: tx.clone(),
                    prompts: prompts.clone(),
                };
                let stream_tokens = Arc::clone(&state.stream_tokens);
                let forwarder = tokio::spawn(async move {
                    while let Some(event) = bridge_rx.recv().await {
                        forward_event(&conn_b, event, Arc::clone(&stream_tokens));
                    }
                });

                let run = agent::run_agentic_loop(provider.as_ref(), &registry, messages.clone(), bridge_tx).await;
                let _ = forwarder.await;
                match run {
                    Ok(ok) => {
                        outcome = Some(Ok(ok));
                        break;
                    }
                    Err(e) => {
                        last_error = Some(format!("{candidate}: {e}"));
                        tracing::warn!("Provider {candidate} failed — trying next in failover chain");
                        if candidate != chain.last().map(String::as_str).unwrap_or("") {
                            let _ = tx.send(ServerMsg::System {
                                message: format!(
                                    "[system] {candidate} errored ({e}) — failing over to next provider…"
                                ),
                            });
                        }
                    }
                }
            }

            let latency = started.elapsed().as_millis() as u64;
            state.latency_ms.store(latency, Ordering::SeqCst);

            // ---- Record usage / cost journal entry ----
            let est_in = super::usage::estimate_tokens(
                messages.iter().map(|m| m.content.len()).sum(),
            );
            let used_provider = chain.first().cloned().unwrap_or_else(|| provider_name.clone());
            let used_model = if used_provider == provider_name {
                model.clone()
            } else {
                factory.default_model(&used_provider)
            };

            match outcome {
                Some(Ok(outcome)) => {
                    let n_tokens = outcome
                        .messages
                        .iter()
                        .map(|m| m.content.chars().count() as u64 / 4)
                        .sum::<u64>();
                    state.tokens.fetch_add(n_tokens, Ordering::SeqCst);
                    *state.history.lock().unwrap() = outcome.messages;
                    if let Ok(mut session) = state.session.lock() {
                        session.add_message("assistant", &outcome.final_text);
                        if let Ok(manager) = state.sessions.lock() {
                            let _ = manager.save_session(&session);
                        }
                    }
                    // With token streaming disabled the frontend never saw the
                    // per-token chunks — deliver the assembled reply now.
                    if !state.stream_tokens.load(Ordering::SeqCst) {
                        let _ = tx.send(ServerMsg::Token {
                            id: req_id.clone(),
                            text: outcome.final_text.clone(),
                        });
                    }
                    if let Ok(usage) = state.usage.lock() {
                        let _ = usage.record(&super::usage::UsageRecord {
                            ts: chrono::Utc::now(),
                            provider: used_provider,
                            model: used_model,
                            prompt_tokens: est_in,
                            completion_tokens: n_tokens,
                            latency_ms: latency,
                            success: true,
                        });
                    }
                }
                None | Some(Err(_)) => {
                    let err = last_error
                        .unwrap_or_else(|| "No provider available".to_string());
                    let _ = tx.send(ServerMsg::Error {
                        id: Some(req_id.clone()),
                        message: format!("Agent loop error: {err}"),
                    });
                    let _ = tx.send(ServerMsg::StreamDone { id: req_id.clone() });
                    if let Ok(usage) = state.usage.lock() {
                        let _ = usage.record(&super::usage::UsageRecord {
                            ts: chrono::Utc::now(),
                            provider: used_provider,
                            model: used_model,
                            prompt_tokens: est_in,
                            completion_tokens: 0,
                            latency_ms: latency,
                            success: false,
                        });
                    }
                }
            }

            state.streaming.store(false, Ordering::SeqCst);
            state.broadcast_status();
            let _ = tx.send(ServerMsg::StreamDone { id: req_id });
        });
    } else {
        // ---- Offline intent router (no provider required) ----
        let conn = conn.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let tx = conn.tx.clone();
            match IntentRouter::classify_offline(&text) {
                Some(OfflineIntent::Wifi) => match hardware::wifi_list() {
                    Ok(result) if result.success => {
                        let _ = tx.send(ServerMsg::System {
                            message: "[local] offline intent executed".into(),
                        });
                        forward_event(
                            &conn,
                            crate::tui::event::Event::OfflineWifiListed(result.output),
                            Arc::clone(&state.stream_tokens),
                        );
                        // Capture the SSID from the NEXT chat.send.
                        *state.pending_wifi_ssid.write().unwrap() = true;
                    }
                    Ok(result) => {
                        let err = result.error.unwrap_or_default();
                        let _ = tx.send(ServerMsg::Error {
                            id: None,
                            message: format!("wifi_list failed: {err}"),
                        });
                    }
                    Err(e) => {
                        let _ = tx.send(ServerMsg::Error {
                            id: None,
                            message: format!("wifi_list error: {e}"),
                        });
                    }
                },
                Some(OfflineIntent::SetVolume(percent)) => {
                    match hardware::set_volume(percent.min(100) as u8) {
                    Ok(result) if result.success => {
                        let _ = tx.send(ServerMsg::System {
                            message: format!("[local] volume set to {percent}%"),
                        });
                    }
                    Ok(result) => {
                        let err = result.error.unwrap_or_default();
                        let _ = tx.send(ServerMsg::System {
                            message: format!("[tool] set_volume ✗ {err}"),
                        });
                    }
                    Err(e) => {
                        let _ = tx.send(ServerMsg::Error {
                            id: None,
                            message: format!("set_volume error: {e}"),
                        });
                    }
                    }
                },
                Some(OfflineIntent::SetBrightness(percent)) => {
                    match hardware::set_brightness(percent.min(100) as u8) {
                        Ok(result) if result.success => {
                            let _ = tx.send(ServerMsg::System {
                                message: format!("[local] brightness set to {percent}%"),
                            });
                        }
                        Ok(result) => {
                            let err = result.error.unwrap_or_default();
                            let _ = tx.send(ServerMsg::System {
                                message: format!("[tool] set_brightness ✗ {err}"),
                            });
                        }
                        Err(e) => {
                            let _ = tx.send(ServerMsg::Error {
                                id: None,
                                message: format!("set_brightness error: {e}"),
                            });
                        }
                    }
                }
                Some(OfflineIntent::Power(action)) => {
                    let _ = tx.send(ServerMsg::System {
                        message: "[local] offline intent executed".into(),
                    });
                    // Destructive — always confirm via the WS broker.
                    let (reply_tx, mut reply_rx) = unbounded_channel::<bool>();
                    let cid = next_id("confirm");
                    conn.prompts.register_confirm(cid.clone(), reply_tx);
                    let _ = tx.send(ServerMsg::ConfirmRequest {
                        id: cid,
                        label: format!(
                            "Run `systemctl {action}`? This will {action} the system."
                        ),
                    });
                    let confirmed = reply_rx.recv().await.unwrap_or(false);
                    if !confirmed {
                        let _ = tx.send(ServerMsg::System {
                            message: "[local] power action cancelled.".into(),
                        });
                    } else if let Ok(result) = hardware::power(&action) {
                        if result.success {
                            let _ = tx.send(ServerMsg::System {
                                message: format!("[tool] power ✓ — system is {action}ing"),
                            });
                        } else {
                            let err = result.error.unwrap_or_default();
                            let _ = tx.send(ServerMsg::System {
                                message: format!("[tool] power ✗ {err}"),
                            });
                        }
                    }
                }
                Some(OfflineIntent::Help)
                | Some(OfflineIntent::Tools)
                | Some(OfflineIntent::Version) => {}
                None => {
                    let _ = tx.send(ServerMsg::System {
                        message: "[system] Offline — no API key configured. Connect a provider \
                                   (F6 / /model) or use /connect <name> to add a key."
                            .into(),
                    });
                }
            }
            state.streaming.store(false, Ordering::SeqCst);
            state.broadcast_status();
            let _ = tx.send(ServerMsg::StreamDone { id });
        });
    }
}

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

/// Route one client message.
async fn handle_client_msg(state: &DaemonState, conn: &Conn, msg: ClientMsg) {
    match msg {
        ClientMsg::ChatSend { id, text } => handle_chat_send(state, conn, id, text).await,
        ClientMsg::ToolApprove { id } => {
            conn.prompts.resolve_confirm(&id, true);
        }
        ClientMsg::ToolReject { id } => {
            conn.prompts.resolve_confirm(&id, false);
        }
        ClientMsg::SecretSubmit { id, value } => {
            conn.prompts.resolve_secret(&id, value);
        }
        ClientMsg::SecretCancel { id } => {
            conn.prompts.resolve_secret(&id, String::new());
        }
        ClientMsg::EditApprove { id } => {
            conn.prompts.resolve_diff(&id, true);
        }
        ClientMsg::EditReject { id } => {
            conn.prompts.resolve_diff(&id, false);
        }
        ClientMsg::SpawnTui => {
            crate::core::spawn_tui();
        }
        ClientMsg::TermInput { data } => {
            let tx = conn.tx.clone();
            if let Err(e) = state.pty.input(&data, tx) {
                let _ = conn.tx.send(ServerMsg::Error {
                    id: None,
                    message: format!("pty error: {e}"),
                });
            }
        }
        ClientMsg::TermResize { cols, rows } => {
            state.pty.resize(cols, rows);
        }
        ClientMsg::FileRead { id, path } => {
            let registry = Arc::clone(&state.registry);
            let tx = conn.tx.clone();
            tokio::task::spawn_blocking(move || {
                if !registry.is_path_allowed(&path) {
                    let _ = tx.send(ServerMsg::Error {
                        id: Some(id.clone()),
                        message: format!("path not allowed: {path}"),
                    });
                    return;
                }
                match std::fs::read_to_string(shellexpand::tilde(&path).as_ref()) {
                    Ok(content) => {
                        let _ = tx.send(ServerMsg::FileContent { id, path, content });
                    }
                    Err(e) => {
                        let _ = tx.send(ServerMsg::Error {
                            id: Some(id.clone()),
                            message: format!("read {path}: {e}"),
                        });
                    }
                }
            });
        }
        ClientMsg::FileWrite { id, path, content } => {
            let registry = Arc::clone(&state.registry);
            let tx = conn.tx.clone();
            tokio::task::spawn_blocking(move || {
                if !registry.is_path_allowed(&path) {
                    let _ = tx.send(ServerMsg::Error {
                        id: Some(id.clone()),
                        message: format!("path not allowed: {path}"),
                    });
                    return;
                }
                match std::fs::write(shellexpand::tilde(&path).as_ref(), content) {
                    Ok(()) => {
                        let _ = tx.send(ServerMsg::FileWritten { id, path });
                    }
                    Err(e) => {
                        let _ = tx.send(ServerMsg::Error {
                            id: Some(id.clone()),
                            message: format!("write {path}: {e}"),
                        });
                    }
                }
            });
        }
        ClientMsg::FileList { id, path } => {
            let registry = Arc::clone(&state.registry);
            let tx = conn.tx.clone();
            tokio::task::spawn_blocking(move || {
                if !registry.is_path_allowed(&path) {
                    let _ = tx.send(ServerMsg::Error {
                        id: Some(id.clone()),
                        message: format!("path not allowed: {path}"),
                    });
                    return;
                }
                let entries = list_dir_entries(&path);
                let _ = tx.send(ServerMsg::Files { id, path, entries });
            });
        }
        ClientMsg::ProviderSwitch { name } => {
            let key_present = state.factory.has_key(&name);
            *state.provider.write().unwrap() = name.clone();
            state
                .connected
                .store(key_present, Ordering::SeqCst);
            state.broadcast_status();
            // Offer the model list for the new provider.
            let models = state.factory.models_for(&name);
            let current = state.model.read().unwrap().clone();
            let _ = conn.tx.send(ServerMsg::Models {
                id: "model".into(),
                provider: name,
                models,
                current,
            });
            let _ = conn.tx.send(ServerMsg::System {
                message: "[system] Provider switched — pick a model.".to_string(),
            });
        }
        ClientMsg::ModelSwitch { model } => {
            *state.model.write().unwrap() = model;
            state.broadcast_status();
        }
        ClientMsg::ProviderConnect { id, name, key } => {
            let factory = Arc::clone(&state.factory);
            let vault = Arc::clone(&state.vault);
            let conn2 = conn.clone();
            let state2 = state.clone();
            tokio::spawn(async move {
                match factory.validate(&name, &key).await {
                    Ok(()) => {
                        if let Err(e) = vault.store_token(&name, &key) {
                            let _ = conn2.tx.send(ServerMsg::Error {
                                id: Some(id.clone()),
                                message: format!("vault error: {e}"),
                            });
                            return;
                        }
                        let _ = conn2.tx.send(ServerMsg::System {
                            message: format!("[system] Key stored for {name}."),
                        });
                        let _ = conn2.tx.send(ServerMsg::Providers {
                            id,
                            entries: state2.provider_entries_dto(),
                        });
                        if state2.provider.read().unwrap().eq_ignore_ascii_case(&name) {
                            state2.connected.store(true, Ordering::SeqCst);
                        }
                        state2.broadcast_status();
                    }
                    Err(e) => {
                        let _ = conn2.tx.send(ServerMsg::Error {
                            id: Some(id),
                            message: format!("API key validation failed for {name}: {e}"),
                        });
                    }
                }
            });
        }
        ClientMsg::AgentDelegate { id, agent, task } => {
            let tx = conn.tx.clone();
            tokio::spawn(async move {
                match external::delegate_to_agent(&agent, &task) {
                    Ok(result) => {
                        let _ = tx.send(ServerMsg::System {
                            message: format!(
                                "[agent:{agent}] {}",
                                if result.success {
                                    "done".to_string()
                                } else {
                                    result.error.unwrap_or_default()
                                }
                            ),
                        });
                    }
                    Err(e) => {
                        let _ = tx.send(ServerMsg::Error {
                            id: Some(id.clone()),
                            message: format!("agent delegate error: {e}"),
                        });
                    }
                }
                let _ = tx.send(ServerMsg::StreamDone { id });
            });
        }
        ClientMsg::ConfigSet { key, value } => {
            apply_config_set(state, &key, value);
        }
        ClientMsg::ProvidersList { id } => {
            let _ = conn
                .tx
                .send(ServerMsg::Providers {
                    id,
                    entries: state.provider_entries_dto(),
                });
        }
        ClientMsg::ModelsList { id, provider } => {
            let models = state.factory.models_for(&provider);
            let current = state.model.read().unwrap().clone();
            let _ = conn.tx.send(ServerMsg::Models {
                id,
                provider,
                models,
                current,
            });
        }
        ClientMsg::StatsGet { id } => {
            let state2 = state.clone();
            let tx = conn.tx.clone();
            tokio::spawn(async move {
                let battery = state2
                    .run_tool("get_battery_status", serde_json::json!({}))
                    .await
                    .unwrap_or_else(|| "n/a".into());
                let wifi = state2
                    .run_tool("wifi_status", serde_json::json!({}))
                    .await
                    .unwrap_or_else(|| "offline".into());
                let cwd = std::env::current_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "~".into());
                let _ = tx.send(ServerMsg::Stats {
                    id,
                    tokens: state2.tokens.load(Ordering::SeqCst),
                    latency_ms: state2.latency_ms.load(Ordering::SeqCst),
                    battery,
                    wifi,
                    cwd,
                });
            });
        }
        ClientMsg::AgentsList { id } => {
            let _ = conn.tx.send(ServerMsg::Agents {
                id,
                agents: state.detected_agents(),
            });
        }
    }
}

fn apply_config_set(state: &DaemonState, key: &str, value: serde_json::Value) {
    let mut config = state.config.read().unwrap().clone();
    match key {
        "default_provider" => {
            if let Some(v) = value.as_str() {
                config.default_provider = v.into();
            }
        }
        "theme" => {
            if let Some(v) = value.as_str() {
                config.theme = v.into();
            }
        }
        "confirm_destructive" => {
            if let Some(v) = value.as_bool() {
                config.confirm_destructive = v;
            }
        }
        "stream_tokens" => {
            if let Some(v) = value.as_bool() {
                config.stream_tokens = v;
                state.stream_tokens.store(v, Ordering::SeqCst);
            }
        }
        "max_history" => {
            if let Some(v) = value.as_u64() {
                config.max_history = v as usize;
            }
        }
        _ => {
            state.broadcast(ServerMsg::Error {
                id: None,
                message: format!("config.set: unknown key '{key}'"),
            });
            return;
        }
    }
    if let Err(e) = config.save(None) {
        tracing::warn!("failed to save config: {e}");
    }
    *state.config.write().unwrap() = config;
    state.broadcast_status();
}

// ---------------------------------------------------------------------------
// File listing helpers (file.list)
// ---------------------------------------------------------------------------

/// List a directory's entries, git-aware, dirs first.
pub(crate) fn list_dir_entries(dir: &str) -> Vec<FileEntryDto> {
    let expanded = shellexpand::tilde(dir).to_string();
    let path = std::path::Path::new(&expanded);

    let git_badges = git_status_badges(path);

    let mut entries = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(path) else {
        return entries;
    };
    for entry in read_dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let kind = if file_type.is_dir() {
            "dir"
        } else if file_type.is_symlink() {
            "symlink"
        } else {
            "file"
        };
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        let git = git_badges.get(&name).cloned();
        entries.push(FileEntryDto {
            name,
            kind: kind.into(),
            size,
            git,
        });
    }
    entries.sort_by(|a, b| {
        let a_dir = a.kind == "dir";
        let b_dir = b.kind == "dir";
        b_dir
            .cmp(&a_dir)
            .then_with(|| a.name.cmp(&b.name))
    });
    entries
}

/// Run `git status --porcelain` once and map top-level names → badges.
fn git_status_badges(dir: &std::path::Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if !dir.join(".git").exists() {
        return map;
    }
    let Ok(out) = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .arg("status")
        .arg("--porcelain")
        .output()
    else {
        return map;
    };
    if !out.status.success() {
        return map;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if line.len() < 4 {
            continue;
        }
        let badge = line[0..2].to_string();
        let name = line[3..].trim();
        let name = name.rsplit('/').next().unwrap_or(name).to_string();
        map.entry(name).or_insert_with(|| badge.clone());
    }
    map
}

// ---------------------------------------------------------------------------
// Static file serving
// ---------------------------------------------------------------------------
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

impl DaemonState {
    /// Seconds since the daemon process started.
    pub fn uptime_seconds(&self) -> u64 {
        static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        START
            .get_or_init(std::time::Instant::now)
            .elapsed()
            .as_secs()
    }

    pub(crate) fn provider_entries_dto(&self) -> Vec<ProviderEntryDto> {
        self.factory
            .provider_entries()
            .into_iter()
            .map(|e| ProviderEntryDto {
                name: e.name,
                needs_key: e.needs_key,
                has_key: e.has_key,
            })
            .collect()
    }

    /// External AI coding CLIs detected on this machine.
    pub fn detected_agents(&self) -> Vec<String> {
        external::detect_agents()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::ProviderFactory;
    use tokio::sync::mpsc::unbounded_channel;

    fn test_state() -> Arc<DaemonState> {
        let vault = Arc::new(Vault::new().expect("vault"));
        let config = Arc::new(DiadConfig::default());
        let factory = Arc::new(ProviderFactory::new(Arc::clone(&vault), Arc::clone(&config)));
        let registry = Arc::new(crate::tools::create_default_registry());
        Arc::new(DaemonState::new(config, vault, factory, registry))
    }

    #[test]
    fn test_status_snapshot_shape() {
        let state = test_state();
        match state.status_snapshot() {
            ServerMsg::Status {
                provider,
                connected,
                streaming,
                ..
            } => {
                assert_eq!(provider, "anthropic");
                assert!(!streaming);
                let _ = connected;
            }
            _ => panic!("expected Status"),
        }
    }

    #[test]
    fn test_prompt_broker_secret_roundtrip() {
        let broker = PromptBroker::default();
        let (tx, mut rx) = unbounded_channel::<String>();
        broker.register_secret("s1".into(), tx);
        assert!(broker.resolve_secret("s1", "sekrit".into()));
        let value = rx.try_recv().unwrap();
        assert_eq!(value, "sekrit");
        // Second resolve is a no-op.
        assert!(!broker.resolve_secret("s1", "x".into()));
    }

    #[test]
    fn test_prompt_broker_confirm_roundtrip() {
        let broker = PromptBroker::default();
        let (tx, mut rx) = unbounded_channel::<bool>();
        broker.register_confirm("c1".into(), tx);
        assert!(broker.resolve_confirm("c1", true));
        assert!(rx.try_recv().unwrap());
        assert!(!broker.resolve_confirm("c1", false));
    }

    #[test]
    fn test_provider_entries_dto() {
        let state = test_state();
        let entries = state.provider_entries_dto();
        assert!(entries.len() >= 10, "expected 10+ providers, got {}", entries.len());
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        for expected in ["anthropic", "gemini", "openai", "kimi", "deepseek", "groq", "ollama"] {
            assert!(names.contains(&expected), "missing {expected}");
        }
    }

    #[test]
    fn test_config_set_default_provider() {
        let state = test_state();
        apply_config_set(&state, "default_provider", serde_json::json!("openai"));
        assert_eq!(state.config.read().unwrap().default_provider, "openai");
    }
}
