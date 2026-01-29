//! Entangled - Literate Programming Engine
//!
//! This library provides the core functionality for the Entangled literate programming system.
//! It allows extracting code from markdown files (tangling) and updating markdown from code changes (stitching).
//!
//! # Features
//!
//! - **Tangle**: Extract code blocks from markdown into source files
//! - **Stitch**: Update markdown when tangled files are modified
//! - **Sync**: Bidirectional synchronization between markdown and code
//! - **Watch**: Monitor files for changes and sync automatically
//!
//! # Example
//!
//! ```no_run
//! use entangled::interface::Context;
//! use entangled::commands::{tangle, TangleOptions};
//!
//! let mut ctx = Context::from_current_dir().unwrap();
//! tangle(&mut ctx, TangleOptions::default()).unwrap();
//! ```

pub mod commands;
pub mod config;
pub mod errors;
pub mod hooks;
pub mod interface;
pub mod io;
pub mod model;
pub mod readers;
pub mod text_location;

// Re-export commonly used types
pub use config::Config;
pub use errors::{EntangledError, Result};
pub use interface::Context;
pub use model::{CodeBlock, ReferenceId, ReferenceMap, ReferenceName};

// Re-export command options
pub use commands::{
    ResetOptions, StatusOptions, StitchOptions, SyncOptions, TangleOptions, WatchOptions,
};
