//! DIAD AGENT daemon — the brain.
//!
//! Owns all application state (`DaemonState`), the prompt broker,
//! the orchestrator bridge (`forward_event`), the chat agent loop
//! (`handle_chat_send`), and the WebSocket message router
//! (`handle_client_msg`). Transport lives in `server`; REST lives
//! in `rest`. Both frontends talk to this brain.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use tokio::sync::mpsc::{unbounded_channel, Sender, UnboundedSender};

use crate::auth::vault::Vault;
use crate::config::settings::DiadConfig;
use crate::orchestrator::agent;
use crate::orchestrator::router::{IntentRouter, OfflineIntent};
use crate::orchestrator::session::{Session, SessionManager};
use crate::providers::traits::Message;
use crate::providers::ProviderFactory;
use crate::tools::external;
use crate::tools::hardware::device;
use crate::tools::ToolRegistry;

use super::debug::DebugManager;
use super::idle::{IdleStage, TaskRegistry};
use super::notes::NotesStore;
use super::protocol::{ClientMsg, FileEntryDto, ProviderEntryDto, ServerMsg};
use super::pty::PtyManager;
use super::usage::UsageTracker;

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
    /// In-flight turn counter (F-19): two concurrent chat_send from GUI+TUI
    /// each increment this and decrement on completion, so `streaming` only
    /// reads false after the LAST turn finishes — no more premature clear.
    pub streaming: Arc<AtomicUsize>,
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

    /// Performance tier word (ULTRA_LOW/LOW/MID/HIGH/ULTRA) chosen at boot
    /// by `diad-perf-tier.sh`; read by the bench/status surfaces.
    pub perf_tier: String,

    // Idle engine (spec §3): stage + central periodic-task registry.
    pub idle: Arc<super::idle::IdleState>,
    pub tasks: Arc<TaskRegistry>,
    /// When the last client disconnected (drives the idle-stage clock).
    pub last_disconnect: Arc<Mutex<std::time::Instant>>,
    /// Power state maintained by the env monitor (spec 3.7 / 10.x).
    pub on_battery: Arc<AtomicBool>,
    pub battery_pct: Arc<AtomicU8>,
    pub thermal_throttled: Arc<AtomicBool>,
    /// Chat requests currently in flight, keyed for dedup (spec 7.1).
    pub inflight_chat: Arc<Mutex<std::collections::HashSet<String>>>,

    // Connected frontends (for broadcasts).
    clients: Arc<RwLock<Vec<Sender<ServerMsg>>>>,

    /// Brute-force guard for the web lockscreen's `/auth/verify` endpoint.
    /// A sliding window of recent attempts; once the window fills, further
    /// attempts are rejected with 429 until the window drains (or a verified
    /// password clears it).
    pub lock_auth: Arc<Mutex<LockAuthGuard>>,
}

/// Sliding-window rate limiter for lockscreen password attempts.
///
/// `MAX_ATTEMPTS` attempts are allowed inside `WINDOW`; the window slides so
/// a patient attacker is still capped at `MAX_ATTEMPTS / WINDOW` per second
/// indefinitely. A successful verify clears the window immediately.
pub struct LockAuthGuard {
    attempts: Vec<std::time::Instant>,
}

impl LockAuthGuard {
    const WINDOW: std::time::Duration = std::time::Duration::from_secs(30);
    const MAX_ATTEMPTS: usize = 5;

    pub fn new() -> Self {
        Self {
            attempts: Vec::new(),
        }
    }

    /// Allow one more attempt if the window is not full. `true` reserves
    /// the attempt; `false` means the caller must reject with 429.
    pub fn try_attempt(&mut self) -> bool {
        let now = std::time::Instant::now();
        self.attempts
            .retain(|t| now.duration_since(*t) < Self::WINDOW);
        if self.attempts.len() >= Self::MAX_ATTEMPTS {
            return false;
        }
        self.attempts.push(now);
        true
    }

    /// A verified password clears the window (legitimate user, not an
    /// attacker hammering the endpoint).
    pub fn note_success(&mut self) {
        self.attempts.clear();
    }
}

impl Default for LockAuthGuard {
    fn default() -> Self {
        Self::new()
    }
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

        let sessions = Arc::new(Mutex::new(SessionManager::new().unwrap_or_else(|_| {
            // Fallback manager pointing at a temp dir so the daemon can
            // always start, even if the sessions dir is unwritable.
            SessionManager::new_with_dir(std::env::temp_dir().join("diad-sessions"))
        })));
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
            streaming: Arc::new(AtomicUsize::new(0)),
            stream_tokens: Arc::new(AtomicBool::new(config.stream_tokens)),
            history: Arc::new(Mutex::new(Vec::new())),
            session,
            pending_wifi_ssid: Arc::new(RwLock::new(false)),
            tokens: Arc::new(AtomicU64::new(0)),
            latency_ms: Arc::new(AtomicU64::new(0)),
            perf_tier: serde_json::to_string(&crate::core::perf::active_tier())
                .unwrap_or_default()
                .replace('"', ""),
            idle: Arc::new(super::idle::IdleState::default()),
            tasks: Arc::new(TaskRegistry::default()),
            last_disconnect: Arc::new(Mutex::new(std::time::Instant::now())),
            on_battery: Arc::new(AtomicBool::new(false)),
            battery_pct: Arc::new(AtomicU8::new(100)),
            thermal_throttled: Arc::new(AtomicBool::new(false)),
            inflight_chat: Arc::new(Mutex::new(std::collections::HashSet::new())),
            clients: Arc::new(RwLock::new(Vec::new())),
            lock_auth: Arc::new(Mutex::new(LockAuthGuard::new())),
        }
    }

    /// The current status snapshot.
    pub fn status_snapshot(&self) -> ServerMsg {
        ServerMsg::Status {
            provider: self.provider.read().unwrap().clone(),
            model: self.model.read().unwrap().clone(),
            connected: self.connected.load(Ordering::SeqCst),
            streaming: self.streaming.load(Ordering::SeqCst) > 0,
            daemon_version: env!("CARGO_PKG_VERSION").into(),
            config: config_snapshot(&self.config.read().unwrap()),
        }
    }

    /// Broadcast a message to every connected frontend.
    pub fn broadcast(&self, msg: ServerMsg) {
        let clients = self.clients.read().unwrap();
        for tx in clients.iter() {
            let _ = tx.try_send(msg.clone());
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

    pub(crate) fn register_client(&self, tx: Sender<ServerMsg>) {
        self.clients.write().unwrap().push(tx);
        self.wake_if_idle();
    }

    pub(crate) fn unregister_client(&self, tx: &Sender<ServerMsg>) {
        let mut clients = self.clients.write().unwrap();
        clients.retain(|c| !c.same_channel(tx));
        if clients.is_empty() {
            // Start (or restart) the idle clock (spec 3.2/3.3).
            *self.last_disconnect.lock().unwrap() = std::time::Instant::now();
        }
    }

    /// Number of connected frontends (GUI/TUI WebSocket clients).
    pub(crate) fn clients_count(&self) -> usize {
        self.clients.read().unwrap().len()
    }

    /// Wake path (spec 3.4): a new client restores the ACTIVE stage and
    /// re-enables every registered periodic task. O(number of tasks) —
    /// microseconds, far below the 200 ms wake budget. Caches re-populate
    /// lazily on first use.
    fn wake_if_idle(&self) {
        if self.idle.get() != IdleStage::Active {
            self.tasks.set_all_enabled(true);
            self.idle.set(IdleStage::Active);
            tracing::info!("diad daemon active");
        }
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
    pub tx: Sender<ServerMsg>,
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

/// RBRO-004 WP-06.6 — map a power slash command to the `device::power`
/// action name. Mirrors `device::power_verb` (shutdown→poweroff) so the
/// confirm label always shows the real systemctl verb.
fn slash_power_action(cmd: &str) -> Option<&'static str> {
    match cmd {
        "/shutdown" => Some("shutdown"),
        "/reboot" => Some("reboot"),
        "/sleep" => Some("suspend"),
        _ => None,
    }
}

/// Convert an orchestrator `Event` into frontend messages through a `Conn`.
///
/// Mirrors what the old monolithic TUI event loop did locally, but over WS.
pub fn forward_event(conn: &Conn, event: crate::tui::event::Event, stream_tokens: Arc<AtomicBool>) {
    use crate::tui::event::Event;
    match event {
        Event::StreamChunk(text) => {
            // `stream_tokens=false` (config preference) suppresses per-token
            // delivery; the caller sends the assembled reply in one chunk.
            if stream_tokens.load(Ordering::SeqCst) {
                let _ = conn.tx.try_send(ServerMsg::Token {
                    id: "chat".into(),
                    text,
                });
            }
        }
        Event::StreamDone => {
            let _ = conn
                .tx
                .try_send(ServerMsg::StreamDone { id: "chat".into() });
        }
        Event::AppendMessage(line) => {
            let _ = conn.tx.try_send(ServerMsg::System { message: line });
        }
        Event::Error(message) => {
            let _ = conn.tx.try_send(ServerMsg::Error { id: None, message });
        }
        Event::ToolResult(output) => {
            let _ = conn.tx.try_send(ServerMsg::System {
                message: format!("[Tool Result] {output}"),
            });
        }
        Event::SecurePromptRequest { label, reply } => {
            let id = next_id("secret");
            conn.prompts.register_secret(id.clone(), reply);
            let _ = conn.tx.try_send(ServerMsg::SecretRequest { id, label });
        }
        Event::ConfirmationRequest { label, reply } => {
            let id = next_id("confirm");
            conn.prompts.register_confirm(id.clone(), reply);
            let _ = conn.tx.try_send(ServerMsg::ConfirmRequest { id, label });
        }
        Event::EditDiffRequest {
            id,
            path,
            diff,
            reply,
        } => {
            conn.prompts.register_diff(id.clone(), reply);
            let _ = conn.tx.try_send(ServerMsg::EditDiff { id, path, diff });
        }
        Event::ToolCallStart {
            call_id,
            name,
            args,
            confirm_required,
        } => {
            let _ = conn.tx.try_send(ServerMsg::ToolUse {
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
            let _ = conn.tx.try_send(ServerMsg::ToolResult {
                id: format!("tool:{call_id}"),
                call_id,
                success,
                output,
                status_line: if success { "✓".into() } else { "✗".into() },
            });
        }
        Event::OfflineWifiListed(output) => {
            for line in output.lines() {
                let line = line.trim();
                if !line.is_empty() {
                    let _ = conn.tx.try_send(ServerMsg::System {
                        message: format!("[local] {line}"),
                    });
                }
            }
            let _ = conn.tx.try_send(ServerMsg::System {
                message: "[system] Type the SSID you want to connect to, then press Enter.".into(),
            });
        }
        Event::HistoryUpdated(_) => {
            // Handled by the chat handler (state.history) directly.
        }
        Event::ProviderConnected(_) | Event::ProviderResponse(_) => {
            let _ = conn.tx.try_send(ServerMsg::System {
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

/// Removes a chat request's dedup key when the request finishes
/// (success, failure, or cancellation — Drop always runs).
struct InflightGuard {
    set: Arc<Mutex<std::collections::HashSet<String>>>,
    key: String,
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        self.set.lock().unwrap().remove(&self.key);
    }
}

/// Mark one in-flight turn as finished. Saturating: a stray decrement with
/// nothing in flight must never wrap to usize::MAX (that would pin the
/// `streaming` status on forever — F-19).
pub(crate) fn end_turn(state: &DaemonState) {
    let _ = state
        .streaming
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
            Some(n.saturating_sub(1))
        });
}

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
        // Count this turn in-flight so concurrent turns (GUI + TUI) don't
        // clear each other's streaming state (F-19).
        state.streaming.fetch_add(1, Ordering::SeqCst);
        tokio::spawn(async move {
            let (reply_tx, mut reply_rx) = unbounded_channel::<String>();
            let sid = next_id("secret");
            prompts.register_secret(sid.clone(), reply_tx);
            let _ = tx.try_send(ServerMsg::SecretRequest {
                id: sid,
                label: format!("Enter password for {ssid} (hidden):"),
            });
            let password = reply_rx.recv().await.unwrap_or_default();
            if password.is_empty() {
                let _ = tx.try_send(ServerMsg::System {
                    message: "[local] wifi_connect cancelled — no password entered.".into(),
                });
                let _ = tx.try_send(ServerMsg::StreamDone { id: req_id });
                return;
            }
            match device::wifi_connect(&ssid, Some(&password)) {
                Ok(result) if result.success => {
                    let _ = tx.try_send(ServerMsg::System {
                        message: format!("[local] connected to {ssid}"),
                    });
                }
                Ok(result) => {
                    let err = result.error.unwrap_or_default();
                    let _ = tx.try_send(ServerMsg::System {
                        message: format!("[tool] wifi_connect ✗ {err}"),
                    });
                }
                Err(e) => {
                    let _ = tx.try_send(ServerMsg::Error {
                        id: None,
                        message: format!("wifi_connect error: {e}"),
                    });
                }
            }
            end_turn(&state);
            state.broadcast_status();
            let _ = tx.try_send(ServerMsg::StreamDone { id: req_id });
        });
        return;
    }

    // ---- Coalesce identical in-flight requests (spec 7.1) --------------
    // A second identical chat_send while the first is still running never
    // fires a duplicate provider call; the key is released when the request
    // finishes (InflightGuard's Drop).
    let dedup_key = format!("{provider_name}:{model}:{}", text.trim());
    {
        let mut inflight = state.inflight_chat.lock().unwrap();
        if inflight.contains(&dedup_key) {
            let _ = conn.tx.try_send(ServerMsg::System {
                message: "[system] identical request already in flight — coalesced.".into(),
            });
            let _ = conn.tx.try_send(ServerMsg::StreamDone { id });
            return;
        }
        inflight.insert(dedup_key.clone());
    }
    let _inflight_guard = InflightGuard {
        set: Arc::clone(&state.inflight_chat),
        key: dedup_key,
    };

    // Persist the user turn.
    if let Ok(mut session) = state.session.lock() {
        session.add_message("user", &text);
        if let Ok(manager) = state.sessions.lock() {
            let _ = manager.save_session(&session);
        }
    }

    // Local slash commands never need a provider.
    let trimmed = text.trim();
    if matches!(
        trimmed,
        "/help" | "/tools" | "/version" | "/shutdown" | "/reboot" | "/sleep"
    ) {
        let _ = conn.tx.try_send(ServerMsg::Token {
            id: id.clone(),
            text: String::new(),
        });

        // RBRO-004 WP-06.6 — power slash commands always work, connected or
        // not (THE LAW: same confirmed `device::power` flow as the offline
        // router, converging on the same systemctl verbs as diad-power.sh).
        if let Some(action) = slash_power_action(trimmed) {
            let conn = conn.clone();
            let state = state.clone();
            let tx = conn.tx.clone();
            let req_id = id.clone();
            let action = action.to_string();
            state.streaming.fetch_add(1, Ordering::SeqCst);
            state.broadcast_status();
            tokio::spawn(async move {
                let verb = device::power_verb(&action).unwrap_or(&action);
                let human = match verb {
                    "poweroff" => "power off",
                    "reboot" => "reboot",
                    _ => "suspend",
                };
                let (reply_tx, mut reply_rx) = unbounded_channel::<bool>();
                let cid = next_id("confirm");
                conn.prompts.register_confirm(cid.clone(), reply_tx);
                let _ = tx.try_send(ServerMsg::ConfirmRequest {
                    id: cid,
                    label: format!("Run `systemctl {verb}`? This will {human} the system."),
                });
                let confirmed = reply_rx.recv().await.unwrap_or(false);
                if !confirmed {
                    let _ = tx.try_send(ServerMsg::System {
                        message: "[local] power action cancelled.".into(),
                    });
                } else if let Ok(result) = device::power(&action) {
                    if result.success {
                        let human = match verb {
                            "poweroff" => "powering off",
                            "reboot" => "rebooting",
                            _ => "suspending",
                        };
                        let _ = tx.try_send(ServerMsg::System {
                            message: format!("[tool] power ✓ — {human}"),
                        });
                    } else {
                        let err = result.error.unwrap_or_default();
                        let _ = tx.try_send(ServerMsg::System {
                            message: format!("[tool] power ✗ {err}"),
                        });
                    }
                }
                end_turn(&state);
                state.broadcast_status();
                let _ = tx.try_send(ServerMsg::StreamDone { id: req_id });
            });
            return;
        }

        let result = match trimmed {
            "/help" => crate::tools::help::show_help(),
            "/tools" => crate::tools::help::show_tool_list(),
            _ => crate::tools::help::show_version(),
        };
        match result {
            Ok(r) => {
                let _ = conn.tx.try_send(ServerMsg::System {
                    message: format!("[local] {}", r.output),
                });
            }
            Err(e) => {
                let _ = conn.tx.try_send(ServerMsg::Error {
                    id: Some(id.clone()),
                    message: format!("{trimmed} error: {e}"),
                });
            }
        }
        let _ = conn.tx.try_send(ServerMsg::StreamDone { id });
        return;
    }

    state.streaming.fetch_add(1, Ordering::SeqCst);
    state.broadcast_status();

    let mut messages = state.history.lock().unwrap().clone();
    messages.push(Message::user(&text));

    // ---- Session memory: ground the answer in past sessions -------------
    // Recall the most relevant past messages and inject them as context so
    // the model already knows what the user did before. Zero-config memory.
    if let Ok(manager) = state.sessions.lock() {
        let hits = super::usage::recall(&manager, &text, 4);
        if !hits.is_empty() {
            let mut ctx =
                String::from("Relevant context from your past sessions (grounding memory):\n");
            for h in hits {
                ctx.push_str(&format!(
                    "- [{} · {} · {}] {}\n",
                    h.title,
                    h.role,
                    h.ts.format("%Y-%m-%d %H:%M"),
                    h.snippet
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
                // Streaming backpressure (spec 7.3): batch token frames to
                // at most 60 Hz. Tokens accumulate in `buf` and flush when
                // 16 ms have passed or any non-token event arrives; the
                // remainder flushes when the bridge closes.
                let forwarder = tokio::spawn(async move {
                    let mut buf = String::new();
                    let mut last_flush = std::time::Instant::now();
                    let flush_gap = std::time::Duration::from_millis(16);
                    while let Some(event) = bridge_rx.recv().await {
                        if let crate::tui::event::Event::StreamChunk(text) = &event {
                            buf.push_str(text);
                            if last_flush.elapsed() < flush_gap {
                                continue; // hold — next flush window
                            }
                            forward_event(
                                &conn_b,
                                crate::tui::event::Event::StreamChunk(std::mem::take(&mut buf)),
                                Arc::clone(&stream_tokens),
                            );
                            last_flush = std::time::Instant::now();
                            continue;
                        }
                        if !buf.is_empty() {
                            forward_event(
                                &conn_b,
                                crate::tui::event::Event::StreamChunk(std::mem::take(&mut buf)),
                                Arc::clone(&stream_tokens),
                            );
                            last_flush = std::time::Instant::now();
                        }
                        forward_event(&conn_b, event, Arc::clone(&stream_tokens));
                    }
                    if !buf.is_empty() {
                        forward_event(
                            &conn_b,
                            crate::tui::event::Event::StreamChunk(std::mem::take(&mut buf)),
                            Arc::clone(&stream_tokens),
                        );
                    }
                });

                let run = agent::run_agentic_loop(
                    provider.as_ref(),
                    Arc::clone(&registry),
                    messages.clone(),
                    bridge_tx,
                )
                .await;
                let _ = forwarder.await;
                match run {
                    Ok(ok) => {
                        outcome = Some(Ok(ok));
                        break;
                    }
                    Err(e) => {
                        last_error = Some(format!("{candidate}: {e}"));
                        tracing::warn!(
                            "Provider {candidate} failed — trying next in failover chain"
                        );
                        if candidate != chain.last().map(String::as_str).unwrap_or("") {
                            let _ = tx.try_send(ServerMsg::System {
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
            let est_in =
                super::usage::estimate_tokens(messages.iter().map(|m| m.content.len()).sum());
            let used_provider = chain
                .first()
                .cloned()
                .unwrap_or_else(|| provider_name.clone());
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
                        let _ = tx.try_send(ServerMsg::Token {
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
                    let raw_err = last_error.unwrap_or_else(|| {
                        format!("No API key configured for '{used_provider}'. Add your API key in Settings (F5) or press F6 to switch providers.")
                    });
                    let friendly_err = if raw_err.contains("localhost:11434") {
                        format!("Ollama is not running locally (http://localhost:11434). Start Ollama (`ollama serve`) or press F6 to select a cloud provider (Groq, Anthropic, OpenAI, Gemini, DeepSeek).")
                    } else {
                        raw_err
                    };
                    let _ = tx.try_send(ServerMsg::Error {
                        id: Some(req_id.clone()),
                        message: friendly_err,
                    });
                    let _ = tx.try_send(ServerMsg::StreamDone { id: req_id.clone() });
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

            end_turn(&state);
            state.broadcast_status();
            let _ = tx.try_send(ServerMsg::StreamDone { id: req_id });
        });
    } else {
        // ---- Offline intent router (no provider required) ----
        let conn = conn.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let tx = conn.tx.clone();
            match IntentRouter::classify_offline(&text) {
                Some(OfflineIntent::Wifi) => match device::wifi_list() {
                    Ok(result) if result.success => {
                        let _ = tx.try_send(ServerMsg::System {
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
                        let _ = tx.try_send(ServerMsg::Error {
                            id: None,
                            message: format!("wifi_list failed: {err}"),
                        });
                    }
                    Err(e) => {
                        let _ = tx.try_send(ServerMsg::Error {
                            id: None,
                            message: format!("wifi_list error: {e}"),
                        });
                    }
                },
                Some(OfflineIntent::SetVolume(percent)) => {
                    match device::set_volume(percent.min(100) as u8) {
                        Ok(result) if result.success => {
                            let _ = tx.try_send(ServerMsg::System {
                                message: format!("[local] volume set to {percent}%"),
                            });
                        }
                        Ok(result) => {
                            let err = result.error.unwrap_or_default();
                            let _ = tx.try_send(ServerMsg::System {
                                message: format!("[tool] set_volume ✗ {err}"),
                            });
                        }
                        Err(e) => {
                            let _ = tx.try_send(ServerMsg::Error {
                                id: None,
                                message: format!("set_volume error: {e}"),
                            });
                        }
                    }
                }
                Some(OfflineIntent::SetBrightness(percent)) => {
                    match device::set_brightness(percent.min(100) as u8) {
                        Ok(result) if result.success => {
                            let _ = tx.try_send(ServerMsg::System {
                                message: format!("[local] brightness set to {percent}%"),
                            });
                        }
                        Ok(result) => {
                            let err = result.error.unwrap_or_default();
                            let _ = tx.try_send(ServerMsg::System {
                                message: format!("[tool] set_brightness ✗ {err}"),
                            });
                        }
                        Err(e) => {
                            let _ = tx.try_send(ServerMsg::Error {
                                id: None,
                                message: format!("set_brightness error: {e}"),
                            });
                        }
                    }
                }
                Some(OfflineIntent::Power(action)) => {
                    let _ = tx.try_send(ServerMsg::System {
                        message: "[local] offline intent executed".into(),
                    });
                    // Destructive — always confirm via the WS broker. The
                    // label shows the REAL systemctl verb (poweroff, never
                    // shutdown — RBRO-004 WP-06.6).
                    let (reply_tx, mut reply_rx) = unbounded_channel::<bool>();
                    let cid = next_id("confirm");
                    conn.prompts.register_confirm(cid.clone(), reply_tx);
                    let verb = device::power_verb(&action).unwrap_or(&action);
                    let human = match verb {
                        "poweroff" => "power off",
                        "reboot" => "reboot",
                        _ => "suspend",
                    };
                    let _ = tx.try_send(ServerMsg::ConfirmRequest {
                        id: cid,
                        label: format!("Run `systemctl {verb}`? This will {human} the system."),
                    });
                    let confirmed = reply_rx.recv().await.unwrap_or(false);
                    if !confirmed {
                        let _ = tx.try_send(ServerMsg::System {
                            message: "[local] power action cancelled.".into(),
                        });
                    } else if let Ok(result) = device::power(&action) {
                        if result.success {
                            let human = match verb {
                                "poweroff" => "powering off",
                                "reboot" => "rebooting",
                                _ => "suspending",
                            };
                            let _ = tx.try_send(ServerMsg::System {
                                message: format!("[tool] power ✓ — {human}"),
                            });
                        } else {
                            let err = result.error.unwrap_or_default();
                            let _ = tx.try_send(ServerMsg::System {
                                message: format!("[tool] power ✗ {err}"),
                            });
                        }
                    }
                }
                Some(OfflineIntent::Help)
                | Some(OfflineIntent::Tools)
                | Some(OfflineIntent::Version) => {}
                None => {
                    let _ = tx.try_send(ServerMsg::System {
                        message: "[system] Offline — no API key configured. Connect a provider \
                                   (F6 / /model) or use /connect <name> to add a key."
                            .into(),
                    });
                }
            }
            end_turn(&state);
            state.broadcast_status();
            let _ = tx.try_send(ServerMsg::StreamDone { id });
        });
    }
}

/// Route one client message.
pub(crate) async fn handle_client_msg(state: &DaemonState, conn: &Conn, msg: ClientMsg) {
    match msg {
        ClientMsg::ChatSend { id, text, .. } => handle_chat_send(state, conn, id, text).await,
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
        // MASTER FIX S2 — first-run wizard STEP 0: set the login password.
        // Runs the root wrapper `diad-set-password` (sudoers argv-pinned)
        // with the value piped on stdin — never in argv, never logged, never
        // sent to any LLM. The wrapper pins the user to `diad` and enforces
        // length >= 8.
        ClientMsg::SetUserPassword { id, value } => {
            let tx = conn.tx.clone();
            // The DIAD login user is always `diad`; the wrapper refuses any
            // other user as a backstop.
            const DIAD_USER: &str = "diad";
            std::process::Command::new("sudo")
                .arg("/usr/local/bin/diad-set-password")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut child| {
                    use std::io::Write;
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = writeln!(stdin, "{DIAD_USER}:{value}");
                    }
                    let output = child.wait_with_output()?;
                    Ok(output)
                })
                .map(|output| {
                    if output.status.success() {
                        let _ = tx.try_send(ServerMsg::System {
                            message: format!(
                                "Password changed (id {id}) — use it at the login screen."
                            ),
                        });
                    } else {
                        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
                        let _ = tx.try_send(ServerMsg::Error {
                            id: Some(id.clone()),
                            message: if err.is_empty() {
                                "Password change failed".to_string()
                            } else {
                                format!("Password change failed: {err}")
                            },
                        });
                    }
                })
                .unwrap_or_else(|e| {
                    let _ = tx.try_send(ServerMsg::Error {
                        id: Some(id),
                        message: format!("Could not run diad-set-password: {e}"),
                    });
                });
        }
        ClientMsg::TermInput { data } => {
            let tx = conn.tx.clone();
            if let Err(e) = state.pty.input(&data, tx) {
                let _ = conn.tx.try_send(ServerMsg::Error {
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
                    let _ = tx.try_send(ServerMsg::Error {
                        id: Some(id.clone()),
                        message: format!("path not allowed: {path}"),
                    });
                    return;
                }
                match std::fs::read_to_string(shellexpand::tilde(&path).as_ref()) {
                    Ok(content) => {
                        let _ = tx.try_send(ServerMsg::FileContent { id, path, content });
                    }
                    Err(e) => {
                        let _ = tx.try_send(ServerMsg::Error {
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
                    let _ = tx.try_send(ServerMsg::Error {
                        id: Some(id.clone()),
                        message: format!("path not allowed: {path}"),
                    });
                    return;
                }
                match std::fs::write(shellexpand::tilde(&path).as_ref(), content) {
                    Ok(()) => {
                        let _ = tx.try_send(ServerMsg::FileWritten { id, path });
                    }
                    Err(e) => {
                        let _ = tx.try_send(ServerMsg::Error {
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
                    let _ = tx.try_send(ServerMsg::Error {
                        id: Some(id.clone()),
                        message: format!("path not allowed: {path}"),
                    });
                    return;
                }
                let entries = list_dir_entries(&path);
                let _ = tx.try_send(ServerMsg::Files { id, path, entries });
            });
        }
        ClientMsg::ProviderSwitch { name } => {
            let key_present = state.factory.has_key(&name);
            *state.provider.write().unwrap() = name.clone();
            state.connected.store(key_present, Ordering::SeqCst);
            state.broadcast_status();
            // Offer the model list for the new provider.
            let models = state.factory.models_for(&name);
            let current = state.model.read().unwrap().clone();
            let _ = conn.tx.try_send(ServerMsg::Models {
                id: "model".into(),
                provider: name,
                models,
                current,
            });
            let _ = conn.tx.try_send(ServerMsg::System {
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
                            let _ = conn2.tx.try_send(ServerMsg::Error {
                                id: Some(id.clone()),
                                message: format!("vault error: {e}"),
                            });
                            return;
                        }
                        let _ = conn2.tx.try_send(ServerMsg::System {
                            message: format!("[system] Key stored for {name}."),
                        });
                        let _ = conn2.tx.try_send(ServerMsg::Providers {
                            id,
                            entries: state2.provider_entries_dto(),
                        });
                        if state2.provider.read().unwrap().eq_ignore_ascii_case(&name) {
                            state2.connected.store(true, Ordering::SeqCst);
                        }
                        state2.broadcast_status();
                    }
                    Err(e) => {
                        let _ = conn2.tx.try_send(ServerMsg::Error {
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
                        let _ = tx.try_send(ServerMsg::System {
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
                        let _ = tx.try_send(ServerMsg::Error {
                            id: Some(id.clone()),
                            message: format!("agent delegate error: {e}"),
                        });
                    }
                }
                let _ = tx.try_send(ServerMsg::StreamDone { id });
            });
        }
        ClientMsg::ConfigSet { key, value } => {
            apply_config_set(state, &key, value);
        }
        ClientMsg::ProvidersList { id } => {
            let _ = conn.tx.try_send(ServerMsg::Providers {
                id,
                entries: state.provider_entries_dto(),
            });
        }
        ClientMsg::ModelsList { id, provider } => {
            let models = state.factory.models_for(&provider);
            let current = state.model.read().unwrap().clone();
            let _ = conn.tx.try_send(ServerMsg::Models {
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
                let _ = tx.try_send(ServerMsg::Stats {
                    id,
                    tokens: state2.tokens.load(Ordering::SeqCst),
                    latency_ms: state2.latency_ms.load(Ordering::SeqCst),
                    battery,
                    wifi,
                    cwd,
                    idle_stage: state2.idle.get().label().to_string(),
                    rss_mb: crate::core::perf::read_self_rss_kb() / 1024,
                    perf_tier: state2.perf_tier.clone(),
                });
            });
        }
        ClientMsg::AgentsList { id } => {
            let _ = conn.tx.try_send(ServerMsg::Agents {
                id,
                agents: state.detected_agents(),
            });
        }
        ClientMsg::TrashList { id } => {
            let tx = conn.tx.clone();
            tokio::task::spawn_blocking(move || {
                let result = crate::tools::files::trash::trash_list();
                let _ = tx.try_send(server_trash_list_response(id, result));
            });
        }
        ClientMsg::TrashMove { id, path } => {
            let registry = Arc::clone(&state.registry);
            let tx = conn.tx.clone();
            tokio::task::spawn_blocking(move || {
                if !registry.is_path_allowed(&path) {
                    let _ = tx.try_send(ServerMsg::Error {
                        id: Some(id.clone()),
                        message: format!("path not allowed: {path}"),
                    });
                    return;
                }
                // Expand `~` like file.read/file.write so GUI paths work;
                // trash_move's own valid_path requires an absolute path.
                let expanded = shellexpand::tilde(&path).to_string();
                let result = crate::tools::files::trash::trash_move(&expanded);
                match result {
                    Ok(r) if r.success => {
                        let _ = tx.try_send(ServerMsg::TrashMoved { id });
                    }
                    Ok(r) => {
                        let _ = tx.try_send(ServerMsg::Error {
                            id: Some(id),
                            message: r.error.unwrap_or_default(),
                        });
                    }
                    Err(e) => {
                        let _ = tx.try_send(ServerMsg::Error {
                            id: Some(id),
                            message: format!("trash: {e}"),
                        });
                    }
                }
            });
        }
        ClientMsg::TrashRestore { id, name } => {
            let tx = conn.tx.clone();
            tokio::task::spawn_blocking(move || {
                let result = crate::tools::files::trash::trash_restore(&name);
                match result {
                    Ok(r) if r.success => {
                        let _ = tx.try_send(ServerMsg::TrashRestored { id });
                    }
                    Ok(r) => {
                        let _ = tx.try_send(ServerMsg::Error {
                            id: Some(id),
                            message: r.error.unwrap_or_default(),
                        });
                    }
                    Err(e) => {
                        let _ = tx.try_send(ServerMsg::Error {
                            id: Some(id),
                            message: format!("trash: {e}"),
                        });
                    }
                }
            });
        }
        ClientMsg::TrashEmpty { id } => {
            let tx = conn.tx.clone();
            tokio::task::spawn_blocking(move || {
                let result = crate::tools::files::trash::trash_empty();
                match result {
                    Ok(r) if r.success => {
                        let _ = tx.try_send(ServerMsg::TrashEmptied { id });
                    }
                    Ok(r) => {
                        let _ = tx.try_send(ServerMsg::Error {
                            id: Some(id),
                            message: r.error.unwrap_or_default(),
                        });
                    }
                    Err(e) => {
                        let _ = tx.try_send(ServerMsg::Error {
                            id: Some(id),
                            message: format!("trash: {e}"),
                        });
                    }
                }
            });
        }
        ClientMsg::ClipboardList { id } => {
            let tx = conn.tx.clone();
            tokio::task::spawn_blocking(move || {
                let result = crate::tools::files::files_universal::get_clipboard_history();
                let entries = match result {
                    Ok(r) if r.success => parse_clipboard_entries(&r.output),
                    _ => Vec::new(),
                };
                let _ = tx.try_send(ServerMsg::ClipboardList { id, items: entries });
            });
        }
        ClientMsg::ClipboardPick { id, index } => {
            let tx = conn.tx.clone();
            tokio::task::spawn_blocking(move || {
                let result = crate::tools::files::files_universal::clipboard_pick(index);
                match result {
                    Ok(r) if r.success => {
                        let _ = tx.try_send(ServerMsg::ClipboardPicked { id });
                    }
                    Ok(r) => {
                        let _ = tx.try_send(ServerMsg::Error {
                            id: Some(id),
                            message: r.error.unwrap_or_default(),
                        });
                    }
                    Err(e) => {
                        let _ = tx.try_send(ServerMsg::Error {
                            id: Some(id),
                            message: format!("clipboard: {e}"),
                        });
                    }
                }
            });
        }
        ClientMsg::ClipboardClear { id } => {
            let tx = conn.tx.clone();
            tokio::task::spawn_blocking(move || {
                let result = crate::tools::files::files_universal::clear_clipboard_history();
                match result {
                    Ok(r) if r.success => {
                        let _ = tx.try_send(ServerMsg::ClipboardCleared { id });
                    }
                    Ok(r) => {
                        let _ = tx.try_send(ServerMsg::Error {
                            id: Some(id),
                            message: r.error.unwrap_or_default(),
                        });
                    }
                    Err(e) => {
                        let _ = tx.try_send(ServerMsg::Error {
                            id: Some(id),
                            message: format!("clipboard: {e}"),
                        });
                    }
                }
            });
        }
    }
}

/// Map a `trash_list` result onto the structured `ServerMsg::TrashList`
/// (empty trash degrades to an empty item list, never an error).
fn server_trash_list_response(
    id: String,
    result: anyhow::Result<crate::tools::ToolResult>,
) -> ServerMsg {
    let items = match result {
        Ok(r) if r.success => parse_trash_entries(&r.output),
        _ => Vec::new(),
    };
    ServerMsg::TrashList { id, items }
}

/// Parse `trash_list` output lines: `name | date | size | original-path`.
fn parse_trash_entries(output: &str) -> Vec<crate::core::protocol::TrashEntryDto> {
    let mut entries = Vec::new();
    if output.trim().is_empty() || output.trim() == "trash is empty" {
        return entries;
    }
    for line in output.lines() {
        let mut parts = line.splitn(4, " | ");
        let name = parts.next().unwrap_or("").trim();
        let deleted_at = parts.next().unwrap_or("").trim();
        let size = parts
            .next()
            .unwrap_or("0")
            .trim()
            .parse::<u64>()
            .unwrap_or(0);
        let original_path = parts.next().unwrap_or("").trim();
        if !name.is_empty() {
            entries.push(crate::core::protocol::TrashEntryDto {
                name: name.to_string(),
                original_path: original_path.to_string(),
                deleted_at: deleted_at.to_string(),
                size,
            });
        }
    }
    entries
}

/// Parse `cliphist list` output (`<id>\t<preview>`) into clipboard entries.
fn parse_clipboard_entries(output: &str) -> Vec<crate::core::protocol::ClipboardEntryDto> {
    let mut entries = Vec::new();
    for line in output.lines() {
        let preview = match line.split_once('\t') {
            Some((_, v)) => v,
            None => line,
        };
        let preview = preview.trim();
        if !preview.is_empty() {
            entries.push(crate::core::protocol::ClipboardEntryDto {
                index: entries.len(),
                preview: preview.to_string(),
            });
        }
    }
    entries
}

/// THE LAW F-15: serializable snapshot of the user-configurable surface.
/// Sent in `Status`/`Connected` so every frontend reflects current values on
/// open. Keys must stay in sync with `apply_config_set`.
pub(crate) fn config_snapshot(config: &crate::config::settings::DiadConfig) -> serde_json::Value {
    serde_json::json!({
        "theme": config.theme,
        "default_provider": config.default_provider,
        "confirm_destructive": config.confirm_destructive,
        "stream_tokens": config.stream_tokens,
        "max_history": config.max_history,
        "auto_save_interval": config.auto_save_interval,
        "dnd_enabled": config.dnd_enabled,
        "night_light_temp": config.night_light_temp,
    })
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
                if v > 0 {
                    config.max_history = v as usize;
                }
            }
        }
        "auto_save_interval" => {
            // THE LAW F-15: auto-save cadence in seconds, 0 = disabled,
            // validated 0..=3600. The server autosave loop re-reads config
            // every tick, so this applies live without a restart.
            if let Some(v) = value.as_u64() {
                config.auto_save_interval = v.min(3600);
            }
        }
        "perf_tier" => {
            // THE LAW F-15: tier override → diad-conf-write (validated both
            // here and in the wrapper).
            if let Some(v) = value.as_str() {
                if let Ok(result) = crate::tools::system::prefs::set_perf_tier(v) {
                    if !result.success {
                        state.broadcast(ServerMsg::Error {
                            id: None,
                            message: format!(
                                "config.set: perf_tier: {}",
                                result.error.unwrap_or_default()
                            ),
                        });
                        return;
                    }
                }
            }
        }
        "waybar_mode" => {
            if let Some(v) = value.as_str() {
                if let Ok(result) = crate::tools::system::prefs::set_waybar_mode(v) {
                    if !result.success {
                        state.broadcast(ServerMsg::Error {
                            id: None,
                            message: format!(
                                "config.set: waybar_mode: {}",
                                result.error.unwrap_or_default()
                            ),
                        });
                        return;
                    }
                }
            }
        }
        "keybinding" => {
            // Expects {"from": "...", "to": "..."} — remap a sway keybinding.
            let from = value.get("from").and_then(|v| v.as_str()).unwrap_or("");
            let to = value.get("to").and_then(|v| v.as_str()).unwrap_or("");
            if from.is_empty() || to.is_empty() {
                state.broadcast(ServerMsg::Error {
                    id: None,
                    message: "config.set: keybinding needs {\"from\": ..., \"to\": ...}".into(),
                });
                return;
            }
            if let Ok(result) = crate::tools::system::prefs::set_keybinding(from, to) {
                if !result.success {
                    state.broadcast(ServerMsg::Error {
                        id: None,
                        message: format!(
                            "config.set: keybinding: {}",
                            result.error.unwrap_or_default()
                        ),
                    });
                    return;
                }
            }
        }
        "dnd_enabled" => {
            // A7 parity: mako do-not-disturb via makoctl.
            if let Some(v) = value.as_bool() {
                config.dnd_enabled = v;
                let _ = crate::tools::system::desktop::dnd(v);
            }
        }
        "night_light" => {
            // A7 parity: gammastep. Value is {"enabled": bool,
            // "temperature": u64} or a bare bool/u64 for compatibility.
            let enabled = match value.get("enabled") {
                Some(v) => v.as_bool().unwrap_or(false),
                None => value.as_bool().unwrap_or(false),
            };
            let temp = value
                .get("temperature")
                .and_then(|v| v.as_u64())
                .unwrap_or(config.night_light_temp);
            config.night_light_temp = if enabled { temp } else { 0 };
            let _ = crate::tools::system::desktop::night_light(enabled, temp as u32);
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
        b_dir.cmp(&a_dir).then_with(|| a.name.cmp(&b.name))
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
        let factory = Arc::new(ProviderFactory::new(
            Arc::clone(&vault),
            Arc::clone(&config),
        ));
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
    fn test_streaming_counter_survives_concurrent_turns() {
        // F-19 regression: streaming must reflect the LAST in-flight turn,
        // not the first to finish. With a bool, two concurrent chat_send
        // raced and the earlier finisher cleared the flag while the second
        // was still running.
        let state = test_state();

        // Two turns start while none are running.
        state.streaming.fetch_add(1, Ordering::SeqCst);
        state.streaming.fetch_add(1, Ordering::SeqCst);
        assert!(state.streaming.load(Ordering::SeqCst) > 0);

        // First turn finishes — streaming must STILL be on (second is live).
        state.streaming.fetch_sub(1, Ordering::SeqCst);
        assert!(
            state.streaming.load(Ordering::SeqCst) > 0,
            "streaming cleared while a concurrent turn was still in flight"
        );

        // Second turn finishes — now (and only now) streaming is off.
        state.streaming.fetch_sub(1, Ordering::SeqCst);
        assert_eq!(state.streaming.load(Ordering::SeqCst), 0);

        // Status snapshot reflects the counter as a bool.
        assert!(!matches!(
            state.status_snapshot(),
            ServerMsg::Status {
                streaming: true,
                ..
            }
        ));
    }

    #[test]
    fn test_streaming_counter_never_underflows() {
        // A stray end_turn when nothing is in flight must not wrap to a
        // huge positive count (which would pin streaming=true forever).
        let state = test_state();
        end_turn(&state);
        end_turn(&state);
        assert_eq!(state.streaming.load(Ordering::SeqCst), 0);
        assert!(!matches!(
            state.status_snapshot(),
            ServerMsg::Status {
                streaming: true,
                ..
            }
        ));
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
        assert!(
            entries.len() >= 10,
            "expected 10+ providers, got {}",
            entries.len()
        );
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        for expected in [
            "anthropic",
            "gemini",
            "openai",
            "kimi",
            "deepseek",
            "groq",
            "ollama",
        ] {
            assert!(names.contains(&expected), "missing {expected}");
        }
    }

    #[test]
    fn test_config_set_default_provider() {
        let state = test_state();
        apply_config_set(&state, "default_provider", serde_json::json!("openai"));
        assert_eq!(state.config.read().unwrap().default_provider, "openai");
    }

    /// THE LAW F-15 regression: every new config key round-trips through
    /// `apply_config_set` and lands in the persisted `DiadConfig`.
    #[test]
    fn test_config_set_the_law_roundtrip() {
        let state = test_state();

        apply_config_set(&state, "auto_save_interval", serde_json::json!(120));
        assert_eq!(state.config.read().unwrap().auto_save_interval, 120);

        apply_config_set(&state, "max_history", serde_json::json!(250));
        assert_eq!(state.config.read().unwrap().max_history, 250);

        apply_config_set(&state, "stream_tokens", serde_json::json!(true));
        assert!(state.config.read().unwrap().stream_tokens);
        assert!(state.stream_tokens.load(Ordering::SeqCst));

        apply_config_set(&state, "confirm_destructive", serde_json::json!(true));
        assert!(state.config.read().unwrap().confirm_destructive);

        // auto_save_interval clamps to the validated 0..=3600 range.
        apply_config_set(&state, "auto_save_interval", serde_json::json!(999999));
        assert_eq!(state.config.read().unwrap().auto_save_interval, 3600);
    }

    /// THE LAW F-15 regression: invalid values for the intent-backed keys are
    /// rejected with an Error broadcast, never persisted or executed.
    #[test]
    fn test_config_set_rejects_invalid_intent_values() {
        let state = test_state();

        // Invalid tier → Error, config untouched.
        apply_config_set(&state, "perf_tier", serde_json::json!("GODMODE"));
        // Invalid keybinding shape → Error.
        apply_config_set(&state, "keybinding", serde_json::json!({}));
        // Invalid waybar mode → Error.
        apply_config_set(&state, "waybar_mode", serde_json::json!("gigantic"));
        // Unknown key → Error.
        apply_config_set(&state, "not_a_real_key", serde_json::json!(1));

        // None of the above should have crashed or mutated the config.
        let cfg = state.config.read().unwrap();
        assert_eq!(
            cfg.auto_save_interval,
            DiadConfig::default().auto_save_interval
        );
        assert_eq!(cfg.max_history, DiadConfig::default().max_history);
    }

    /// RBRO-004 WP-06.6 — slash power commands map to device::power actions
    /// (which in turn map to real systemctl verbs via device::power_verb).
    #[test]
    fn test_slash_power_action_mapping() {
        assert_eq!(slash_power_action("/shutdown"), Some("shutdown"));
        assert_eq!(slash_power_action("/reboot"), Some("reboot"));
        assert_eq!(slash_power_action("/sleep"), Some("suspend"));
        // Non-power slashes and junk are NOT power actions.
        assert_eq!(slash_power_action("/help"), None);
        assert_eq!(slash_power_action("/tools"), None);
        assert_eq!(slash_power_action("shutdown"), None);
        assert_eq!(slash_power_action(""), None);
        assert_eq!(slash_power_action("rm -rf /"), None);
    }

    /// RBRO-004 WP-06.6 — the slash path converges on the same systemctl verb
    /// the sudoers file pins (poweroff, never `shutdown`).
    #[test]
    fn test_slash_power_verbs_converge_with_sudoers() {
        // device::power_verb maps the slash action to the verb that actually
        // runs. `shutdown` must become `poweroff` (sudoers pins poweroff).
        assert_eq!(device::power_verb("shutdown"), Some("poweroff"));
        assert_eq!(device::power_verb("reboot"), Some("reboot"));
        assert_eq!(device::power_verb("suspend"), Some("suspend"));
    }
}
