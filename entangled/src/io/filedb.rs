//! File database for tracking tangled file states.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::stat::FileData;
use crate::errors::Result;

/// Database of file states for conflict detection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileDB {
    /// Map from file path to its recorded state.
    #[serde(default)]
    pub files: HashMap<PathBuf, FileData>,

    /// Version of the database format.
    #[serde(default = "default_version")]
    pub version: String,
}

fn default_version() -> String {
    "1.0".to_string()
}

impl FileDB {
    /// Creates a new empty file database.
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
            version: default_version(),
        }
    }

    /// Loads the file database from a JSON file.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }

        let content = fs::read_to_string(path)?;
        let db: FileDB = serde_json::from_str(&content)?;
        Ok(db)
    }

    /// Saves the file database to a JSON file.
    pub fn save(&self, path: &Path) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Records a file's state.
    pub fn record(&mut self, path: PathBuf, data: FileData) {
        self.files.insert(path, data);
    }

    /// Removes a file from the database.
    pub fn remove(&mut self, path: &Path) {
        self.files.remove(path);
    }

    /// Gets the recorded state for a file.
    pub fn get(&self, path: &Path) -> Option<&FileData> {
        self.files.get(path)
    }

    /// Checks if a file is tracked.
    pub fn is_tracked(&self, path: &Path) -> bool {
        self.files.contains_key(path)
    }

    /// Returns all tracked file paths.
    pub fn tracked_files(&self) -> impl Iterator<Item = &PathBuf> {
        self.files.keys()
    }

    /// Returns the number of tracked files.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Returns true if no files are tracked.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Clears all tracked files.
    pub fn clear(&mut self) {
        self.files.clear();
    }

    /// Checks if a file has been modified externally.
    ///
    /// Returns true if the file exists and its hash differs from the recorded hash.
    pub fn is_modified(&self, path: &Path, current_data: &FileData) -> bool {
        match self.get(path) {
            Some(recorded) => recorded.hexdigest != current_data.hexdigest,
            None => false, // Not tracked, so not "modified"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::tempdir;

    fn make_file_data(content: &str) -> FileData {
        FileData::from_content(content, Utc::now())
    }

    #[test]
    fn test_new_db() {
        let db = FileDB::new();
        assert!(db.is_empty());
        assert_eq!(db.version, "1.0");
    }

    #[test]
    fn test_record_and_get() {
        let mut db = FileDB::new();
        let path = PathBuf::from("test.py");
        let data = make_file_data("print('hello')");

        db.record(path.clone(), data.clone());

        assert!(db.is_tracked(&path));
        assert_eq!(db.get(&path).unwrap().hexdigest, data.hexdigest);
    }

    #[test]
    fn test_remove() {
        let mut db = FileDB::new();
        let path = PathBuf::from("test.py");
        db.record(path.clone(), make_file_data("content"));

        db.remove(&path);

        assert!(!db.is_tracked(&path));
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join(".entangled/filedb.json");

        let mut db = FileDB::new();
        db.record(PathBuf::from("a.py"), make_file_data("a"));
        db.record(PathBuf::from("b.py"), make_file_data("b"));

        db.save(&db_path).unwrap();

        let loaded = FileDB::load(&db_path).unwrap();
        assert_eq!(loaded.len(), 2);
        assert!(loaded.is_tracked(Path::new("a.py")));
        assert!(loaded.is_tracked(Path::new("b.py")));
    }

    #[test]
    fn test_load_nonexistent() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("nonexistent.json");

        let db = FileDB::load(&db_path).unwrap();
        assert!(db.is_empty());
    }

    #[test]
    fn test_is_modified() {
        let mut db = FileDB::new();
        let path = PathBuf::from("test.py");
        let original = make_file_data("original");
        db.record(path.clone(), original);

        let same = make_file_data("original");
        assert!(!db.is_modified(&path, &same));

        let different = make_file_data("modified");
        assert!(db.is_modified(&path, &different));
    }

    #[test]
    fn test_tracked_files() {
        let mut db = FileDB::new();
        db.record(PathBuf::from("a.py"), make_file_data("a"));
        db.record(PathBuf::from("b.py"), make_file_data("b"));

        let files: Vec<_> = db.tracked_files().collect();
        assert_eq!(files.len(), 2);
    }
}
