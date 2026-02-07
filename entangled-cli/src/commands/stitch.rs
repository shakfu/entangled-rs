//! Stitch command implementation.

use std::path::PathBuf;

use entangled::errors::Result;
use entangled::interface::{stitch_documents, stitch_files, Context};

use super::helpers::{TransactionOptions, run_transaction};

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
    /// Specific files to stitch (empty means all).
    pub files: Vec<PathBuf>,
}

/// Executes the stitch command.
pub fn stitch(ctx: &mut Context, options: StitchOptions) -> Result<()> {
    tracing::info!("Stitching documents...");

    let transaction = if options.files.is_empty() {
        stitch_documents(ctx)?
    } else {
        let filtered = ctx.source_files_filtered(&options.files)?;
        stitch_files(ctx, &filtered)?
    };

    run_transaction(ctx, transaction, &TransactionOptions {
        force: options.force,
        dry_run: options.dry_run,
        diff: options.diff,
        quiet: options.quiet,
    }, "stitch")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_stitch_no_changes() {
        let dir = tempdir().unwrap();
        let mut ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();

        let options = StitchOptions::default();
        stitch(&mut ctx, options).unwrap();
        // Should complete without error when no files exist
    }
}
