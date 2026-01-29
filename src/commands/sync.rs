//! Sync command implementation.

use crate::errors::Result;
use crate::interface::{sync_documents, Context};

/// Options for the sync command.
#[derive(Debug, Clone, Default)]
pub struct SyncOptions {
    /// Force overwrite even if files have been modified externally.
    pub force: bool,
}

/// Executes the sync command.
pub fn sync(ctx: &mut Context, _options: SyncOptions) -> Result<()> {
    tracing::info!("Synchronizing documents...");

    sync_documents(ctx)?;

    println!("Synchronization complete.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_sync_basic() {
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

        let options = SyncOptions::default();
        sync(&mut ctx, options).unwrap();

        // Output should be created
        assert!(dir.path().join("output.py").exists());
    }
}
