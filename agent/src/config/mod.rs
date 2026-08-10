//! Configuration loading and validation.
//!
//! `settings` loads the layered TOML configuration (system `/etc/diad/diad.conf`
//! plus user `~/.config/diad/config.toml`) into typed [`DiadConfig`] structs
//! with defaults for every field.

pub mod settings;
