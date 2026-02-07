//! Locate command implementation.
//!
//! Maps a line in a tangled output file back to its markdown source.

use std::path::PathBuf;

use entangled::errors::Result;
use entangled::interface::{locate_source, Context};

/// Options for the locate command.
#[derive(Debug, Clone)]
pub struct LocateOptions {
    /// Target file path.
    pub file: PathBuf,
    /// Line number in the target file (1-indexed).
    pub line: usize,
}

/// Executes the locate command.
///
/// Parses `file:line` and prints the corresponding markdown source location.
pub fn locate(ctx: &Context, options: LocateOptions) -> Result<()> {
    let full_path = ctx.resolve_path(&options.file);

    if !full_path.exists() {
        return Err(entangled::EntangledError::Other(format!(
            "File not found: {}",
            full_path.display()
        )));
    }

    match locate_source(ctx, &full_path, options.line)? {
        Some(loc) => {
            println!("{}", loc);
        }
        None => {
            eprintln!(
                "No source mapping for {}:{}",
                options.file.display(),
                options.line
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_locate_basic() {
        let dir = tempdir().unwrap();
        let mut ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();

        fs::write(
            dir.path().join("test.md"),
            r#"# Title

```python #main file=output.py
print('line1')
print('line2')
print('line3')
```
"#,
        )
        .unwrap();

        // Tangle first
        let tx = entangled::interface::tangle_documents(&ctx).unwrap();
        tx.execute(&mut ctx.filedb).unwrap();

        // The tangled output.py should look like:
        // 1: # ~/~ begin <<main[0]>>
        // 2: print('line1')
        // 3: print('line2')
        // 4: print('line3')
        // 5: # ~/~ end

        let options = LocateOptions {
            file: PathBuf::from("output.py"),
            line: 2,
        };
        let result = locate_source(&ctx, &ctx.resolve_path(&options.file), options.line).unwrap();
        assert!(result.is_some());
        let loc = result.unwrap();
        // Line 2 of output.py is print('line1'), which is line 4 of test.md
        // (line 3 is the fence, line 4 is first content line)
        assert_eq!(loc.source_line, 4);
        assert!(loc.source_file.ends_with("test.md"));
    }

    #[test]
    fn test_locate_annotation_line() {
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

        let tx = entangled::interface::tangle_documents(&ctx).unwrap();
        tx.execute(&mut ctx.filedb).unwrap();

        // Line 1 is the begin marker
        let result =
            locate_source(&ctx, &ctx.resolve_path(&PathBuf::from("output.py")), 1).unwrap();
        assert!(result.is_none(), "Annotation lines should return None");
    }
}
