//! Sync command implementation.

use entangled::errors::Result;
use entangled::interface::{stitch_documents, tangle_documents, Context};

/// Options for the sync command.
#[derive(Debug, Clone, Default)]
pub struct SyncOptions {
    /// Force overwrite even if files have been modified externally.
    pub force: bool,
}

/// Executes the sync command.
///
/// Performs stitch first (to capture any code changes), then tangle.
pub fn sync(ctx: &mut Context, options: SyncOptions) -> Result<()> {
    tracing::info!("Synchronizing documents...");

    // First stitch any changes from tangled files
    let stitch_tx = stitch_documents(ctx)?;
    if !stitch_tx.is_empty() {
        if options.force {
            stitch_tx.execute_force(&mut ctx.filedb)?;
        } else {
            stitch_tx.execute(&mut ctx.filedb)?;
        }
    }

    // Then tangle all documents
    let tangle_tx = tangle_documents(ctx)?;
    if !tangle_tx.is_empty() {
        if options.force {
            tangle_tx.execute_force(&mut ctx.filedb)?;
        } else {
            tangle_tx.execute(&mut ctx.filedb)?;
        }
    }

    // Save file database
    ctx.save_filedb()?;

    println!("Synchronization complete.");

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
