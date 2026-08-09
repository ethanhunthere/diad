//! DIAD AGENT daemon — one brain, two faces.
//!
//! The daemon owns all application logic (providers, tools, orchestrator,
//! vault, sessions, offline router, PTY) and serves two interchangeable
//! frontends over a versioned WebSocket JSON API bound to loopback:
//!
//! - **GUI** (default): `diad-agent` starts the daemon and opens the
//!   DIAD AGENT window (`brave --app=http://127.0.0.1:7860`).
//! - **TUI**: `diad-agent --tui` runs the ratatui client, which is a thin
//!   WebSocket consumer of the same brain.

pub mod debug;
pub mod notes;
pub mod protocol;
pub mod pty;
pub mod rest;
pub mod server;
pub mod usage;

pub use protocol::{ClientMsg, ServerMsg};
pub use server::DaemonState;

/// The loopback port the daemon listens on.
pub const DAEMON_PORT: u16 = 7860;

/// The full base URL of the daemon.
pub fn base_url() -> String {
    format!("http://127.0.0.1:{DAEMON_PORT}")
}

/// The WebSocket endpoint URL.
pub fn ws_url() -> String {
    format!("ws://127.0.0.1:{DAEMON_PORT}/ws")
}

/// Is a daemon already listening on the loopback port?
pub async fn daemon_alive() -> bool {
    tokio::net::TcpStream::connect(("127.0.0.1", DAEMON_PORT)).await.is_ok()
}

/// Wait (bounded) for the daemon to accept connections.
pub async fn wait_for_daemon(timeout: std::time::Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if daemon_alive().await {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    false
}

/// Open the DIAD AGENT GUI in the system browser (Brave app-mode preferred,
/// xdg-open fallback).
pub fn open_gui() {
    let url = base_url();
    let brave = std::process::Command::new("brave")
        .arg("--app")
        .arg(&url)
        .spawn();

    if brave.is_err() {
        // Fallback: default browser.
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
    }
    tracing::info!("GUI opened at {url}");
}

/// Launch the TUI client in a foot terminal (used by Ctrl+Shift+X in GUI).
pub fn spawn_tui() {
    let _ = std::process::Command::new("foot")
        .args(["--app-id", "foot", "--title", "DIAD AGENT (TUI)", "-e", "diad-agent", "--tui"])
        .spawn();
}
