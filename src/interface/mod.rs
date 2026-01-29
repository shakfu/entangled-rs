//! High-level interface for Entangled operations.

mod context;
mod document;

pub use context::Context;
pub use document::{stitch_documents, sync_documents, tangle_documents, Document};
