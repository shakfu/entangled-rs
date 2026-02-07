//! High-level interface for Entangled operations.

mod context;
mod document;

pub use context::Context;
pub use document::{
    locate_source, stitch_documents, stitch_files, sync_documents, tangle_documents, tangle_files,
    Document, SourceLocation,
};
