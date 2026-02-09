//! Execution context for Entangled operations.

use std::path::PathBuf;
use std::sync::Arc;

use crate::config::Config;
use crate::hooks::{HookRegistry, ShebangHook, SpdxLicenseHook};
use crate::io::{FileCache, FileDB, RealFileCache};

/// Context for Entangled operations.
///
/// Contains configuration, hooks, and file system access.
#[derive(Debug)]
pub struct Context {
    /// Configuration.
    pub config: Config,
    /// Hook registry.
    pub hooks: HookRegistry,
    /// File cache for reading files.
    pub file_cache: Arc<dyn FileCache>,
    /// File database for tracking tangled files.
    pub filedb: FileDB,
    /// Base directory for operations.
    pub base_dir: PathBuf,
    /// Path to the file database.
    pub filedb_path: PathBuf,
}

impl Context {
    /// Creates a new context with the given configuration.
    pub fn new(config: Config, base_dir: PathBuf) -> std::io::Result<Self> {
        let filedb_path = base_dir.join(&config.filedb_path);
        let filedb = match FileDB::load(&filedb_path) {
            Ok(db) => db,
            Err(e) => {
                if filedb_path.exists() {
                    // File exists but failed to parse -- warn about data loss
                    tracing::warn!(
                        "Failed to load file database at {}: {}. Starting with empty database.",
                        filedb_path.display(),
                        e
                    );
                }
                FileDB::default()
            }
        };
        let file_cache = Arc::new(RealFileCache::new(base_dir.clone()));

        let mut hooks = HookRegistry::new();
        if config.hooks.shebang {
            hooks.add(ShebangHook::new());
        }
        if config.hooks.spdx_license {
            hooks.add(SpdxLicenseHook::new());
        }

        Ok(Self {
            config,
            hooks,
            file_cache,
            filedb,
            base_dir,
            filedb_path,
        })
    }

    /// Creates a context with default configuration.
    pub fn default_for_dir(base_dir: PathBuf) -> std::io::Result<Self> {
        Self::new(Config::default(), base_dir)
    }

    /// Creates a context from the current directory.
    pub fn from_current_dir() -> std::io::Result<Self> {
        let base_dir = std::env::current_dir()?;
        let config = crate::config::read_config(&base_dir).unwrap_or_default();
        Self::new(config, base_dir)
    }

    /// Adds a hook to the registry.
    pub fn add_hook<H: crate::hooks::Hook + 'static>(&mut self, hook: H) {
        self.hooks.add(hook);
    }

    /// Saves the file database.
    pub fn save_filedb(&self) -> crate::errors::Result<()> {
        self.filedb.save(&self.filedb_path)
    }

    /// Returns source file paths matching the configured patterns.
    pub fn source_files(&self) -> crate::errors::Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        for pattern in &self.config.source_patterns {
            files.extend(self.file_cache.glob(pattern)?);
        }
        // Remove duplicates and sort
        files.sort();
        files.dedup();
        Ok(files)
    }

    /// Returns source files filtered to only include the specified paths.
    ///
    /// Each filter path is resolved relative to `base_dir` and compared
    /// against the full set of source files. Returns an error if any
    /// filter path does not match a known source file.
    pub fn source_files_filtered(&self, filter: &[PathBuf]) -> crate::errors::Result<Vec<PathBuf>> {
        let all_files = self.source_files()?;
        let resolved_filters: Vec<PathBuf> = filter
            .iter()
            .map(|f| {
                if f.is_absolute() {
                    f.clone()
                } else {
                    self.base_dir.join(f)
                }
            })
            .collect();

        let mut result = Vec::new();
        for filter_path in &resolved_filters {
            if let Some(found) = all_files.iter().find(|f| *f == filter_path) {
                result.push(found.clone());
            } else {
                return Err(crate::errors::EntangledError::Config(format!(
                    "File {} is not a source file (does not match source_patterns)",
                    filter_path.display()
                )));
            }
        }

        result.sort();
        result.dedup();
        Ok(result)
    }

    /// Returns source files matching any of the given glob patterns.
    ///
    /// Only files that are both matched by a glob AND present in
    /// `source_files()` are returned. Returns an error if a pattern
    /// matches no source files.
    pub fn source_files_glob(&self, patterns: &[String]) -> crate::errors::Result<Vec<PathBuf>> {
        let all_files = self.source_files()?;
        let mut matched = Vec::new();
        for pattern in patterns {
            let expanded = self.file_cache.glob(pattern)?;
            let before = matched.len();
            matched.extend(expanded.into_iter().filter(|p| all_files.contains(p)));
            if matched.len() == before {
                return Err(crate::errors::EntangledError::Config(format!(
                    "Glob pattern '{}' matched no source files",
                    pattern
                )));
            }
        }
        matched.sort();
        matched.dedup();
        Ok(matched)
    }

    /// Resolves a path relative to the base directory.
    pub fn resolve_path(&self, path: &std::path::Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.base_dir.join(path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_context_new() {
        let dir = tempdir().unwrap();
        let ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();

        assert_eq!(ctx.base_dir, dir.path());
        assert!(ctx.filedb.is_empty());
    }

    #[test]
    fn test_resolve_path() {
        let dir = tempdir().unwrap();
        let ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();

        let relative = std::path::Path::new("src/main.rs");
        let resolved = ctx.resolve_path(relative);
        assert_eq!(resolved, dir.path().join("src/main.rs"));

        // Use the tempdir itself as a known-absolute path for cross-platform correctness
        let absolute = dir.path().join("absolute/path");
        let resolved = ctx.resolve_path(&absolute);
        assert_eq!(resolved, absolute);
    }

    #[test]
    fn test_source_files() {
        let dir = tempdir().unwrap();

        // Create some test files
        std::fs::write(dir.path().join("test.md"), "# Test").unwrap();
        std::fs::write(dir.path().join("other.txt"), "text").unwrap();

        let ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();
        let files = ctx.source_files().unwrap();

        assert!(files
            .iter()
            .any(|p| p.to_string_lossy().contains("test.md")));
        assert!(!files
            .iter()
            .any(|p| p.to_string_lossy().contains("other.txt")));
    }
}
