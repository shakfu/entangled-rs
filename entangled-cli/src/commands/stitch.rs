//! Stitch command implementation.

use std::path::PathBuf;

use entangled::errors::Result;
use entangled::interface::{stitch_documents, stitch_files, Context};

use super::helpers::{run_transaction, TransactionOptions};

/// Options for the stitch command.
#[derive(Debug, Clone, Default)]
pub struct StitchOptions {
    /// Force overwrite even if files have been modified externally.
    pub force: bool,
    /// Dry run - show what would be done without doing it.
    pub dry_run: bool,
    /// Show unified diffs of what would change.
    pub diff: bool,
    /// Suppress normal output.
    pub quiet: bool,
    /// Glob patterns to filter source files.
    pub glob: Vec<String>,
    /// Specific files to stitch (empty means all).
    pub files: Vec<PathBuf>,
}

/// Executes the stitch command.
pub fn stitch(ctx: &mut Context, options: StitchOptions) -> Result<()> {
    tracing::info!("Stitching documents...");

    let has_filters = !options.files.is_empty() || !options.glob.is_empty();

    let transaction = if !has_filters {
        stitch_documents(ctx)?
    } else {
        let mut selected = Vec::new();
        if !options.files.is_empty() {
            selected.extend(ctx.source_files_filtered(&options.files)?);
        }
        if !options.glob.is_empty() {
            selected.extend(ctx.source_files_glob(&options.glob)?);
        }
        selected.sort();
        selected.dedup();
        stitch_files(ctx, &selected)?
    };

    run_transaction(
        ctx,
        transaction,
        &TransactionOptions {
            force: options.force,
            dry_run: options.dry_run,
            diff: options.diff,
            quiet: options.quiet,
        },
        "stitch",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_stitch_no_changes() {
        let dir = tempdir().unwrap();
        let mut ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();

        let options = StitchOptions::default();
        stitch(&mut ctx, options).unwrap();
        // Should complete without error when no files exist
    }

    #[test]
    fn test_stitch_glob() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("docs")).unwrap();
        fs::create_dir(dir.path().join("other")).unwrap();

        fs::write(
            dir.path().join("docs/a.md"),
            "```python #main file=a.py\nprint('a')\n```\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("other/b.md"),
            "```python #main file=b.py\nprint('b')\n```\n",
        )
        .unwrap();

        let mut ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();

        // Stitch with glob filtering -- should succeed and only process docs/
        let options = StitchOptions {
            glob: vec!["docs/*.md".to_string()],
            ..Default::default()
        };
        stitch(&mut ctx, options).unwrap();
    }

    #[test]
    fn test_stitch_glob_no_match() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.md"), "# hello\n").unwrap();

        let mut ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();
        let options = StitchOptions {
            glob: vec!["nonexistent/*.md".to_string()],
            ..Default::default()
        };
        let result = stitch(&mut ctx, options);
        assert!(result.is_err());
    }
}
