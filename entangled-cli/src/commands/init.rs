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

/// Executes the init command.
pub fn init(base_dir: &Path) -> Result<()> {
    let config_path = base_dir.join("entangled.toml");

    if config_path.exists() {
        return Err(EntangledError::Config(format!(
            "{} already exists",
            config_path.display()
        )));
    }

    std::fs::write(&config_path, DEFAULT_CONFIG)?;
    println!("Created {}", config_path.display());

    // Create .entangled directory
    let db_dir = base_dir.join(".entangled");
    if !db_dir.exists() {
        std::fs::create_dir_all(&db_dir)?;
        println!("Created {}/", db_dir.display());
    }

    // Add .entangled/ to .gitignore if not already present
    ensure_gitignore(base_dir);

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
        init(dir.path()).unwrap();

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

        let result = init(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_init_creates_entangled_dir() {
        let dir = tempdir().unwrap();
        init(dir.path()).unwrap();

        assert!(dir.path().join(".entangled").is_dir());
    }

    #[test]
    fn test_init_creates_gitignore() {
        let dir = tempdir().unwrap();
        init(dir.path()).unwrap();

        let gitignore = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(gitignore.contains(".entangled/"));
    }

    #[test]
    fn test_init_appends_to_existing_gitignore() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
        init(dir.path()).unwrap();

        let gitignore = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(gitignore.contains("target/"));
        assert!(gitignore.contains(".entangled/"));
    }

    #[test]
    fn test_init_skips_duplicate_gitignore_entry() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), ".entangled/\n").unwrap();
        init(dir.path()).unwrap();

        let gitignore = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(gitignore.matches(".entangled/").count(), 1);
    }
}
