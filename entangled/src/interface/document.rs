//! Document orchestrator for tangle and stitch operations.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::config::{AnnotationMethod, Comment, Markers, REF_PATTERN};
use crate::errors::Result;
use crate::io::Transaction;
use crate::model::{tangle_ref, ReferenceId, ReferenceMap};
use crate::readers::{parse_markdown, read_annotated_file, split_yaml_header, ParsedDocument};

use super::context::Context;

/// A document being processed by Entangled.
#[derive(Debug, Clone)]
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
    let source_files = ctx.source_files()?;
    tangle_files(ctx, &source_files)
}

/// Tangles specific source files and produces output files.
pub fn tangle_files(ctx: &Context, source_files: &[PathBuf]) -> Result<Transaction> {
    let mut transaction = Transaction::new();

    // Collect all references from all source files
    let mut all_refs = ReferenceMap::new();

    for path in source_files {
        let doc = Document::load(path, ctx)?;
        for (id, block) in doc.refs().iter_arcs() {
            all_refs.insert_arc_with_id(id.clone(), Arc::clone(block));
        }
    }

    // Tangle each target file
    let mut tangled: HashMap<PathBuf, String> = HashMap::new();

    for target in all_refs.targets() {
        let name = all_refs.get_target_name(target).ok_or_else(|| {
            crate::errors::EntangledError::Other(format!(
                "Internal error: target {} has no associated reference name",
                target.display()
            ))
        })?;

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
///
/// Reads annotated tangled output files, compares each code block with the
/// corresponding source block in the markdown, and produces write actions
/// to update the markdown with any changes made in the tangled files.
pub fn stitch_documents(ctx: &Context) -> Result<Transaction> {
    let source_files = ctx.source_files()?;
    stitch_files(ctx, &source_files)
}

/// Location of a code block's content lines within the original markdown file.
struct BlockLocation {
    source_path: PathBuf,
    /// First line of content (after opening fence), 1-indexed in the original file.
    content_start: usize,
    /// Last line of content (before closing fence), 1-indexed in the original file.
    content_end: usize,
}

/// Stitches specific source files.
///
/// For each source file, parses code blocks and their locations, then compares
/// with the annotated tangled output. Modified blocks produce write actions
/// that update the markdown source.
pub fn stitch_files(ctx: &Context, source_files: &[PathBuf]) -> Result<Transaction> {
    let mut transaction = Transaction::new();

    // Collect all references from source files, tracking block locations
    let mut source_refs = ReferenceMap::new();
    let mut block_locations: HashMap<ReferenceId, BlockLocation> = HashMap::new();

    for path in source_files {
        let raw_content = ctx.file_cache.read(path)?;

        // Compute YAML header offset: line numbers from parse_markdown are
        // relative to content after YAML header stripping
        let (yaml_header, _) = split_yaml_header(&raw_content);
        let yaml_offset = yaml_header.map(|h| h.lines_consumed).unwrap_or(0);

        let doc = Document::load(path, ctx)?;

        for (id, block) in doc.refs().iter_arcs() {
            // Correct line number for the YAML header offset
            let actual_fence_line = block.location.line + yaml_offset;
            let line_count = block.source.lines().count();
            let content_start = actual_fence_line + 1;
            // If source is empty, content_end < content_start (no lines to replace)
            let content_end = actual_fence_line + line_count;

            block_locations.insert(
                id.clone(),
                BlockLocation {
                    source_path: path.clone(),
                    content_start,
                    content_end,
                },
            );
            source_refs.insert_arc_with_id(id.clone(), Arc::clone(block));
        }
    }

    // Read tangled files and find modified blocks
    // Group changes by source file for batch application
    let mut changes_by_file: HashMap<PathBuf, Vec<(usize, usize, String)>> = HashMap::new();

    for target in source_refs.targets() {
        let full_path = ctx.resolve_path(target);
        if !full_path.exists() {
            continue;
        }

        // Only stitch from annotated files (naked mode has no annotations)
        if ctx.config.annotation == AnnotationMethod::Naked {
            continue;
        }

        let tangled_refs = read_annotated_file(&full_path)?;

        for (id, tangled_block) in tangled_refs.iter() {
            if let Some(source_block) = source_refs.get(id) {
                // Skip blocks containing <<reference>> patterns -- these are
                // expanded during tangle so their tangled content will differ
                // from source. Only leaf blocks can be meaningfully stitched.
                // REF_PATTERN uses ^/$ anchors, so check each line
                let has_refs = source_block
                    .source
                    .lines()
                    .any(|line| REF_PATTERN.is_match(line));
                if has_refs {
                    continue;
                }

                if source_block.source != tangled_block.source {
                    if let Some(loc) = block_locations.get(id) {
                        tracing::info!(
                            "Block {} modified in {}, updating {}",
                            id,
                            target.display(),
                            loc.source_path.display(),
                        );
                        changes_by_file
                            .entry(loc.source_path.clone())
                            .or_default()
                            .push((
                                loc.content_start,
                                loc.content_end,
                                tangled_block.source.clone(),
                            ));
                    }
                }
            }
        }
    }

    // Apply changes to each markdown file
    for (path, mut changes) in changes_by_file {
        let content = ctx.file_cache.read(&path)?;
        let lines: Vec<&str> = content.lines().collect();

        // Sort by start line descending -- apply from bottom to top
        // so earlier line numbers remain valid after splicing
        changes.sort_by(|a, b| b.0.cmp(&a.0));

        let mut new_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();

        for (content_start, content_end, new_content) in &changes {
            let start_idx = content_start - 1; // 1-indexed to 0-indexed
            let end_idx = *content_end; // 1-indexed inclusive -> 0-indexed exclusive

            let replacement: Vec<String> = if new_content.is_empty() {
                Vec::new()
            } else {
                new_content.lines().map(|l| l.to_string()).collect()
            };

            new_lines.splice(start_idx..end_idx, replacement);
        }

        let mut new_file_content = new_lines.join("\n");
        if content.ends_with('\n') {
            new_file_content.push('\n');
        }

        let full_path = ctx.resolve_path(&path);
        transaction.write(full_path, new_file_content);
    }

    Ok(transaction)
}

/// Result of locating a source position from a tangled file position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    /// Path to the markdown source file.
    pub source_file: PathBuf,
    /// Line number in the markdown source (1-indexed).
    pub source_line: usize,
    /// The reference ID of the containing block.
    pub block_id: ReferenceId,
}

impl std::fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.source_file.display(), self.source_line)
    }
}

/// Maps a line in a tangled output file back to its markdown source location.
///
/// Given a tangled file path and a line number within it, reads the annotation
/// markers to determine which code block the line belongs to, then looks up
/// that block's position in the markdown source.
///
/// Returns `None` if the line is an annotation marker or the file has no annotations.
pub fn locate_source(
    ctx: &Context,
    target_file: &Path,
    target_line: usize,
) -> Result<Option<SourceLocation>> {
    use once_cell::sync::Lazy;
    use regex::Regex;

    static BEGIN_PAT: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^\s*\S+\s+~/~\s+begin\s+<<(?P<ref>[^>]+)>>").unwrap());
    static END_PAT: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\s*\S+\s+~/~\s+end\s*$").unwrap());

    // Read the tangled file
    let tangled_content = std::fs::read_to_string(target_file)?;

    // Walk the file tracking annotation context
    // For each content line, track (block_id, offset_within_block)
    let mut stack: Vec<(ReferenceId, usize)> = Vec::new(); // (id, content_line_count)
    let mut result_id: Option<ReferenceId> = None;
    let mut result_offset: usize = 0;

    for (line_idx, line) in tangled_content.lines().enumerate() {
        let line_number = line_idx + 1;

        if let Some(caps) = BEGIN_PAT.captures(line) {
            if line_number == target_line {
                return Ok(None); // Target is an annotation marker
            }
            let ref_str = &caps["ref"];
            if let Some(id) = ReferenceId::parse(ref_str) {
                stack.push((id, 0));
            }
        } else if END_PAT.is_match(line) {
            if line_number == target_line {
                return Ok(None); // Target is an annotation marker
            }
            stack.pop();
        } else if let Some((_id, ref mut count)) = stack.last_mut() {
            if line_number == target_line {
                result_id = Some(_id.clone());
                result_offset = *count;
                break;
            }
            *count += 1;
        } else if line_number == target_line {
            // Line is outside any annotated block
            return Ok(None);
        }
    }

    let block_id = match result_id {
        Some(id) => id,
        None => return Ok(None),
    };

    // Now find the markdown source location for this block
    let source_files = ctx.source_files()?;
    for path in &source_files {
        let raw_content = ctx.file_cache.read(path)?;
        let (yaml_header, _) = split_yaml_header(&raw_content);
        let yaml_offset = yaml_header.map(|h| h.lines_consumed).unwrap_or(0);

        let doc = Document::load(path, ctx)?;
        if let Some(block) = doc.refs().get(&block_id) {
            // block.location.line is relative to post-YAML content
            let fence_line = block.location.line + yaml_offset;
            // Content starts on the line after the fence
            let source_line = fence_line + 1 + result_offset;

            return Ok(Some(SourceLocation {
                source_file: path.clone(),
                source_line,
                block_id,
            }));
        }
    }

    // Block ID not found in any source file
    Ok(None)
}

/// Synchronizes documents (stitch then tangle).
///
/// When `force` is true, file conflict checks are skipped.
pub fn sync_documents(ctx: &mut Context, force: bool) -> Result<()> {
    // First stitch any changes from tangled files
    let stitch_tx = stitch_documents(ctx)?;
    if !stitch_tx.is_empty() {
        if force {
            stitch_tx.execute_force(&mut ctx.filedb)?;
        } else {
            stitch_tx.execute(&mut ctx.filedb)?;
        }
    }

    // Then tangle all documents
    let tangle_tx = tangle_documents(ctx)?;
    if !tangle_tx.is_empty() {
        if force {
            tangle_tx.execute_force(&mut ctx.filedb)?;
        } else {
            tangle_tx.execute(&mut ctx.filedb)?;
        }
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

    #[test]
    fn test_stitch_detects_no_changes() {
        let (dir, mut ctx) = setup_test_dir();

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

        // Tangle first to create the output file
        let tangle_tx = tangle_documents(&ctx).unwrap();
        tangle_tx.execute(&mut ctx.filedb).unwrap();

        // Stitch should find no changes
        let stitch_tx = stitch_documents(&ctx).unwrap();
        assert!(
            stitch_tx.is_empty(),
            "Expected no changes after fresh tangle"
        );
    }

    #[test]
    fn test_stitch_detects_modification() {
        let (dir, mut ctx) = setup_test_dir();

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

        // Tangle first
        let tangle_tx = tangle_documents(&ctx).unwrap();
        assert!(!tangle_tx.is_empty(), "Tangle should produce actions");
        tangle_tx.execute(&mut ctx.filedb).unwrap();

        // Modify the tangled file
        let output_path = dir.path().join("output.py");
        assert!(output_path.exists(), "output.py should exist after tangle");
        let tangled_content = fs::read_to_string(&output_path).unwrap();
        let modified = tangled_content.replace("print('hello')", "print('world')");
        fs::write(&output_path, modified).unwrap();

        // Stitch should detect the change and produce a write action
        let stitch_tx = stitch_documents(&ctx).unwrap();
        assert!(
            !stitch_tx.is_empty(),
            "Expected stitch to detect modification"
        );

        // Execute the stitch
        stitch_tx.execute_force(&mut ctx.filedb).unwrap();

        // Verify the markdown was updated
        let updated_md = fs::read_to_string(&md_path).unwrap();
        assert!(
            updated_md.contains("print('world')"),
            "Markdown should contain modified code. Got:\n{}",
            updated_md
        );
        assert!(
            !updated_md.contains("print('hello')"),
            "Markdown should not contain original code. Got:\n{}",
            updated_md
        );
    }

    #[test]
    fn test_stitch_preserves_markdown_structure() {
        let (dir, mut ctx) = setup_test_dir();

        let md_path = dir.path().join("test.md");
        fs::write(
            &md_path,
            r#"# My Document

Some description.

```python #main file=output.py
print('hello')
```

More text after the code block.
"#,
        )
        .unwrap();

        // Tangle
        let tangle_tx = tangle_documents(&ctx).unwrap();
        tangle_tx.execute(&mut ctx.filedb).unwrap();

        // Modify tangled file
        let output_path = dir.path().join("output.py");
        let tangled_content = fs::read_to_string(&output_path).unwrap();
        let modified = tangled_content.replace("print('hello')", "print('world')");
        fs::write(&output_path, modified).unwrap();

        // Stitch
        let stitch_tx = stitch_documents(&ctx).unwrap();
        stitch_tx.execute_force(&mut ctx.filedb).unwrap();

        let updated_md = fs::read_to_string(&md_path).unwrap();
        assert!(updated_md.contains("# My Document"));
        assert!(updated_md.contains("Some description."));
        assert!(updated_md.contains("```python #main file=output.py"));
        assert!(updated_md.contains("print('world')"));
        assert!(updated_md.contains("More text after the code block."));
    }

    #[test]
    fn test_stitch_with_yaml_frontmatter() {
        let (dir, mut ctx) = setup_test_dir();

        let md_path = dir.path().join("test.md");
        fs::write(
            &md_path,
            "---\ntitle: Test\n---\n\n```python #main file=output.py\noriginal_code()\n```\n",
        )
        .unwrap();

        // Tangle
        let tangle_tx = tangle_documents(&ctx).unwrap();
        tangle_tx.execute(&mut ctx.filedb).unwrap();

        // Modify tangled file
        let output_path = dir.path().join("output.py");
        let tangled_content = fs::read_to_string(&output_path).unwrap();
        let modified = tangled_content.replace("original_code()", "modified_code()");
        fs::write(&output_path, modified).unwrap();

        // Stitch
        let stitch_tx = stitch_documents(&ctx).unwrap();
        assert!(!stitch_tx.is_empty());
        stitch_tx.execute_force(&mut ctx.filedb).unwrap();

        let updated_md = fs::read_to_string(&md_path).unwrap();
        assert!(
            updated_md.contains("---\ntitle: Test\n---"),
            "YAML frontmatter should be preserved. Got:\n{}",
            updated_md
        );
        assert!(
            updated_md.contains("modified_code()"),
            "Modified code should be present. Got:\n{}",
            updated_md
        );
    }

    #[test]
    fn test_stitch_multiple_blocks() {
        let dir = tempdir().unwrap();
        let mut config = crate::config::Config::default();
        config.namespace_default = crate::config::NamespaceDefault::None;
        let mut ctx = Context::new(config, dir.path().to_path_buf()).unwrap();

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

        // Tangle
        let tangle_tx = tangle_documents(&ctx).unwrap();
        tangle_tx.execute(&mut ctx.filedb).unwrap();

        // Modify the body block in the tangled file
        let output_path = dir.path().join("output.py");
        let tangled_content = fs::read_to_string(&output_path).unwrap();
        let modified = tangled_content.replace("print('hello')", "print('goodbye')");
        fs::write(&output_path, modified).unwrap();

        // Stitch
        let stitch_tx = stitch_documents(&ctx).unwrap();
        assert!(!stitch_tx.is_empty());
        stitch_tx.execute_force(&mut ctx.filedb).unwrap();

        let updated_md = fs::read_to_string(&md_path).unwrap();
        // The main block should still have <<body>> reference
        assert!(
            updated_md.contains("<<body>>"),
            "Reference should be preserved. Got:\n{}",
            updated_md
        );
        // The body block should be updated
        assert!(
            updated_md.contains("print('goodbye')"),
            "Body block should be updated. Got:\n{}",
            updated_md
        );
    }

    #[test]
    fn test_stitch_naked_mode_skipped() {
        let dir = tempdir().unwrap();
        let mut config = crate::config::Config::default();
        config.annotation = crate::config::AnnotationMethod::Naked;
        let mut ctx = Context::new(config, dir.path().to_path_buf()).unwrap();

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

        // Tangle in naked mode (no annotations)
        let tangle_tx = tangle_documents(&ctx).unwrap();
        tangle_tx.execute(&mut ctx.filedb).unwrap();

        // Modify tangled file
        let output_path = dir.path().join("output.py");
        fs::write(&output_path, "print('world')\n").unwrap();

        // Stitch should produce no changes (can't parse naked files)
        let stitch_tx = stitch_documents(&ctx).unwrap();
        assert!(stitch_tx.is_empty(), "Stitch should skip naked-mode files");
    }
}
