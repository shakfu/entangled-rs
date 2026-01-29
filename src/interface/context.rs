//! Execution context for Entangled operations.

use std::path::PathBuf;
use std::sync::Arc;

use crate::config::Config;
use crate::hooks::HookRegistry;
use crate::io::{FileCache, FileDB, RealFileCache};

/// Context for Entangled operations.
///
/// Contains configuration, hooks, and file system access.
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
        let filedb = FileDB::load(&filedb_path).unwrap_or_default();
        let file_cache = Arc::new(RealFileCache::new(base_dir.clone()));

        Ok(Self {
            config,
            hooks: HookRegistry::new(),
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

        let absolute = std::path::Path::new("/absolute/path");
        let resolved = ctx.resolve_path(absolute);
        assert_eq!(resolved, std::path::PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_source_files() {
        let dir = tempdir().unwrap();

        // Create some test files
        std::fs::write(dir.path().join("test.md"), "# Test").unwrap();
        std::fs::write(dir.path().join("other.txt"), "text").unwrap();

        let ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();
        let files = ctx.source_files().unwrap();

        assert!(files.iter().any(|p| p.to_string_lossy().contains("test.md")));
        assert!(!files.iter().any(|p| p.to_string_lossy().contains("other.txt")));
    }
}
