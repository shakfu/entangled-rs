//! Code block representation.

use std::path::PathBuf;

use super::reference_id::ReferenceId;
use super::reference_name::ReferenceName;
use crate::text_location::TextLocation;

/// A code block extracted from a markdown document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlock {
    /// Unique identifier for this code block instance.
    pub id: ReferenceId,

    /// The language identifier (e.g., "python", "rust").
    pub language: Option<String>,

    /// Additional classes from the code fence.
    pub classes: Vec<String>,

    /// Target output file, if specified.
    pub target: Option<PathBuf>,

    /// The source code content.
    pub source: String,

    /// Location in the source document.
    pub location: TextLocation,

    /// Additional attributes from the code fence.
    pub attributes: Vec<(String, String)>,
}

impl CodeBlock {
    /// Creates a new CodeBlock.
    pub fn new(
        id: ReferenceId,
        language: Option<String>,
        source: String,
        location: TextLocation,
    ) -> Self {
        Self {
            id,
            language,
            classes: Vec::new(),
            target: None,
            source,
            location,
            attributes: Vec::new(),
        }
    }

    /// Returns the reference name for this block.
    pub fn name(&self) -> &ReferenceName {
        &self.id.name
    }

    /// Returns true if this block has a target file.
    pub fn has_target(&self) -> bool {
        self.target.is_some()
    }

    /// Returns the line count of the source.
    pub fn line_count(&self) -> usize {
        self.source.lines().count()
    }

    /// Returns true if the source is empty or whitespace only.
    pub fn is_empty(&self) -> bool {
        self.source.trim().is_empty()
    }

    /// Sets the target file.
    pub fn with_target(mut self, target: PathBuf) -> Self {
        self.target = Some(target);
        self
    }

    /// Adds a class.
    pub fn with_class(mut self, class: String) -> Self {
        self.classes.push(class);
        self
    }

    /// Adds an attribute.
    pub fn with_attribute(mut self, key: String, value: String) -> Self {
        self.attributes.push((key, value));
        self
    }

    /// Gets an attribute value by key.
    pub fn get_attribute(&self, key: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_id(name: &str, count: usize) -> ReferenceId {
        ReferenceId::new(ReferenceName::new(name), count)
    }

    #[test]
    fn test_new_code_block() {
        let block = CodeBlock::new(
            make_id("main", 0),
            Some("python".to_string()),
            "print('hello')".to_string(),
            TextLocation::line_only(10),
        );

        assert_eq!(block.name().as_str(), "main");
        assert_eq!(block.language, Some("python".to_string()));
        assert_eq!(block.source, "print('hello')");
        assert_eq!(block.location.line, 10);
    }

    #[test]
    fn test_with_target() {
        let block = CodeBlock::new(
            make_id("main", 0),
            Some("python".to_string()),
            "".to_string(),
            TextLocation::default(),
        )
        .with_target(PathBuf::from("output.py"));

        assert!(block.has_target());
        assert_eq!(block.target, Some(PathBuf::from("output.py")));
    }

    #[test]
    fn test_line_count() {
        let block = CodeBlock::new(
            make_id("test", 0),
            None,
            "line1\nline2\nline3".to_string(),
            TextLocation::default(),
        );

        assert_eq!(block.line_count(), 3);
    }

    #[test]
    fn test_is_empty() {
        let empty = CodeBlock::new(
            make_id("empty", 0),
            None,
            "   \n  ".to_string(),
            TextLocation::default(),
        );
        assert!(empty.is_empty());

        let non_empty = CodeBlock::new(
            make_id("non_empty", 0),
            None,
            "code".to_string(),
            TextLocation::default(),
        );
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn test_attributes() {
        let block = CodeBlock::new(
            make_id("test", 0),
            None,
            "".to_string(),
            TextLocation::default(),
        )
        .with_attribute("mode".to_string(), "0755".to_string())
        .with_attribute("exec".to_string(), "true".to_string());

        assert_eq!(block.get_attribute("mode"), Some("0755"));
        assert_eq!(block.get_attribute("exec"), Some("true"));
        assert_eq!(block.get_attribute("nonexistent"), None);
    }
}
