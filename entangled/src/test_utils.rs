//! Shared test utilities.

use std::path::PathBuf;

use crate::model::{CodeBlock, ReferenceId, ReferenceName};
use crate::text_location::TextLocation;

/// Creates a test code block with the given name and source.
pub fn make_block(name: &str, source: &str) -> CodeBlock {
    CodeBlock::new(
        ReferenceId::first(ReferenceName::new(name)),
        Some("python".to_string()),
        source.to_string(),
        TextLocation::default(),
    )
}

/// Creates a test code block with a language override.
pub fn make_block_lang(name: &str, source: &str, language: &str) -> CodeBlock {
    CodeBlock::new(
        ReferenceId::first(ReferenceName::new(name)),
        Some(language.to_string()),
        source.to_string(),
        TextLocation::default(),
    )
}

/// Creates a test code block with an optional target file.
pub fn make_block_with_target(name: &str, source: &str, target: &str) -> CodeBlock {
    make_block(name, source).with_target(PathBuf::from(target))
}
