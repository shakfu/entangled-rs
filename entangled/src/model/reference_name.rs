//! Reference names for code blocks.

use std::fmt;

/// A reference name identifies a named code block.
///
/// Names can include namespaces separated by `::`, e.g., `module::submodule::name`.
/// They can also be file targets like `file:path/to/output.py`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReferenceName(String);

impl ReferenceName {
    /// Creates a new ReferenceName from a string.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns the name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Checks if this is a file target (starts with "file:").
    pub fn is_file_target(&self) -> bool {
        self.0.starts_with("file:")
    }

    /// Returns the file path if this is a file target.
    pub fn file_path(&self) -> Option<&str> {
        if self.is_file_target() {
            Some(&self.0[5..])
        } else {
            None
        }
    }

    /// Returns the namespace parts of the name.
    pub fn namespace_parts(&self) -> Vec<&str> {
        let base = if self.is_file_target() {
            &self.0[5..]
        } else {
            &self.0
        };
        base.split("::").collect()
    }

    /// Returns the base name (last component after ::).
    pub fn base_name(&self) -> &str {
        let parts = self.namespace_parts();
        parts.last().copied().unwrap_or(&self.0)
    }

    /// Creates a file target reference name from a path.
    pub fn from_file_path(path: &str) -> Self {
        Self(format!("file:{}", path))
    }
}

impl fmt::Display for ReferenceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for ReferenceName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ReferenceName {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for ReferenceName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_name() {
        let name = ReferenceName::new("main");
        assert_eq!(name.as_str(), "main");
        assert!(!name.is_file_target());
        assert_eq!(name.base_name(), "main");
    }

    #[test]
    fn test_namespaced_name() {
        let name = ReferenceName::new("module::submodule::function");
        assert_eq!(
            name.namespace_parts(),
            vec!["module", "submodule", "function"]
        );
        assert_eq!(name.base_name(), "function");
    }

    #[test]
    fn test_file_target() {
        let name = ReferenceName::from_file_path("src/main.rs");
        assert!(name.is_file_target());
        assert_eq!(name.file_path(), Some("src/main.rs"));
        assert_eq!(name.as_str(), "file:src/main.rs");
    }

    #[test]
    fn test_display() {
        let name = ReferenceName::new("test::name");
        assert_eq!(format!("{}", name), "test::name");
    }
}
