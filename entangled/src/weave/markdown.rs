//! Clean-markdown backend for weaving.
//!
//! Produces renderer-agnostic markdown suitable for handing to Pandoc (for PDF,
//! LaTeX, docx, epub, ...) or Quarto (`.qmd`). Entangled's fenced-block
//! attributes (`#name file=path`) are replaced with a readable caption line, and
//! `<<reference>>` lines are kept literal (markdown cannot hyperlink inside a
//! code fence). The output renders correctly in any CommonMark processor without
//! the Entangled-specific syntax leaking through.

use std::fmt::Write;

use super::{CodeLine, WeaveCodeBlock, WeaveElement, WovenDocument};

impl WovenDocument {
    /// Renders this document to clean, portable markdown.
    ///
    /// When the source had YAML frontmatter it is re-emitted verbatim so that
    /// downstream tools (Pandoc/Quarto) can consume document metadata.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();

        if let Some(fm) = &self.frontmatter {
            out.push_str("---\n");
            out.push_str(fm.trim_end());
            out.push_str("\n---\n\n");
        }

        for element in &self.elements {
            match element {
                WeaveElement::Prose(text) => {
                    out.push_str(text);
                    ensure_blank_line(&mut out);
                }
                WeaveElement::Code(block) => {
                    render_block(&mut out, block);
                    ensure_blank_line(&mut out);
                }
            }
        }

        // Collapse a trailing run of blank lines to a single newline.
        format!("{}\n", out.trim_end())
    }
}

/// Renders a code block as a captioned fenced block.
fn render_block(out: &mut String, block: &WeaveCodeBlock) {
    if let Some(caption) = caption(block) {
        out.push_str(&caption);
        out.push_str("\n\n");
    }

    let fence = choose_fence(block);
    let lang = block.language.as_deref().unwrap_or("");
    let _ = writeln!(out, "{}{}", fence, lang);
    for line in &block.lines {
        match line {
            CodeLine::Text(text) => {
                out.push_str(text);
                out.push('\n');
            }
            CodeLine::Reference { indent, name } => {
                let _ = writeln!(out, "{}<<{}>>", indent, name);
            }
        }
    }
    out.push_str(&fence);
    out.push('\n');

    render_output(out, block);
}

/// Emits a captured-output block after the code, if any.
fn render_output(out: &mut String, block: &WeaveCodeBlock) {
    let Some(output) = &block.output else {
        return;
    };
    let label = if output.success {
        "**output:**"
    } else {
        "**output (failed):**"
    };
    let _ = write!(out, "\n{label}\n\n");
    out.push_str("```\n");
    if !output.stdout.is_empty() {
        out.push_str(output.stdout.trim_end_matches('\n'));
        out.push('\n');
    }
    if !output.stderr.is_empty() {
        out.push_str(output.stderr.trim_end_matches('\n'));
        out.push('\n');
    }
    out.push_str("```\n");
}

/// Builds a bold caption line for an Entangled block, or `None` for plain code.
fn caption(block: &WeaveCodeBlock) -> Option<String> {
    if !block.is_entangled() {
        return None;
    }
    let mut c = String::from("**");
    if let Some(name) = &block.name {
        let _ = write!(c, "\u{00ab}{}\u{00bb}", name);
        if block.total > 1 {
            let _ = write!(c, " {}/{}", block.index + 1, block.total);
        }
    }
    if let Some(target) = &block.target {
        if block.name.is_some() {
            c.push(' ');
        }
        let _ = write!(c, "\u{2192} `{}`", target);
    }
    c.push_str("**");
    Some(c)
}

/// Chooses a fence long enough to not be closed prematurely by block content.
///
/// A fenced block is closed by a line consisting solely of backticks that is at
/// least as long as the opening fence, so the opening fence must be longer than
/// any all-backtick line inside the content.
fn choose_fence(block: &WeaveCodeBlock) -> String {
    let mut max_backticks = 0;
    for line in &block.lines {
        if let CodeLine::Text(text) = line {
            let trimmed = text.trim();
            if !trimmed.is_empty() && trimmed.chars().all(|c| c == '`') {
                max_backticks = max_backticks.max(trimmed.len());
            }
        }
    }
    "`".repeat(max_backticks.max(2) + 1)
}

/// Ensures `out` ends with exactly one blank line separator.
fn ensure_blank_line(out: &mut String) {
    while out.ends_with('\n') {
        out.pop();
    }
    out.push_str("\n\n");
}

#[cfg(test)]
mod tests {
    use crate::config::{Config, NamespaceDefault};
    use crate::weave::weave_document;

    fn md(input: &str) -> String {
        let mut c = Config::default();
        c.namespace_default = NamespaceDefault::None;
        weave_document(input, None, &c).unwrap().to_markdown()
    }

    #[test]
    fn strips_entangled_attributes_into_caption() {
        let out = md("```python #main file=main.py\nprint('hi')\n```\n");
        assert!(out.contains("**\u{00ab}main\u{00bb} \u{2192} `main.py`**"));
        // The raw info string must not leak.
        assert!(!out.contains("#main file=main.py"));
        assert!(out.contains("```python"));
        assert!(out.contains("print('hi')"));
    }

    #[test]
    fn keeps_references_literal() {
        let out = md("```python #main file=main.py\n    <<body>>\n```\n");
        assert!(out.contains("    <<body>>"));
    }

    #[test]
    fn preserves_prose() {
        let out = md("# Heading\n\nA paragraph.\n\n```python #a\nx=1\n```\n");
        assert!(out.contains("# Heading"));
        assert!(out.contains("A paragraph."));
    }

    #[test]
    fn plain_code_has_no_caption() {
        let out = md("```python\nprint('anon')\n```\n");
        assert!(!out.contains("**"));
        assert!(out.contains("```python"));
    }

    #[test]
    fn re_emits_frontmatter() {
        let out = md("---\ntitle: Doc\n---\n\n# H\n");
        assert!(out.starts_with("---\ntitle: Doc\n---"));
    }

    #[test]
    fn continuation_marker_in_caption() {
        let out = md("```python #s\na=1\n```\n\n```python #s\nb=2\n```\n");
        assert!(out.contains("**\u{00ab}s\u{00bb} 1/2**"));
        assert!(out.contains("**\u{00ab}s\u{00bb} 2/2**"));
    }

    #[test]
    fn renders_captured_output_block() {
        use crate::weave::{weave_document_with_outputs, BlockOutput};
        use std::collections::HashMap;

        let input = "```python #demo eval=python\nprint(6 * 7)\n```\n";
        let mut outputs = HashMap::new();
        outputs.insert(
            "demo".to_string(),
            BlockOutput {
                stdout: "42\n".to_string(),
                stderr: String::new(),
                success: true,
            },
        );
        let mut c = Config::default();
        c.namespace_default = NamespaceDefault::None;
        let out = weave_document_with_outputs(input, None, &c, &outputs)
            .unwrap()
            .to_markdown();
        assert!(out.contains("**output:**"));
        assert!(out.contains("42"));
    }

    #[test]
    fn escapes_fence_when_content_has_backticks() {
        let out = md("````markdown #doc file=d.md\n```\nnested\n```\n````\n");
        // Outer fence must be longer than the inner ``` so it is not closed early.
        assert!(out.contains("````markdown"));
    }
}
