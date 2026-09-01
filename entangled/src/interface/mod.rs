//! High-level interface for Entangled operations.

mod context;
mod document;

pub use context::Context;
pub use document::{
    analyze_project, combined_reference_map, locate_source, stitch_documents, stitch_files,
    sync_documents, tangle_documents, tangle_files, BlockLocation, Document, ProjectAnalysis,
    SourceLocation,
};
