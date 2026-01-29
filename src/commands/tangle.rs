//! Tangle command implementation.

use std::path::PathBuf;

use crate::errors::Result;
use crate::interface::{tangle_documents, Context};

/// Options for the tangle command.
#[derive(Debug, Clone, Default)]
pub struct TangleOptions {
    /// Force overwrite even if files have been modified externally.
    pub force: bool,
    /// Dry run - show what would be done without doing it.
    pub dry_run: bool,
    /// Specific files to tangle (empty means all).
    pub files: Vec<PathBuf>,
}

/// Executes the tangle command.
pub fn tangle(ctx: &mut Context, options: TangleOptions) -> Result<()> {
    tracing::info!("Tangling documents...");

    let transaction = tangle_documents(ctx)?;

    if transaction.is_empty() {
        println!("No files to tangle.");
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

    println!("Tangled {} files.", transaction.len());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_tangle_basic() {
        let dir = tempdir().unwrap();
        let mut ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();

        // Create a test markdown file
        fs::write(
            dir.path().join("test.md"),
            r#"
```python #main file=output.py
print('hello')
```
"#,
        )
        .unwrap();

        let options = TangleOptions::default();
        tangle(&mut ctx, options).unwrap();

        // Check output was created
        let output_path = dir.path().join("output.py");
        assert!(output_path.exists());

        let content = fs::read_to_string(output_path).unwrap();
        assert!(content.contains("print('hello')"));
    }

    #[test]
    fn test_tangle_dry_run() {
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

        let options = TangleOptions {
            dry_run: true,
            ..Default::default()
        };
        tangle(&mut ctx, options).unwrap();

        // Output should NOT be created
        assert!(!dir.path().join("output.py").exists());
    }
}
