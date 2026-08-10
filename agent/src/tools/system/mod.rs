//! System tooling — OS inspection, packages, services, processes,
//! power, updates, security, and the agent's own help surface.

pub mod exec;
pub mod help;
pub mod os;
pub mod power;
pub mod prefs;
pub mod process;
pub mod security;
pub mod update;

pub use crate::tools::{valid_identifier, ToolResult};
