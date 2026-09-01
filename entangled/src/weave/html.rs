//! Self-contained HTML backend for weaving.
//!
//! Renders a [`WovenDocument`] to a single HTML string with no external assets:
//! prose is converted with `pulldown-cmark`, and each Entangled code block
//! becomes a captioned, anchored figure whose `<<reference>>` lines are rendered
//! as intra-document links. The result is offline-friendly and theme-aware
//! (light/dark via `prefers-color-scheme`), matching the project's
//! single-binary, zero-runtime-dependency ethos.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use pulldown_cmark::{html, Options, Parser};
use sha2::{Digest, Sha256};

use super::highlight;
use super::{CodeLine, RefScope, WeaveCodeBlock, WeaveElement, WovenDocument};

/// Options controlling HTML rendering.
#[derive(Debug, Clone)]
pub struct HtmlOptions {
    /// When true, emit a complete HTML document (doctype, `<head>`, embedded
    /// CSS). When false, emit only the body fragment for embedding elsewhere.
    pub standalone: bool,
    /// Overrides the document title used in `<title>`. Falls back to the
    /// document's frontmatter title, then to `"Woven document"`.
    pub title: Option<String>,
}

impl Default for HtmlOptions {
    fn default() -> Self {
        Self {
            standalone: true,
            title: None,
        }
    }
}

impl WovenDocument {
    /// Renders this document to HTML.
    pub fn to_html(&self, options: &HtmlOptions) -> String {
        // Map each named block to the anchor of its first occurrence so that
        // references and "used in" links resolve to a stable target.
        //
        // Slugs are made unique across the whole document: `a!` and `a?` both
        // reduce to `block-a`, and a name of pure punctuation reduces to
        // nothing at all, so distinct blocks could otherwise end up sharing one
        // HTML id and every link to either would be ambiguous.
        let mut first_anchor: HashMap<&str, String> = HashMap::new();
        let mut claimed: HashSet<String> = HashSet::new();
        for element in &self.elements {
            if let WeaveElement::Code(block) = element {
                if let Some(name) = &block.name {
                    if !first_anchor.contains_key(name.as_str()) {
                        let anchor = unique_slug(name, &claimed);
                        claimed.insert(anchor.clone());
                        first_anchor.insert(name.as_str(), anchor);
                    }
                }
            }
        }

        let mut body = String::new();
        for element in &self.elements {
            match element {
                WeaveElement::Prose(text) => {
                    body.push_str(&render_prose(text));
                }
                WeaveElement::Code(block) => {
                    render_block(&mut body, block, &first_anchor);
                }
            }
        }

        if !options.standalone {
            return body;
        }

        let title = options
            .title
            .clone()
            .or_else(|| self.title.clone())
            .unwrap_or_else(|| "Woven document".to_string());

        let mut out = String::new();
        out.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
        out.push_str("<meta charset=\"utf-8\">\n");
        out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
        let _ = writeln!(out, "<title>{}</title>", escape_html(&title));
        out.push_str("<style>\n");
        out.push_str(CSS);
        out.push_str(&highlight::theme_css());
        out.push_str("</style>\n</head>\n<body>\n<main class=\"entangled-doc\">\n");
        out.push_str(&body);
        out.push_str("</main>\n</body>\n</html>\n");
        out
    }
}

/// Converts a run of markdown prose to HTML.
fn render_prose(text: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(text, opts);
    let mut html_out = String::new();
    html::push_html(&mut html_out, parser);
    html_out
}

/// Renders a single code block (Entangled or plain) into `out`.
fn render_block(out: &mut String, block: &WeaveCodeBlock, first_anchor: &HashMap<&str, String>) {
    if !block.is_entangled() {
        // Plain fenced code: no caption, no anchors.
        let lang = block.language.as_deref().unwrap_or("");
        let _ = write!(
            out,
            "<pre class=\"entangled-plain\"><code{}>",
            lang_class(lang)
        );
        out.push_str(&render_code_lines(block, first_anchor));
        out.push_str("</code></pre>\n");
        return;
    }

    // Anchor: the name's unique base slug, plus the occurrence index for
    // continuation blocks.
    let anchor = match &block.name {
        Some(name) => {
            let base = first_anchor
                .get(name.as_str())
                .cloned()
                .unwrap_or_else(|| slug(name));
            if block.index == 0 {
                base
            } else {
                format!("{}-{}", base, block.index)
            }
        }
        None => block
            .target
            .as_deref()
            .map(slug)
            .unwrap_or_else(|| "block".to_string()),
    };

    let _ = writeln!(
        out,
        "<figure class=\"entangled-block\" id=\"{}\">",
        escape_attr(&anchor)
    );
    out.push_str("<figcaption class=\"entangled-caption\">");
    render_caption(out, block);
    out.push_str("</figcaption>\n");

    let lang = block.language.as_deref().unwrap_or("");
    let _ = write!(out, "<pre><code{}>", lang_class(lang));
    out.push_str(&render_code_lines(block, first_anchor));
    out.push_str("</code></pre>\n");

    render_output(out, block);
    render_used_by(out, block, first_anchor);
    out.push_str("</figure>\n");
}

/// Writes the caption line: block name, continuation marker, and target file.
fn render_caption(out: &mut String, block: &WeaveCodeBlock) {
    if let Some(name) = &block.name {
        let _ = write!(
            out,
            "<span class=\"entangled-name\">&laquo;{}&raquo;</span>",
            escape_html(name)
        );
        if block.total > 1 {
            let _ = write!(
                out,
                " <span class=\"entangled-part\">{}/{}</span>",
                block.index + 1,
                block.total
            );
        }
    }
    if let Some(target) = &block.target {
        let _ = write!(
            out,
            " <span class=\"entangled-file\">&#8594; {}</span>",
            escape_html(target)
        );
    }
    out.push_str(" <span class=\"entangled-eq\">&equiv;</span>");
}

/// Renders code lines with references as links (when resolvable) and, when the
/// `highlight` feature is enabled, syntax-highlighted literal lines.
fn render_code_lines(block: &WeaveCodeBlock, first_anchor: &HashMap<&str, String>) -> String {
    let lang = block.language.as_deref().unwrap_or("");
    let mut out = String::new();
    for (i, line) in block.lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        match line {
            CodeLine::Text(text) => match highlight::highlight_line(lang, text) {
                Some(highlighted) => out.push_str(&highlighted),
                None => out.push_str(&escape_html(text)),
            },
            CodeLine::Reference {
                indent,
                name,
                scope,
            } => {
                out.push_str(&escape_html(indent));
                let label = format!("&laquo;{}&raquo;", escape_html(name));
                match (first_anchor.get(name.as_str()), scope) {
                    // Defined in this document: link straight to it.
                    (Some(anchor), _) => {
                        let _ = write!(
                            out,
                            "<a class=\"entangled-ref\" href=\"#{}\">{}</a>",
                            escape_attr(anchor),
                            label
                        );
                    }
                    // Defined in another source file. Weave renders one
                    // document at a time, so there is nothing to link to from
                    // here -- but the reference is correct, and marking it
                    // missing would be a lie.
                    (None, RefScope::Project) => {
                        let _ = write!(
                            out,
                            "<span class=\"entangled-ref-external\" \
                             title=\"defined in another source document\">{}</span>",
                            label
                        );
                    }
                    (None, _) => {
                        let _ = write!(
                            out,
                            "<span class=\"entangled-ref-missing\">{}</span>",
                            label
                        );
                    }
                }
            }
        }
    }
    out
}

/// Writes the captured execution output panel, if any.
fn render_output(out: &mut String, block: &WeaveCodeBlock) {
    let Some(output) = &block.output else {
        return;
    };
    let class = if output.success {
        "entangled-output"
    } else {
        "entangled-output entangled-output-error"
    };
    let _ = write!(out, "<div class=\"{class}\">");
    out.push_str("<div class=\"entangled-output-label\">output</div>");
    if !output.stdout.is_empty() {
        let _ = write!(
            out,
            "<pre class=\"entangled-stdout\">{}</pre>",
            escape_html(output.stdout.trim_end_matches('\n'))
        );
    }
    if !output.stderr.is_empty() {
        let _ = write!(
            out,
            "<pre class=\"entangled-stderr\">{}</pre>",
            escape_html(output.stderr.trim_end_matches('\n'))
        );
    }
    out.push_str("</div>\n");
}

/// Writes the "used in" cross-reference footer, if any.
fn render_used_by(out: &mut String, block: &WeaveCodeBlock, first_anchor: &HashMap<&str, String>) {
    if block.used_by.is_empty() {
        return;
    }
    out.push_str("<div class=\"entangled-usedby\">used in ");
    for (i, name) in block.used_by.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        let label = format!("&laquo;{}&raquo;", escape_html(name));
        match first_anchor.get(name.as_str()) {
            Some(anchor) => {
                let _ = write!(out, "<a href=\"#{}\">{}</a>", escape_attr(anchor), label);
            }
            None => out.push_str(&label),
        }
    }
    out.push_str("</div>\n");
}

/// Builds a `class="language-xxx"` attribute (empty string when no language).
fn lang_class(lang: &str) -> String {
    if lang.is_empty() {
        String::new()
    } else {
        format!(" class=\"language-{}\"", escape_attr(lang))
    }
}

/// Builds a slug for `name` that no other name in the document has claimed.
///
/// The readable slug is preferred; when it is already taken (or is empty
/// because the name held no alphanumerics), a short digest of the full name is
/// appended. The digest is derived from the name alone, so the same name always
/// produces the same anchor -- links stay stable across re-weaves.
fn unique_slug(name: &str, claimed: &HashSet<String>) -> String {
    let base = slug(name);
    if base != "block" && !claimed.contains(&base) {
        return base;
    }

    let digest = &hex::encode(Sha256::digest(name.as_bytes()))[..8];
    let candidate = format!("{base}-{digest}");
    if !claimed.contains(&candidate) {
        return candidate;
    }

    // Two distinct names with the same SHA-256 prefix: fall back to counting.
    (1..)
        .map(|n| format!("{candidate}-{n}"))
        .find(|c| !claimed.contains(c))
        .expect("an unclaimed suffix always exists")
}

/// Converts a block name into a URL/id-safe slug.
///
/// Returns the bare prefix `block` for a name with no alphanumerics; callers
/// disambiguate through [`unique_slug`].
fn slug(name: &str) -> String {
    let mut s = String::with_capacity(name.len() + 10);
    s.push_str("block-");
    let mut prev_dash = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            s.push('-');
            prev_dash = true;
        }
    }
    s.trim_end_matches('-').to_string()
}

/// Escapes text for use in HTML element content.
fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

/// Escapes text for use inside a double-quoted HTML attribute.
fn escape_attr(s: &str) -> String {
    let mut out = escape_html(s);
    out = out.replace('"', "&quot;");
    out
}

/// Embedded stylesheet: minimal, readable, theme-aware.
const CSS: &str = r#"
:root {
  --bg: #ffffff; --fg: #1c1e21; --muted: #6b7280;
  --code-bg: #f6f8fa; --border: #e2e5e9; --accent: #2563eb;
  --link: #2563eb;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #0d1117; --fg: #e6edf3; --muted: #8b949e;
    --code-bg: #161b22; --border: #30363d; --accent: #58a6ff;
    --link: #58a6ff;
  }
}
* { box-sizing: border-box; }
body {
  background: var(--bg); color: var(--fg); margin: 0;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  line-height: 1.6;
}
main.entangled-doc { max-width: 46rem; margin: 0 auto; padding: 2.5rem 1.25rem 6rem; }
main.entangled-doc h1, main.entangled-doc h2, main.entangled-doc h3 { line-height: 1.25; }
a { color: var(--link); }
pre, code { font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace; }
:not(pre) > code { background: var(--code-bg); padding: 0.1em 0.35em; border-radius: 4px; font-size: 0.9em; }
pre { background: var(--code-bg); padding: 0.85rem 1rem; border-radius: 6px; overflow-x: auto; margin: 0; }
pre code { font-size: 0.875rem; }
figure.entangled-block {
  margin: 1.5rem 0; border: 1px solid var(--border); border-radius: 8px; overflow: hidden;
}
figcaption.entangled-caption {
  background: var(--code-bg); border-bottom: 1px solid var(--border);
  padding: 0.4rem 1rem; font-family: ui-monospace, monospace; font-size: 0.8rem;
}
figure.entangled-block > pre { border-radius: 0; }
.entangled-name { color: var(--accent); font-weight: 600; }
.entangled-part { color: var(--muted); }
.entangled-file { color: var(--muted); }
.entangled-eq { color: var(--muted); }
a.entangled-ref { text-decoration: none; border-bottom: 1px dotted currentColor; }
a.entangled-ref:hover { border-bottom-style: solid; }
.entangled-ref-missing { color: var(--muted); text-decoration: underline dotted; }
.entangled-ref-external { color: var(--muted); }
.entangled-usedby {
  padding: 0.4rem 1rem; font-size: 0.78rem; color: var(--muted);
  border-top: 1px solid var(--border); background: var(--bg);
}
.entangled-usedby a { color: var(--muted); }
pre.entangled-plain { margin: 1.25rem 0; border: 1px solid var(--border); }
.entangled-output { border-top: 1px solid var(--border); }
.entangled-output-label {
  padding: 0.3rem 1rem; font-size: 0.7rem; text-transform: uppercase;
  letter-spacing: 0.05em; color: var(--muted); background: var(--bg);
}
.entangled-output > pre { border-radius: 0; margin: 0; background: var(--bg); }
.entangled-output pre.entangled-stderr { color: #b91c1c; }
@media (prefers-color-scheme: dark) {
  .entangled-output pre.entangled-stderr { color: #f87171; }
}
.entangled-output-error .entangled-output-label { color: #b91c1c; }
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, NamespaceDefault};
    use crate::weave::weave_document;

    fn weave(input: &str) -> WovenDocument {
        let c = Config {
            namespace_default: NamespaceDefault::None,
            ..Default::default()
        };
        weave_document(input, None, &c).unwrap()
    }

    #[test]
    fn standalone_wraps_document() {
        let html = weave("# Hello\n").to_html(&HtmlOptions::default());
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("<title>"));
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(html.trim_end().ends_with("</html>"));
    }

    #[test]
    fn fragment_omits_wrapper() {
        let opts = HtmlOptions {
            standalone: false,
            title: None,
        };
        let html = weave("# Hello\n").to_html(&opts);
        assert!(!html.contains("<!DOCTYPE"));
        assert!(html.contains("<h1>Hello</h1>"));
    }

    #[test]
    fn reference_becomes_link_to_definition() {
        let input = "\
```python #main file=main.py
<<body>>
```

```python #body
pass
```
";
        let html = weave(input).to_html(&HtmlOptions::default());
        assert!(html.contains("href=\"#block-body\""));
        assert!(html.contains("id=\"block-body\""));
        assert!(html.contains("class=\"entangled-ref\""));
    }

    #[test]
    fn unresolved_reference_is_not_a_link() {
        let input = "```python #main file=main.py\n<<missing>>\n```\n";
        let html = weave(input).to_html(&HtmlOptions::default());
        assert!(html.contains("entangled-ref-missing"));
        assert!(!html.contains("href=\"#block-missing\""));
    }

    #[test]
    fn caption_shows_target_and_continuation() {
        let input = "\
```python #setup file=s.py
a = 1
```

```python #setup
b = 2
```
";
        let html = weave(input).to_html(&HtmlOptions::default());
        assert!(html.contains("&laquo;setup&raquo;"));
        assert!(html.contains("1/2"));
        assert!(html.contains("s.py"));
    }

    #[test]
    fn used_by_footer_links_back() {
        let input = "\
```python #main file=main.py
<<helper>>
```

```python #helper
pass
```
";
        let html = weave(input).to_html(&HtmlOptions::default());
        assert!(html.contains("used in"));
        assert!(html.contains("href=\"#block-main\""));
    }

    #[test]
    fn escapes_html_in_code() {
        // Robust to syntax highlighting, which may wrap tokens in <span>s: the
        // key invariant is that the raw metacharacters are HTML-escaped and never
        // appear literally as an unescaped run.
        let input = "```python #main file=main.py\nx = a < b & c > d\n```\n";
        let html = weave(input).to_html(&HtmlOptions::default());
        assert!(html.contains("&lt;"));
        assert!(html.contains("&amp;"));
        assert!(html.contains("&gt;"));
        assert!(!html.contains("a < b & c > d"));
    }

    #[test]
    fn renders_captured_output_panel() {
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
        let c = Config {
            namespace_default: NamespaceDefault::None,
            ..Default::default()
        };
        let html = weave_document_with_outputs(input, None, &c, &outputs)
            .unwrap()
            .to_html(&HtmlOptions::default());
        assert!(html.contains("class=\"entangled-output\""));
        assert!(html.contains("entangled-output-label"));
        assert!(html.contains(">42</pre>"));
    }

    #[test]
    fn failed_output_is_marked_and_shows_stderr() {
        use crate::weave::{weave_document_with_outputs, BlockOutput};
        use std::collections::HashMap;

        let input = "```sh #boom eval=sh\nexit 1\n```\n";
        let mut outputs = HashMap::new();
        outputs.insert(
            "boom".to_string(),
            BlockOutput {
                stdout: String::new(),
                stderr: "boom!".to_string(),
                success: false,
            },
        );
        let c = Config {
            namespace_default: NamespaceDefault::None,
            ..Default::default()
        };
        let html = weave_document_with_outputs(input, None, &c, &outputs)
            .unwrap()
            .to_html(&HtmlOptions::default());
        assert!(html.contains("entangled-output-error"));
        assert!(html.contains("entangled-stderr"));
        assert!(html.contains("boom!"));
    }

    #[cfg(feature = "highlight")]
    #[test]
    fn highlighting_adds_classes_and_theme_css() {
        let input = "```python #main file=main.py\ndef greet():\n    return 1\n```\n";
        let html = weave(input).to_html(&HtmlOptions::default());
        // Highlight spans use the st- prefix, and the theme stylesheet is embedded.
        assert!(html.contains("st-"));
        assert!(html.contains("@media (prefers-color-scheme: dark)"));
    }

    #[test]
    fn slug_is_id_safe() {
        assert_eq!(slug("main"), "block-main");
        assert_eq!(slug("mod::func"), "block-mod-func");
        assert_eq!(slug("a/b.c"), "block-a-b-c");
    }
}
