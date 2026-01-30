//! File statistics and hashing.

use std::fs;
use std::io::{self, Read};
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// File statistics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stat {
    /// Modification time.
    pub mtime: DateTime<Utc>,
    /// File size in bytes.
    pub size: u64,
}

impl Stat {
    /// Creates a new Stat.
    pub fn new(mtime: DateTime<Utc>, size: u64) -> Self {
        Self { mtime, size }
    }

    /// Gets the stat for a file.
    pub fn from_path(path: &Path) -> io::Result<Self> {
        let metadata = fs::metadata(path)?;
        let mtime = metadata.modified()?.into();
        let size = metadata.len();
        Ok(Self { mtime, size })
    }
}

/// File data with content hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileData {
    /// File statistics.
    pub stat: Stat,
    /// SHA256 hash of the file content (hex encoded).
    pub hexdigest: String,
}

impl FileData {
    /// Creates a new FileData.
    pub fn new(stat: Stat, hexdigest: String) -> Self {
        Self { stat, hexdigest }
    }

    /// Creates FileData from file path.
    pub fn from_path(path: &Path) -> io::Result<Self> {
        let stat = Stat::from_path(path)?;
        let hexdigest = hexdigest_file(path)?;
        Ok(Self { stat, hexdigest })
    }

    /// Creates FileData from content string.
    pub fn from_content(content: &str, mtime: DateTime<Utc>) -> Self {
        let size = content.len() as u64;
        let hexdigest = hexdigest_str(content);
        Self {
            stat: Stat::new(mtime, size),
            hexdigest,
        }
    }
}

/// Computes SHA256 hash of a string, returning hex-encoded digest.
pub fn hexdigest_str(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

/// Computes SHA256 hash of a file, returning hex-encoded digest.
pub fn hexdigest_file(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_hexdigest_str() {
        let hash = hexdigest_str("hello world");
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_hexdigest_empty() {
        let hash = hexdigest_str("");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_hexdigest_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.txt");

        let mut file = fs::File::create(&path).unwrap();
        file.write_all(b"hello world").unwrap();

        let hash = hexdigest_file(&path).unwrap();
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_stat_from_path() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.txt");

        let mut file = fs::File::create(&path).unwrap();
        file.write_all(b"hello").unwrap();

        let stat = Stat::from_path(&path).unwrap();
        assert_eq!(stat.size, 5);
    }

    #[test]
    fn test_filedata_from_path() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.txt");

        let mut file = fs::File::create(&path).unwrap();
        file.write_all(b"test content").unwrap();

        let data = FileData::from_path(&path).unwrap();
        assert_eq!(data.stat.size, 12);
        assert!(!data.hexdigest.is_empty());
    }

    #[test]
    fn test_filedata_from_content() {
        let now = Utc::now();
        let data = FileData::from_content("test", now);
        assert_eq!(data.stat.size, 4);
        assert_eq!(data.stat.mtime, now);
    }
}
