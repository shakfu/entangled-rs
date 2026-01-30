//! Status command implementation.

use entangled::errors::Result;
use entangled::interface::{Context, Document};
use entangled::io::FileData;

/// Options for the status command.
#[derive(Debug, Clone, Default)]
pub struct StatusOptions {
    /// Show verbose output.
    pub verbose: bool,
}

/// File status information.
#[derive(Debug)]
pub enum FileStatus {
    /// File is up to date.
    UpToDate,
    /// File needs to be tangled (new or modified source).
    NeedsTangle,
    /// File has been modified externally.
    ExternallyModified,
    /// File is missing.
    Missing,
}

/// Executes the status command.
pub fn status(ctx: &Context, options: StatusOptions) -> Result<()> {
    let source_files = ctx.source_files()?;

    println!("Source files: {}", source_files.len());

    if options.verbose {
        for file in &source_files {
            println!("  {}", file.display());
        }
    }

    // Load all documents and collect targets
    let mut targets = Vec::new();
    for path in &source_files {
        let doc = Document::load(path, ctx)?;
        targets.extend(doc.targets());
    }

    println!("\nTarget files: {}", targets.len());

    let mut up_to_date = 0;
    let mut needs_tangle = 0;
    let mut modified = 0;
    let mut missing = 0;

    for target in &targets {
        let full_path = ctx.resolve_path(target);
        let status = get_file_status(&full_path, &ctx.filedb)?;

        match status {
            FileStatus::UpToDate => up_to_date += 1,
            FileStatus::NeedsTangle => needs_tangle += 1,
            FileStatus::ExternallyModified => modified += 1,
            FileStatus::Missing => missing += 1,
        }

        if options.verbose {
            let status_str = match status {
                FileStatus::UpToDate => "up-to-date",
                FileStatus::NeedsTangle => "needs tangle",
                FileStatus::ExternallyModified => "modified externally",
                FileStatus::Missing => "missing",
            };
            println!("  {} ({})", target.display(), status_str);
        }
    }

    println!("\nStatus summary:");
    println!("  Up to date: {}", up_to_date);
    println!("  Needs tangle: {}", needs_tangle);
    println!("  Externally modified: {}", modified);
    println!("  Missing: {}", missing);

    // Show tracked files in database
    let tracked = ctx.filedb.len();
    println!("\nTracked files in database: {}", tracked);

    Ok(())
}

/// Gets the status of a target file.
fn get_file_status(path: &std::path::Path, filedb: &entangled::io::FileDB) -> Result<FileStatus> {
    if !path.exists() {
        if filedb.is_tracked(path) {
            return Ok(FileStatus::Missing);
        } else {
            return Ok(FileStatus::NeedsTangle);
        }
    }

    let current = FileData::from_path(path)?;

    if let Some(recorded) = filedb.get(path) {
        if recorded.hexdigest == current.hexdigest {
            Ok(FileStatus::UpToDate)
        } else {
            Ok(FileStatus::ExternallyModified)
        }
    } else {
        Ok(FileStatus::NeedsTangle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_status_empty() {
        let dir = tempdir().unwrap();
        let ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();

        let options = StatusOptions::default();
        status(&ctx, options).unwrap();
    }

    #[test]
    fn test_status_with_files() {
        let dir = tempdir().unwrap();
        let ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();

        fs::write(
            dir.path().join("test.md"),
            r#"
```python #main file=output.py
print('hello')
```
"#,
        )
        .unwrap();

        let options = StatusOptions { verbose: true };
        status(&ctx, options).unwrap();
    }
}
