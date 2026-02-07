//! Configuration data structures.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::annotation_method::AnnotationMethod;
use super::language::Language;
use super::markers::Markers;
use super::namespace_default::NamespaceDefault;
use crate::style::Style;

/// Main configuration structure for Entangled.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Version of the configuration format.
    #[serde(default = "default_version")]
    pub version: String,

    /// Glob patterns for markdown source files.
    #[serde(default = "default_source_patterns")]
    pub source_patterns: Vec<String>,

    /// Directory for generated/tangled files.
    #[serde(default)]
    pub output_dir: Option<PathBuf>,

    /// How to annotate tangled output.
    #[serde(default)]
    pub annotation: AnnotationMethod,

    /// Default namespace handling.
    #[serde(default)]
    pub namespace_default: NamespaceDefault,

    /// Marker patterns for annotations.
    #[serde(default)]
    pub markers: Markers,

    /// Language configurations (overrides built-ins).
    #[serde(default)]
    pub languages: Vec<Language>,

    /// Watch configuration.
    #[serde(default)]
    pub watch: WatchConfig,

    /// Hook configurations.
    #[serde(default)]
    pub hooks: HooksConfig,

    /// File database path.
    #[serde(default = "default_filedb_path")]
    pub filedb_path: PathBuf,

    /// Code block syntax style.
    #[serde(default)]
    pub style: Style,

    /// Whether to strip #| comment lines from tangled output (Quarto style).
    #[serde(default = "default_strip_quarto_options")]
    pub strip_quarto_options: bool,

    /// Additional custom settings.
    #[serde(default, flatten)]
    pub extra: HashMap<String, toml::Value>,
}

fn default_version() -> String {
    "2.0".to_string()
}

fn default_source_patterns() -> Vec<String> {
    vec![
        "**/*.md".to_string(),
        "**/*.qmd".to_string(),
        "**/*.Rmd".to_string(),
    ]
}

fn default_filedb_path() -> PathBuf {
    PathBuf::from(".entangled/filedb.json")
}

fn default_strip_quarto_options() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: default_version(),
            source_patterns: default_source_patterns(),
            output_dir: None,
            annotation: AnnotationMethod::default(),
            namespace_default: NamespaceDefault::default(),
            markers: Markers::default(),
            languages: Vec::new(),
            watch: WatchConfig::default(),
            hooks: HooksConfig::default(),
            filedb_path: default_filedb_path(),
            style: Style::default(),
            strip_quarto_options: default_strip_quarto_options(),
            extra: HashMap::new(),
        }
    }
}

impl Config {
    /// Creates a new default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Looks up a language by identifier, checking custom languages first.
    pub fn find_language(&self, identifier: &str) -> Option<Language> {
        // Check custom languages first
        if let Some(lang) = self.languages.iter().find(|l| l.matches(identifier)) {
            return Some(lang.clone());
        }
        // Fall back to built-in languages
        super::templates::find_language(identifier)
    }

    /// Returns all source patterns.
    pub fn source_patterns(&self) -> &[String] {
        &self.source_patterns
    }

    /// Returns the output directory, if configured.
    pub fn output_dir(&self) -> Option<&Path> {
        self.output_dir.as_deref()
    }
}

/// Watch mode configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WatchConfig {
    /// Debounce delay in milliseconds.
    #[serde(default = "default_debounce")]
    pub debounce_ms: u64,

    /// Additional directories to watch.
    #[serde(default)]
    pub include: Vec<String>,

    /// Patterns to exclude from watching.
    #[serde(default)]
    pub exclude: Vec<String>,
}

fn default_debounce() -> u64 {
    100
}

/// Hook configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HooksConfig {
    /// Enable shebang extraction.
    #[serde(default)]
    pub shebang: bool,

    /// Enable SPDX license header extraction.
    #[serde(default)]
    pub spdx_license: bool,

    /// Absorb unknown hook keys (forward-compat with Python Entangled configs).
    #[serde(default, flatten)]
    pub extra: HashMap<String, toml::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.version, "2.0");
        assert_eq!(config.source_patterns, vec!["**/*.md", "**/*.qmd", "**/*.Rmd"]);
        assert_eq!(config.annotation, AnnotationMethod::Standard);
    }

    #[test]
    fn test_find_language_builtin() {
        let config = Config::default();
        let lang = config.find_language("python").unwrap();
        assert_eq!(lang.name, "python");
    }

    #[test]
    fn test_find_language_custom() {
        let mut config = Config::default();
        config.languages.push(Language::new(
            "mylang",
            super::super::language::Comment::line("##"),
        ));

        let lang = config.find_language("mylang").unwrap();
        assert_eq!(lang.name, "mylang");
    }

    #[test]
    fn test_serde_roundtrip() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.version, config.version);
    }
}
