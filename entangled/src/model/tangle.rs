//! Tangle algorithm for expanding code block references.

use std::collections::HashSet;

use crate::config::{annotation_begin, annotation_end, Comment, Markers, REF_PATTERN};
use crate::errors::{EntangledError, Result};

use super::reference_map::ReferenceMap;
use super::reference_name::ReferenceName;

/// Cycle detector for preventing infinite loops during tangling.
#[derive(Debug, Clone, Default)]
pub struct CycleDetector {
    /// Stack of reference names currently being expanded (for error reporting).
    stack: Vec<ReferenceName>,
    /// Set for O(1) membership checks.
    seen: HashSet<ReferenceName>,
}

impl CycleDetector {
    /// Creates a new cycle detector.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enters a reference, checking for cycles.
    ///
    /// Returns an error if entering this reference would create a cycle.
    pub fn enter(&mut self, name: &ReferenceName) -> Result<()> {
        if self.seen.contains(name) {
            let mut cycle = self.stack.clone();
            cycle.push(name.clone());
            return Err(EntangledError::CycleDetected(cycle));
        }
        self.seen.insert(name.clone());
        self.stack.push(name.clone());
        Ok(())
    }

    /// Exits a reference.
    pub fn exit(&mut self) {
        if let Some(name) = self.stack.pop() {
            self.seen.remove(&name);
        }
    }

    /// Returns the current depth.
    pub fn depth(&self) -> usize {
        self.stack.len()
    }
}

/// Tangles a reference without annotations (naked output).
///
/// Expands all `<<refname>>` patterns recursively.
pub fn tangle_naked(
    refs: &ReferenceMap,
    name: &ReferenceName,
    base_indent: &str,
    detector: &mut CycleDetector,
) -> Result<String> {
    detector.enter(name)?;

    let source = refs.concatenate_source(name)?;
    let mut output = Vec::new();

    for line in source.lines() {
        if let Some(caps) = REF_PATTERN.captures(line) {
            let indent = &caps["indent"];
            let refname = &caps["refname"];
            let combined_indent = format!("{}{}", base_indent, indent);

            let ref_name = ReferenceName::new(refname);
            let expanded = tangle_naked(refs, &ref_name, &combined_indent, detector)?;
            output.push(expanded);
        } else {
            output.push(format!("{}{}", base_indent, line));
        }
    }

    detector.exit();
    Ok(output.join("\n"))
}

/// Tangles a reference with annotation comments.
///
/// Adds begin/end markers around each expanded reference.
pub fn tangle_annotated(
    refs: &ReferenceMap,
    name: &ReferenceName,
    base_indent: &str,
    comment: &Comment,
    markers: &Markers,
    detector: &mut CycleDetector,
) -> Result<String> {
    detector.enter(name)?;

    let ids = refs.get_ids_by_name(name);
    if ids.is_empty() {
        detector.exit();
        return Err(EntangledError::ReferenceNotFound(name.clone()));
    }

    let mut output = Vec::new();
    let prefix = comment.prefix();

    for id in ids {
        let block = refs.get(id).ok_or_else(|| {
            EntangledError::Other(format!(
                "Internal error: ReferenceMap has ID {} in name index but not in block storage",
                id
            ))
        })?;

        // Add begin marker
        let begin_marker = format!(
            "{}{}",
            base_indent,
            annotation_begin(prefix, markers, &id.to_string())
        );
        output.push(begin_marker);

        // Process source lines
        for line in block.source.lines() {
            if let Some(caps) = REF_PATTERN.captures(line) {
                let indent = &caps["indent"];
                let refname = &caps["refname"];
                let combined_indent = format!("{}{}", base_indent, indent);

                let ref_name = ReferenceName::new(refname);
                let expanded = tangle_annotated(
                    refs,
                    &ref_name,
                    &combined_indent,
                    comment,
                    markers,
                    detector,
                )?;
                output.push(expanded);
            } else {
                output.push(format!("{}{}", base_indent, line));
            }
        }

        // Add end marker
        let end_marker = format!("{}{}", base_indent, annotation_end(prefix, markers));
        output.push(end_marker);
    }

    detector.exit();
    Ok(output.join("\n"))
}

/// Tangles a reference with blank-line separators between blocks (bare output).
///
/// Like `tangle_annotated` but emits blank lines instead of marker comments,
/// then collapses consecutive blank lines and trims leading/trailing blanks.
pub fn tangle_bare(
    refs: &ReferenceMap,
    name: &ReferenceName,
    base_indent: &str,
    detector: &mut CycleDetector,
) -> Result<String> {
    detector.enter(name)?;

    let ids = refs.get_ids_by_name(name);
    if ids.is_empty() {
        detector.exit();
        return Err(EntangledError::ReferenceNotFound(name.clone()));
    }

    let mut output = Vec::new();

    for id in ids {
        let block = refs.get(id).ok_or_else(|| {
            EntangledError::Other(format!(
                "Internal error: ReferenceMap has ID {} in name index but not in block storage",
                id
            ))
        })?;

        // Blank line as block separator
        output.push(String::new());

        // Process source lines
        for line in block.source.lines() {
            if let Some(caps) = REF_PATTERN.captures(line) {
                let indent = &caps["indent"];
                let refname = &caps["refname"];
                let combined_indent = format!("{}{}", base_indent, indent);

                let ref_name = ReferenceName::new(refname);
                let expanded = tangle_bare(refs, &ref_name, &combined_indent, detector)?;
                output.push(expanded);
            } else {
                output.push(format!("{}{}", base_indent, line));
            }
        }

        // Blank line as block separator
        output.push(String::new());
    }

    detector.exit();
    let joined = output.join("\n");
    Ok(collapse_blank_lines(&joined))
}

/// Collapses runs of 2+ consecutive blank lines into a single blank line,
/// and trims leading/trailing blank lines.
fn collapse_blank_lines(s: &str) -> String {
    let mut result = Vec::new();
    let mut prev_blank = false;

    for line in s.lines() {
        let is_blank = line.trim().is_empty();
        if is_blank {
            if !prev_blank {
                result.push("");
            }
            prev_blank = true;
        } else {
            result.push(line);
            prev_blank = false;
        }
    }

    // Trim leading/trailing blank lines
    while result.first() == Some(&"") {
        result.remove(0);
    }
    while result.last() == Some(&"") {
        result.pop();
    }

    result.join("\n")
}

/// Tangles a single reference (entry point).
///
/// This is a convenience function that creates a cycle detector and tangles
/// with or without annotations based on the `annotated` parameter.
///
/// Dispatch:
/// - `(Some(comment), Some(markers))` → annotated output
/// - `(None, Some(markers))` → bare output (blank-line separators)
/// - `_` → naked output
pub fn tangle_ref(
    refs: &ReferenceMap,
    name: &ReferenceName,
    comment: Option<&Comment>,
    markers: Option<&Markers>,
) -> Result<String> {
    let mut detector = CycleDetector::new();

    match (comment, markers) {
        (Some(c), Some(m)) => tangle_annotated(refs, name, "", c, m, &mut detector),
        (None, Some(_)) => tangle_bare(refs, name, "", &mut detector),
        _ => tangle_naked(refs, name, "", &mut detector),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::make_block;

    #[test]
    fn test_tangle_naked_simple() {
        let mut refs = ReferenceMap::new();
        refs.insert(make_block("main", "print('hello')\nprint('world')"));

        let result = tangle_ref(&refs, &ReferenceName::new("main"), None, None).unwrap();
        assert_eq!(result, "print('hello')\nprint('world')");
    }

    #[test]
    fn test_tangle_naked_with_reference() {
        let mut refs = ReferenceMap::new();
        refs.insert(make_block("main", "def main():\n    <<body>>"));
        refs.insert(make_block("body", "print('hello')"));

        let result = tangle_ref(&refs, &ReferenceName::new("main"), None, None).unwrap();
        assert_eq!(result, "def main():\n    print('hello')");
    }

    #[test]
    fn test_tangle_naked_nested_indentation() {
        let mut refs = ReferenceMap::new();
        refs.insert(make_block("main", "if True:\n    <<inner>>"));
        refs.insert(make_block("inner", "if True:\n    <<deepest>>"));
        refs.insert(make_block("deepest", "print('deep')"));

        let result = tangle_ref(&refs, &ReferenceName::new("main"), None, None).unwrap();
        assert_eq!(result, "if True:\n    if True:\n        print('deep')");
    }

    #[test]
    fn test_tangle_cycle_detection() {
        let mut refs = ReferenceMap::new();
        refs.insert(make_block("a", "<<b>>"));
        refs.insert(make_block("b", "<<c>>"));
        refs.insert(make_block("c", "<<a>>"));

        let result = tangle_ref(&refs, &ReferenceName::new("a"), None, None);
        assert!(matches!(result, Err(EntangledError::CycleDetected(_))));
    }

    #[test]
    fn test_tangle_annotated() {
        let mut refs = ReferenceMap::new();
        refs.insert(make_block("main", "print('hello')"));

        let comment = Comment::line("#");
        let markers = Markers::default();

        let result = tangle_ref(
            &refs,
            &ReferenceName::new("main"),
            Some(&comment),
            Some(&markers),
        )
        .unwrap();

        assert!(result.contains("# ~/~ begin <<main[0]>>"));
        assert!(result.contains("print('hello')"));
        assert!(result.contains("# ~/~ end"));
    }

    #[test]
    fn test_tangle_annotated_with_reference() {
        let mut refs = ReferenceMap::new();
        refs.insert(make_block("main", "def main():\n    <<body>>"));
        refs.insert(make_block("body", "pass"));

        let comment = Comment::line("#");
        let markers = Markers::default();

        let result = tangle_ref(
            &refs,
            &ReferenceName::new("main"),
            Some(&comment),
            Some(&markers),
        )
        .unwrap();

        assert!(result.contains("# ~/~ begin <<main[0]>>"));
        assert!(result.contains("    # ~/~ begin <<body[0]>>"));
        assert!(result.contains("    pass"));
        assert!(result.contains("    # ~/~ end"));
        assert!(result.contains("# ~/~ end"));
    }

    #[test]
    fn test_tangle_multiple_blocks_same_name() {
        let mut refs = ReferenceMap::new();
        refs.insert(make_block("main", "line1"));
        refs.insert(make_block("main", "line2"));

        let result = tangle_ref(&refs, &ReferenceName::new("main"), None, None).unwrap();
        assert_eq!(result, "line1\nline2");
    }

    #[test]
    fn test_tangle_not_found() {
        let refs = ReferenceMap::new();
        let result = tangle_ref(&refs, &ReferenceName::new("nonexistent"), None, None);
        assert!(matches!(result, Err(EntangledError::ReferenceNotFound(_))));
    }

    #[test]
    fn test_tangle_bare_simple() {
        let mut refs = ReferenceMap::new();
        refs.insert(make_block("main", "print('hello')"));

        let markers = Markers::default();
        let result = tangle_ref(&refs, &ReferenceName::new("main"), None, Some(&markers)).unwrap();
        assert_eq!(result, "print('hello')");
        // No annotation markers
        assert!(!result.contains("~/~"));
    }

    #[test]
    fn test_tangle_bare_multiple_blocks() {
        let mut refs = ReferenceMap::new();
        refs.insert(make_block("main", "line1"));
        refs.insert(make_block("main", "line2"));

        let markers = Markers::default();
        let result = tangle_ref(&refs, &ReferenceName::new("main"), None, Some(&markers)).unwrap();
        // Blocks separated by a single blank line
        assert_eq!(result, "line1\n\nline2");
    }

    #[test]
    fn test_tangle_bare_with_reference() {
        let mut refs = ReferenceMap::new();
        refs.insert(make_block("main", "def main():\n    <<body>>"));
        refs.insert(make_block("body", "print('hello')"));

        let markers = Markers::default();
        let result = tangle_ref(&refs, &ReferenceName::new("main"), None, Some(&markers)).unwrap();
        assert!(result.contains("def main():"));
        assert!(result.contains("    print('hello')"));
        assert!(!result.contains("~/~"));
    }

    #[test]
    fn test_tangle_bare_adjacent_references() {
        let mut refs = ReferenceMap::new();
        refs.insert(make_block("main", "<<a>>\n<<b>>"));
        refs.insert(make_block("a", "alpha"));
        refs.insert(make_block("b", "beta"));

        let markers = Markers::default();
        let result = tangle_ref(&refs, &ReferenceName::new("main"), None, Some(&markers)).unwrap();
        assert!(result.contains("alpha"));
        assert!(result.contains("beta"));
        assert!(!result.contains("~/~"));
    }

    #[test]
    fn test_collapse_blank_lines() {
        assert_eq!(collapse_blank_lines("a\n\n\n\nb"), "a\n\nb");
        assert_eq!(collapse_blank_lines("\n\na\n\nb\n\n"), "a\n\nb");
        assert_eq!(collapse_blank_lines("a\nb"), "a\nb");
        assert_eq!(collapse_blank_lines(""), "");
    }

    #[test]
    fn test_cycle_detector() {
        let mut detector = CycleDetector::new();

        detector.enter(&ReferenceName::new("a")).unwrap();
        detector.enter(&ReferenceName::new("b")).unwrap();
        detector.enter(&ReferenceName::new("c")).unwrap();

        assert_eq!(detector.depth(), 3);

        // Trying to enter 'a' again should fail
        let result = detector.enter(&ReferenceName::new("a"));
        assert!(result.is_err());

        detector.exit();
        detector.exit();
        detector.exit();

        assert_eq!(detector.depth(), 0);
    }
}
