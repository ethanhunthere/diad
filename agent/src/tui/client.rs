//! TUI WebSocket client — the ratatui app as a thin face of the daemon.
//!
//! `diad-agent --tui` no longer owns providers, tools or the orchestrator.
//! It connects to the daemon on `ws://127.0.0.1:7860/ws` and maps:
//!
//! - App actions → `ClientMsg` (chat.send, provider.switch, …)
//! - `ServerMsg` → `Event` (the exact `Event` enum the TUI always consumed)
//!
//! This guarantees feature parity between GUI and TUI: both render the same
//! streamed tokens, confirmations, secure prompts, offline routing and diffs.

use std::sync::atomic::{AtomicU64, Ordering};

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio::net::TcpStream;

use crate::core::protocol::{ClientMsg, Envelope, ServerMsg, PROTOCOL_VERSION};
use crate::providers::ProviderEntry;
use crate::tui::event::Event;

/// A connection to the DIAD AGENT daemon.
pub struct DaemonClient {
    /// Read half of the socket (owned by `spawn_reader`'s task).
    stream: futures_util::stream::SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    /// Sink for outgoing client messages (writer task).
    out: UnboundedSender<ClientMsg>,
    next_id: AtomicU64,
    /// Where daemon events are delivered (the TUI event loop).
    events: UnboundedSender<Event>,
}

impl DaemonClient {
    /// Connect to the daemon, spawn the writer task and return the client.
    pub async fn connect(events: UnboundedSender<Event>) -> anyhow::Result<Self> {
        let url = crate::core::ws_url();
        let (ws, _resp) = tokio_tungstenite::connect_async(&url).await?;

        // Split: the writer owns the sink; the reader owns the stream.
        let (mut sink, stream) = ws.split();
        let (out, mut out_rx) = unbounded_channel::<ClientMsg>();

        // Writer task: ClientMsg → WS text frames (versioned envelopes).
        tokio::spawn(async move {
            while let Some(msg) = out_rx.recv().await {
                let env = Envelope {
                    v: PROTOCOL_VERSION,
                    msg,
                };
                let text = match serde_json::to_string(&env) {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::error!("client: failed to serialize message: {e}");
                        continue;
                    }
                };
                if sink.send(WsMessage::Text(text.into())).await.is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            stream,
            out,
            next_id: AtomicU64::new(0),
            events,
        })
    }

    /// Spawn the reader task mapping daemon `ServerMsg`s to TUI `Event`s.
    ///
    /// Consumes the client and returns the send-only `ClientHandle` plus a
    /// handle to the reader task.
    pub fn spawn_reader(self) -> (ClientHandle, tokio::task::JoinHandle<()>) {
        let DaemonClient {
            stream,
            out,
            next_id,
            events,
        } = self;
        let mut stream = stream;
        let reader_events = events.clone();
        let reader_out = out.clone();
        let reader = tokio::spawn(async move {
            while let Some(msg) = stream.next().await {
                match msg {
                    Ok(WsMessage::Text(text)) => {
                        let server: ServerMsg = match serde_json::from_str(&text) {
                            Ok(m) => m,
                            Err(e) => {
                                let _ = reader_events.send(Event::Error(format!(
                                    "protocol error: {e}"
                                )));
                                continue;
                            }
                        };
                        map_server_msg(server, &reader_events, &reader_out);
                    }
                    Ok(WsMessage::Close(_)) => break,
                    Err(e) => {
                        let _ = reader_events.send(Event::Error(format!(
                            "daemon connection lost: {e}"
                        )));
                        break;
                    }
                    _ => {}
                }
            }
            let _ = reader_events.send(Event::AppendMessage(
                "[system] daemon connection closed — reconnecting…".into(),
            ));
        });
        (
            ClientHandle {
                out,
                next_id,
                events,
            },
            reader,
        )
    }
}

/// Send-only handle to the daemon, plus local conveniences.
pub struct ClientHandle {
    out: UnboundedSender<ClientMsg>,
    next_id: AtomicU64,
    events: UnboundedSender<Event>,
}

impl ClientHandle {
    /// Unique id for request/response correlation.
    pub fn next_id(&self) -> String {
        format!("tui-{}", self.next_id.fetch_add(1, Ordering::Relaxed))
    }

    fn send(&self, msg: ClientMsg) {
        let _ = self.out.send(msg);
    }

    // ---- Outbound actions -------------------------------------------------

    pub fn chat_send(&self, text: String) {
        self.send(ClientMsg::ChatSend {
            id: self.next_id(),
            text,
        });
    }

    pub fn provider_switch(&self, name: String) {
        self.send(ClientMsg::ProviderSwitch { name });
    }

    pub fn model_switch(&self, model: String) {
        self.send(ClientMsg::ModelSwitch { model });
    }

    pub fn provider_connect(&self, name: String, key: String) {
        self.send(ClientMsg::ProviderConnect {
            id: self.next_id(),
            name,
            key,
        });
    }

    pub fn providers_list(&self) {
        self.send(ClientMsg::ProvidersList {
            id: self.next_id(),
        });
    }

    pub fn models_list(&self, provider: String) {
        self.send(ClientMsg::ModelsList {
            id: self.next_id(),
            provider,
        });
    }

    pub fn agents_list(&self) {
        self.send(ClientMsg::AgentsList {
            id: self.next_id(),
        });
    }

    pub fn agent_delegate(&self, agent: String, task: String) {
        self.send(ClientMsg::AgentDelegate {
            id: self.next_id(),
            agent,
            task,
        });
    }

    pub fn config_set(&self, key: String, value: serde_json::Value) {
        self.send(ClientMsg::ConfigSet { key, value });
    }

    pub fn event(&self, event: Event) {
        let _ = self.events.send(event);
    }

    /// Open the GUI (used by F9).
    pub fn open_gui(&self) {
        crate::core::open_gui();
    }
}

/// Map one `ServerMsg` onto the TUI `Event` stream.
///
/// Secret/confirm/diff prompts register a reply channel; when the TUI answers
/// (via `App` request flows), the value is forwarded over WS to the daemon.
fn map_server_msg(msg: ServerMsg, events: &UnboundedSender<Event>, out: &UnboundedSender<ClientMsg>) {
    match msg {
        ServerMsg::Token { text, .. } => {
            let _ = events.send(Event::StreamChunk(text));
        }
        ServerMsg::StreamDone { .. } => {
            let _ = events.send(Event::StreamDone);
        }
        ServerMsg::System { message } => {
            let _ = events.send(Event::AppendMessage(message));
        }
        ServerMsg::Error { message, .. } => {
            let _ = events.send(Event::Error(message));
        }
        ServerMsg::SecretRequest { id, label } => {
            let (reply_tx, mut reply_rx) = unbounded_channel::<String>();
            let out = out.clone();
            tokio::spawn(async move {
                if let Some(value) = reply_rx.recv().await {
                    let _ = out.send(ClientMsg::SecretSubmit { id, value });
                }
            });
            let _ = events.send(Event::SecurePromptRequest { label, reply: reply_tx });
        }
        ServerMsg::ConfirmRequest { id, label } => {
            let (reply_tx, mut reply_rx) = unbounded_channel::<bool>();
            let out = out.clone();
            tokio::spawn(async move {
                if let Some(ok) = reply_rx.recv().await {
                    let _ = out.send(if ok {
                        ClientMsg::ToolApprove { id }
                    } else {
                        ClientMsg::ToolReject { id }
                    });
                }
            });
            let _ = events.send(Event::ConfirmationRequest { label, reply: reply_tx });
        }
        ServerMsg::EditDiff { id, path, diff: _ } => {
            // TUI parity: surface edit approval as a y/N confirmation.
            let (reply_tx, mut reply_rx) = unbounded_channel::<bool>();
            let out = out.clone();
            tokio::spawn(async move {
                if let Some(ok) = reply_rx.recv().await {
                    let _ = out.send(if ok {
                        ClientMsg::EditApprove { id }
                    } else {
                        ClientMsg::EditReject { id }
                    });
                }
            });
            let _ = events.send(Event::ConfirmationRequest {
                label: format!("Apply proposed edit to {path}? [y/N]"),
                reply: reply_tx,
            });
        }
        ServerMsg::Status {
            provider,
            model,
            connected,
            ..
        } => {
            let _ = events.send(Event::StatusUpdate {
                provider,
                model,
                connected,
            });
        }
        ServerMsg::Models {
            provider,
            models,
            current,
            ..
        } => {
            let _ = events.send(Event::ModelsList {
                provider,
                models,
                current,
            });
        }
        ServerMsg::Providers { entries, .. } => {
            let entries: Vec<ProviderEntry> = entries
                .into_iter()
                .map(|e| ProviderEntry {
                    name: e.name,
                    needs_key: e.needs_key,
                    has_key: e.has_key,
                })
                .collect();
            let _ = events.send(Event::ProvidersList(entries));
        }
        ServerMsg::ToolUse {
            call_id,
            name,
            args,
            confirm_required,
            ..
        } => {
            let _ = events.send(Event::ToolCallStart {
                call_id,
                name,
                args,
                confirm_required,
            });
        }
        ServerMsg::ToolResult {
            call_id,
            success,
            status_line,
            ..
        } => {
            let _ = events.send(Event::ToolCallResult {
                call_id,
                success,
                output: status_line,
            });
        }
        ServerMsg::Connected { .. } => {
            let _ = events.send(Event::AppendMessage(
                "[system] connected to DIAD AGENT daemon".into(),
            ));
        }
        ServerMsg::TermOutput { .. }
        | ServerMsg::Stats { .. }
        | ServerMsg::Agents { .. }
        | ServerMsg::FileContent { .. }
        | ServerMsg::Files { .. }
        | ServerMsg::FileWritten { .. } => {}
    }
}

/// Convenience: try to connect; if the daemon is down, start one first.
pub async fn connect_or_spawn(events: UnboundedSender<Event>) -> anyhow::Result<DaemonClient> {
    if !crate::core::daemon_alive().await {
        crate::core::wait_for_daemon(std::time::Duration::from_secs(1)).await;
    }
    DaemonClient::connect(events).await
}
