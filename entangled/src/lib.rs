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
//! use entangled::interface::tangle_documents;
//!
//! let ctx = Context::from_current_dir().unwrap();
//! let transaction = tangle_documents(&ctx).unwrap();
//! // transaction.execute(&mut ctx.filedb).unwrap();
//! // ctx.save_filedb().unwrap();
//! ```

pub mod config;
pub mod errors;
pub mod eval;
pub mod hooks;
pub mod interface;
pub mod io;
pub mod model;
pub mod readers;
pub mod style;
pub mod text_location;
pub mod weave;

#[cfg(test)]
pub(crate) mod test_utils;

// Re-export commonly used types
pub use config::Config;
pub use errors::{EntangledError, Result};
pub use eval::{eval_documents, EvalCache, EvalOptions, EvalResult};
pub use interface::Context;
pub use model::{CodeBlock, ReferenceId, ReferenceMap, ReferenceName};
pub use style::Style;
pub use weave::{
    weave_document, weave_document_with_outputs, weave_to_html, weave_to_markdown, BlockOutput,
    HtmlOptions, WovenDocument,
};
