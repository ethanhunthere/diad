//! Idle engine — the daemon costs ~nothing when nobody is looking (spec §3).
//!
//! Two idle stages, driven by the connected-client count:
//!
//! | Stage | Trigger            | Effect                                        |
//! |-------|--------------------|-----------------------------------------------|
//! | 1     | no clients for 30s | every registered interval task is disabled,   |
//! |       |                    | volatile caches are dropped                   |
//! | 2     | no clients for 5m  | additionally PTY children are shut down       |
//!
//! Wake path: the first new WS registration flips every task back on and
//! restores `Active` — O(number of tasks), well under the 200 ms budget
//! (spec 3.4); caches re-populate lazily on demand.
//!
//! Every periodic activity in the daemon is registered in the
//! [`TaskRegistry`] with an enable flag (spec 3.5). No busy loops, no
//! `sleep(0)`: the registry plus plain tokio timers are the only timing
//! machinery.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

/// Idle stages, ordered. Stored as `u8` in daemon state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum IdleStage {
    Active = 0,
    Stage1 = 1,
    Stage2 = 2,
}

impl IdleStage {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => IdleStage::Active,
            1 => IdleStage::Stage1,
            _ => IdleStage::Stage2,
        }
    }

    /// Label shown in the bench report / status bar.
    pub fn label(self) -> &'static str {
        match self {
            IdleStage::Active => "ACTIVE",
            IdleStage::Stage1 => "IDLE1",
            IdleStage::Stage2 => "IDLE2",
        }
    }
}

/// Thresholds (spec 3.2 / 3.3).
pub const STAGE1_AFTER_SECS: u64 = 30;
pub const STAGE2_AFTER_SECS: u64 = 300;

/// One registered periodic task with a cheap atomic enable flag.
pub struct TaskEntry {
    pub name: String,
    pub interval_secs: u64,
    pub enabled: Arc<AtomicBool>,
}

/// Central registry of every interval task the daemon runs (spec 3.5).
///
/// Idle transitions flip all flags off; wake flips them back on. Tasks must
/// check their flag each tick and skip work while disabled.
#[derive(Default)]
pub struct TaskRegistry {
    tasks: Mutex<HashMap<String, TaskEntry>>,
}

impl TaskRegistry {
    /// Register a task and get its enable flag. Re-registering a name
    /// returns the existing flag (idempotent).
    pub fn register(&self, name: &str, interval_secs: u64) -> Arc<AtomicBool> {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = tasks.get(name) {
            return Arc::clone(&entry.enabled);
        }
        let entry = TaskEntry {
            name: name.to_string(),
            interval_secs,
            enabled: Arc::new(AtomicBool::new(true)),
        };
        let flag = Arc::clone(&entry.enabled);
        tasks.insert(name.to_string(), entry);
        flag
    }

    /// Flip every task's enable flag (idle → false, wake → true).
    pub fn set_all_enabled(&self, enabled: bool) {
        let tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        for entry in tasks.values() {
            entry.enabled.store(enabled, Ordering::SeqCst);
        }
    }

    /// Snapshot for diagnostics: (name, enabled, interval_secs).
    pub fn snapshot(&self) -> Vec<(String, bool, u64)> {
        let tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        let mut out: Vec<_> = tasks
            .values()
            .map(|e| {
                (
                    e.name.clone(),
                    e.enabled.load(Ordering::SeqCst),
                    e.interval_secs,
                )
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    pub fn len(&self) -> usize {
        self.tasks.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Atomic idle-stage cell for daemon state.
#[derive(Default)]
pub struct IdleState {
    stage: AtomicU8,
}

impl IdleState {
    pub fn get(&self) -> IdleStage {
        IdleStage::from_u8(self.stage.load(Ordering::SeqCst))
    }

    /// Returns true when the stage actually changed.
    pub fn set(&self, stage: IdleStage) -> bool {
        self.stage
            .compare_exchange(
                self.stage.load(Ordering::SeqCst),
                stage as u8,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_roundtrip_and_labels() {
        assert_eq!(IdleStage::from_u8(0), IdleStage::Active);
        assert_eq!(IdleStage::from_u8(1), IdleStage::Stage1);
        assert_eq!(IdleStage::from_u8(2), IdleStage::Stage2);
        assert_eq!(IdleStage::from_u8(99), IdleStage::Stage2);
        assert_eq!(IdleStage::Active.label(), "ACTIVE");
        assert_eq!(IdleStage::Stage1.label(), "IDLE1");
        assert_eq!(IdleStage::Stage2.label(), "IDLE2");
    }

    #[test]
    fn stage_ordering_matches_deepening_idle() {
        assert!(IdleStage::Active < IdleStage::Stage1);
        assert!(IdleStage::Stage1 < IdleStage::Stage2);
    }

    #[test]
    fn registry_register_and_toggle() {
        let reg = TaskRegistry::default();
        let a = reg.register("session-autosave", 300);
        let b = reg.register("env-monitor", 15);
        assert!(a.load(Ordering::SeqCst));
        assert!(b.load(Ordering::SeqCst));
        assert_eq!(reg.len(), 2);

        reg.set_all_enabled(false);
        assert!(!a.load(Ordering::SeqCst));
        assert!(!b.load(Ordering::SeqCst));

        reg.set_all_enabled(true);
        assert!(a.load(Ordering::SeqCst));
        assert!(b.load(Ordering::SeqCst));
    }

    #[test]
    fn registry_reregister_is_idempotent() {
        let reg = TaskRegistry::default();
        let a = reg.register("x", 10);
        let b = reg.register("x", 99);
        assert!(Arc::ptr_eq(&a, &b), "same task name must share one flag");
        assert_eq!(reg.len(), 1);
        let snap = reg.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].0, "x");
        assert_eq!(snap[0].2, 10, "first registration wins");
    }

    #[test]
    fn idle_state_transitions() {
        let state = IdleState::default();
        assert_eq!(state.get(), IdleStage::Active);
        assert!(state.set(IdleStage::Stage1));
        assert_eq!(state.get(), IdleStage::Stage1);
        assert!(state.set(IdleStage::Stage2));
        assert_eq!(state.get(), IdleStage::Stage2);
        assert!(state.set(IdleStage::Active));
        assert_eq!(state.get(), IdleStage::Active);
    }
}
