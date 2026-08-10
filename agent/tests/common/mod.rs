//! Shared helpers for integration tests.
//!
//! Integration test binaries are compiled per-file, so this module is
//! included with `mod common;` inside each test crate that needs it.

/// A default tool registry with every built-in tool registered.
///
/// The default sandbox allowlist is `~`, `/tmp` and `/etc/diad` (see
/// `ToolRegistry::default`), which is what the sandbox tests rely on.
#[allow(dead_code)] // Not every test file uses every helper.
pub fn default_registry() -> diad_agent::tools::ToolRegistry {
    diad_agent::tools::create_default_registry()
}
