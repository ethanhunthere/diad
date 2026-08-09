use std::time::Duration;

use crossterm::event::{self, KeyEvent};
use tokio::sync::mpsc;

/// Events that can occur in the DIAD Agent event loop.
#[derive(Debug, Clone)]
pub enum Event {
    /// A keyboard event from the terminal
    Key(KeyEvent),
    /// Periodic tick (every 250ms)
    Tick,
    /// A streaming response chunk from an LLM provider (complete message)
    ProviderResponse(String),
    /// A live text delta from a streaming provider response
    StreamChunk(String),
    /// A streaming response has finished
    StreamDone,
    /// A result from a tool execution
    ToolResult(String),
    /// An error event
    Error(String),
    /// Append a raw message line to the chat (prefix included by sender:
    /// "[You]", "[Assistant]", "[tool]", "[system]", "[local]", ...)
    AppendMessage(String),
    /// Request a masked (hidden) input from the user — e.g. Wi-Fi password.
    /// The value is delivered ONLY through `reply`; it is never logged,
    /// stored, or sent to the LLM. An empty string means the user cancelled.
    SecurePromptRequest {
        label: String,
        reply: tokio::sync::mpsc::UnboundedSender<String>,
    },
    /// Request an explicit yes/no confirmation from the user in the TUI.
    ConfirmationRequest {
        label: String,
        reply: tokio::sync::mpsc::UnboundedSender<bool>,
    },
    /// Offline Wi-Fi flow: the network list finished. Payload is the raw
    /// nmcli listing; the TUI displays it and asks the user for an SSID.
    OfflineWifiListed(String),
    /// The agentic loop finished a turn and returns the updated conversation
    /// history (including tool_use / tool_result messages).
    HistoryUpdated(Vec<crate::providers::traits::Message>),
    /// A provider was selected/connected (from the picker or /connect). The
    /// payload is the provider name; the TUI switches to it and opens the
    /// model picker.
    ProviderConnected(String),
    /// Status snapshot from the daemon (WS bridge).
    StatusUpdate {
        provider: String,
        model: String,
        connected: bool,
    },
    /// Model list for a provider (WS bridge).
    ModelsList {
        provider: String,
        models: Vec<String>,
        current: String,
    },
    /// Provider list with connection status (WS bridge).
    ProvidersList(Vec<crate::providers::ProviderEntry>),
    /// The model requested a tool call (rendered as a card in the GUI;
    /// the TUI renders the confirm prompt separately).
    ToolCallStart {
        call_id: String,
        name: String,
        args: serde_json::Value,
        confirm_required: bool,
    },
    /// A tool call finished executing.
    ToolCallResult {
        call_id: String,
        success: bool,
        output: String,
    },
    /// The model proposed a file edit; the user must approve/reject it.
    /// Only the GUI renders a diff card; the TUI maps this onto the
    /// confirmation prompt (label carries the path).
    EditDiffRequest {
        id: String,
        path: String,
        diff: String,
        reply: tokio::sync::mpsc::UnboundedSender<bool>,
    },
}

/// Polls for terminal events and sends them through an mpsc channel.
pub struct EventLoop {
    /// Receiver end of the event channel
    rx: mpsc::UnboundedReceiver<Event>,
    /// Sender end (cloned for async tasks)
    tx: mpsc::UnboundedSender<Event>,
}

impl EventLoop {
    /// Create a new EventLoop.
    ///
    /// Spawns a background tokio task that polls for crossterm events
    /// and sends them through an unbounded channel.
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let tick_tx = tx.clone();

        tokio::spawn(async move {
            let tick_rate = Duration::from_millis(250);
            let mut last_tick = tokio::time::Instant::now();

            loop {
                // Poll with a short timeout so we can emit ticks
                let timeout = tick_rate
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or(Duration::from_millis(0));

                if event::poll(timeout).unwrap_or(false) {
                    match event::read() {
                        Ok(event::Event::Key(key)) => {
                            if tick_tx.send(Event::Key(key)).is_err() {
                                break; // Receiver dropped
                            }
                        }
                        Ok(event::Event::Resize(_, _)) => {
                            // Resize events are handled automatically by ratatui
                        }
                        Ok(_) => {}
                        Err(e) => {
                            let _ = tick_tx.send(Event::Error(format!("Event read error: {e}")));
                            break;
                        }
                    }
                }

                if last_tick.elapsed() >= tick_rate {
                    if tick_tx.send(Event::Tick).is_err() {
                        break;
                    }
                    last_tick = tokio::time::Instant::now();
                }
            }
        });

        Self { rx, tx }
    }

    /// Get a clone of the event sender, for use by async tasks.
    pub fn sender(&self) -> mpsc::UnboundedSender<Event> {
        self.tx.clone()
    }

    /// Wait for and return the next event.
    pub async fn next(&mut self) -> anyhow::Result<Event> {
        self.rx
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("Event loop channel closed"))
    }
}
