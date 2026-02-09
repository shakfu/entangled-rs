//! Type definitions for readers.

use crate::text_location::TextLocation;

/// A token from the input stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputToken {
    /// A code block with its info string and content.
    CodeBlock {
        /// The info string (language and attributes).
        info: String,
        /// The code content.
        content: String,
        /// Location in source.
        location: TextLocation,
    },

    /// Raw markdown text (not inside a code block).
    Markdown {
        /// The markdown content.
        content: String,
        /// Location in source.
        location: TextLocation,
    },

    /// YAML frontmatter.
    YamlHeader {
        /// The YAML content.
        content: String,
        /// Location in source.
        location: TextLocation,
    },
}

impl InputToken {
    /// Creates a code block token.
    pub fn code_block(info: String, content: String, location: TextLocation) -> Self {
        Self::CodeBlock {
            info,
            content,
            location,
        }
    }

    /// Creates a markdown token.
    pub fn markdown(content: String, location: TextLocation) -> Self {
        Self::Markdown { content, location }
    }

    /// Creates a YAML header token.
    pub fn yaml_header(content: String, location: TextLocation) -> Self {
        Self::YamlHeader { content, location }
    }

    /// Returns the location of this token.
    pub fn location(&self) -> &TextLocation {
        match self {
            Self::CodeBlock { location, .. } => location,
            Self::Markdown { location, .. } => location,
            Self::YamlHeader { location, .. } => location,
        }
    }

    /// Returns true if this is a code block.
    pub fn is_code_block(&self) -> bool {
        matches!(self, Self::CodeBlock { .. })
    }

    /// Returns true if this is markdown.
    pub fn is_markdown(&self) -> bool {
        matches!(self, Self::Markdown { .. })
    }

    /// Returns true if this is a YAML header.
    pub fn is_yaml_header(&self) -> bool {
        matches!(self, Self::YamlHeader { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_block_token() {
        let token = InputToken::code_block(
            "python".to_string(),
            "print('hello')".to_string(),
            TextLocation::line_only(10),
        );

        assert!(token.is_code_block());
        assert!(!token.is_markdown());
        assert_eq!(token.location().line, 10);
    }

    #[test]
    fn test_markdown_token() {
        let token = InputToken::markdown("# Header".to_string(), TextLocation::line_only(1));

        assert!(token.is_markdown());
        assert!(!token.is_code_block());
    }
}
