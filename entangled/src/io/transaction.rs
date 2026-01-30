//! Transaction system for atomic file operations.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use chrono::Utc;

use super::filedb::FileDB;
use super::stat::FileData;
use crate::errors::{EntangledError, Result};

/// An action that can be executed as part of a transaction.
pub trait Action: std::fmt::Debug + Send + Sync {
    /// Returns the target file path.
    fn target(&self) -> &Path;

    /// Checks if this action conflicts with the current file state.
    fn check_conflict(&self, db: &FileDB) -> Result<()>;

    /// Executes the action.
    fn execute(&self) -> Result<()>;

    /// Updates the file database after execution.
    fn update_db(&self, db: &mut FileDB) -> Result<()>;

    /// Returns a description of this action.
    fn describe(&self) -> String;
}

/// Create a new file (fails if file exists).
#[derive(Debug)]
pub struct Create {
    /// Target file path.
    pub path: PathBuf,
    /// Content to write.
    pub content: String,
}

impl Create {
    /// Creates a new Create action.
    pub fn new(path: impl Into<PathBuf>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content: content.into(),
        }
    }
}

impl Action for Create {
    fn target(&self) -> &Path {
        &self.path
    }

    fn check_conflict(&self, _db: &FileDB) -> Result<()> {
        if self.path.exists() {
            return Err(EntangledError::FileConflict {
                path: self.path.clone(),
            });
        }
        Ok(())
    }

    fn execute(&self) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write atomically via temp file
        atomic_write(&self.path, &self.content)?;
        Ok(())
    }

    fn update_db(&self, db: &mut FileDB) -> Result<()> {
        let data = FileData::from_content(&self.content, Utc::now());
        db.record(self.path.clone(), data);
        Ok(())
    }

    fn describe(&self) -> String {
        format!("create {}", self.path.display())
    }
}

/// Write to an existing file (checks for external modifications).
#[derive(Debug)]
pub struct WriteAction {
    /// Target file path.
    pub path: PathBuf,
    /// Content to write.
    pub content: String,
}

impl WriteAction {
    /// Creates a new Write action.
    pub fn new(path: impl Into<PathBuf>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content: content.into(),
        }
    }
}

impl Action for WriteAction {
    fn target(&self) -> &Path {
        &self.path
    }

    fn check_conflict(&self, db: &FileDB) -> Result<()> {
        // If file exists and is tracked, check for external modifications
        if self.path.exists() && db.is_tracked(&self.path) {
            let current = FileData::from_path(&self.path)?;
            if db.is_modified(&self.path, &current) {
                return Err(EntangledError::FileConflict {
                    path: self.path.clone(),
                });
            }
        }
        Ok(())
    }

    fn execute(&self) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write atomically via temp file
        atomic_write(&self.path, &self.content)?;
        Ok(())
    }

    fn update_db(&self, db: &mut FileDB) -> Result<()> {
        let data = FileData::from_content(&self.content, Utc::now());
        db.record(self.path.clone(), data);
        Ok(())
    }

    fn describe(&self) -> String {
        format!("write {}", self.path.display())
    }
}

/// Delete a file.
#[derive(Debug)]
pub struct Delete {
    /// Target file path.
    pub path: PathBuf,
}

impl Delete {
    /// Creates a new Delete action.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl Action for Delete {
    fn target(&self) -> &Path {
        &self.path
    }

    fn check_conflict(&self, db: &FileDB) -> Result<()> {
        // If file exists and is tracked, check for external modifications
        if self.path.exists() && db.is_tracked(&self.path) {
            let current = FileData::from_path(&self.path)?;
            if db.is_modified(&self.path, &current) {
                return Err(EntangledError::FileConflict {
                    path: self.path.clone(),
                });
            }
        }
        Ok(())
    }

    fn execute(&self) -> Result<()> {
        if self.path.exists() {
            fs::remove_file(&self.path)?;
        }
        Ok(())
    }

    fn update_db(&self, db: &mut FileDB) -> Result<()> {
        db.remove(&self.path);
        Ok(())
    }

    fn describe(&self) -> String {
        format!("delete {}", self.path.display())
    }
}

/// A collection of actions to execute atomically.
#[derive(Debug, Default)]
pub struct Transaction {
    /// Actions to execute.
    actions: Vec<Box<dyn Action>>,
}

impl Transaction {
    /// Creates a new empty transaction.
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    /// Adds an action to the transaction.
    pub fn add(&mut self, action: impl Action + 'static) {
        self.actions.push(Box::new(action));
    }

    /// Adds a create action.
    pub fn create(&mut self, path: impl Into<PathBuf>, content: impl Into<String>) {
        self.add(Create::new(path, content));
    }

    /// Adds a write action.
    pub fn write(&mut self, path: impl Into<PathBuf>, content: impl Into<String>) {
        self.add(WriteAction::new(path, content));
    }

    /// Adds a delete action.
    pub fn delete(&mut self, path: impl Into<PathBuf>) {
        self.add(Delete::new(path));
    }

    /// Returns the number of actions.
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// Returns true if there are no actions.
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Returns descriptions of all actions.
    pub fn describe(&self) -> Vec<String> {
        self.actions.iter().map(|a| a.describe()).collect()
    }

    /// Checks all actions for conflicts.
    pub fn check_conflicts(&self, db: &FileDB) -> Result<()> {
        for action in &self.actions {
            action.check_conflict(db)?;
        }
        Ok(())
    }

    /// Executes all actions and updates the database.
    pub fn execute(&self, db: &mut FileDB) -> Result<()> {
        // First check all conflicts
        self.check_conflicts(db)?;

        // Execute all actions
        for action in &self.actions {
            action.execute()?;
            action.update_db(db)?;
        }

        Ok(())
    }

    /// Executes all actions, ignoring conflicts, and updates the database.
    pub fn execute_force(&self, db: &mut FileDB) -> Result<()> {
        for action in &self.actions {
            action.execute()?;
            action.update_db(db)?;
        }
        Ok(())
    }
}

/// Writes content to a file atomically using a temp file.
fn atomic_write(path: &Path, content: &str) -> io::Result<()> {
    // Create temp file in the same directory
    let parent = path.parent().unwrap_or(Path::new("."));
    let temp_path = parent.join(format!(
        ".entangled-tmp-{}",
        std::process::id()
    ));

    // Write to temp file
    {
        let mut file = File::create(&temp_path)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
    }

    // Rename to target
    fs::rename(&temp_path, path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_action() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("new.txt");

        let action = Create::new(&path, "content");
        let mut db = FileDB::new();

        action.check_conflict(&db).unwrap();
        action.execute().unwrap();
        action.update_db(&mut db).unwrap();

        assert!(path.exists());
        assert_eq!(fs::read_to_string(&path).unwrap(), "content");
        assert!(db.is_tracked(&path));
    }

    #[test]
    fn test_create_conflict() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("existing.txt");
        fs::write(&path, "existing").unwrap();

        let action = Create::new(&path, "new");
        let db = FileDB::new();

        assert!(action.check_conflict(&db).is_err());
    }

    #[test]
    fn test_write_action() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("file.txt");
        fs::write(&path, "original").unwrap();

        let mut db = FileDB::new();
        let original_data = FileData::from_path(&path).unwrap();
        db.record(path.clone(), original_data);

        let action = WriteAction::new(&path, "updated");
        action.check_conflict(&db).unwrap();
        action.execute().unwrap();
        action.update_db(&mut db).unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "updated");
    }

    #[test]
    fn test_write_conflict() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("file.txt");
        fs::write(&path, "original").unwrap();

        let mut db = FileDB::new();
        let original_data = FileData::from_content("recorded", Utc::now());
        db.record(path.clone(), original_data);

        // File has different content than recorded
        let action = WriteAction::new(&path, "updated");
        assert!(action.check_conflict(&db).is_err());
    }

    #[test]
    fn test_delete_action() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("file.txt");
        fs::write(&path, "content").unwrap();

        let mut db = FileDB::new();
        let data = FileData::from_path(&path).unwrap();
        db.record(path.clone(), data);

        let action = Delete::new(&path);
        action.check_conflict(&db).unwrap();
        action.execute().unwrap();
        action.update_db(&mut db).unwrap();

        assert!(!path.exists());
        assert!(!db.is_tracked(&path));
    }

    #[test]
    fn test_transaction() {
        let dir = tempdir().unwrap();
        let path1 = dir.path().join("a.txt");
        let path2 = dir.path().join("b.txt");

        let mut tx = Transaction::new();
        tx.create(&path1, "content a");
        tx.create(&path2, "content b");

        let mut db = FileDB::new();
        tx.execute(&mut db).unwrap();

        assert!(path1.exists());
        assert!(path2.exists());
        assert_eq!(db.len(), 2);
    }

    #[test]
    fn test_transaction_rollback_on_conflict() {
        let dir = tempdir().unwrap();
        let path1 = dir.path().join("new.txt");
        let path2 = dir.path().join("existing.txt");
        fs::write(&path2, "existing").unwrap();

        let mut tx = Transaction::new();
        tx.create(&path1, "new");
        tx.create(&path2, "conflict"); // This will conflict

        let mut db = FileDB::new();
        assert!(tx.execute(&mut db).is_err());

        // Neither file should be created (conflict check happens first)
        assert!(!path1.exists());
    }

    #[test]
    fn test_transaction_force() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("file.txt");
        fs::write(&path, "original").unwrap();

        let mut db = FileDB::new();
        // Record different content to create conflict
        db.record(path.clone(), FileData::from_content("different", Utc::now()));

        let mut tx = Transaction::new();
        tx.write(&path, "forced");

        // Normal execute would fail
        assert!(tx.check_conflicts(&db).is_err());

        // Force execute succeeds
        tx.execute_force(&mut db).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "forced");
    }
}
