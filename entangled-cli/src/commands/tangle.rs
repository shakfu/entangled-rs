//! Tangle command implementation.

use std::path::PathBuf;

use entangled::errors::Result;
use entangled::interface::{tangle_documents, tangle_files, Context};

use super::helpers::{run_transaction, TransactionOptions};

/// Options for the tangle command.
#[derive(Debug, Clone, Default)]
pub struct TangleOptions {
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
    /// Specific files to tangle (empty means all).
    pub files: Vec<PathBuf>,
}

/// Executes the tangle command.
pub fn tangle(ctx: &mut Context, options: TangleOptions) -> Result<()> {
    tracing::info!("Tangling documents...");

    let has_filters = !options.files.is_empty() || !options.glob.is_empty();

    let transaction = if !has_filters {
        tangle_documents(ctx)?
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
        tangle_files(ctx, &selected)?
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
        "tangle",
    )
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
    fn test_tangle_glob() {
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
        let options = TangleOptions {
            glob: vec!["docs/*.md".to_string()],
            ..Default::default()
        };
        tangle(&mut ctx, options).unwrap();

        assert!(dir.path().join("a.py").exists());
        assert!(!dir.path().join("b.py").exists());
    }

    #[test]
    fn test_tangle_glob_no_match() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.md"), "# hello\n").unwrap();

        let mut ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();
        let options = TangleOptions {
            glob: vec!["nonexistent/*.md".to_string()],
            ..Default::default()
        };
        let result = tangle(&mut ctx, options);
        assert!(result.is_err());
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
