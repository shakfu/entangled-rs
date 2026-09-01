//! Authoring-time validation of a literate project.
//!
//! [`check_documents`] parses every source file into one combined reference map
//! and reports structural problems that would otherwise only be discovered at
//! tangle time (or not at all):
//!
//! - **dangling reference** -- a `<<name>>` with no defining block (error);
//! - **target collision** -- two different block names writing the same `file=`
//!   target (error);
//! - **reference cycle** -- blocks that reference each other in a loop (error);
//! - **orphan block** -- a named block that is neither a file target nor
//!   referenced by any other block, so it is never tangled (warning).
//!
//! The intent is a fast, CI-friendly gate: `entangled check` exits non-zero when
//! any error is found.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::config::REF_PATTERN;
use crate::errors::Result;
use crate::interface::{combined_reference_map, Context};
use crate::model::ReferenceMap;
use crate::text_location::TextLocation;

/// Severity of a [`Finding`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// A problem that will break tangling or produce wrong output.
    Error,
    /// A suspicious-but-not-fatal condition.
    Warning,
}

/// A single validation finding.
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    /// How serious the finding is.
    pub severity: Severity,
    /// Stable machine-readable category slug.
    pub kind: &'static str,
    /// Human-readable description.
    pub message: String,
    /// Source file the finding relates to, if known.
    pub file: Option<String>,
    /// 1-indexed source line, if known.
    pub line: Option<usize>,
}

impl Finding {
    fn new(
        severity: Severity,
        kind: &'static str,
        message: String,
        loc: Option<&TextLocation>,
    ) -> Self {
        Self {
            severity,
            kind,
            message,
            file: loc
                .and_then(|l| l.filename.as_ref())
                .map(|p| p.display().to_string()),
            line: loc.map(|l| l.line).filter(|&n| n > 0),
        }
    }
}

/// Validates all source files and returns findings (errors first, then warnings;
/// stable order within each severity).
pub fn check_documents(ctx: &Context) -> Result<Vec<Finding>> {
    let refs = combined_reference_map(ctx, &ctx.source_files()?)?;
    Ok(check_refs(&refs))
}

/// Validates an already-built reference map.
pub fn check_refs(refs: &ReferenceMap) -> Vec<Finding> {
    let defined: BTreeSet<String> = refs.names().map(|n| n.as_str().to_string()).collect();

    // Build the reference graph and collect the set of referenced names.
    let mut adjacency: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut referenced: BTreeSet<String> = BTreeSet::new();
    let mut roots: BTreeSet<String> = BTreeSet::new();
    let mut runnable: BTreeSet<String> = BTreeSet::new();
    let mut target_owners: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut seen_dangling: BTreeSet<(String, String)> = BTreeSet::new();

    for block in refs.blocks() {
        let name = block.id.name.as_str().to_string();

        if let Some(target) = &block.target {
            roots.insert(name.clone());
            target_owners
                .entry(target.display().to_string())
                .or_default()
                .insert(name.clone());
        }

        // Runnable blocks (`eval=`) are intentionally standalone -- executed by
        // `entangled eval`, not tangled or referenced -- so they are not orphans.
        if block.get_attribute("eval").is_some() {
            runnable.insert(name.clone());
        }

        for line in block.source.lines() {
            if let Some(caps) = REF_PATTERN.captures(line) {
                // Resolve exactly as tangle does, so `check` agrees with what
                // tangling would actually expand -- including a bare reference
                // that resolves inside its own document's file namespace.
                let refname = refs
                    .resolve_reference(&block.id.name, &caps["refname"])
                    .as_str()
                    .to_string();
                referenced.insert(refname.clone());
                if defined.contains(&refname) {
                    adjacency
                        .entry(name.clone())
                        .or_default()
                        .insert(refname.clone());
                } else if seen_dangling.insert((name.clone(), refname.clone())) {
                    errors.push(Finding::new(
                        Severity::Error,
                        "dangling-reference",
                        format!("block `{name}` references `<<{refname}>>`, which is not defined"),
                        Some(&block.location),
                    ));
                }
            }
        }
    }

    // Target collisions: one output file claimed by two different block names.
    for (target, owners) in &target_owners {
        if owners.len() > 1 {
            let names = owners
                .iter()
                .map(|s| format!("`{s}`"))
                .collect::<Vec<_>>()
                .join(", ");
            errors.push(Finding::new(
                Severity::Error,
                "target-collision",
                format!("output file `{target}` is written by multiple blocks: {names}"),
                None,
            ));
        }
    }

    // Reference cycles.
    for cycle in find_cycles(&adjacency) {
        errors.push(Finding::new(
            Severity::Error,
            "reference-cycle",
            format!("reference cycle: {}", cycle.join(" -> ")),
            None,
        ));
    }

    // Orphans: defined, not a root, not runnable, and never referenced.
    for name in &defined {
        if !roots.contains(name) && !referenced.contains(name) && !runnable.contains(name) {
            warnings.push(Finding::new(
                Severity::Warning,
                "orphan-block",
                format!("block `{name}` is never referenced or tangled (orphan)"),
                None,
            ));
        }
    }

    errors.extend(warnings);
    errors
}

/// Returns true if any finding is an error.
pub fn has_errors(findings: &[Finding]) -> bool {
    findings.iter().any(|f| f.severity == Severity::Error)
}

/// Finds cycles in the reference graph via DFS, returning one representative
/// path per distinct cycle (as a closed loop `a -> b -> a`).
fn find_cycles(adjacency: &BTreeMap<String, BTreeSet<String>>) -> Vec<Vec<String>> {
    #[derive(Clone, Copy, PartialEq)]
    enum Color {
        White,
        Gray,
        Black,
    }

    let mut color: BTreeMap<&str, Color> = BTreeMap::new();
    for k in adjacency.keys() {
        color.insert(k.as_str(), Color::White);
    }

    let mut cycles: Vec<Vec<String>> = Vec::new();
    let mut seen_cycles: BTreeSet<BTreeSet<String>> = BTreeSet::new();

    // Iterative DFS with an explicit stack recording the active path.
    for start in adjacency.keys() {
        if color.get(start.as_str()).copied() != Some(Color::White) {
            continue;
        }
        // (node, index of next child to visit)
        let mut path: Vec<&str> = Vec::new();
        let mut stack: Vec<(&str, usize)> = vec![(start.as_str(), 0)];
        color.insert(start.as_str(), Color::Gray);
        path.push(start.as_str());

        while let Some(&mut (node, ref mut idx)) = stack.last_mut() {
            let children = adjacency.get(node);
            let next = children.and_then(|set| set.iter().nth(*idx));
            match next {
                Some(child) => {
                    *idx += 1;
                    let child = child.as_str();
                    match color.get(child).copied().unwrap_or(Color::Black) {
                        Color::White => {
                            color.insert(child, Color::Gray);
                            path.push(child);
                            stack.push((child, 0));
                        }
                        Color::Gray => {
                            // Back-edge: extract the loop from the active path.
                            if let Some(pos) = path.iter().position(|&n| n == child) {
                                let mut loop_nodes: Vec<String> =
                                    path[pos..].iter().map(|s| s.to_string()).collect();
                                let key: BTreeSet<String> = loop_nodes.iter().cloned().collect();
                                if seen_cycles.insert(key) {
                                    loop_nodes.push(child.to_string());
                                    cycles.push(loop_nodes);
                                }
                            }
                        }
                        Color::Black => {}
                    }
                }
                None => {
                    color.insert(node, Color::Black);
                    path.pop();
                    stack.pop();
                }
            }
        }
    }

    cycles
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, NamespaceDefault};
    use crate::readers::parse_markdown;

    fn refs_from(input: &str) -> ReferenceMap {
        let c = Config {
            namespace_default: NamespaceDefault::None,
            ..Default::default()
        };
        parse_markdown(input, None, &c).unwrap().refs
    }

    fn kinds(findings: &[Finding]) -> Vec<&str> {
        findings.iter().map(|f| f.kind).collect()
    }

    #[test]
    fn clean_document_has_no_findings() {
        let refs = refs_from(
            "```python #main file=out.py\n<<body>>\n```\n\n```python #body\nprint(1)\n```\n",
        );
        assert!(check_refs(&refs).is_empty());
    }

    #[test]
    fn detects_dangling_reference() {
        let refs = refs_from("```python #main file=out.py\n<<missing>>\n```\n");
        let findings = check_refs(&refs);
        assert!(kinds(&findings).contains(&"dangling-reference"));
        assert!(has_errors(&findings));
        assert!(findings[0].message.contains("missing"));
    }

    #[test]
    fn detects_target_collision() {
        let refs = refs_from(
            "```python #a file=out.py\nprint(1)\n```\n\n```python #b file=out.py\nprint(2)\n```\n",
        );
        let findings = check_refs(&refs);
        assert!(kinds(&findings).contains(&"target-collision"));
    }

    #[test]
    fn same_name_same_target_is_not_a_collision() {
        // Two blocks with the same name and target concatenate -- legal.
        let refs = refs_from(
            "```python #a file=out.py\nprint(1)\n```\n\n```python #a file=out.py\nprint(2)\n```\n",
        );
        assert!(!kinds(&check_refs(&refs)).contains(&"target-collision"));
    }

    #[test]
    fn detects_reference_cycle() {
        let refs = refs_from("```python #a file=out.py\n<<b>>\n```\n\n```python #b\n<<a>>\n```\n");
        let findings = check_refs(&refs);
        assert!(kinds(&findings).contains(&"reference-cycle"));
    }

    #[test]
    fn detects_orphan_block() {
        // `lonely` has no target and nobody references it.
        let refs = refs_from(
            "```python #main file=out.py\nprint(1)\n```\n\n```python #lonely\nprint(2)\n```\n",
        );
        let findings = check_refs(&refs);
        assert!(kinds(&findings).contains(&"orphan-block"));
        // Orphan is a warning, not an error.
        assert!(!has_errors(
            &findings
                .iter()
                .filter(|f| f.kind == "orphan-block")
                .cloned()
                .collect::<Vec<_>>()
        ));
    }

    #[test]
    fn errors_sort_before_warnings() {
        let refs = refs_from(
            "```python #main file=out.py\n<<missing>>\n```\n\n```python #lonely\nx\n```\n",
        );
        let findings = check_refs(&refs);
        assert_eq!(findings[0].severity, Severity::Error);
        assert_eq!(findings.last().unwrap().severity, Severity::Warning);
    }

    #[test]
    fn runnable_block_is_not_an_orphan() {
        // An eval= block is standalone by design and must not warn as orphan.
        let refs = refs_from(
            "```python #main file=out.py\nprint(1)\n```\n\n```python #demo eval=python\nprint(2)\n```\n",
        );
        assert!(!kinds(&check_refs(&refs)).contains(&"orphan-block"));
    }

    #[test]
    fn self_cycle_is_detected() {
        let refs = refs_from("```python #loop file=out.py\n<<loop>>\n```\n");
        assert!(kinds(&check_refs(&refs)).contains(&"reference-cycle"));
    }
}
