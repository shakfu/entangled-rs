//! Language configuration for code blocks.

use serde::{Deserialize, Serialize};

/// Comment style configuration for a language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Comment {
    /// Single line comment prefix, e.g., "//" or "#"
    Line(String),
    /// Block comment with open and close delimiters, e.g., ["/*", "*/"]
    Block { open: String, close: String },
}

impl Comment {
    /// Creates a line comment style.
    pub fn line(prefix: impl Into<String>) -> Self {
        Comment::Line(prefix.into())
    }

    /// Creates a block comment style.
    pub fn block(open: impl Into<String>, close: impl Into<String>) -> Self {
        Comment::Block {
            open: open.into(),
            close: close.into(),
        }
    }

    /// Wraps text in a comment.
    pub fn wrap(&self, text: &str) -> String {
        match self {
            Comment::Line(prefix) => format!("{} {}", prefix, text),
            Comment::Block { open, close } => format!("{} {} {}", open, text, close),
        }
    }

    /// Returns the comment prefix for annotation markers.
    pub fn prefix(&self) -> &str {
        match self {
            Comment::Line(prefix) => prefix,
            Comment::Block { open, .. } => open,
        }
    }
}

impl Default for Comment {
    fn default() -> Self {
        Comment::Line("#".to_string())
    }
}

/// Language configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Language {
    /// Language identifier (e.g., "python", "rust")
    pub name: String,

    /// File extensions for this language
    #[serde(default)]
    pub identifiers: Vec<String>,

    /// Comment style
    pub comment: Comment,
}

impl Language {
    /// Creates a new Language configuration.
    pub fn new(name: impl Into<String>, comment: Comment) -> Self {
        Self {
            name: name.into(),
            identifiers: Vec::new(),
            comment,
        }
    }

    /// Adds file extensions/identifiers.
    pub fn with_identifiers(mut self, identifiers: Vec<String>) -> Self {
        self.identifiers = identifiers;
        self
    }

    /// Checks if this language matches a given identifier.
    pub fn matches(&self, identifier: &str) -> bool {
        self.name == identifier || self.identifiers.iter().any(|id| id == identifier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_comment() {
        let comment = Comment::line("//");
        assert_eq!(comment.wrap("test"), "// test");
        assert_eq!(comment.prefix(), "//");
    }

    #[test]
    fn test_block_comment() {
        let comment = Comment::block("/*", "*/");
        assert_eq!(comment.wrap("test"), "/* test */");
        assert_eq!(comment.prefix(), "/*");
    }

    #[test]
    fn test_language_matches() {
        let lang = Language::new("python", Comment::line("#"))
            .with_identifiers(vec!["py".to_string(), "python3".to_string()]);

        assert!(lang.matches("python"));
        assert!(lang.matches("py"));
        assert!(lang.matches("python3"));
        assert!(!lang.matches("rust"));
    }

    #[test]
    fn test_comment_serde() {
        let line: Comment = serde_json::from_str("\"#\"").unwrap();
        assert_eq!(line, Comment::Line("#".to_string()));

        let block: Comment =
            serde_json::from_str(r#"{"open": "/*", "close": "*/"}"#).unwrap();
        assert_eq!(
            block,
            Comment::Block {
                open: "/*".to_string(),
                close: "*/".to_string()
            }
        );
    }
}
