//! Eval command implementation.
//!
//! Executes runnable code blocks (those marked with an `eval` attribute),
//! captures their output, and caches it for reproducible weave rendering.
//! Because this runs arbitrary code, it only happens on the explicit `eval`
//! action -- never during tangle, stitch, or weave.

use entangled::errors::Result;
use entangled::eval::{eval_documents, EvalOptions};
use entangled::interface::Context;

/// Options for the eval command.
#[derive(Debug, Clone, Default)]
pub struct EvalCommandOptions {
    /// Re-run every block even if a fresh cached result exists.
    pub force: bool,
    /// Report which blocks would run without executing them.
    pub dry_run: bool,
    /// Suppress normal output.
    pub quiet: bool,
}

/// Executes the eval command.
pub fn eval(ctx: &Context, options: EvalCommandOptions) -> Result<()> {
    let results = eval_documents(
        ctx,
        &EvalOptions {
            force: options.force,
            dry_run: options.dry_run,
        },
    )?;

    if results.is_empty() {
        if !options.quiet {
            println!("No runnable blocks found (mark a block with `eval=<runner>`).");
        }
        return Ok(());
    }

    let mut failures = 0;
    for r in &results {
        if options.dry_run {
            if !options.quiet {
                println!("would run: {} ({})", r.block_id, r.runner);
            }
            continue;
        }

        if r.success() {
            if !options.quiet {
                println!("ok: {} ({})", r.block_id, r.runner);
                print_indented(&r.stdout);
            }
        } else {
            failures += 1;
            let status = match r.exit_code {
                Some(code) => format!("exit {code}"),
                None => "did not run".to_string(),
            };
            eprintln!("FAILED: {} ({}) -- {}", r.block_id, r.runner, status);
            print_indented_err(&r.stdout);
            print_indented_err(&r.stderr);
        }
    }

    if !options.quiet && !options.dry_run {
        let ran = results.len();
        println!(
            "{} block(s) evaluated, {} succeeded, {} failed.",
            ran,
            ran - failures,
            failures
        );
    }

    Ok(())
}

/// Prints captured text indented under a result line (stdout stream).
fn print_indented(text: &str) {
    for line in text.trim_end_matches('\n').lines() {
        println!("    {line}");
    }
}

/// Prints captured text indented under a result line (stderr stream).
fn print_indented_err(text: &str) {
    for line in text.trim_end_matches('\n').lines() {
        eprintln!("    {line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn eval_reports_no_runnable_blocks() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("doc.md"), "```python #plain\nx=1\n```\n").unwrap();
        let ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();
        // Should succeed and not create a cache when nothing is runnable.
        eval(
            &ctx,
            EvalCommandOptions {
                quiet: true,
                ..Default::default()
            },
        )
        .unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn eval_runs_and_reports() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("doc.md"),
            "```sh #hi eval=sh\necho hi-there\n```\n",
        )
        .unwrap();
        let mut ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();
        ctx.config.namespace_default = entangled::config::NamespaceDefault::None;

        eval(
            &ctx,
            EvalCommandOptions {
                quiet: true,
                ..Default::default()
            },
        )
        .unwrap();

        // The cache should now hold the block's output.
        let cache =
            entangled::eval::EvalCache::load(&entangled::eval::eval_cache_path(&ctx)).unwrap();
        assert_eq!(cache.results["hi"].stdout.trim(), "hi-there");
    }
}
