//! Transaction system for atomic file operations.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

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

    /// Returns the proposed new content, if any.
    fn proposed_content(&self) -> Option<&str> {
        None
    }
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

    fn proposed_content(&self) -> Option<&str> {
        Some(&self.content)
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

    fn proposed_content(&self) -> Option<&str> {
        Some(&self.content)
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
    #[must_use]
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

    /// Returns unified diffs for all actions that modify file content.
    ///
    /// For each write/create action, reads the existing file (if any) and
    /// produces a unified diff against the proposed content. Delete actions
    /// show the full file as removed.
    pub fn diffs(&self) -> Vec<String> {
        self.actions
            .iter()
            .filter_map(|action| {
                let path = action.target();
                let path_str = path.display().to_string();

                if let Some(new_content) = action.proposed_content() {
                    let old_content = if path.exists() {
                        fs::read_to_string(path).unwrap_or_default()
                    } else {
                        String::new()
                    };

                    if old_content == new_content {
                        return None;
                    }

                    let old_label = format!("a/{}", path_str);
                    let new_label = format!("b/{}", path_str);
                    let diff = unified_diff(&old_content, new_content, &old_label, &new_label);
                    if diff.is_empty() {
                        None
                    } else {
                        Some(diff)
                    }
                } else {
                    // Delete action
                    if path.exists() {
                        if let Ok(content) = fs::read_to_string(path) {
                            let old_label = format!("a/{}", path_str);
                            let diff = unified_diff(&content, "", &old_label, "/dev/null");
                            Some(diff)
                        } else {
                            Some(format!("delete {}", path_str))
                        }
                    } else {
                        None
                    }
                }
            })
            .collect()
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

/// Produces a unified diff between two strings.
fn unified_diff(old: &str, new: &str, old_label: &str, new_label: &str) -> String {
    let old_lines: Vec<&str> = if old.is_empty() {
        Vec::new()
    } else {
        old.lines().collect()
    };
    let new_lines: Vec<&str> = if new.is_empty() {
        Vec::new()
    } else {
        new.lines().collect()
    };

    // Simple line-by-line diff using longest common subsequence
    let lcs = lcs_table(&old_lines, &new_lines);
    let mut hunks = collect_hunks(&old_lines, &new_lines, &lcs, 3);

    if hunks.is_empty() {
        return String::new();
    }

    let mut output = Vec::new();
    output.push(format!("--- {}", old_label));
    output.push(format!("+++ {}", new_label));

    for hunk in &mut hunks {
        output.push(format!(
            "@@ -{},{} +{},{} @@",
            hunk.old_start + 1,
            hunk.old_count,
            hunk.new_start + 1,
            hunk.new_count,
        ));
        output.append(&mut hunk.lines);
    }

    output.join("\n")
}

struct DiffHunk {
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
    lines: Vec<String>,
}

fn lcs_table(old: &[&str], new: &[&str]) -> Vec<Vec<usize>> {
    let m = old.len();
    let n = new.len();
    let mut table = vec![vec![0usize; n + 1]; m + 1];

    for i in 1..=m {
        for j in 1..=n {
            if old[i - 1] == new[j - 1] {
                table[i][j] = table[i - 1][j - 1] + 1;
            } else {
                table[i][j] = table[i - 1][j].max(table[i][j - 1]);
            }
        }
    }

    table
}

fn collect_hunks(
    old: &[&str],
    new: &[&str],
    lcs: &[Vec<usize>],
    context: usize,
) -> Vec<DiffHunk> {
    // Build edit script from LCS table
    let mut edits: Vec<(char, usize, usize)> = Vec::new(); // (type, old_idx, new_idx)
    let mut i = old.len();
    let mut j = new.len();

    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old[i - 1] == new[j - 1] {
            edits.push((' ', i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || lcs[i][j - 1] >= lcs[i - 1][j]) {
            edits.push(('+', i, j - 1));
            j -= 1;
        } else {
            edits.push(('-', i - 1, j));
            i -= 1;
        }
    }

    edits.reverse();

    // Find changed regions and create hunks with context
    let mut change_indices: Vec<usize> = Vec::new();
    for (idx, (edit_type, _, _)) in edits.iter().enumerate() {
        if *edit_type != ' ' {
            change_indices.push(idx);
        }
    }

    if change_indices.is_empty() {
        return Vec::new();
    }

    // Group changes into hunks (merge if within 2*context lines of each other)
    let mut groups: Vec<(usize, usize)> = Vec::new(); // (first_change_idx, last_change_idx)
    let mut group_start = change_indices[0];
    let mut group_end = change_indices[0];

    for &ci in &change_indices[1..] {
        if ci - group_end <= 2 * context {
            group_end = ci;
        } else {
            groups.push((group_start, group_end));
            group_start = ci;
            group_end = ci;
        }
    }
    groups.push((group_start, group_end));

    // Build hunks
    let mut hunks = Vec::new();
    for (gs, ge) in groups {
        let hunk_start = gs.saturating_sub(context);
        let hunk_end = (ge + context + 1).min(edits.len());

        let mut lines = Vec::new();
        let mut old_start = usize::MAX;
        let mut new_start = usize::MAX;
        let mut old_count = 0;
        let mut new_count = 0;

        for edit in &edits[hunk_start..hunk_end] {
            match edit.0 {
                ' ' => {
                    if old_start == usize::MAX {
                        old_start = edit.1;
                        new_start = edit.2;
                    }
                    lines.push(format!(" {}", old[edit.1]));
                    old_count += 1;
                    new_count += 1;
                }
                '-' => {
                    if old_start == usize::MAX {
                        old_start = edit.1;
                        new_start = edit.2;
                    }
                    lines.push(format!("-{}", old[edit.1]));
                    old_count += 1;
                }
                '+' => {
                    if old_start == usize::MAX {
                        old_start = edit.1;
                        new_start = edit.2;
                    }
                    lines.push(format!("+{}", new[edit.2]));
                    new_count += 1;
                }
                _ => {}
            }
        }

        hunks.push(DiffHunk {
            old_start,
            old_count,
            new_start,
            new_count,
            lines,
        });
    }

    hunks
}

/// Counter for unique temp file names.
static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Writes content to a file atomically using a temp file.
fn atomic_write(path: &Path, content: &str) -> io::Result<()> {
    // Create temp file in the same directory with unique name
    let parent = path.parent().unwrap_or(Path::new("."));
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp_path = parent.join(format!(
        ".entangled-tmp-{}-{}",
        std::process::id(),
        counter,
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
