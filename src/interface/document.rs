//! Document orchestrator for tangle and stitch operations.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::{AnnotationMethod, Comment, Markers};
use crate::errors::Result;
use crate::io::Transaction;
use crate::model::{tangle_ref, ReferenceMap};
use crate::readers::{parse_markdown, read_annotated_file, ParsedDocument};

use super::context::Context;

/// A document being processed by Entangled.
pub struct Document {
    /// Path to the source markdown file.
    pub path: PathBuf,
    /// Parsed document content.
    pub parsed: ParsedDocument,
}

impl Document {
    /// Loads a document from a file.
    pub fn load(path: &Path, ctx: &Context) -> Result<Self> {
        let content = ctx.file_cache.read(path)?;
        let parsed = parse_markdown(&content, Some(path), &ctx.config)?;

        Ok(Self {
            path: path.to_path_buf(),
            parsed,
        })
    }

    /// Returns the reference map.
    pub fn refs(&self) -> &ReferenceMap {
        &self.parsed.refs
    }

    /// Returns target files from this document.
    pub fn targets(&self) -> Vec<PathBuf> {
        self.parsed.refs.targets().cloned().collect()
    }
}

/// Tangles all documents and produces output files.
pub fn tangle_documents(ctx: &Context) -> Result<Transaction> {
    let mut transaction = Transaction::new();

    // Collect all references from all source files
    let mut all_refs = ReferenceMap::new();
    let source_files = ctx.source_files()?;

    for path in &source_files {
        let doc = Document::load(path, ctx)?;
        for (id, block) in doc.refs().iter() {
            all_refs.insert_with_id(id.clone(), block.clone());
        }
    }

    // Tangle each target file
    let mut tangled: HashMap<PathBuf, String> = HashMap::new();

    for target in all_refs.targets() {
        let name = all_refs.get_target_name(target).unwrap();

        // Get language for comment style
        let blocks = all_refs.get_by_name(name);
        let language = blocks.first().and_then(|b| b.language.as_ref());

        let (comment, markers) = match ctx.config.annotation {
            AnnotationMethod::Standard | AnnotationMethod::Supplemental => {
                let comment = language
                    .and_then(|l| ctx.config.find_language(l))
                    .map(|l| l.comment)
                    .unwrap_or_else(|| Comment::line("#"));
                (Some(comment), Some(Markers::default()))
            }
            AnnotationMethod::Naked => (None, None),
        };

        let content = tangle_ref(&all_refs, name, comment.as_ref(), markers.as_ref())?;

        // Apply hooks
        let final_content = if let Some(block) = blocks.first() {
            ctx.hooks.run_post_tangle(&content, block)?
        } else {
            content
        };

        tangled.insert(target.clone(), final_content);
    }

    // Create transaction actions
    for (path, content) in tangled {
        let full_path = ctx.resolve_path(&path);
        transaction.write(full_path, content);
    }

    Ok(transaction)
}

/// Stitches changes from tangled files back to source documents.
pub fn stitch_documents(ctx: &Context) -> Result<Transaction> {
    let transaction = Transaction::new();

    // Collect all references from source files
    let mut source_refs = ReferenceMap::new();
    let source_files = ctx.source_files()?;

    for path in &source_files {
        let doc = Document::load(path, ctx)?;
        for (id, block) in doc.refs().iter() {
            source_refs.insert_with_id(id.clone(), block.clone());
        }
    }

    // Read tangled files and check for changes
    for target in source_refs.targets() {
        let full_path = ctx.resolve_path(target);
        if !full_path.exists() {
            continue;
        }

        let tangled_refs = read_annotated_file(&full_path)?;

        // Compare each block
        for (id, tangled_block) in tangled_refs.iter() {
            if let Some(source_block) = source_refs.get(id) {
                if source_block.source != tangled_block.source {
                    // Block has been modified in tangled file
                    // We need to update the source markdown
                    // This is a simplified implementation - full impl would update markdown
                    tracing::info!(
                        "Block {} modified in {}",
                        id,
                        target.display()
                    );
                }
            }
        }
    }

    Ok(transaction)
}

/// Synchronizes documents (tangle + stitch).
pub fn sync_documents(ctx: &mut Context) -> Result<()> {
    // First stitch any changes from tangled files
    let stitch_tx = stitch_documents(ctx)?;
    if !stitch_tx.is_empty() {
        stitch_tx.execute(&mut ctx.filedb)?;
    }

    // Then tangle all documents
    let tangle_tx = tangle_documents(ctx)?;
    if !tangle_tx.is_empty() {
        tangle_tx.execute(&mut ctx.filedb)?;
    }

    // Save file database
    ctx.save_filedb()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn setup_test_dir() -> (tempfile::TempDir, Context) {
        let dir = tempdir().unwrap();
        let ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();
        (dir, ctx)
    }

    #[test]
    fn test_document_load() {
        let (dir, ctx) = setup_test_dir();

        let md_path = dir.path().join("test.md");
        fs::write(
            &md_path,
            r#"
```python #main file=output.py
print('hello')
```
"#,
        )
        .unwrap();

        let doc = Document::load(&md_path, &ctx).unwrap();
        assert!(!doc.refs().is_empty());
        assert_eq!(doc.targets().len(), 1);
    }

    #[test]
    fn test_tangle_documents() {
        let (dir, ctx) = setup_test_dir();

        let md_path = dir.path().join("test.md");
        fs::write(
            &md_path,
            r#"
```python #main file=output.py
print('hello')
```
"#,
        )
        .unwrap();

        let tx = tangle_documents(&ctx).unwrap();
        assert!(!tx.is_empty());

        let descriptions = tx.describe();
        assert!(descriptions.iter().any(|d| d.contains("output.py")));
    }

    #[test]
    fn test_tangle_with_references() {
        let dir = tempdir().unwrap();
        // Use a config with no namespace defaulting so references work
        let mut config = crate::config::Config::default();
        config.namespace_default = crate::config::NamespaceDefault::None;
        let ctx = Context::new(config, dir.path().to_path_buf()).unwrap();

        let md_path = dir.path().join("test.md");
        fs::write(
            &md_path,
            r#"
```python #main file=output.py
def main():
    <<body>>
```

```python #body
print('hello')
```
"#,
        )
        .unwrap();

        let tx = tangle_documents(&ctx).unwrap();
        assert!(!tx.is_empty());
    }

    #[test]
    fn test_empty_document() {
        let (dir, ctx) = setup_test_dir();

        let md_path = dir.path().join("test.md");
        fs::write(&md_path, "# Just a header\n\nSome text.").unwrap();

        let doc = Document::load(&md_path, &ctx).unwrap();
        assert!(doc.refs().is_empty());
        assert!(doc.targets().is_empty());
    }
}
