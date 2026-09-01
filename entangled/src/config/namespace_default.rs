//! Namespace default configuration.

use std::path::Path;

use serde::{Deserialize, Deserializer, Serialize};

/// How to handle default namespace for code blocks without explicit naming.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NamespaceDefault {
    /// Use the filename as the default namespace.
    /// Also known as "private" in Python Entangled.
    #[default]
    File,

    /// No default namespace - all blocks are in global scope.
    /// Also known as "global" in Python Entangled.
    None,
}

impl<'de> Deserialize<'de> for NamespaceDefault {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "file" | "private" => Ok(NamespaceDefault::File),
            "none" | "global" => Ok(NamespaceDefault::None),
            _ => Err(serde::de::Error::custom(format!(
                "unknown namespace_default: '{}' (expected 'file', 'private', 'none', or 'global')",
                s
            ))),
        }
    }
}

impl NamespaceDefault {
    /// Returns the namespace prefix for a source document.
    ///
    /// The whole (project-relative) path is used, not just the file name:
    /// `chapter/a.md` and `other/a.md` must not share a namespace, or their
    /// same-named blocks would collide in the project-wide reference map.
    /// Separators are normalised to `/` so the prefix is identical on every
    /// platform.
    pub fn prefix_for(&self, source_path: &Path) -> Option<String> {
        match self {
            NamespaceDefault::File => {
                let mut parts = Vec::new();
                for component in source_path.components() {
                    if let std::path::Component::Normal(part) = component {
                        parts.push(part.to_string_lossy().into_owned());
                    }
                }
                if parts.is_empty() {
                    None
                } else {
                    Some(parts.join("/"))
                }
            }
            NamespaceDefault::None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        assert_eq!(NamespaceDefault::default(), NamespaceDefault::File);
    }

    #[test]
    fn test_prefix_for_file() {
        let ns = NamespaceDefault::File;
        assert_eq!(
            ns.prefix_for(Path::new("test.md")),
            Some("test.md".to_string())
        );
    }

    #[test]
    fn test_prefix_for_nested_path_keeps_directories() {
        // Distinct directories must yield distinct namespaces, otherwise
        // `chapter/a.md#part` and `other/a.md#part` would be the same block.
        let ns = NamespaceDefault::File;
        assert_eq!(
            ns.prefix_for(Path::new("chapter/a.md")),
            Some("chapter/a.md".to_string())
        );
        assert_ne!(
            ns.prefix_for(Path::new("chapter/a.md")),
            ns.prefix_for(Path::new("other/a.md"))
        );
    }

    #[test]
    fn test_prefix_for_none() {
        let ns = NamespaceDefault::None;
        assert_eq!(ns.prefix_for(Path::new("test.md")), None);
    }

    #[test]
    fn test_serde() {
        // Test standard names
        let file: NamespaceDefault = serde_json::from_str("\"file\"").unwrap();
        assert_eq!(file, NamespaceDefault::File);

        let none: NamespaceDefault = serde_json::from_str("\"none\"").unwrap();
        assert_eq!(none, NamespaceDefault::None);

        // Test Python Entangled aliases
        let private: NamespaceDefault = serde_json::from_str("\"private\"").unwrap();
        assert_eq!(private, NamespaceDefault::File);

        let global: NamespaceDefault = serde_json::from_str("\"global\"").unwrap();
        assert_eq!(global, NamespaceDefault::None);
    }
}
