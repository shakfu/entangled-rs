//! Reference-dependency graph of a literate project.
//!
//! [`graph_documents`] renders how code blocks reference one another, as either
//! Graphviz DOT or Mermaid. Nodes are block names; an edge `a -> b` means block
//! `a` contains `<<b>>`. File-target blocks (tangle roots) and undefined
//! references (dangling) are styled distinctly so the shape of the program --
//! and any broken links -- is visible at a glance.

use std::collections::{BTreeMap, BTreeSet};

use crate::config::REF_PATTERN;
use crate::errors::Result;
use crate::interface::{combined_reference_map, Context};
use crate::model::ReferenceMap;

/// Output format for [`graph_documents`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphFormat {
    /// Graphviz DOT.
    Dot,
    /// Mermaid `graph` syntax.
    Mermaid,
}

impl std::str::FromStr for GraphFormat {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "dot" | "graphviz" => Ok(GraphFormat::Dot),
            "mermaid" | "mmd" => Ok(GraphFormat::Mermaid),
            other => Err(format!(
                "unknown graph format '{other}' (use dot or mermaid)"
            )),
        }
    }
}

/// The extracted dependency graph.
struct Graph {
    /// All node names, in stable order.
    nodes: BTreeSet<String>,
    /// Names that are tangle roots (have a `file=` target).
    roots: BTreeSet<String>,
    /// Names referenced but never defined (dangling).
    missing: BTreeSet<String>,
    /// Edges `from -> to`.
    edges: BTreeSet<(String, String)>,
}

/// Renders the reference graph of all source files in the given format.
pub fn graph_documents(ctx: &Context, format: GraphFormat) -> Result<String> {
    let refs = combined_reference_map(ctx, &ctx.source_files()?)?;
    Ok(render_graph(&refs, format))
}

/// Renders an already-built reference map in the given format.
pub fn render_graph(refs: &ReferenceMap, format: GraphFormat) -> String {
    let graph = build_graph(refs);
    match format {
        GraphFormat::Dot => render_dot(&graph),
        GraphFormat::Mermaid => render_mermaid(&graph),
    }
}

fn build_graph(refs: &ReferenceMap) -> Graph {
    let defined: BTreeSet<String> = refs.names().map(|n| n.as_str().to_string()).collect();
    let mut nodes = defined.clone();
    let mut roots = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut edges = BTreeSet::new();

    for block in refs.blocks() {
        let name = block.id.name.as_str().to_string();
        if block.target.is_some() {
            roots.insert(name.clone());
        }
        for line in block.source.lines() {
            if let Some(caps) = REF_PATTERN.captures(line) {
                let refname = caps["refname"].to_string();
                if !defined.contains(&refname) {
                    missing.insert(refname.clone());
                }
                nodes.insert(refname.clone());
                edges.insert((name.clone(), refname));
            }
        }
    }

    Graph {
        nodes,
        roots,
        missing,
        edges,
    }
}

fn render_dot(graph: &Graph) -> String {
    let mut out =
        String::from("digraph entangled {\n  rankdir=LR;\n  node [fontname=\"monospace\"];\n");
    for node in &graph.nodes {
        let label = dot_escape(node);
        if graph.missing.contains(node) {
            out.push_str(&format!(
                "  \"{label}\" [shape=box, style=dashed, color=\"#b91c1c\", fontcolor=\"#b91c1c\"];\n"
            ));
        } else if graph.roots.contains(node) {
            out.push_str(&format!(
                "  \"{label}\" [shape=box, style=filled, fillcolor=\"#dbeafe\"];\n"
            ));
        } else {
            out.push_str(&format!("  \"{label}\";\n"));
        }
    }
    for (from, to) in &graph.edges {
        out.push_str(&format!(
            "  \"{}\" -> \"{}\";\n",
            dot_escape(from),
            dot_escape(to)
        ));
    }
    out.push_str("}\n");
    out
}

fn render_mermaid(graph: &Graph) -> String {
    let mut out = String::from("graph LR\n");
    // Assign stable, syntax-safe ids to each node.
    let ids: BTreeMap<&String, String> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(i, name)| (name, format!("n{i}")))
        .collect();

    for node in &graph.nodes {
        let id = &ids[node];
        let label = mermaid_escape(node);
        let class = if graph.missing.contains(node) {
            ":::missing"
        } else if graph.roots.contains(node) {
            ":::root"
        } else {
            ""
        };
        out.push_str(&format!("  {id}[\"{label}\"]{class}\n"));
    }
    for (from, to) in &graph.edges {
        out.push_str(&format!("  {} --> {}\n", ids[from], ids[to]));
    }
    out.push_str("  classDef root fill:#dbeafe,stroke:#2563eb;\n");
    out.push_str("  classDef missing fill:#fee2e2,stroke:#b91c1c,stroke-dasharray:4;\n");
    out
}

fn dot_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn mermaid_escape(s: &str) -> String {
    s.replace('"', "&quot;")
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

    const DOC: &str = "```python #main file=out.py\n<<imports>>\n<<body>>\n```\n\n```python #imports\nimport os\n```\n\n```python #body\nprint(1)\n```\n";

    #[test]
    fn dot_has_nodes_edges_and_root_styling() {
        let dot = render_graph(&refs_from(DOC), GraphFormat::Dot);
        assert!(dot.starts_with("digraph entangled {"));
        assert!(dot.contains("\"main\" -> \"imports\""));
        assert!(dot.contains("\"main\" -> \"body\""));
        // main is a root -> filled box.
        assert!(dot.contains("\"main\" [shape=box, style=filled"));
    }

    #[test]
    fn mermaid_renders_graph_with_classes() {
        let mmd = render_graph(&refs_from(DOC), GraphFormat::Mermaid);
        assert!(mmd.starts_with("graph LR"));
        assert!(mmd.contains(":::root"));
        assert!(mmd.contains("classDef root"));
        assert!(mmd.contains("-->"));
    }

    #[test]
    fn missing_reference_is_styled() {
        let dot = render_graph(
            &refs_from("```python #main file=out.py\n<<ghost>>\n```\n"),
            GraphFormat::Dot,
        );
        assert!(dot.contains("\"ghost\" [shape=box, style=dashed"));
    }

    #[test]
    fn format_parses_from_str() {
        assert_eq!("dot".parse::<GraphFormat>().unwrap(), GraphFormat::Dot);
        assert_eq!(
            "mermaid".parse::<GraphFormat>().unwrap(),
            GraphFormat::Mermaid
        );
        assert!("svg".parse::<GraphFormat>().is_err());
    }
}
