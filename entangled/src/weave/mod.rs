//! Weave: render literate documents to human-readable output.
//!
//! Where [`tangle`](crate::model::tangle) produces machine-readable source files,
//! weave produces the *documentation* half of a literate program: a typeset
//! document that interleaves prose with the code blocks.
//!
//! The design is deliberately two-layered:
//!
//! 1. A **transform** ([`weave_document`]) turns an Entangled-flavored markdown
//!    document into a renderer-agnostic [`WovenDocument`]. This is where the
//!    literate-programming value lives: block captions, continuation markers,
//!    and cross-reference metadata (which block a `<<reference>>` points at, and
//!    which blocks a given block is *used by*).
//! 2. Pluggable **backends** render a [`WovenDocument`]. A self-contained HTML
//!    backend ([`WovenDocument::to_html`]) ships in-tree; a clean-markdown
//!    backend ([`WovenDocument::to_markdown`]) produces Pandoc/Quarto-ready
//!    output for every other target.
//!
//! Pointing Pandoc or Quarto at the *raw* Entangled markdown does not work: they
//! render `<<imports>>` as literal text and choke on `#name file=path`
//! attributes. The transform is what makes the downstream render meaningful.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::config::{Config, REF_PATTERN};
use crate::errors::Result;
use crate::model::{extract_quarto_options, Properties};
use crate::readers::{extract_all_tokens, parse_simple_yaml, split_yaml_header, ExtractResult};
use crate::style::Style;

mod highlight;
mod html;
mod markdown;

pub use html::HtmlOptions;

/// A single line of a code block, classified for weaving.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeLine {
    /// A literal line of code.
    Text(String),
    /// A `<<reference>>` line pointing at another block.
    Reference {
        /// Leading indentation preserved from the source line.
        indent: String,
        /// The referenced block name, resolved the way tangle resolves it.
        name: String,
        /// Where the referenced block is defined.
        scope: RefScope,
    },
}

/// Where a `<<reference>>` resolves to.
///
/// Weave renders one document at a time, so a reference to a block defined in
/// another source file cannot be linked -- but it is not broken either, and
/// reporting it as missing (which is what happened before this distinction
/// existed) is simply wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefScope {
    /// Defined in this document; the renderer can link to it.
    Local,
    /// Defined elsewhere in the project; correct, but not linkable from here.
    Project,
    /// Not defined anywhere the weaver was told about.
    Unknown,
}

/// Captured output of a runnable block, for reproducible-report rendering.
#[derive(Debug, Clone)]
pub struct BlockOutput {
    /// Captured standard output.
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
    /// Whether the block ran and exited successfully.
    pub success: bool,
}

/// A code block prepared for weaving.
#[derive(Debug, Clone)]
pub struct WeaveCodeBlock {
    /// Block name (id), if this is a named Entangled block.
    pub name: Option<String>,
    /// Target output file, if the block specifies one.
    pub target: Option<String>,
    /// Language identifier (e.g. `python`), if known.
    pub language: Option<String>,
    /// Source lines, classified as literal text or references.
    pub lines: Vec<CodeLine>,
    /// Occurrence index among blocks sharing this name (0-based).
    pub index: usize,
    /// Total number of blocks sharing this name.
    pub total: usize,
    /// Names of blocks that reference this block (sorted, de-duplicated).
    pub used_by: Vec<String>,
    /// Captured execution output, when the block was evaluated and this is its
    /// first occurrence (output is shown once per block name).
    pub output: Option<BlockOutput>,
}

impl WeaveCodeBlock {
    /// Returns true if this is a named/targeted Entangled block (as opposed to a
    /// plain fenced code block that carries no Entangled metadata).
    pub fn is_entangled(&self) -> bool {
        self.name.is_some() || self.target.is_some()
    }
}

/// An element of a woven document, in source order.
#[derive(Debug, Clone)]
pub enum WeaveElement {
    /// A run of raw markdown prose.
    Prose(String),
    /// A code block.
    Code(WeaveCodeBlock),
}

/// A document prepared for weaving: prose and code blocks in source order,
/// annotated with cross-reference metadata.
#[derive(Debug, Clone)]
pub struct WovenDocument {
    /// Document title (from a frontmatter `title:` field), if present.
    pub title: Option<String>,
    /// Raw YAML frontmatter content, if present.
    pub frontmatter: Option<String>,
    /// Elements in source order.
    pub elements: Vec<WeaveElement>,
}

/// Intermediate representation while classifying tokens.
enum Raw {
    Prose(String),
    Code {
        name: Option<String>,
        target: Option<String>,
        language: Option<String>,
        lines: Vec<CodeLine>,
    },
}

/// What weaving one document knows about the project around it.
#[derive(Debug, Default)]
pub struct WeaveContext<'a> {
    /// Captured execution output, keyed by block name.
    pub outputs: HashMap<String, BlockOutput>,
    /// Every block name defined anywhere in the project, so a reference to
    /// another document can be told apart from a genuinely dangling one.
    /// Empty means "only this document is known".
    pub project_names: std::collections::HashSet<&'a str>,
}

/// Parses an Entangled-flavored markdown document into a [`WovenDocument`].
///
/// The document style is detected from `source_path` (`.qmd` -> Quarto,
/// `.Rmd` -> knitr) falling back to `config.style`, matching the tangle path.
pub fn weave_document(
    input: &str,
    source_path: Option<&Path>,
    config: &Config,
) -> Result<WovenDocument> {
    weave_document_with_context(input, source_path, config, &WeaveContext::default())
}

/// Like [`weave_document`], but attaches captured execution output (keyed by
/// block name) to each runnable block's first occurrence, so weave backends can
/// render a reproducible-output panel beneath it.
pub fn weave_document_with_outputs(
    input: &str,
    source_path: Option<&Path>,
    config: &Config,
    outputs: &HashMap<String, BlockOutput>,
) -> Result<WovenDocument> {
    weave_document_with_context(
        input,
        source_path,
        config,
        &WeaveContext {
            outputs: outputs.clone(),
            ..Default::default()
        },
    )
}

/// Weaves one document with knowledge of the surrounding project.
pub fn weave_document_with_context(
    input: &str,
    source_path: Option<&Path>,
    config: &Config,
    weave_ctx: &WeaveContext<'_>,
) -> Result<WovenDocument> {
    let outputs = &weave_ctx.outputs;
    let doc_style = Style::for_document(source_path, config.style);

    // Block names must be the ones tangle uses, namespace included; otherwise
    // the names shown in the woven document -- and every anchor derived from
    // them -- do not correspond to the blocks the reader can reference.
    let namespace = source_path.and_then(|p| config.namespace_default.prefix_for(p));

    let (yaml_header, content) = split_yaml_header(input);
    let frontmatter = yaml_header.map(|h| h.content);
    let title = frontmatter
        .as_deref()
        .and_then(|fm| parse_simple_yaml(fm).get("title").cloned());

    // Pass 1: classify tokens into interleaved prose / code, parsing block metadata.
    let mut raws: Vec<Raw> = Vec::new();
    let mut prose_buf: Vec<String> = Vec::new();

    let flush_prose = |buf: &mut Vec<String>, raws: &mut Vec<Raw>| {
        if !buf.is_empty() {
            raws.push(Raw::Prose(buf.join("\n")));
            buf.clear();
        }
    };

    for result in extract_all_tokens(content) {
        match result {
            ExtractResult::NotDelimited(line) => prose_buf.push(line),
            ExtractResult::Token(token) => {
                flush_prose(&mut prose_buf, &mut raws);
                raws.push(classify_code(
                    &token.info,
                    &token.content,
                    config,
                    doc_style,
                    namespace.as_deref(),
                ));
            }
            ExtractResult::Unclosed { info, content, .. } => {
                // Best-effort: treat an unterminated fence as plain code.
                flush_prose(&mut prose_buf, &mut raws);
                raws.push(classify_code(
                    &info,
                    &content,
                    config,
                    doc_style,
                    namespace.as_deref(),
                ));
            }
        }
    }
    flush_prose(&mut prose_buf, &mut raws);

    // Pass 2: resolve reference names and compute cross-reference metadata.
    //
    // Resolution mirrors tangle: inside `a.md`, a bare `<<part>>` means
    // `a.md#part` when this document defines it, and otherwise stands for
    // whatever the project-wide name is.
    let local_names: HashSet<String> = raws
        .iter()
        .filter_map(|raw| match raw {
            Raw::Code { name: Some(n), .. } => Some(n.clone()),
            _ => None,
        })
        .collect();

    let resolve = |refname: &str| -> (String, RefScope) {
        if let Some(ns) = namespace.as_deref() {
            let qualified = format!("{ns}#{refname}");
            if local_names.contains(&qualified) {
                return (qualified, RefScope::Local);
            }
            if weave_ctx.project_names.contains(qualified.as_str()) {
                return (qualified, RefScope::Project);
            }
        }
        let scope = if local_names.contains(refname) {
            RefScope::Local
        } else if weave_ctx.project_names.contains(refname) {
            RefScope::Project
        } else {
            RefScope::Unknown
        };
        (refname.to_string(), scope)
    };

    for raw in &mut raws {
        if let Raw::Code { lines, .. } = raw {
            for line in lines.iter_mut() {
                if let CodeLine::Reference { name, scope, .. } = line {
                    let (resolved, resolved_scope) = resolve(name);
                    *name = resolved;
                    *scope = resolved_scope;
                }
            }
        }
    }

    let mut name_total: HashMap<String, usize> = HashMap::new();
    let mut used_by: HashMap<String, HashSet<String>> = HashMap::new();
    for raw in &raws {
        if let Raw::Code {
            name: Some(name),
            lines,
            ..
        } = raw
        {
            *name_total.entry(name.clone()).or_insert(0) += 1;
            for line in lines {
                if let CodeLine::Reference { name: refname, .. } = line {
                    if refname != name {
                        used_by
                            .entry(refname.clone())
                            .or_default()
                            .insert(name.clone());
                    }
                }
            }
        }
    }

    // Pass 3: assemble the final document, assigning per-name occurrence indices.
    let mut name_seen: HashMap<String, usize> = HashMap::new();
    let mut elements = Vec::with_capacity(raws.len());
    for raw in raws {
        match raw {
            Raw::Prose(text) => elements.push(WeaveElement::Prose(text)),
            Raw::Code {
                name,
                target,
                language,
                lines,
            } => {
                let (index, total) = match &name {
                    Some(n) => {
                        let idx = *name_seen.get(n).unwrap_or(&0);
                        name_seen.insert(n.clone(), idx + 1);
                        (idx, *name_total.get(n).unwrap_or(&1))
                    }
                    None => (0, 1),
                };
                let mut used = name
                    .as_ref()
                    .and_then(|n| used_by.get(n))
                    .map(|set| set.iter().cloned().collect::<Vec<_>>())
                    .unwrap_or_default();
                used.sort();
                // Attach captured output once, on the block's first occurrence.
                let output = if index == 0 {
                    name.as_ref().and_then(|n| outputs.get(n)).cloned()
                } else {
                    None
                };
                elements.push(WeaveElement::Code(WeaveCodeBlock {
                    name,
                    target,
                    language,
                    lines,
                    index,
                    total,
                    used_by: used,
                    output,
                }));
            }
        }
    }

    Ok(WovenDocument {
        title,
        frontmatter,
        elements,
    })
}

/// Classifies a fenced code token into a [`Raw::Code`], parsing Entangled
/// metadata according to the document style. Parse failures degrade gracefully
/// to a plain (unnamed) code block rather than aborting the weave.
fn classify_code(
    info: &str,
    content: &str,
    config: &Config,
    style: Style,
    namespace: Option<&str>,
) -> Raw {
    let (name, target, language, body) = parse_block_meta(info, content, config, style);
    let name = match (name, namespace) {
        (Some(name), Some(ns)) => Some(format!("{ns}#{name}")),
        (name, _) => name,
    };
    let lines = classify_lines(&body, namespace);
    Raw::Code {
        name,
        target,
        language,
        lines,
    }
}

/// Extracts (name, target, language, effective-body) from a fenced block.
fn parse_block_meta(
    info: &str,
    content: &str,
    config: &Config,
    style: Style,
) -> (Option<String>, Option<String>, Option<String>, String) {
    match style {
        Style::EntangledRs => meta_from_props(Properties::parse(info).ok(), content.to_string()),
        Style::Pandoc => meta_from_props(Properties::parse_pandoc(info).ok(), content.to_string()),
        Style::Knitr => meta_from_props(Properties::parse_knitr(info).ok(), content.to_string()),
        Style::Quarto => {
            let lang = Properties::parse_quarto_info(info)
                .ok()
                .and_then(|p| p.first_class().map(str::to_string));
            let (opts, remaining) = extract_quarto_options(content);
            let props = opts.to_properties(lang.as_deref());
            let body = if config.strip_quarto_options {
                remaining
            } else {
                content.to_string()
            };
            meta_from_props(Some(props), body)
        }
    }
}

fn meta_from_props(
    props: Option<Properties>,
    body: String,
) -> (Option<String>, Option<String>, Option<String>, String) {
    match props {
        Some(p) => (
            p.first_id().map(str::to_string),
            p.file().map(str::to_string),
            p.first_class().map(str::to_string),
            body,
        ),
        None => (None, None, None, body),
    }
}

/// Splits a code body into classified lines, detecting `<<reference>>` lines.
///
/// Reference names are left as written here; pass 2 qualifies them with
/// `namespace` when that resolves to a block in this document, mirroring how
/// tangle resolves a bare reference inside its own file namespace. `scope` is
/// filled in there too, once every name in the document is known.
fn classify_lines(body: &str, _namespace: Option<&str>) -> Vec<CodeLine> {
    if body.is_empty() {
        return Vec::new();
    }
    body.lines()
        .map(|line| {
            if let Some(caps) = REF_PATTERN.captures(line) {
                CodeLine::Reference {
                    indent: caps["indent"].to_string(),
                    name: caps["refname"].to_string(),
                    scope: RefScope::Unknown,
                }
            } else {
                CodeLine::Text(line.to_string())
            }
        })
        .collect()
}

/// Weaves a document directly to a standalone (or fragment) HTML string.
pub fn weave_to_html(
    input: &str,
    source_path: Option<&Path>,
    config: &Config,
    options: &HtmlOptions,
) -> Result<String> {
    Ok(weave_document(input, source_path, config)?.to_html(options))
}

/// Weaves a document to clean, renderer-agnostic markdown (Pandoc/Quarto-ready).
pub fn weave_to_markdown(
    input: &str,
    source_path: Option<&Path>,
    config: &Config,
) -> Result<String> {
    Ok(weave_document(input, source_path, config)?.to_markdown())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NamespaceDefault;

    fn config() -> Config {
        Config {
            namespace_default: NamespaceDefault::None,
            ..Default::default()
        }
    }

    fn code_blocks(doc: &WovenDocument) -> Vec<&WeaveCodeBlock> {
        doc.elements
            .iter()
            .filter_map(|e| match e {
                WeaveElement::Code(c) => Some(c),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn interleaves_prose_and_code() {
        let input = "# Title\n\nSome prose.\n\n```python #main file=main.py\nprint('hi')\n```\n\nMore prose.\n";
        let doc = weave_document(input, None, &config()).unwrap();

        assert!(matches!(doc.elements[0], WeaveElement::Prose(_)));
        assert!(matches!(doc.elements[1], WeaveElement::Code(_)));
        assert!(matches!(doc.elements[2], WeaveElement::Prose(_)));

        let blocks = code_blocks(&doc);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].name.as_deref(), Some("main"));
        assert_eq!(blocks[0].target.as_deref(), Some("main.py"));
        assert_eq!(blocks[0].language.as_deref(), Some("python"));
    }

    #[test]
    fn detects_reference_lines() {
        let input = "```python #main file=main.py\ndef f():\n    <<body>>\n```\n";
        let doc = weave_document(input, None, &config()).unwrap();
        let blocks = code_blocks(&doc);
        assert_eq!(
            blocks[0].lines[1],
            CodeLine::Reference {
                indent: "    ".to_string(),
                name: "body".to_string(),
                // Nothing in the document or the project defines `body`.
                scope: RefScope::Unknown,
            }
        );
    }

    #[test]
    fn computes_used_by() {
        let input = "\
```python #main file=main.py
<<imports>>
```

```python #imports
import os
```
";
        let doc = weave_document(input, None, &config()).unwrap();
        let blocks = code_blocks(&doc);
        let imports = blocks
            .iter()
            .find(|b| b.name.as_deref() == Some("imports"))
            .unwrap();
        assert_eq!(imports.used_by, vec!["main".to_string()]);
    }

    #[test]
    fn tracks_continuation_index_and_total() {
        let input = "\
```python #setup
import sys
```

```python #setup
import os
```
";
        let doc = weave_document(input, None, &config()).unwrap();
        let blocks = code_blocks(&doc);
        assert_eq!(blocks[0].index, 0);
        assert_eq!(blocks[0].total, 2);
        assert_eq!(blocks[1].index, 1);
        assert_eq!(blocks[1].total, 2);
    }

    #[test]
    fn plain_code_block_is_not_entangled() {
        let input = "```python\nprint('anon')\n```\n";
        let doc = weave_document(input, None, &config()).unwrap();
        let blocks = code_blocks(&doc);
        assert_eq!(blocks.len(), 1);
        assert!(!blocks[0].is_entangled());
        assert_eq!(blocks[0].language.as_deref(), Some("python"));
    }

    #[test]
    fn extracts_title_from_frontmatter() {
        let input = "---\ntitle: My Doc\n---\n\n# Heading\n";
        let doc = weave_document(input, None, &config()).unwrap();
        assert_eq!(doc.title.as_deref(), Some("My Doc"));
    }

    #[test]
    fn self_reference_does_not_list_itself_in_used_by() {
        let input = "```python #loop\n<<loop>>\n```\n";
        let doc = weave_document(input, None, &config()).unwrap();
        let blocks = code_blocks(&doc);
        assert!(blocks[0].used_by.is_empty());
    }
}
