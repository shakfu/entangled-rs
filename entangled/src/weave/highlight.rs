//! Syntax highlighting for the weave HTML backend.
//!
//! Highlighting is gated behind the `highlight` cargo feature (on by default).
//! When enabled, code is highlighted server-side with `syntect` using its
//! pure-Rust `fancy-regex` backend, producing class-based `<span>`s plus a
//! theme stylesheet that adapts to light and dark mode. When disabled, both
//! entry points degrade to no-ops so the HTML backend falls back to plain,
//! semantically-classed code.
//!
//! Highlighting is applied per line so that `<<reference>>` lines (rendered as
//! links by the caller) can be interleaved cleanly. The tradeoff is that
//! constructs spanning multiple lines (e.g. a triple-quoted string) are
//! highlighted line-by-line rather than as a whole; for the short blocks typical
//! of literate programs this is not noticeable.

#[cfg(feature = "highlight")]
mod imp {
    use once_cell::sync::Lazy;
    use syntect::highlighting::ThemeSet;
    use syntect::html::{css_for_theme_with_class_style, ClassStyle, ClassedHTMLGenerator};
    use syntect::parsing::{SyntaxReference, SyntaxSet};

    static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(SyntaxSet::load_defaults_newlines);
    static THEME_SET: Lazy<ThemeSet> = Lazy::new(ThemeSet::load_defaults);

    /// Prefixed class style keeps highlight classes (`st-*`) from colliding with
    /// the page's own `entangled-*` classes.
    const CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "st-" };

    /// Resolves a language identifier to a syntect syntax.
    ///
    /// Entangled block languages are names like `python` or `rust`, but syntect's
    /// token lookup keys on file extensions and exact names. We therefore also
    /// try a case-insensitive name match.
    fn find_syntax(lang: &str) -> Option<&'static SyntaxReference> {
        SYNTAX_SET.find_syntax_by_token(lang).or_else(|| {
            SYNTAX_SET
                .syntaxes()
                .iter()
                .find(|s| s.name.eq_ignore_ascii_case(lang))
        })
    }

    /// Highlights a single line of code, returning class-based span HTML.
    ///
    /// Returns `None` when the language is unknown, letting the caller fall back
    /// to plain escaping.
    pub fn highlight_line(lang: &str, text: &str) -> Option<String> {
        if lang.is_empty() {
            return None;
        }
        let syntax = find_syntax(lang)?;
        let mut generator =
            ClassedHTMLGenerator::new_with_class_style(syntax, &SYNTAX_SET, CLASS_STYLE);
        // The generator expects newline-terminated input.
        let mut owned = String::with_capacity(text.len() + 1);
        owned.push_str(text);
        owned.push('\n');
        generator
            .parse_html_for_line_which_includes_newline(&owned)
            .ok()?;
        // syntect includes the trailing newline as token text *inside* the
        // wrapping spans. Since this only ever highlights a single line, all
        // newlines are that artifact and can be dropped; the caller re-inserts
        // line separators between lines.
        let html = generator.finalize().replace('\n', "");
        Some(html)
    }

    /// Produces the theme stylesheet: a light theme at top level and a dark theme
    /// scoped under `prefers-color-scheme: dark`.
    pub fn theme_css() -> String {
        let css = |name: &str| {
            THEME_SET
                .themes
                .get(name)
                .and_then(|t| css_for_theme_with_class_style(t, CLASS_STYLE).ok())
                .unwrap_or_default()
        };
        let light = css("InspiredGitHub");
        let dark = css("base16-ocean.dark");
        format!("{light}\n@media (prefers-color-scheme: dark) {{\n{dark}\n}}\n")
    }
}

#[cfg(feature = "highlight")]
pub use imp::{highlight_line, theme_css};

/// Fallback when the `highlight` feature is disabled: no highlighting.
#[cfg(not(feature = "highlight"))]
pub fn highlight_line(_lang: &str, _text: &str) -> Option<String> {
    None
}

/// Fallback when the `highlight` feature is disabled: no theme stylesheet.
#[cfg(not(feature = "highlight"))]
pub fn theme_css() -> String {
    String::new()
}

#[cfg(all(test, feature = "highlight"))]
mod tests {
    use super::*;

    #[test]
    fn highlights_known_language_by_name() {
        let html = highlight_line("python", "def f(): pass").unwrap();
        assert!(html.contains("st-"), "expected prefixed highlight classes");
        // Escaping is preserved by syntect.
        assert!(!html.contains("def f(): pass") || html.contains("<span"));
    }

    #[test]
    fn unknown_language_returns_none() {
        assert!(highlight_line("no-such-lang-xyz", "code").is_none());
        assert!(highlight_line("", "code").is_none());
    }

    #[test]
    fn theme_css_covers_light_and_dark() {
        let css = theme_css();
        assert!(css.contains(".st-"));
        assert!(css.contains("@media (prefers-color-scheme: dark)"));
    }
}
