//! Network tooling — HTTP, ping, and delegation to external AI CLIs.

pub mod connectivity;
pub mod external;

pub use crate::tools::{valid_identifier, ToolResult};
