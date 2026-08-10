//! Authentication and secrets management.
//!
//! - `vault` — OS keyring-backed (libsecret) storage of per-provider API keys;
//!   keys are never written to disk in plaintext and never logged.
//! - `oauth` — OAuth 2.1 flow support for providers that require it.

pub mod oauth;
pub mod vault;
