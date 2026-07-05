//! Annotation method configuration.

use serde::{Deserialize, Serialize};

/// How to annotate tangled output files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnnotationMethod {
    /// Add annotation comments showing source references.
    #[default]
    Standard,

    /// No annotations, just raw code.
    Naked,

    /// No annotations, but blank lines between block boundaries.
    Bare,

    /// Annotations for supplemental/weaved output.
    Supplemental,
}

impl AnnotationMethod {
    /// Returns true if this method produces annotations.
    pub fn has_annotations(&self) -> bool {
        matches!(
            self,
            AnnotationMethod::Standard | AnnotationMethod::Supplemental
        )
    }

    /// Returns true if this method produces no annotations and no stitch support.
    pub fn is_one_way(&self) -> bool {
        matches!(self, AnnotationMethod::Naked | AnnotationMethod::Bare)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        assert_eq!(AnnotationMethod::default(), AnnotationMethod::Standard);
    }

    #[test]
    fn test_has_annotations() {
        assert!(AnnotationMethod::Standard.has_annotations());
        assert!(AnnotationMethod::Supplemental.has_annotations());
        assert!(!AnnotationMethod::Naked.has_annotations());
        assert!(!AnnotationMethod::Bare.has_annotations());
    }

    #[test]
    fn test_serde() {
        let standard: AnnotationMethod = serde_json::from_str("\"standard\"").unwrap();
        assert_eq!(standard, AnnotationMethod::Standard);

        let naked: AnnotationMethod = serde_json::from_str("\"naked\"").unwrap();
        assert_eq!(naked, AnnotationMethod::Naked);

        let bare: AnnotationMethod = serde_json::from_str("\"bare\"").unwrap();
        assert_eq!(bare, AnnotationMethod::Bare);
    }
}
