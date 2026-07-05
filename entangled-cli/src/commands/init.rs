//! Init command implementation.

use std::path::Path;

use entangled::errors::{EntangledError, Result};

const DEFAULT_CONFIG: &str = r##"version = "2.0"

# Glob patterns for source markdown files
source_patterns = ["**/*.md"]

# Code block syntax style for .md files
# Options: "entangled-rs" (default), "pandoc", "quarto", "knitr"
style = "entangled-rs"

# How to annotate output files
# Options: "standard" (default), "naked", "supplemental"
annotation = "standard"

# Default namespace for code block IDs
# Options: "file" (prefix with filename, default), "none"
namespace_default = "file"

# File database location
filedb_path = ".entangled/filedb.json"

# Watch configuration
[watch]
debounce_ms = 100

# Hook configuration
[hooks]
# shebang = true      # Move shebang lines to top of tangled output
# spdx_license = true # Move SPDX license headers to top of tangled output

# Custom language definitions (uncomment to add)
# [[languages]]
# name = "mylang"
# comment = "#"
# identifiers = ["ml", "myl"]
"##;

/// A runnable starter document written by `init --example`. It exercises the
/// whole loop: a `file=` target assembled from `<<reference>>` blocks (tangle),
/// a runnable block (eval), and prose with cross-references (weave).
const EXAMPLE_DOC: &str = r##"# Hello, Entangled

This is a *literate program*: prose and code live together. The block below is
**tangled** into `hello.py`, with its `<<imports>>` and `<<greeting>>` references
expanded in place.

```python #main file=hello.py
<<imports>>


def greet(name):
    <<greeting>>


if __name__ == "__main__":
    greet("world")
```

The imports:

```python #imports
import sys
```

The greeting body (indented to match the function):

```python #greeting
message = f"Hello, {name}!"
print(message, file=sys.stdout)
```

## Reproducible output

A block marked `eval=` is executed by `entangled eval`, and its output is shown
here when you **weave** the document.

```python #demo eval=python
print(sum(range(1, 11)))
```

## Try it

- `entangled tangle` writes `hello.py`
- `entangled eval` runs the demo block (prints `55`)
- `entangled weave -o hello.html` renders this page with clickable cross-references
- `entangled check` validates references and targets
- `entangled graph --format mermaid` shows the block dependency graph
"##;

/// Config used by `init --example`: `namespace_default = "none"` so the simple
/// `<<imports>>` references in the starter document resolve directly.
const EXAMPLE_CONFIG: &str = r##"version = "2.0"

# Glob patterns for source markdown files
source_patterns = ["**/*.md"]

# Code block syntax style for .md files
style = "entangled-rs"

# Use bare reference names (so `<<imports>>` resolves without a file prefix)
namespace_default = "none"

# File database location
filedb_path = ".entangled/filedb.json"
"##;

/// Executes the init command. When `example` is set, also scaffolds a runnable
/// starter document (`hello.md`).
pub fn init(base_dir: &Path, example: bool) -> Result<()> {
    let config_path = base_dir.join("entangled.toml");

    if config_path.exists() {
        return Err(EntangledError::Config(format!(
            "{} already exists",
            config_path.display()
        )));
    }

    let config = if example {
        EXAMPLE_CONFIG
    } else {
        DEFAULT_CONFIG
    };
    std::fs::write(&config_path, config)?;
    println!("Created {}", config_path.display());

    // Create .entangled directory
    let db_dir = base_dir.join(".entangled");
    if !db_dir.exists() {
        std::fs::create_dir_all(&db_dir)?;
        println!("Created {}/", db_dir.display());
    }

    // Add .entangled/ to .gitignore if not already present
    ensure_gitignore(base_dir);

    if example {
        let doc_path = base_dir.join("hello.md");
        if doc_path.exists() {
            return Err(EntangledError::Config(format!(
                "{} already exists",
                doc_path.display()
            )));
        }
        std::fs::write(&doc_path, EXAMPLE_DOC)?;
        println!("Created {}", doc_path.display());
        println!();
        println!("Next steps:");
        println!("  entangled tangle          # write hello.py");
        println!("  entangled eval            # run the demo block");
        println!("  entangled weave -o hello.html   # render the document");
    }

    Ok(())
}

/// Ensures `.entangled/` is listed in `.gitignore`.
fn ensure_gitignore(base_dir: &Path) {
    let gitignore_path = base_dir.join(".gitignore");
    let entry = ".entangled/";

    if gitignore_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&gitignore_path) {
            if content.lines().any(|line| line.trim() == entry) {
                return; // Already present
            }
            // Append to existing .gitignore
            let suffix = if content.ends_with('\n') { "" } else { "\n" };
            if std::fs::write(&gitignore_path, format!("{}{}{}\n", content, suffix, entry)).is_ok()
            {
                println!("Added {} to {}", entry, gitignore_path.display());
            }
        }
    } else {
        // Create new .gitignore
        if std::fs::write(&gitignore_path, format!("{}\n", entry)).is_ok() {
            println!("Created {} with {}", gitignore_path.display(), entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_init_creates_config() {
        let dir = tempdir().unwrap();
        init(dir.path(), false).unwrap();

        let config_path = dir.path().join("entangled.toml");
        assert!(config_path.exists());

        let content = std::fs::read_to_string(config_path).unwrap();
        assert!(content.contains("version = \"2.0\""));
        assert!(content.contains("source_patterns"));
    }

    #[test]
    fn test_init_fails_if_exists() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("entangled.toml"), "existing").unwrap();

        let result = init(dir.path(), false);
        assert!(result.is_err());
    }

    #[test]
    fn test_init_creates_entangled_dir() {
        let dir = tempdir().unwrap();
        init(dir.path(), false).unwrap();

        assert!(dir.path().join(".entangled").is_dir());
    }

    #[test]
    fn test_init_creates_gitignore() {
        let dir = tempdir().unwrap();
        init(dir.path(), false).unwrap();

        let gitignore = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(gitignore.contains(".entangled/"));
    }

    #[test]
    fn test_init_appends_to_existing_gitignore() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
        init(dir.path(), false).unwrap();

        let gitignore = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(gitignore.contains("target/"));
        assert!(gitignore.contains(".entangled/"));
    }

    #[test]
    fn test_init_example_scaffolds_runnable_doc() {
        let dir = tempdir().unwrap();
        init(dir.path(), true).unwrap();

        let doc = std::fs::read_to_string(dir.path().join("hello.md")).unwrap();
        assert!(doc.contains("#main file=hello.py"));
        assert!(doc.contains("<<imports>>"));
        assert!(doc.contains("eval=python"));

        // Example config uses namespace_default = none so references resolve.
        let config = std::fs::read_to_string(dir.path().join("entangled.toml")).unwrap();
        assert!(config.contains("namespace_default = \"none\""));
    }

    #[test]
    fn test_init_skips_duplicate_gitignore_entry() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), ".entangled/\n").unwrap();
        init(dir.path(), false).unwrap();

        let gitignore = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(gitignore.matches(".entangled/").count(), 1);
    }
}
