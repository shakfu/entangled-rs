//! Reference IDs for uniquely identifying code blocks.

use std::fmt;

use super::ReferenceName;

/// A reference ID uniquely identifies a code block instance.
///
/// Multiple code blocks can have the same name (they get concatenated),
/// so we need an ID that includes the instance count.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReferenceId {
    /// The reference name.
    pub name: ReferenceName,
    /// The instance count (0-indexed).
    pub count: usize,
}

impl ReferenceId {
    /// Creates a new ReferenceId.
    pub fn new(name: ReferenceName, count: usize) -> Self {
        Self { name, count }
    }

    /// Creates a ReferenceId with count 0.
    pub fn first(name: ReferenceName) -> Self {
        Self { name, count: 0 }
    }

    /// Parses a reference ID from string format "name[count]".
    pub fn parse(s: &str) -> Option<Self> {
        if let Some(bracket_pos) = s.rfind('[') {
            if s.ends_with(']') {
                let name = &s[..bracket_pos];
                let count_str = &s[bracket_pos + 1..s.len() - 1];
                if let Ok(count) = count_str.parse::<usize>() {
                    return Some(Self::new(ReferenceName::new(name), count));
                }
            }
        }
        None
    }
}

impl fmt::Display for ReferenceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}[{}]", self.name, self.count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let id = ReferenceId::new(ReferenceName::new("test"), 5);
        assert_eq!(id.name.as_str(), "test");
        assert_eq!(id.count, 5);
    }

    #[test]
    fn test_first() {
        let id = ReferenceId::first(ReferenceName::new("main"));
        assert_eq!(id.count, 0);
    }

    #[test]
    fn test_display() {
        let id = ReferenceId::new(ReferenceName::new("function"), 2);
        assert_eq!(format!("{}", id), "function[2]");
    }

    #[test]
    fn test_parse() {
        let id = ReferenceId::parse("test::name[3]").unwrap();
        assert_eq!(id.name.as_str(), "test::name");
        assert_eq!(id.count, 3);
    }

    #[test]
    fn test_parse_invalid() {
        assert!(ReferenceId::parse("no_brackets").is_none());
        assert!(ReferenceId::parse("bad[count]").is_none());
        assert!(ReferenceId::parse("unclosed[3").is_none());
    }

    #[test]
    fn test_equality() {
        let id1 = ReferenceId::new(ReferenceName::new("test"), 1);
        let id2 = ReferenceId::new(ReferenceName::new("test"), 1);
        let id3 = ReferenceId::new(ReferenceName::new("test"), 2);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }
}
