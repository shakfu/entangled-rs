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
    ///
    /// The path is made project-relative before parsing so that the default
    /// file namespace (and therefore every block name) is identical whether the
    /// caller passed a relative or an absolute path.
    pub fn load(path: &Path, ctx: &Context) -> Result<Self> {
        let content = ctx.file_cache.read(path)?;
        let relative = path.strip_prefix(&ctx.base_dir).unwrap_or(path);
        let parsed = parse_markdown(&content, Some(relative), &ctx.config)?;

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

/// Location of a code block's content lines within the original markdown file.
#[derive(Debug, Clone)]
pub struct BlockLocation {
    /// The source document the block was parsed from.
    pub source_path: PathBuf,
    /// First line of content (after opening fence), 1-indexed in the original file.
    pub content_start: usize,
    /// Last line of content (before closing fence), 1-indexed in the original file.
    pub content_end: usize,
}

/// One validated, project-wide view of every code block in the project.
///
/// Tangle, stitch, locate, check, graph and eval all work from this so they
/// cannot disagree about block identity, names or target ownership.
#[derive(Debug)]
pub struct ProjectAnalysis {
    /// Every block in the project, under project-wide unique IDs.
    pub refs: ReferenceMap,
    /// Where each block's content lives in its source document.
    pub locations: HashMap<ReferenceId, BlockLocation>,
}

impl ProjectAnalysis {
    /// Returns an error naming every output file claimed by more than one
    /// distinct block name.
    ///
    /// Tangling a collided target can only keep one owner, silently discarding
    /// the other blocks' code, so the whole operation is refused before any
    /// file is touched.
    pub fn ensure_no_target_collisions(&self) -> Result<()> {
        let collisions = self.refs.target_collisions();
        if collisions.is_empty() {
            return Ok(());
        }

        let detail = collisions
            .iter()
            .map(|(target, owners)| {
                let names = owners
                    .iter()
                    .map(|n| format!("`{n}`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("  {} is claimed by {}", target.display(), names)
            })
            .collect::<Vec<_>>()
            .join("\n");

        Err(crate::errors::EntangledError::Other(format!(
            "refusing to tangle: {} output file(s) are claimed by more than one code block, \
             so tangling would discard code:\n{}\n\
             Give each output file a single block name (continuation blocks may reuse the name).",
            collisions.len(),
            detail
        )))
    }
}

/// Parses every source file in the project into one reference map with
/// project-wide unique IDs.
///
/// IDs are assigned during combination rather than reused from each document:
/// per-document IDs restart at zero in every file, so a `#part` block in `a.md`
/// and in `b.md` would both be `part[0]` and collide. Because the map is always
/// built over the *whole* project (never a caller-supplied subset), the IDs a
/// block gets -- and therefore the annotations written into tangled output --
/// do not change when the user tangles or stitches only some files.
pub fn analyze_project(ctx: &Context) -> Result<ProjectAnalysis> {
    let mut refs = ReferenceMap::new();
    let mut locations = HashMap::new();

    for path in &ctx.source_files()? {
        let raw_content = ctx.file_cache.read(path)?;

        // Line numbers from parse_markdown are relative to the content after
        // YAML header stripping.
        let (yaml_header, _) = split_yaml_header(&raw_content);
        let yaml_offset = yaml_header.map(|h| h.lines_consumed).unwrap_or(0);

        let doc = Document::load(path, ctx)?;

        for (_, block) in doc.refs().iter_arcs() {
            let fence_line = block.location.line + yaml_offset;
            let line_count = block.source.lines().count();
            let id = refs.insert_arc(Arc::clone(block));
            locations.insert(
                id,
                BlockLocation {
                    source_path: path.clone(),
                    content_start: fence_line + 1,
                    // An empty block gives content_end < content_start: no lines to replace.
                    content_end: fence_line + line_count,
                },
            );
        }
    }

    Ok(ProjectAnalysis { refs, locations })
}

/// Builds a single reference map combining the code blocks of every given
/// source file, with project-wide unique IDs.
///
/// This is the shared basis for whole-project analysis (tangle, eval, check,
/// graph): references can resolve across files, and every block is visible.
///
/// Note that `source_files` selects which documents are parsed; for stable
/// block identity, callers that write files should use [`analyze_project`],
/// which always covers the whole project.
pub fn combined_reference_map(ctx: &Context, source_files: &[PathBuf]) -> Result<ReferenceMap> {
    let mut all_refs = ReferenceMap::new();
    for path in source_files {
        let doc = Document::load(path, ctx)?;
        for (_, block) in doc.refs().iter_arcs() {
            all_refs.insert_arc(Arc::clone(block));
        }
    }
    Ok(all_refs)
}

/// Applies pre-tangle hooks to each target's first block.
///
/// Hooks such as `shebang` and `spdx_license` move a file header out of the
/// code block and to the top of the generated file. The stripping half has to
/// happen *before* reference expansion (otherwise the header stays embedded in
/// the annotated block), and the re-adding half exactly once per target
/// (otherwise every continuation block contributes another copy). So the header
/// is collected here, the block's source is replaced with the stripped version,
/// and the caller decides where the header goes.
///
/// Returns the header lines lifted out of each block, keyed by block ID.
fn strip_block_headers(
    ctx: &Context,
    refs: &mut ReferenceMap,
) -> Result<HashMap<ReferenceId, Vec<String>>> {
    let mut headers: HashMap<ReferenceId, Vec<String>> = HashMap::new();
    if ctx.hooks.is_empty() {
        return Ok(headers);
    }

    // A header may only appear at the very top of a file, so only a target's
    // first block can carry one.
    let first_blocks: Vec<ReferenceId> = refs
        .targets()
        .filter_map(|target| {
            let name = refs.get_target_name(target)?;
            refs.get_ids_by_name(name).first().copied().cloned()
        })
        .collect();

    for id in first_blocks {
        let Some(block) = refs.get(&id) else { continue };
        let Some(result) = ctx.hooks.run_pre_tangle(block)? else {
            continue;
        };

        if !result.header.is_empty() {
            headers.insert(id.clone(), result.header);
        }
        refs.replace_source(&id, result.source);
    }

    Ok(headers)
}

/// Tangles specific source files and produces output files.
///
/// Reference resolution and block identity are always project-wide;
/// `source_files` only selects which targets are written.
pub fn tangle_files(ctx: &Context, source_files: &[PathBuf]) -> Result<Transaction> {
    let analysis = analyze_project(ctx)?;

    // Refuse the whole operation on collision rather than silently dropping
    // one owner's code (which the transaction could not undo afterwards).
    analysis.ensure_no_target_collisions()?;

    let ProjectAnalysis {
        mut refs,
        locations,
    } = analysis;

    let selected: std::collections::HashSet<PathBuf> =
        source_files.iter().map(|p| ctx.resolve_path(p)).collect();

    let headers = strip_block_headers(ctx, &mut refs)?;

    let mut transaction = Transaction::new();

    for target in refs.targets() {
        let name = refs.get_target_name(target).ok_or_else(|| {
            crate::errors::EntangledError::Other(format!(
                "Internal error: target {} has no associated reference name",
                target.display()
            ))
        })?;

        let blocks = refs.get_by_name(name);

        // Only emit targets owned by one of the selected source files.
        let owned_by_selection = refs.get_ids_by_name(name).iter().any(|id| {
            locations
                .get(id)
                .is_some_and(|loc| selected.contains(&ctx.resolve_path(&loc.source_path)))
        });
        if !owned_by_selection {
            continue;
        }

        // Get language for comment style
        let language = blocks.first().and_then(|b| b.language.as_ref());

        let (comment, markers) = match ctx.config.annotation {
            AnnotationMethod::Standard | AnnotationMethod::Supplemental => {
                let comment = language
                    .and_then(|l| ctx.config.find_language(l))
                    .map(|l| l.comment)
                    .unwrap_or_else(|| Comment::line("#"));
                (Some(comment), Some(Markers::default()))
            }
            AnnotationMethod::Bare => (None, Some(Markers::default())),
            AnnotationMethod::Naked => (None, None),
        };

        let content = tangle_ref(&refs, name, comment.as_ref(), markers.as_ref())?;

        // Re-add the file header (shebang, SPDX) exactly once, above the
        // annotations, then let any remaining hooks post-process.
        let content = match refs
            .get_ids_by_name(name)
            .first()
            .and_then(|id| headers.get(*id))
        {
            Some(header) => format!("{}\n{}", header.join("\n"), content),
            None => content,
        };
        let final_content = match blocks.first() {
            Some(block) => ctx.hooks.run_post_tangle(&content, block)?,
            None => content,
        };

        transaction.write(ctx.resolve_target(target)?, final_content);
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

/// Stitches specific source files.
///
/// For each source file, parses code blocks and their locations, then compares
/// with the annotated tangled output. Modified blocks produce write actions
/// that update the markdown source.
pub fn stitch_files(ctx: &Context, source_files: &[PathBuf]) -> Result<Transaction> {
    let mut transaction = Transaction::new();

    // Only annotated output carries the markers stitch reads back.
    if ctx.config.annotation.is_one_way() {
        return Ok(transaction);
    }

    let ProjectAnalysis {
        refs: mut source_refs,
        locations: block_locations,
    } = analyze_project(ctx)?;

    // Compare against the same source tangle produced its output from: a block
    // whose shebang was hoisted into the file header no longer contains it, so
    // comparing against the raw markdown source would report every such block
    // as modified and then strip the header out of the document.
    let headers = strip_block_headers(ctx, &mut source_refs)?;

    let selected: std::collections::HashSet<PathBuf> =
        source_files.iter().map(|p| ctx.resolve_path(p)).collect();

    // Read tangled files and find modified blocks
    // Group changes by source file for batch application
    let mut changes_by_file: HashMap<PathBuf, Vec<(usize, usize, String)>> = HashMap::new();

    for target in source_refs.targets() {
        let full_path = ctx.resolve_target(target)?;
        if !full_path.exists() {
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
                        // Respect the caller's file selection.
                        if !selected.contains(&ctx.resolve_path(&loc.source_path)) {
                            continue;
                        }
                        tracing::info!(
                            "Block {} modified in {}, updating {}",
                            id,
                            target.display(),
                            loc.source_path.display(),
                        );
                        // Put the hoisted header back so the markdown keeps
                        // owning it and the next tangle can hoist it again.
                        let replacement = match headers.get(id) {
                            Some(header) => {
                                format!("{}\n{}", header.join("\n"), tangled_block.source)
                            }
                            None => tangled_block.source.clone(),
                        };
                        changes_by_file
                            .entry(loc.source_path.clone())
                            .or_default()
                            .push((loc.content_start, loc.content_end, replacement));
                    }
                }
            }
        }
    }

    // Apply changes to each markdown file
    for (path, mut changes) in changes_by_file {
        let content = ctx.file_cache.read(&path)?;
        let newline = dominant_newline(&content);
        let lines: Vec<&str> = content.lines().collect();

        // Sort by start line descending -- apply from bottom to top
        // so earlier line numbers remain valid after splicing
        changes.sort_by_key(|c| std::cmp::Reverse(c.0));

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

        // Rejoin with the file's own newline convention: rewriting a CRLF
        // document with LF would show up as a whole-file diff.
        let mut new_file_content = new_lines.join(newline);
        if content.ends_with('\n') {
            new_file_content.push_str(newline);
        }

        let full_path = ctx.resolve_path(&path);
        transaction.write(full_path, new_file_content);
    }

    Ok(transaction)
}

/// Returns the newline sequence a document predominantly uses.
fn dominant_newline(content: &str) -> &'static str {
    let crlf = content.matches("\r\n").count();
    let lf = content.matches('\n').count() - crlf;
    if crlf > lf {
        "\r\n"
    } else {
        "\n"
    }
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

    // Now find the markdown source location for this block. IDs in annotations
    // are project-wide, so they are looked up in the project-wide analysis --
    // the same model tangle used to write them.
    let analysis = analyze_project(ctx)?;
    if let Some(loc) = analysis.locations.get(&block_id) {
        return Ok(Some(SourceLocation {
            source_file: loc.source_path.clone(),
            source_line: loc.content_start + result_offset,
            block_id,
        }));
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
