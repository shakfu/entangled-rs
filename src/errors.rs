//! Error types for the Entangled system.

use std::path::PathBuf;
use thiserror::Error;

use crate::model::ReferenceName;
use crate::text_location::TextLocation;

/// Main error type for Entangled operations.
#[derive(Error, Debug)]
pub enum EntangledError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("YAML parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),

    #[error("Parse error at {location}: {message}")]
    Parse { location: TextLocation, message: String },

    #[error("Reference not found: {0}")]
    ReferenceNotFound(ReferenceName),

    #[error("Cycle detected in references: {0:?}")]
    CycleDetected(Vec<ReferenceName>),

    #[error("Duplicate reference: {0}")]
    DuplicateReference(ReferenceName),

    #[error("Unknown language: {0}")]
    UnknownLanguage(String),

    #[error("File conflict: {path} has been modified externally")]
    FileConflict { path: PathBuf },

    #[error("Transaction error: {0}")]
    Transaction(String),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("Invalid property: {0}")]
    InvalidProperty(String),

    #[error("Missing required property: {0}")]
    MissingProperty(String),

    #[error("Glob pattern error: {0}")]
    GlobPattern(#[from] glob::PatternError),

    #[error("Watch error: {0}")]
    Watch(String),

    #[error("{0}")]
    Other(String),
}

/// Result type alias for Entangled operations.
pub type Result<T> = std::result::Result<T, EntangledError>;
