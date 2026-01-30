//! Reset command implementation.

use std::fs;
use std::io::{self, Write};

use entangled::errors::Result;
use entangled::interface::Context;

/// Options for the reset command.
#[derive(Debug, Clone, Default)]
pub struct ResetOptions {
    /// Also delete tangled files.
    pub delete_files: bool,
    /// Don't ask for confirmation.
    pub force: bool,
}

/// Executes the reset command.
pub fn reset(ctx: &mut Context, options: ResetOptions) -> Result<()> {
    if options.delete_files {
        // Get list of tracked files
        let tracked: Vec<_> = ctx.filedb.tracked_files().cloned().collect();

        if tracked.is_empty() {
            println!("No tracked files to delete.");
        } else {
            // Confirm unless --force is specified
            if !options.force {
                println!("This will delete {} tracked files:", tracked.len());
                for path in &tracked {
                    println!("  {}", path.display());
                }
                print!("Continue? [y/N] ");
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Aborted.");
                    return Ok(());
                }
            }

            // Delete all tracked files
            for path in &tracked {
                let full_path = ctx.resolve_path(path);
                if full_path.exists() {
                    tracing::info!("Deleting {}", full_path.display());
                    fs::remove_file(&full_path)?;
                }
            }

            println!("Deleted {} tracked files.", tracked.len());
        }
    }

    // Clear the file database
    ctx.filedb.clear();
    ctx.save_filedb()?;

    // Delete the database file itself
    if ctx.filedb_path.exists() {
        fs::remove_file(&ctx.filedb_path)?;
    }

    // Try to remove the .entangled directory if empty
    if let Some(parent) = ctx.filedb_path.parent() {
        let _ = fs::remove_dir(parent); // Ignore error if not empty
    }

    println!("Reset complete. File database cleared.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    use chrono::Utc;
    use entangled::io::FileData;

    #[test]
    fn test_reset_clears_db() {
        let dir = tempdir().unwrap();
        let mut ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();

        // Add some entries to the database
        ctx.filedb.record(
            std::path::PathBuf::from("test.py"),
            FileData::from_content("content", Utc::now()),
        );
        ctx.save_filedb().unwrap();

        assert!(!ctx.filedb.is_empty());

        // Reset
        let options = ResetOptions::default();
        reset(&mut ctx, options).unwrap();

        // Database should be cleared
        let reloaded = Context::default_for_dir(dir.path().to_path_buf()).unwrap();
        assert!(reloaded.filedb.is_empty());
    }

    #[test]
    fn test_reset_delete_files() {
        let dir = tempdir().unwrap();
        let mut ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();

        // Create a tracked file
        let file_path = dir.path().join("output.py");
        fs::write(&file_path, "print('hello')").unwrap();

        ctx.filedb.record(
            std::path::PathBuf::from("output.py"),
            FileData::from_content("print('hello')", Utc::now()),
        );
        ctx.save_filedb().unwrap();

        assert!(file_path.exists());

        // Reset with delete (force to skip confirmation)
        let options = ResetOptions {
            delete_files: true,
            force: true,
        };
        reset(&mut ctx, options).unwrap();

        // File should be deleted
        assert!(!file_path.exists());
    }
}
