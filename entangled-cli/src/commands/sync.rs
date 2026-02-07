//! Sync command implementation.

use entangled::errors::Result;
use entangled::interface::{stitch_documents, sync_documents, tangle_documents, Context};

/// Options for the sync command.
#[derive(Debug, Clone, Default)]
pub struct SyncOptions {
    /// Force overwrite even if files have been modified externally.
    pub force: bool,
    /// Dry run - show what would be done without doing it.
    pub dry_run: bool,
    /// Show unified diffs of what would change.
    pub diff: bool,
    /// Suppress normal output.
    pub quiet: bool,
}

/// Executes the sync command.
///
/// Performs stitch first (to capture any code changes), then tangle.
pub fn sync(ctx: &mut Context, options: SyncOptions) -> Result<()> {
    tracing::info!("Synchronizing documents...");

    // For diff/dry-run we need to compute transactions without executing
    if options.diff || options.dry_run {
        let stitch_tx = stitch_documents(ctx)?;
        let tangle_tx = tangle_documents(ctx)?;

        if options.diff {
            for diff in stitch_tx.diffs() {
                println!("{}", diff);
            }
            for diff in tangle_tx.diffs() {
                println!("{}", diff);
            }
            return Ok(());
        }

        // dry_run
        let stitch_count = stitch_tx.len();
        let tangle_count = tangle_tx.len();
        if stitch_count + tangle_count == 0 {
            if !options.quiet {
                println!("Nothing to do.");
            }
        } else {
            if stitch_count > 0 {
                println!("Would stitch {} files:", stitch_count);
                for desc in stitch_tx.describe() {
                    println!("  {}", desc);
                }
            }
            if tangle_count > 0 {
                println!("Would tangle {} files:", tangle_count);
                for desc in tangle_tx.describe() {
                    println!("  {}", desc);
                }
            }
        }
        return Ok(());
    }

    // Normal execution -- delegate to library
    sync_documents(ctx, options.force)?;

    if !options.quiet {
        println!("Synchronization complete.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_sync_basic() {
        let dir = tempdir().unwrap();
        let mut ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();

        fs::write(
            dir.path().join("test.md"),
            r#"
```python #main file=output.py
print('hello')
```
"#,
        )
        .unwrap();

        let options = SyncOptions::default();
        sync(&mut ctx, options).unwrap();

        // Output should be created
        assert!(dir.path().join("output.py").exists());
    }
}
