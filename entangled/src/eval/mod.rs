//! Executable code blocks with reproducible, cached output.
//!
//! A code block marked with an `eval` attribute is *runnable*: its
//! reference-expanded source is piped to a configured runner (interpreter) and
//! the captured stdout/stderr/exit status is recorded. Results are cached in
//! `.entangled/eval-cache.json`, keyed by block name and content hash, so a
//! block is only re-executed when its expanded source (or runner) changes --
//! giving reproducible output that the [`weave`](crate::weave) backends render
//! beneath each block.
//!
//! ````text
//! ```python #demo eval=python
//! print(6 * 7)
//! ```
//! ````
//!
//! Running `entangled eval` executes the block and caches `42`. Because
//! execution runs arbitrary code, it only ever happens on the explicit `eval`
//! action -- never during tangle, stitch, or weave.
//!
//! The `eval` attribute value names the runner. The special values `true`,
//! `yes`, or `1` mean "use the block's language as the runner name".

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::errors::Result;
use crate::interface::{combined_reference_map, Context};
use crate::model::{tangle_ref, ReferenceMap, ReferenceName};

/// Built-in runner name -> command argv. Config entries override these.
const BUILTIN_RUNNERS: &[(&str, &[&str])] = &[
    ("python", &["python3"]),
    ("python3", &["python3"]),
    ("py", &["python3"]),
    ("sh", &["sh"]),
    ("shell", &["sh"]),
    ("bash", &["bash"]),
    ("zsh", &["zsh"]),
    ("node", &["node"]),
    ("javascript", &["node"]),
    ("js", &["node"]),
    ("ruby", &["ruby"]),
    ("rb", &["ruby"]),
    ("perl", &["perl"]),
    ("lua", &["lua"]),
    ("php", &["php"]),
    ("r", &["Rscript", "-"]),
    ("deno", &["deno", "run", "-"]),
];

/// The recorded result of executing (or attempting to execute) a runnable block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalResult {
    /// The block's reference name (cache key).
    pub block_id: String,
    /// The runner name used.
    pub runner: String,
    /// Hash of `runner + expanded source`, used for cache invalidation.
    pub content_hash: String,
    /// Captured standard output.
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
    /// Process exit code, or `None` if it could not be run (e.g. expansion or
    /// spawn failure; details are in `stderr`).
    pub exit_code: Option<i32>,
}

impl EvalResult {
    /// Returns true if the block ran and exited successfully.
    pub fn success(&self) -> bool {
        self.exit_code == Some(0)
    }
}

/// On-disk cache of evaluation results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalCache {
    /// Cache format version.
    #[serde(default = "cache_version")]
    pub version: String,
    /// Results keyed by block name.
    #[serde(default)]
    pub results: HashMap<String, EvalResult>,
}

fn cache_version() -> String {
    "1.0".to_string()
}

impl Default for EvalCache {
    fn default() -> Self {
        Self {
            version: cache_version(),
            results: HashMap::new(),
        }
    }
}

impl EvalCache {
    /// Loads the cache from disk, returning an empty cache if the file is absent.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Saves the cache to disk, creating parent directories as needed.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

/// Options controlling evaluation.
#[derive(Debug, Clone, Default)]
pub struct EvalOptions {
    /// Re-run every block even if a fresh cached result exists.
    pub force: bool,
    /// Identify runnable blocks and report intended runs without executing.
    pub dry_run: bool,
}

/// A runnable block discovered in the document set.
#[derive(Debug, Clone)]
pub struct RunnableBlock {
    /// The block's reference name.
    pub name: String,
    /// The resolved runner name.
    pub runner: String,
}

/// Evaluates all runnable blocks across the context's source files.
///
/// Returns one [`EvalResult`] per runnable block (in document order). Errors for
/// an individual block (expansion failure, unknown runner, spawn failure) are
/// captured in that block's result rather than aborting the whole run. The cache
/// is updated and saved unless `dry_run` is set.
pub fn eval_documents(ctx: &Context, options: &EvalOptions) -> Result<Vec<EvalResult>> {
    let refs = build_global_refs(ctx)?;
    let runnables = find_runnable(&refs);

    let cache_path = eval_cache_path(ctx);
    let mut cache = EvalCache::load(&cache_path)?;
    let mut results = Vec::with_capacity(runnables.len());

    for rb in runnables {
        let result = eval_one(ctx, &refs, &rb, &cache, options);
        if !options.dry_run {
            cache
                .results
                .insert(result.block_id.clone(), result.clone());
        }
        results.push(result);
    }

    if !options.dry_run {
        cache.save(&cache_path)?;
    }

    Ok(results)
}

/// Evaluates a single runnable block, consulting the cache first.
fn eval_one(
    ctx: &Context,
    refs: &ReferenceMap,
    rb: &RunnableBlock,
    cache: &EvalCache,
    options: &EvalOptions,
) -> EvalResult {
    // Expand references to obtain the actual source to run.
    let content = match tangle_ref(refs, &ReferenceName::new(rb.name.clone()), None, None) {
        Ok(c) => c,
        Err(e) => {
            return error_result(rb, "", format!("reference expansion failed: {e}"));
        }
    };
    let hash = content_hash(&rb.runner, &content);

    // Reuse a fresh cached result when possible.
    if !options.force {
        if let Some(prev) = cache.results.get(&rb.name) {
            if prev.content_hash == hash && prev.runner == rb.runner {
                return prev.clone();
            }
        }
    }

    if options.dry_run {
        return EvalResult {
            block_id: rb.name.clone(),
            runner: rb.runner.clone(),
            content_hash: hash,
            stdout: String::new(),
            stderr: String::new(),
            exit_code: None,
        };
    }

    let argv = match resolve_runner(ctx, &rb.runner) {
        Some(a) => a,
        None => {
            return error_result(
                rb,
                &hash,
                format!(
                    "no runner named '{}' (configure it under [eval.runners])",
                    rb.runner
                ),
            );
        }
    };

    match run_process(&argv, &content) {
        Ok((stdout, stderr, exit_code)) => EvalResult {
            block_id: rb.name.clone(),
            runner: rb.runner.clone(),
            content_hash: hash,
            stdout,
            stderr,
            exit_code,
        },
        Err(e) => error_result(rb, &hash, format!("failed to run '{}': {e}", argv[0])),
    }
}

/// Builds an error result with no exit code and the message in stderr.
fn error_result(rb: &RunnableBlock, hash: &str, message: String) -> EvalResult {
    EvalResult {
        block_id: rb.name.clone(),
        runner: rb.runner.clone(),
        content_hash: hash.to_string(),
        stdout: String::new(),
        stderr: message,
        exit_code: None,
    }
}

/// Parses every source file into one combined reference map so that runnable
/// blocks can resolve `<<references>>` defined in other files.
fn build_global_refs(ctx: &Context) -> Result<ReferenceMap> {
    combined_reference_map(ctx, &ctx.source_files()?)
}

/// Finds runnable blocks (those with an `eval` attribute), de-duplicated by name
/// in document order.
pub fn find_runnable(refs: &ReferenceMap) -> Vec<RunnableBlock> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for block in refs.blocks() {
        let Some(eval) = block.get_attribute("eval") else {
            continue;
        };
        let name = block.id.name.to_string();
        if !seen.insert(name.clone()) {
            continue;
        }
        let runner = if matches!(eval, "true" | "yes" | "1") {
            block.language.clone().unwrap_or_default()
        } else {
            eval.to_string()
        };
        out.push(RunnableBlock { name, runner });
    }
    out
}

/// Resolves a runner name to its command argv (config overrides built-ins).
fn resolve_runner(ctx: &Context, name: &str) -> Option<Vec<String>> {
    if let Some(cmd) = ctx.config.eval.runners.get(name) {
        if !cmd.is_empty() {
            return Some(cmd.clone());
        }
    }
    BUILTIN_RUNNERS
        .iter()
        .find(|(k, _)| *k == name)
        .map(|(_, argv)| argv.iter().map(|s| s.to_string()).collect())
}

/// Runs a command with `input` on stdin, capturing stdout, stderr and exit code.
fn run_process(argv: &[String], input: &str) -> std::io::Result<(String, String, Option<i32>)> {
    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Write the script then close stdin so the child can finish reading.
    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| std::io::Error::other("failed to open child stdin"))?;
        stdin.write_all(input.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    Ok((
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    ))
}

/// Hashes the runner and expanded source for cache invalidation.
fn content_hash(runner: &str, content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(runner.as_bytes());
    hasher.update([0u8]);
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

/// Location of the evaluation cache, colocated with the file database.
pub fn eval_cache_path(ctx: &Context) -> PathBuf {
    ctx.filedb_path
        .parent()
        .map(|p| p.join("eval-cache.json"))
        .unwrap_or_else(|| ctx.base_dir.join(".entangled/eval-cache.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, NamespaceDefault};
    use crate::readers::parse_markdown;

    fn refs_from(input: &str) -> ReferenceMap {
        let mut c = Config::default();
        c.namespace_default = NamespaceDefault::None;
        parse_markdown(input, None, &c).unwrap().refs
    }

    #[test]
    fn finds_runnable_blocks_by_eval_attribute() {
        let refs = refs_from(
            "```python #demo eval=python\nprint(1)\n```\n\n```python #plain\nprint(2)\n```\n",
        );
        let runnables = find_runnable(&refs);
        assert_eq!(runnables.len(), 1);
        assert_eq!(runnables[0].name, "demo");
        assert_eq!(runnables[0].runner, "python");
    }

    #[test]
    fn eval_true_uses_block_language_as_runner() {
        let refs = refs_from("```sh #script eval=true\necho hi\n```\n");
        let runnables = find_runnable(&refs);
        assert_eq!(runnables[0].runner, "sh");
    }

    #[test]
    fn content_hash_changes_with_content_and_runner() {
        let a = content_hash("python", "print(1)");
        let b = content_hash("python", "print(2)");
        let c = content_hash("bash", "print(1)");
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_eq!(a, content_hash("python", "print(1)"));
    }

    #[test]
    fn resolve_runner_prefers_config_over_builtin() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();
        assert_eq!(resolve_runner(&ctx, "python").unwrap(), vec!["python3"]);
        assert!(resolve_runner(&ctx, "no-such-runner").is_none());

        ctx.config.eval.runners.insert(
            "python".to_string(),
            vec!["python3.11".to_string(), "-".to_string()],
        );
        assert_eq!(
            resolve_runner(&ctx, "python").unwrap(),
            vec!["python3.11", "-"]
        );
    }

    #[test]
    fn eval_cache_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".entangled/eval-cache.json");
        let mut cache = EvalCache::default();
        cache.results.insert(
            "demo".to_string(),
            EvalResult {
                block_id: "demo".to_string(),
                runner: "python".to_string(),
                content_hash: "abc".to_string(),
                stdout: "42\n".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
            },
        );
        cache.save(&path).unwrap();
        let loaded = EvalCache::load(&path).unwrap();
        assert_eq!(loaded.results["demo"].stdout, "42\n");
        assert!(loaded.results["demo"].success());
    }

    // Execution tests use a real interpreter and a POSIX shell, so they only run
    // on Unix where `sh` is available.
    #[cfg(unix)]
    #[test]
    fn eval_runs_block_and_caches_output() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("doc.md"),
            "```sh #greet eval=sh\necho hello-eval\n```\n",
        )
        .unwrap();
        let mut ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();
        ctx.config.namespace_default = NamespaceDefault::None;

        let results = eval_documents(&ctx, &EvalOptions::default()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].block_id, "greet");
        assert!(results[0].success());
        assert_eq!(results[0].stdout.trim(), "hello-eval");

        // Cache was written and reused on a second run without --force.
        let cache = EvalCache::load(&eval_cache_path(&ctx)).unwrap();
        assert_eq!(cache.results["greet"].stdout.trim(), "hello-eval");
    }

    #[cfg(unix)]
    #[test]
    fn eval_expands_references_before_running() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("doc.md"),
            "```sh #main eval=sh\n<<msg>>\n```\n\n```sh #msg\necho from-ref\n```\n",
        )
        .unwrap();
        let mut ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();
        ctx.config.namespace_default = NamespaceDefault::None;

        let results = eval_documents(&ctx, &EvalOptions::default()).unwrap();
        assert_eq!(results[0].stdout.trim(), "from-ref");
    }

    #[cfg(unix)]
    #[test]
    fn eval_captures_nonzero_exit() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("doc.md"),
            "```sh #fail eval=sh\necho oops >&2\nexit 3\n```\n",
        )
        .unwrap();
        let mut ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();
        ctx.config.namespace_default = NamespaceDefault::None;

        let results = eval_documents(&ctx, &EvalOptions::default()).unwrap();
        assert!(!results[0].success());
        assert_eq!(results[0].exit_code, Some(3));
        assert_eq!(results[0].stderr.trim(), "oops");
    }

    #[test]
    fn dry_run_does_not_execute_or_write_cache() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("doc.md"),
            "```sh #greet eval=sh\necho hi\n```\n",
        )
        .unwrap();
        let mut ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();
        ctx.config.namespace_default = NamespaceDefault::None;

        let results = eval_documents(
            &ctx,
            &EvalOptions {
                dry_run: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].exit_code, None);
        assert!(results[0].stdout.is_empty());
        assert!(!eval_cache_path(&ctx).exists());
    }

    #[test]
    fn unknown_runner_is_reported_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("doc.md"),
            "```made-up #b eval=made-up-runner\nx\n```\n",
        )
        .unwrap();
        let mut ctx = Context::default_for_dir(dir.path().to_path_buf()).unwrap();
        ctx.config.namespace_default = NamespaceDefault::None;

        let results = eval_documents(&ctx, &EvalOptions::default()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].exit_code, None);
        assert!(results[0].stderr.contains("no runner named"));
    }
}
