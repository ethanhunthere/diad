//! Filesystem tooling — sandboxed file operations and the universal
//! file/content surface (type registry, previews, thumbnails).

pub mod files_universal;
pub mod filesystem;
pub mod trash;

pub use crate::tools::{valid_identifier, ToolResult};
