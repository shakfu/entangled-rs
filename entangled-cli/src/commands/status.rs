//! Status command implementation.
//!
//! The status *computation* lives in `entangled::status` so that the Python CLI
//! reports identical values; this module only renders it.

use entangled::errors::Result;
use entangled::interface::Context;
use entangled::status::{collect_status, FileStatus, ProjectStatus};

/// Options for the status command.
#[derive(Debug, Clone, Default)]
pub struct StatusOptions {
    /// Show verbose output.
    pub verbose: bool,
    /// Output machine-readable JSON.
    pub json: bool,
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

fn print_human(data: &ProjectStatus, verbose: bool) {
    println!("Source files: {}", data.source_files.len());

    if verbose {
        for file in &data.source_files {
            println!("  {}", file.display());
        }
    }

    println!("\nTarget files: {}", data.targets.len());

    if verbose {
        for target in &data.targets {
            println!("  {} ({})", target.path.display(), target.status);
        }
    }

    println!("\nStatus summary:");
    println!("  Up to date: {}", data.count(FileStatus::UpToDate));
    println!("  Needs tangle: {}", data.count(FileStatus::NeedsTangle));
    println!(
        "  Externally modified: {}",
        data.count(FileStatus::ExternallyModified)
    );
    println!("  Missing: {}", data.count(FileStatus::Missing));

    println!("\nTracked files in database: {}", data.tracked_count);
}

fn print_json(data: &ProjectStatus) {
    let targets: Vec<serde_json::Value> = data
        .targets
        .iter()
        .map(|target| {
            serde_json::json!({
                "path": target.path.to_string_lossy(),
                "status": target.status.as_str(),
            })
        })
        .collect();

    let source_files: Vec<String> = data
        .source_files
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();

    let output = serde_json::json!({
        "source_files": source_files,
        "targets": targets,
        "tracked_count": data.tracked_count,
    });

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
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

        let options = StatusOptions {
            verbose: true,
            json: false,
        };
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

        let options = StatusOptions {
            verbose: false,
            json: true,
        };
        status(&ctx, options).unwrap();
    }
}
