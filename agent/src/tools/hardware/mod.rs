//! Hardware control — WiFi, audio, brightness, display, input,
//! and universal hardware tooling (touchscreen, pen, sensors).

pub mod device;
pub mod display;
pub mod hardware_universal;
pub mod input;

pub use crate::tools::{valid_identifier, ToolResult};
