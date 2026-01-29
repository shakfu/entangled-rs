//! Content types for representing tangled output.

/// Plain text content (a single line or fragment).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlainText(pub String);

impl PlainText {
    /// Creates new plain text.
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// Returns the text content.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the text with indentation prepended.
    pub fn with_indent(&self, indent: &str) -> String {
        format!("{}{}", indent, self.0)
    }
}

impl From<String> for PlainText {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for PlainText {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Raw content that should not be processed further.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawContent(pub String);

impl RawContent {
    /// Creates new raw content.
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// Returns the content.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Content item in tangled output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Content {
    /// Plain text line.
    Text(PlainText),
    /// Raw content (annotations, markers).
    Raw(RawContent),
    /// A reference to another code block (to be expanded).
    Reference {
        /// The reference name.
        name: String,
        /// Indentation to apply.
        indent: String,
    },
}

impl Content {
    /// Creates a text content item.
    pub fn text(s: impl Into<String>) -> Self {
        Content::Text(PlainText::new(s))
    }

    /// Creates a raw content item.
    pub fn raw(s: impl Into<String>) -> Self {
        Content::Raw(RawContent::new(s))
    }

    /// Creates a reference content item.
    pub fn reference(name: impl Into<String>, indent: impl Into<String>) -> Self {
        Content::Reference {
            name: name.into(),
            indent: indent.into(),
        }
    }

    /// Returns true if this is a reference.
    pub fn is_reference(&self) -> bool {
        matches!(self, Content::Reference { .. })
    }

    /// Converts to a string with optional indentation.
    pub fn to_string_with_indent(&self, base_indent: &str) -> String {
        match self {
            Content::Text(t) => format!("{}{}", base_indent, t.as_str()),
            Content::Raw(r) => r.as_str().to_string(),
            Content::Reference { name, indent } => {
                format!("{}{}<<{}>>", base_indent, indent, name)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_text() {
        let text = PlainText::new("hello");
        assert_eq!(text.as_str(), "hello");
        assert_eq!(text.with_indent("  "), "  hello");
    }

    #[test]
    fn test_content_text() {
        let content = Content::text("line of code");
        assert!(!content.is_reference());
    }

    #[test]
    fn test_content_reference() {
        let content = Content::reference("other_block", "    ");
        assert!(content.is_reference());
    }

    #[test]
    fn test_to_string_with_indent() {
        let text = Content::text("code");
        assert_eq!(text.to_string_with_indent("  "), "  code");

        let raw = Content::raw("# annotation");
        assert_eq!(raw.to_string_with_indent("  "), "# annotation");

        let reference = Content::reference("ref", "  ");
        assert_eq!(reference.to_string_with_indent(""), "  <<ref>>");
    }
}
