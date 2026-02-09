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
    Parse {
        location: TextLocation,
        message: String,
    },

    #[error("Reference not found: {0}")]
    ReferenceNotFound(ReferenceName),

    #[error("Cycle detected in references: {0:?}")]
    CycleDetected(Vec<ReferenceName>),

    #[error("Duplicate reference: {0}")]
    DuplicateReference(ReferenceName),

    #[error("Unknown language: {0}")]
    UnknownLanguage(String),

    #[error("File conflict: {path} has been modified externally (use --force to overwrite)")]
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

impl EntangledError {
    /// Returns a distinct exit code for this error category.
    ///
    /// - 1: file conflict (user can retry with `--force`)
    /// - 2: configuration or parse error
    /// - 3: I/O error
    /// - 4: reference error (not found, cycle, duplicate)
    /// - 5: other / internal error
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::FileConflict { .. } => 1,
            Self::Config(_)
            | Self::TomlParse(_)
            | Self::JsonParse(_)
            | Self::YamlParse(_)
            | Self::InvalidProperty(_)
            | Self::MissingProperty(_)
            | Self::GlobPattern(_) => 2,
            Self::Io(_) | Self::Watch(_) => 3,
            Self::ReferenceNotFound(_)
            | Self::CycleDetected(_)
            | Self::DuplicateReference(_)
            | Self::UnknownLanguage(_) => 4,
            Self::Parse { .. } | Self::Transaction(_) | Self::Regex(_) | Self::Other(_) => 5,
        }
    }
}

/// Result type alias for Entangled operations.
pub type Result<T> = std::result::Result<T, EntangledError>;
