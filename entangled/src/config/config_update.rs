//! Configuration update and merging.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::annotation_method::AnnotationMethod;
use super::config_data::{Config, HooksConfig, WatchConfig};
use super::language::Language;
use super::markers::Markers;
use super::namespace_default::NamespaceDefault;
use crate::style::Style;

/// Partial configuration update that can be merged into a Config.
///
/// All fields are optional. Only specified fields will override the base config.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigUpdate {
    /// Version of the configuration format.
    #[serde(default)]
    pub version: Option<String>,

    /// Glob patterns for markdown source files.
    #[serde(default)]
    pub source_patterns: Option<Vec<String>>,

    /// Directory for generated/tangled files.
    #[serde(default)]
    pub output_dir: Option<PathBuf>,

    /// How to annotate tangled output.
    #[serde(default)]
    pub annotation: Option<AnnotationMethod>,

    /// Default namespace handling.
    #[serde(default)]
    pub namespace_default: Option<NamespaceDefault>,

    /// Marker patterns for annotations.
    #[serde(default)]
    pub markers: Option<Markers>,

    /// Language configurations.
    #[serde(default)]
    pub languages: Option<Vec<Language>>,

    /// Watch configuration.
    #[serde(default)]
    pub watch: Option<WatchConfig>,

    /// Hook configurations.
    #[serde(default)]
    pub hooks: Option<HooksConfig>,

    /// File database path.
    #[serde(default)]
    pub filedb_path: Option<PathBuf>,

    /// Code block syntax style.
    #[serde(default)]
    pub style: Option<Style>,

    /// Whether to strip #| comment lines from tangled output.
    #[serde(default)]
    pub strip_quarto_options: Option<bool>,
}

impl ConfigUpdate {
    /// Creates an empty update.
    pub fn new() -> Self {
        Self::default()
    }

    /// Merges this update into a base configuration, returning a new Config.
    ///
    /// Consumes `self` so fields can be moved instead of cloned.
    pub fn merge_into(self, base: &Config) -> Config {
        Config {
            version: self.version.unwrap_or_else(|| base.version.clone()),
            source_patterns: self
                .source_patterns
                .unwrap_or_else(|| base.source_patterns.clone()),
            output_dir: self.output_dir.or_else(|| base.output_dir.clone()),
            annotation: self.annotation.unwrap_or(base.annotation),
            namespace_default: self.namespace_default.unwrap_or(base.namespace_default),
            markers: self.markers.unwrap_or_else(|| base.markers.clone()),
            languages: merge_languages(
                &base.languages,
                self.languages.as_ref().unwrap_or(&Vec::new()),
            ),
            watch: self.watch.unwrap_or_else(|| base.watch.clone()),
            hooks: merge_hooks(&base.hooks, self.hooks.as_ref()),
            filedb_path: self
                .filedb_path
                .unwrap_or_else(|| base.filedb_path.clone()),
            style: self.style.unwrap_or(base.style),
            strip_quarto_options: self.strip_quarto_options.unwrap_or(base.strip_quarto_options),
            extra: base.extra.clone(),
        }
    }
}

/// Merge language lists, with update languages overriding base languages of the same name.
fn merge_languages(base: &[Language], update: &[Language]) -> Vec<Language> {
    let mut result = base.to_vec();

    for lang in update {
        // Remove any existing language with the same name
        result.retain(|l| l.name != lang.name);
        result.push(lang.clone());
    }

    result
}

/// Merge hooks configurations.
fn merge_hooks(base: &HooksConfig, update: Option<&HooksConfig>) -> HooksConfig {
    match update {
        Some(u) => HooksConfig {
            shebang: u.shebang || base.shebang,
            spdx_license: u.spdx_license || base.spdx_license,
            extra: {
                let mut merged = base.extra.clone();
                merged.extend(u.extra.clone());
                merged
            },
        },
        None => base.clone(),
    }
}

impl From<ConfigUpdate> for Config {
    fn from(update: ConfigUpdate) -> Self {
        update.merge_into(&Config::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::language::Comment;

    #[test]
    fn test_empty_update() {
        let base = Config::default();
        let update = ConfigUpdate::new();
        let merged = update.merge_into(&base);

        assert_eq!(merged.version, base.version);
        assert_eq!(merged.source_patterns, base.source_patterns);
    }

    #[test]
    fn test_partial_update() {
        let base = Config::default();
        let update = ConfigUpdate {
            annotation: Some(AnnotationMethod::Naked),
            ..Default::default()
        };
        let merged = update.merge_into(&base);

        assert_eq!(merged.annotation, AnnotationMethod::Naked);
        assert_eq!(merged.version, base.version); // Unchanged
    }

    #[test]
    fn test_merge_languages() {
        let base_langs = vec![Language::new("python", Comment::line("#"))];
        let update_langs = vec![
            Language::new("python", Comment::line("##")), // Override
            Language::new("rust", Comment::line("//")),   // New
        ];

        let merged = merge_languages(&base_langs, &update_langs);
        assert_eq!(merged.len(), 2);

        let python = merged.iter().find(|l| l.name == "python").unwrap();
        assert_eq!(python.comment, Comment::line("##"));

        assert!(merged.iter().any(|l| l.name == "rust"));
    }

    #[test]
    fn test_from_update() {
        let update = ConfigUpdate {
            version: Some("3.0".to_string()),
            ..Default::default()
        };
        let config: Config = update.into();

        assert_eq!(config.version, "3.0");
    }
}
