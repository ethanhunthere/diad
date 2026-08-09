//! DIAD AGENT — Core library
//!
//! The DIAD AGENT is the intent-driven interface of the DIAD Operating
//! System. The Rust core is a daemon ("one brain") that owns all logic —
//! LLM providers, system tools, the agentic orchestrator, the secure vault,
//! sessions, the offline intent router and the PTY shell — and serves two
//! interchangeable frontends ("two faces"):
//!
//! - the **DIAD AGENT GUI** (React, served over HTTP + WebSocket), and
//! - the **DIAD AGENT TUI** (ratatui, a thin WebSocket client).

pub mod auth;
pub mod config;
pub mod daemon;
pub mod orchestrator;
pub mod providers;
pub mod tools;
pub mod tui;
