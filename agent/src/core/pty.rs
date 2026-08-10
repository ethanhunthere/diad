//! PTY terminal management for the daemon.
//!
//! The daemon spawns a real shell (portable-pty) per GUI connection. Frontends
//! stream input through `term.input` and receive output through `term.output`.
//! The PTY is a single shared instance per daemon (the TUI has its own
//! terminal, so only the GUI consumes this).

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use portable_pty::{native_pty_system, Child, MasterPty, PtySize};
use tokio::sync::mpsc::Sender;

use super::protocol::ServerMsg;

/// One running PTY shell session.
pub struct PtySession {
    _child: Box<dyn Child + Send + Sync>,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    /// Reader task handle kept alive for the lifetime of the session.
    _reader: tokio::task::JoinHandle<()>,
}

impl PtySession {
    /// Spawn the user's login shell (fallback: bash).
    pub fn spawn(tx: Sender<ServerMsg>) -> anyhow::Result<Self> {
        let pty_system = native_pty_system();
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into());

        let pair = pty_system.openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let cmd = portable_pty::CommandBuilder::new(&shell);
        let child = pair.slave.spawn_command(cmd)?;

        // Drop the slave so EOF propagates when the shell exits.
        drop(pair.slave);

        let master = pair.master;
        let mut reader = master.try_clone_reader()?;
        let writer = master.take_writer()?;

        // Reader task: stream output to the frontend. A multibyte UTF-8
        // character can straddle two reads; the trailing incomplete bytes
        // are carried over instead of being mangled into U+FFFD (spec 3.1).
        let reader_tx = tx.clone();
        let reader = tokio::task::spawn_blocking(move || {
            let mut buf = [0u8; 4096];
            let mut pending: Vec<u8> = Vec::new();
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        pending.extend_from_slice(&buf[..n]);
                        // Split at the last point that decodes cleanly.
                        let mut valid_up_to = pending.len();
                        while valid_up_to > 0
                            && std::str::from_utf8(&pending[..valid_up_to]).is_err()
                        {
                            valid_up_to -= 1;
                        }
                        if valid_up_to == 0 {
                            continue; // whole buffer is a partial character
                        }
                        let data = String::from_utf8_lossy(&pending[..valid_up_to]).into_owned();
                        pending.drain(..valid_up_to);
                        if reader_tx.try_send(ServerMsg::TermOutput { data }).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            _child: child,
            master,
            writer,
            _reader: reader,
        })
    }

    /// Write bytes to the PTY (terminal input).
    pub fn write(&mut self, data: &str) -> anyhow::Result<()> {
        self.writer.write_all(data.as_bytes())?;
        self.writer.flush()?;
        Ok(())
    }

    /// Resize the PTY.
    pub fn resize(&mut self, cols: u16, rows: u16) -> anyhow::Result<()> {
        self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }
}

/// PTY registry — the daemon owns at most one live shell per GUI.
pub struct PtyManager {
    session: Arc<Mutex<Option<PtySession>>>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            session: Arc::new(Mutex::new(None)),
        }
    }

    /// Ensure a PTY exists (spawning the shell on first use), then write.
    pub fn input(&self, data: &str, tx: Sender<ServerMsg>) -> anyhow::Result<()> {
        let mut guard = self.session.lock().unwrap();
        if guard.is_none() {
            *guard = Some(PtySession::spawn(tx)?);
        }
        if let Some(session) = guard.as_mut() {
            session.write(data)?;
        }
        Ok(())
    }

    /// Resize the PTY if it exists.
    pub fn resize(&self, cols: u16, rows: u16) {
        if let Ok(mut guard) = self.session.lock() {
            if let Some(session) = guard.as_mut() {
                let _ = session.resize(cols, rows);
            }
        }
    }

    /// Kill the shell if running.
    pub fn shutdown(&self) {
        if let Ok(mut guard) = self.session.lock() {
            *guard = None;
        }
    }
}

impl Default for PtyManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Event-driven wait (F-20): poll the output channel until a TermOutput
    /// containing `needle` arrives or `deadline` passes. A single slow recv
    /// window (shell startup under load) must NOT abort the wait — only
    /// channel close (session gone) ends it early. This replaces the old
    /// pattern where one 500 ms timeout aborted the whole test even though
    /// the echo arrived within the deadline.
    async fn wait_for_term_output(
        rx: &mut tokio::sync::mpsc::Receiver<ServerMsg>,
        needle: &str,
        deadline: tokio::time::Instant,
    ) -> bool {
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(ServerMsg::TermOutput { data })) if data.contains(needle) => return true,
                Ok(Some(_)) => continue,
                Ok(None) => return false, // channel closed — session torn down
                Err(_) => continue,       // slow window; keep waiting (F-20)
            }
        }
        false
    }

    #[test]
    fn test_pty_manager_new_is_empty() {
        let mgr = PtyManager::new();
        let guard = mgr.session.lock().unwrap();
        assert!(guard.is_none());
    }

    #[tokio::test]
    async fn test_pty_spawns_shell_and_writes() {
        // Spawn a real PTY with a shell and verify output arrives.
        let (tx, mut rx) = tokio::sync::mpsc::channel::<ServerMsg>(4096);

        let mut session = match PtySession::spawn(tx) {
            Ok(s) => s,
            Err(e) => {
                // Environments without a tty (CI) may fail; treat as skip.
                eprintln!("pty spawn skipped: {e}");
                return;
            }
        };

        // Write a trivial command and expect echoed output.
        session.write("echo diad-pty-ok\r\n").ok();

        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);
        let got = wait_for_term_output(&mut rx, "diad-pty-ok", deadline).await;
        assert!(got, "expected PTY output echoing 'diad-pty-ok'");
    }

    /// PtyManager lifecycle: resize before spawn is a safe no-op, `input`
    /// lazily spawns the shell, and `shutdown` tears it down so the next
    /// `input` spawns a fresh shell.
    #[tokio::test]
    async fn test_pty_manager_lazy_spawn_and_shutdown() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<ServerMsg>(4096);
        let mgr = PtyManager::new();

        // Resize with no session must not panic.
        mgr.resize(100, 30);

        // First input spawns the shell lazily.
        if let Err(e) = mgr.input("echo diad-lazy\r\n", tx.clone()) {
            eprintln!("pty input spawn skipped: {e}");
            return;
        }

        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);
        let spawned = wait_for_term_output(&mut rx, "diad-lazy", deadline).await;
        assert!(spawned, "lazy spawn should echo 'diad-lazy'");
        {
            let guard = mgr.session.lock().unwrap();
            assert!(guard.is_some(), "input must have spawned a session");
        }

        // Shutdown tears the shell down.
        mgr.shutdown();
        {
            let guard = mgr.session.lock().unwrap();
            assert!(guard.is_none(), "shutdown must clear the session");
        }

        // Resize after shutdown stays a safe no-op.
        mgr.resize(120, 40);
    }
}
