//! Annotation markers for tangled code.

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Marker patterns for annotated code blocks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Markers {
    /// Pattern that appears before the reference in begin/end markers.
    #[serde(default = "default_open")]
    pub open: String,

    /// Pattern that appears after the reference in begin/end markers.
    #[serde(default = "default_close")]
    pub close: String,

    /// The word used to mark the beginning of a block.
    #[serde(default = "default_begin")]
    pub begin: String,

    /// The word used to mark the end of a block.
    #[serde(default = "default_end")]
    pub end: String,
}

fn default_open() -> String {
    "<<".to_string()
}

fn default_close() -> String {
    ">>".to_string()
}

fn default_begin() -> String {
    "begin".to_string()
}

fn default_end() -> String {
    "end".to_string()
}

impl Default for Markers {
    fn default() -> Self {
        Self {
            open: default_open(),
            close: default_close(),
            begin: default_begin(),
            end: default_end(),
        }
    }
}

impl Markers {
    /// Creates a new Markers configuration.
    pub fn new(open: &str, close: &str, begin: &str, end: &str) -> Self {
        Self {
            open: open.to_string(),
            close: close.to_string(),
            begin: begin.to_string(),
            end: end.to_string(),
        }
    }

    /// Formats a begin marker for the given reference.
    pub fn format_begin(&self, reference: &str) -> String {
        format!("{} {}{}{}", self.begin, self.open, reference, self.close)
    }

    /// Formats an end marker.
    pub fn format_end(&self) -> String {
        self.end.clone()
    }

    /// Creates a regex pattern for matching begin markers.
    pub fn begin_pattern(&self) -> String {
        format!(
            r"^\s*{}\s+{}(?P<ref>[^{}]+){}",
            regex::escape(&self.begin),
            regex::escape(&self.open),
            regex::escape(&self.close.chars().next().unwrap_or('>').to_string()),
            regex::escape(&self.close)
        )
    }

    /// Creates a regex pattern for matching end markers.
    pub fn end_pattern(&self) -> String {
        format!(r"^\s*{}\s*$", regex::escape(&self.end))
    }
}

/// Reference pattern for detecting noweb-style references like `<<refname>>`.
pub static REF_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?P<indent>\s*)<<(?P<refname>[\w:/_.-]+)>>\s*$").unwrap());

/// Annotation prefix pattern.
pub static ANNOTATION_PREFIX: &str = "~/~";

/// Creates a full annotation begin marker.
pub fn annotation_begin(comment_prefix: &str, markers: &Markers, reference: &str) -> String {
    format!(
        "{} {} {}",
        comment_prefix,
        ANNOTATION_PREFIX,
        markers.format_begin(reference)
    )
}

/// Creates a full annotation end marker.
pub fn annotation_end(comment_prefix: &str, markers: &Markers) -> String {
    format!(
        "{} {} {}",
        comment_prefix,
        ANNOTATION_PREFIX,
        markers.format_end()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_markers() {
        let markers = Markers::default();
        assert_eq!(markers.open, "<<");
        assert_eq!(markers.close, ">>");
        assert_eq!(markers.begin, "begin");
        assert_eq!(markers.end, "end");
    }

    #[test]
    fn test_format_begin() {
        let markers = Markers::default();
        assert_eq!(markers.format_begin("main"), "begin <<main>>");
    }

    #[test]
    fn test_format_end() {
        let markers = Markers::default();
        assert_eq!(markers.format_end(), "end");
    }

    #[test]
    fn test_annotation_begin() {
        let markers = Markers::default();
        let result = annotation_begin("#", &markers, "file#main[0]");
        assert_eq!(result, "# ~/~ begin <<file#main[0]>>");
    }

    #[test]
    fn test_annotation_end() {
        let markers = Markers::default();
        let result = annotation_end("#", &markers);
        assert_eq!(result, "# ~/~ end");
    }

    #[test]
    fn test_ref_pattern() {
        let caps = REF_PATTERN.captures("    <<some_ref>>").unwrap();
        assert_eq!(&caps["indent"], "    ");
        assert_eq!(&caps["refname"], "some_ref");

        let caps2 = REF_PATTERN.captures("<<module::func>>").unwrap();
        assert_eq!(&caps2["indent"], "");
        assert_eq!(&caps2["refname"], "module::func");

        assert!(REF_PATTERN.captures("not a ref").is_none());
        assert!(REF_PATTERN.captures("<<>>").is_none());
    }

    #[test]
    fn test_ref_pattern_with_path() {
        let caps = REF_PATTERN.captures("<<path/to/file.py>>").unwrap();
        assert_eq!(&caps["refname"], "path/to/file.py");
    }
}
