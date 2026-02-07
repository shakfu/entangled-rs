//! File system abstraction for testability.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use super::stat::{FileData, Stat};

/// Trait for file system operations, allowing both real and virtual implementations.
pub trait FileCache: Send + Sync + std::fmt::Debug {
    /// Reads the contents of a file.
    fn read(&self, path: &Path) -> io::Result<String>;

    /// Checks if a file exists.
    fn exists(&self, path: &Path) -> bool;

    /// Gets file statistics.
    fn stat(&self, path: &Path) -> io::Result<Stat>;

    /// Gets file data including hash.
    fn file_data(&self, path: &Path) -> io::Result<FileData>;

    /// Lists files matching a glob pattern.
    fn glob(&self, pattern: &str) -> io::Result<Vec<PathBuf>>;
}

/// Real file system implementation.
#[derive(Debug, Clone, Default)]
pub struct RealFileCache {
    /// Base directory for relative paths.
    pub base_dir: PathBuf,
}

impl RealFileCache {
    /// Creates a new RealFileCache with the given base directory.
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Creates a RealFileCache using the current directory.
    pub fn current_dir() -> io::Result<Self> {
        Ok(Self::new(std::env::current_dir()?))
    }

    /// Resolves a path relative to the base directory.
    pub fn resolve(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.base_dir.join(path)
        }
    }
}

impl FileCache for RealFileCache {
    fn read(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(self.resolve(path))
    }

    fn exists(&self, path: &Path) -> bool {
        self.resolve(path).exists()
    }

    fn stat(&self, path: &Path) -> io::Result<Stat> {
        Stat::from_path(&self.resolve(path))
    }

    fn file_data(&self, path: &Path) -> io::Result<FileData> {
        FileData::from_path(&self.resolve(path))
    }

    fn glob(&self, pattern: &str) -> io::Result<Vec<PathBuf>> {
        let full_pattern = self.base_dir.join(pattern);
        let pattern_str = full_pattern.to_string_lossy();

        let paths = glob::glob(&pattern_str)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?
            .filter_map(|r| r.ok())
            .map(|p| {
                p.strip_prefix(&self.base_dir)
                    .map(|p| p.to_path_buf())
                    .unwrap_or(p)
            })
            .collect();

        Ok(paths)
    }
}

/// Virtual file system for testing.
#[derive(Debug, Clone, Default)]
pub struct VirtualFS {
    /// Files stored in memory.
    files: HashMap<PathBuf, VirtualFile>,
}

/// A file in the virtual file system.
#[derive(Debug, Clone)]
struct VirtualFile {
    content: String,
    mtime: DateTime<Utc>,
}

impl VirtualFS {
    /// Creates a new empty virtual file system.
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    /// Adds a file to the virtual file system.
    pub fn add_file(&mut self, path: impl Into<PathBuf>, content: impl Into<String>) {
        let path = path.into();
        self.files.insert(
            path,
            VirtualFile {
                content: content.into(),
                mtime: Utc::now(),
            },
        );
    }

    /// Adds a file with a specific modification time.
    pub fn add_file_with_mtime(
        &mut self,
        path: impl Into<PathBuf>,
        content: impl Into<String>,
        mtime: DateTime<Utc>,
    ) {
        let path = path.into();
        self.files.insert(
            path,
            VirtualFile {
                content: content.into(),
                mtime,
            },
        );
    }

    /// Removes a file from the virtual file system.
    pub fn remove_file(&mut self, path: &Path) {
        self.files.remove(path);
    }

    /// Lists all files in the virtual file system.
    pub fn list_files(&self) -> Vec<&PathBuf> {
        self.files.keys().collect()
    }
}

impl FileCache for VirtualFS {
    fn read(&self, path: &Path) -> io::Result<String> {
        self.files
            .get(path)
            .map(|f| f.content.clone())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File not found"))
    }

    fn exists(&self, path: &Path) -> bool {
        self.files.contains_key(path)
    }

    fn stat(&self, path: &Path) -> io::Result<Stat> {
        self.files
            .get(path)
            .map(|f| Stat::new(f.mtime, f.content.len() as u64))
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File not found"))
    }

    fn file_data(&self, path: &Path) -> io::Result<FileData> {
        self.files
            .get(path)
            .map(|f| FileData::from_content(&f.content, f.mtime))
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File not found"))
    }

    fn glob(&self, pattern: &str) -> io::Result<Vec<PathBuf>> {
        let glob_pattern = glob::Pattern::new(pattern)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        let matches = self
            .files
            .keys()
            .filter(|p| glob_pattern.matches_path(p))
            .cloned()
            .collect();

        Ok(matches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_virtual_fs_basic() {
        let mut vfs = VirtualFS::new();
        vfs.add_file("test.txt", "hello world");

        assert!(vfs.exists(Path::new("test.txt")));
        assert!(!vfs.exists(Path::new("other.txt")));

        let content = vfs.read(Path::new("test.txt")).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_virtual_fs_stat() {
        let mut vfs = VirtualFS::new();
        vfs.add_file("test.txt", "hello");

        let stat = vfs.stat(Path::new("test.txt")).unwrap();
        assert_eq!(stat.size, 5);
    }

    #[test]
    fn test_virtual_fs_file_data() {
        let mut vfs = VirtualFS::new();
        vfs.add_file("test.txt", "test");

        let data = vfs.file_data(Path::new("test.txt")).unwrap();
        assert_eq!(data.stat.size, 4);
        assert!(!data.hexdigest.is_empty());
    }

    #[test]
    fn test_virtual_fs_glob() {
        let mut vfs = VirtualFS::new();
        vfs.add_file("src/main.rs", "fn main() {}");
        vfs.add_file("src/lib.rs", "// lib");
        vfs.add_file("README.md", "# Readme");

        let rust_files = vfs.glob("src/*.rs").unwrap();
        assert_eq!(rust_files.len(), 2);

        let md_files = vfs.glob("*.md").unwrap();
        assert_eq!(md_files.len(), 1);
    }

    #[test]
    fn test_real_file_cache() {
        let dir = tempdir().unwrap();
        let cache = RealFileCache::new(dir.path().to_path_buf());

        let path = dir.path().join("test.txt");
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(b"content").unwrap();

        assert!(cache.exists(Path::new("test.txt")));
        let content = cache.read(Path::new("test.txt")).unwrap();
        assert_eq!(content, "content");
    }

    #[test]
    fn test_real_file_cache_glob() {
        let dir = tempdir().unwrap();
        let cache = RealFileCache::new(dir.path().to_path_buf());

        fs::write(dir.path().join("a.txt"), "a").unwrap();
        fs::write(dir.path().join("b.txt"), "b").unwrap();
        fs::write(dir.path().join("c.md"), "c").unwrap();

        let txt_files = cache.glob("*.txt").unwrap();
        assert_eq!(txt_files.len(), 2);
    }
}
