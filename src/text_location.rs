//! Text location tracking for error reporting.

use std::fmt;
use std::path::PathBuf;

/// Represents a location within a text file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextLocation {
    /// The file path (if known).
    pub filename: Option<PathBuf>,
    /// Line number (1-indexed).
    pub line: usize,
    /// Column number (1-indexed).
    pub column: usize,
}

impl TextLocation {
    /// Creates a new TextLocation.
    pub fn new(filename: Option<PathBuf>, line: usize, column: usize) -> Self {
        Self {
            filename,
            line,
            column,
        }
    }

    /// Creates a TextLocation with only line information.
    pub fn line_only(line: usize) -> Self {
        Self {
            filename: None,
            line,
            column: 1,
        }
    }

    /// Creates a TextLocation with file and line.
    pub fn file_line(filename: PathBuf, line: usize) -> Self {
        Self {
            filename: Some(filename),
            line,
            column: 1,
        }
    }

    /// Returns a new location with updated filename.
    pub fn with_filename(mut self, filename: PathBuf) -> Self {
        self.filename = Some(filename);
        self
    }
}

impl Default for TextLocation {
    fn default() -> Self {
        Self {
            filename: None,
            line: 1,
            column: 1,
        }
    }
}

impl fmt::Display for TextLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.filename {
            Some(path) => write!(f, "{}:{}:{}", path.display(), self.line, self.column),
            None => write!(f, "line {}:{}", self.line, self.column),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_with_filename() {
        let loc = TextLocation::new(Some(PathBuf::from("test.md")), 10, 5);
        assert_eq!(format!("{}", loc), "test.md:10:5");
    }

    #[test]
    fn test_display_without_filename() {
        let loc = TextLocation::new(None, 10, 5);
        assert_eq!(format!("{}", loc), "line 10:5");
    }

    #[test]
    fn test_line_only() {
        let loc = TextLocation::line_only(42);
        assert_eq!(loc.line, 42);
        assert_eq!(loc.column, 1);
        assert!(loc.filename.is_none());
    }
}
