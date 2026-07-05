//! Check command implementation.
//!
//! Validates the literate project and reports structural problems (dangling
//! references, target collisions, reference cycles, orphan blocks). Exits
//! non-zero when any error is found, so it works as a CI / pre-commit gate.

use entangled::check::{check_documents, has_errors, Severity};
use entangled::errors::{EntangledError, Result};
use entangled::interface::Context;

/// Options for the check command.
#[derive(Debug, Clone, Default)]
pub struct CheckOptions {
    /// Emit findings as JSON instead of human-readable text.
    pub json: bool,
    /// Treat warnings as errors (fail on any finding).
    pub strict: bool,
    /// Suppress the success message.
    pub quiet: bool,
}

/// Executes the check command.
pub fn check(ctx: &Context, options: CheckOptions) -> Result<()> {
    let findings = check_documents(ctx)?;

    if options.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&findings).map_err(EntangledError::from)?
        );
    } else {
        for f in &findings {
            let severity = match f.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            };
            let loc = match (&f.file, f.line) {
                (Some(file), Some(line)) => format!(" ({file}:{line})"),
                (Some(file), None) => format!(" ({file})"),
                _ => String::new(),
            };
            let line = format!("{severity}[{}]: {}{loc}", f.kind, f.message);
            if f.severity == Severity::Error {
                eprintln!("{line}");
            } else {
                println!("{line}");
            }
        }
    }

    let error_count = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let warning_count = findings.len() - error_count;

    let failed = has_errors(&findings) || (options.strict && warning_count > 0);
    if failed {
        return Err(EntangledError::Other(format!(
            "check failed: {error_count} error(s), {warning_count} warning(s)"
        )));
    }

    if !options.quiet && !options.json {
        if warning_count > 0 {
            println!("check passed with {warning_count} warning(s).");
        } else {
            println!("check passed: no issues found.");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn ctx_with(files: &[(&str, &str)]) -> (tempfile::TempDir, Context) {
        let dir = tempdir().unwrap();
        for (name, content) in files {
            fs::write(dir.path().join(name), content).unwrap();
        }
        let mut ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();
        ctx.config.namespace_default = entangled::config::NamespaceDefault::None;
        (dir, ctx)
    }

    #[test]
    fn clean_project_passes() {
        let (_d, ctx) = ctx_with(&[(
            "doc.md",
            "```python #main file=out.py\n<<body>>\n```\n\n```python #body\nprint(1)\n```\n",
        )]);
        check(
            &ctx,
            CheckOptions {
                quiet: true,
                ..Default::default()
            },
        )
        .unwrap();
    }

    #[test]
    fn dangling_reference_fails() {
        let (_d, mut ctx) = ctx_with(&[("doc.md", "```python #main file=out.py\n<<nope>>\n```\n")]);
        ctx.config.namespace_default = entangled::config::NamespaceDefault::None;
        let result = check(
            &ctx,
            CheckOptions {
                quiet: true,
                ..Default::default()
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn orphan_only_passes_unless_strict() {
        let (_d, mut ctx) = ctx_with(&[(
            "doc.md",
            "```python #main file=out.py\nprint(1)\n```\n\n```python #orphan\nprint(2)\n```\n",
        )]);
        ctx.config.namespace_default = entangled::config::NamespaceDefault::None;

        // Non-strict: warning only, passes.
        check(
            &ctx,
            CheckOptions {
                quiet: true,
                ..Default::default()
            },
        )
        .unwrap();

        // Strict: warning becomes a failure.
        let strict = check(
            &ctx,
            CheckOptions {
                strict: true,
                quiet: true,
                ..Default::default()
            },
        );
        assert!(strict.is_err());
    }
}
