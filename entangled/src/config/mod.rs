//! Configuration loading and management.

mod annotation_method;
mod config_data;
mod config_update;
mod language;
mod markers;
mod namespace_default;
mod templates;

use std::fs;
use std::path::{Path, PathBuf};

pub use annotation_method::AnnotationMethod;
pub use config_data::{Config, HooksConfig, WatchConfig};
pub use config_update::ConfigUpdate;
pub use language::{Comment, Language};
pub use markers::{
    annotation_begin, annotation_end, Markers, ANNOTATION_PREFIX, REF_PATTERN,
};
pub use namespace_default::NamespaceDefault;
pub use templates::{builtin_languages, find_language};
pub use crate::style::Style;

use crate::errors::Result;

/// Standard configuration file names to search for.
const CONFIG_FILES: &[&str] = &["entangled.toml", ".entangled.toml"];

/// Finds the configuration file in the given directory or its parents.
pub fn find_config_file(start_dir: &Path) -> Option<PathBuf> {
    let mut current = start_dir.to_path_buf();

    loop {
        for name in CONFIG_FILES {
            let candidate = current.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
        }

        if !current.pop() {
            break;
        }
    }

    None
}

/// Reads configuration from a TOML file.
pub fn read_config_file(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path)?;
    let update: ConfigUpdate = toml::from_str(&content)?;
    Ok(update.merge_into(&Config::default()))
}

/// Reads configuration, searching from the given directory.
///
/// If no config file is found, returns the default configuration.
pub fn read_config(start_dir: &Path) -> Result<Config> {
    match find_config_file(start_dir) {
        Some(path) => read_config_file(&path),
        None => Ok(Config::default()),
    }
}

/// Reads configuration from a specific file, or returns default if file doesn't exist.
pub fn read_config_or_default(path: &Path) -> Result<Config> {
    if path.exists() {
        read_config_file(path)
    } else {
        Ok(Config::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_find_config_file() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("entangled.toml");
        fs::write(&config_path, "version = \"2.0\"").unwrap();

        let found = find_config_file(dir.path()).unwrap();
        assert_eq!(found, config_path);
    }

    #[test]
    fn test_find_config_file_parent() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("entangled.toml");
        fs::write(&config_path, "version = \"2.0\"").unwrap();

        let subdir = dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();

        let found = find_config_file(&subdir).unwrap();
        assert_eq!(found, config_path);
    }

    #[test]
    fn test_find_config_file_not_found() {
        let dir = tempdir().unwrap();
        assert!(find_config_file(dir.path()).is_none());
    }

    #[test]
    fn test_read_config_file() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("entangled.toml");

        let mut file = fs::File::create(&config_path).unwrap();
        writeln!(
            file,
            r#"
version = "2.0"
source_patterns = ["docs/**/*.md", "README.md"]
annotation = "naked"
"#
        )
        .unwrap();

        let config = read_config_file(&config_path).unwrap();
        assert_eq!(config.version, "2.0");
        assert_eq!(config.source_patterns, vec!["docs/**/*.md", "README.md"]);
        assert_eq!(config.annotation, AnnotationMethod::Naked);
    }

    #[test]
    fn test_read_config_default() {
        let dir = tempdir().unwrap();
        let config = read_config(dir.path()).unwrap();
        assert_eq!(config.version, "2.0");
        assert_eq!(config.source_patterns, vec!["**/*.md", "**/*.qmd", "**/*.Rmd"]);
    }

    #[test]
    fn test_read_config_with_languages() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("entangled.toml");

        let toml_content = "
[[languages]]
name = \"mylang\"
comment = \"#\"
identifiers = [\"ml\", \"myl\"]
";
        fs::write(&config_path, toml_content).unwrap();

        let config = read_config_file(&config_path).unwrap();
        assert_eq!(config.languages.len(), 1);
        assert_eq!(config.languages[0].name, "mylang");

        let lang = config.find_language("myl").unwrap();
        assert_eq!(lang.name, "mylang");
    }
}
