//! The keyboard-first TUI client (`diad-agent --tui`).
//!
//! A thin ratatui consumer of the same daemon WebSocket API the GUI uses:
//!
//! - `app` — application state and key handling (mode switching, input buffer)
//! - `client` — WebSocket client connection to the daemon
//! - `event` — input/network event loop with non-blocking polling
//! - `ui` — ratatui rendering of chat, file tree, code and status views

pub mod app;
pub mod client;
pub mod event;
pub mod ui;
