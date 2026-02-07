//! Status command implementation.

use std::path::PathBuf;

use entangled::errors::Result;
use entangled::interface::{Context, Document};
use entangled::io::FileData;

/// Options for the status command.
#[derive(Debug, Clone, Default)]
pub struct StatusOptions {
    /// Show verbose output.
    pub verbose: bool,
    /// Output machine-readable JSON.
    pub json: bool,
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

impl FileStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::UpToDate => "up-to-date",
            Self::NeedsTangle => "needs-tangle",
            Self::ExternallyModified => "modified",
            Self::Missing => "missing",
        }
    }
}

/// Collected status data for JSON output.
struct StatusData {
    source_files: Vec<PathBuf>,
    targets: Vec<(PathBuf, FileStatus)>,
    tracked_count: usize,
}

/// Executes the status command.
pub fn status(ctx: &Context, options: StatusOptions) -> Result<()> {
    let data = collect_status(ctx)?;

    if options.json {
        print_json(&data);
    } else {
        print_human(&data, options.verbose);
    }

    Ok(())
}

fn collect_status(ctx: &Context) -> Result<StatusData> {
    let source_files = ctx.source_files()?;

    let mut target_paths = Vec::new();
    for path in &source_files {
        let doc = Document::load(path, ctx)?;
        target_paths.extend(doc.targets());
    }

    let mut targets = Vec::new();
    for target in target_paths {
        let full_path = ctx.resolve_path(&target);
        let status = get_file_status(&full_path, &ctx.filedb)?;
        targets.push((target, status));
    }

    Ok(StatusData {
        source_files,
        targets,
        tracked_count: ctx.filedb.len(),
    })
}

fn print_human(data: &StatusData, verbose: bool) {
    println!("Source files: {}", data.source_files.len());

    if verbose {
        for file in &data.source_files {
            println!("  {}", file.display());
        }
    }

    println!("\nTarget files: {}", data.targets.len());

    let mut up_to_date = 0;
    let mut needs_tangle = 0;
    let mut modified = 0;
    let mut missing = 0;

    for (target, status) in &data.targets {
        match status {
            FileStatus::UpToDate => up_to_date += 1,
            FileStatus::NeedsTangle => needs_tangle += 1,
            FileStatus::ExternallyModified => modified += 1,
            FileStatus::Missing => missing += 1,
        }

        if verbose {
            println!("  {} ({})", target.display(), status.as_str());
        }
    }

    println!("\nStatus summary:");
    println!("  Up to date: {}", up_to_date);
    println!("  Needs tangle: {}", needs_tangle);
    println!("  Externally modified: {}", modified);
    println!("  Missing: {}", missing);

    println!("\nTracked files in database: {}", data.tracked_count);
}

fn print_json(data: &StatusData) {
    let source_files: Vec<&str> = data
        .source_files
        .iter()
        .filter_map(|p| p.to_str())
        .collect();

    let targets: Vec<serde_json::Value> = data
        .targets
        .iter()
        .map(|(path, status)| {
            serde_json::json!({
                "path": path.to_string_lossy(),
                "status": status.as_str(),
            })
        })
        .collect();

    let output = serde_json::json!({
        "source_files": source_files,
        "targets": targets,
        "tracked_count": data.tracked_count,
    });

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
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

        let options = StatusOptions { verbose: true, json: false };
        status(&ctx, options).unwrap();
    }

    #[test]
    fn test_status_json() {
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

        let options = StatusOptions { verbose: false, json: true };
        status(&ctx, options).unwrap();
    }
}
