//! Stitch command implementation.

use std::path::PathBuf;

use crate::errors::Result;
use crate::interface::{stitch_documents, Context};

/// Options for the stitch command.
#[derive(Debug, Clone, Default)]
pub struct StitchOptions {
    /// Force overwrite even if files have been modified externally.
    pub force: bool,
    /// Dry run - show what would be done without doing it.
    pub dry_run: bool,
    /// Specific files to stitch (empty means all).
    pub files: Vec<PathBuf>,
}

/// Executes the stitch command.
pub fn stitch(ctx: &mut Context, options: StitchOptions) -> Result<()> {
    tracing::info!("Stitching documents...");

    let transaction = stitch_documents(ctx)?;

    if transaction.is_empty() {
        println!("No files to stitch.");
        return Ok(());
    }

    if options.dry_run {
        println!("Would perform {} actions:", transaction.len());
        for desc in transaction.describe() {
            println!("  {}", desc);
        }
        return Ok(());
    }

    // Execute transaction
    if options.force {
        transaction.execute_force(&mut ctx.filedb)?;
    } else {
        transaction.execute(&mut ctx.filedb)?;
    }

    // Save file database
    ctx.save_filedb()?;

    println!("Stitched {} files.", transaction.len());

    Ok(())
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
