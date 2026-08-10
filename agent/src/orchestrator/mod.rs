//! Intent orchestration — the "brain" between chat input and tools.
//!
//! - `router` — classifies free-text intents (`classify_input`) and routes
//!   them to the right provider or the offline router.
//! - `session` — per-conversation message history persistence.
//! - `agent` — the agent loop that turns tool calls into tool results.

pub mod agent;
pub mod router;
pub mod session;
