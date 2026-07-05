//! Graph command implementation.
//!
//! Emits the reference-dependency graph of the project as Graphviz DOT or
//! Mermaid, to stdout or a file.

use std::path::PathBuf;

use entangled::errors::{EntangledError, Result};
use entangled::graph::graph_documents;
use entangled::interface::Context;
use entangled::GraphFormat;

/// Options for the graph command.
#[derive(Debug, Clone)]
pub struct GraphOptions {
    /// Output format: `dot` or `mermaid`.
    pub format: String,
    /// Output file path; stdout when `None`.
    pub output: Option<PathBuf>,
    /// Suppress the "wrote ..." message.
    pub quiet: bool,
}

impl Default for GraphOptions {
    fn default() -> Self {
        Self {
            format: "dot".to_string(),
            output: None,
            quiet: false,
        }
    }
}

/// Executes the graph command.
pub fn graph(ctx: &Context, options: GraphOptions) -> Result<()> {
    let format: GraphFormat = options.format.parse().map_err(EntangledError::Config)?;

    let rendered = graph_documents(ctx, format)?;

    match &options.output {
        Some(path) => {
            let resolved = ctx.resolve_path(path);
            if let Some(parent) = resolved.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            std::fs::write(&resolved, rendered)?;
            if !options.quiet {
                println!("graph: wrote {}", resolved.display());
            }
        }
        None => print!("{rendered}"),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn graph_writes_dot_file() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("doc.md"),
            "```python #main file=out.py\n<<body>>\n```\n\n```python #body\nprint(1)\n```\n",
        )
        .unwrap();
        let mut ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();
        ctx.config.namespace_default = entangled::config::NamespaceDefault::None;

        graph(
            &ctx,
            GraphOptions {
                output: Some(PathBuf::from("graph.dot")),
                quiet: true,
                ..Default::default()
            },
        )
        .unwrap();

        let dot = fs::read_to_string(dir.path().join("graph.dot")).unwrap();
        assert!(dot.contains("digraph entangled"));
        assert!(dot.contains("\"main\" -> \"body\""));
    }

    #[test]
    fn invalid_format_errors() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("doc.md"), "# empty\n").unwrap();
        let ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();

        let result = graph(
            &ctx,
            GraphOptions {
                format: "svg".to_string(),
                quiet: true,
                ..Default::default()
            },
        );
        assert!(result.is_err());
    }
}
