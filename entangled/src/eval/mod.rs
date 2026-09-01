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
use std::time::{Duration, Instant};

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
    /// Hash of the resolved runner argv plus the expanded source, used for
    /// cache invalidation.
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
    ///
    /// Written atomically for the same reason as the file database: a partial
    /// write from an overlapping process would make the cache unreadable.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        crate::io::atomic_write(path, &serde_json::to_string_pretty(self)?)?;
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
    // The runner is resolved *before* hashing: the cache identity has to cover
    // the actual command line, not just the runner's name. Otherwise editing
    // `[eval.runners] python = [...]` to a different interpreter or argument
    // set leaves the hash unchanged and a stale result is served.
    let argv = resolve_runner(ctx, &rb.runner);
    let hash = content_hash(argv.as_deref(), &content);

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

    let argv = match argv {
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

    let timeout = match ctx.config.eval.timeout_secs {
        0 => None,
        secs => Some(Duration::from_secs(secs)),
    };

    match run_process(&argv, &content, &ctx.base_dir, timeout) {
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
///
/// The child runs in `cwd` (the project root) so that relative imports, file
/// reads and generated artifacts behave the same however Entangled was invoked
/// -- from the project directory, via `-C`, or from a library embedding.
///
/// stdin is written on its own thread and both output streams are drained on
/// theirs. Writing the whole script before reading any output would deadlock as
/// soon as a child filled its stdout or stderr pipe (64 KiB on Linux) while the
/// parent was still blocked in `write_all`.
///
/// `timeout` bounds the whole call, not just the child's own lifetime: a killed
/// child can leave a grandchild (`sleep` under `sh`, say) holding the output
/// pipe open, so the collected output is awaited with a deadline too. Whatever
/// arrived by then is returned with no exit code.
fn run_process(
    argv: &[String],
    input: &str,
    cwd: &Path,
    timeout: Option<Duration>,
) -> std::io::Result<(String, String, Option<i32>)> {
    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| std::io::Error::other("failed to open child stdin"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("failed to open child stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| std::io::Error::other("failed to open child stderr"))?;

    let script = input.to_string();
    // A child that exits without reading its input gives us EPIPE here; that is
    // the child's prerogative, not an error to report.
    std::thread::spawn(move || {
        let _ = stdin.write_all(script.as_bytes());
        drop(stdin);
    });

    let deadline = timeout.map(|t| Instant::now() + t);
    let out_rx = drain(stdout);
    let err_rx = drain(stderr);

    let status = wait_with_deadline(&mut child, deadline)?;

    // Once the child is gone the remaining output is normally already in
    // flight; allow a short grace period so a straggling write is not lost.
    let collect_deadline = Instant::now() + Duration::from_secs(2);
    let out = collect(&out_rx, deadline.map(|_| collect_deadline));
    let err = collect(&err_rx, deadline.map(|_| collect_deadline));

    let mut stderr_text = String::from_utf8_lossy(&err).into_owned();
    let exit_code = match status {
        Some(status) => status.code(),
        None => {
            let secs = timeout.map(|t| t.as_secs()).unwrap_or(0);
            if !stderr_text.is_empty() && !stderr_text.ends_with('\n') {
                stderr_text.push('\n');
            }
            stderr_text.push_str(&format!(
                "entangled: killed after exceeding the {secs}s evaluation timeout (raise or \
                 disable it with `[eval] timeout_secs`)"
            ));
            None
        }
    };

    Ok((
        String::from_utf8_lossy(&out).into_owned(),
        stderr_text,
        exit_code,
    ))
}

/// Reads a pipe to EOF on its own thread, delivering the bytes over a channel.
fn drain<R: std::io::Read + Send + 'static>(mut reader: R) -> std::sync::mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = reader.read_to_end(&mut buf);
        let _ = tx.send(buf);
    });
    rx
}

/// Takes a drained stream's bytes, giving up at `deadline` if one is set.
fn collect(rx: &std::sync::mpsc::Receiver<Vec<u8>>, deadline: Option<Instant>) -> Vec<u8> {
    match deadline {
        Some(deadline) => rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_default(),
        None => rx.recv().unwrap_or_default(),
    }
}

/// Waits for `child`, killing it if `deadline` passes first.
///
/// Returns `None` when the child had to be killed.
fn wait_with_deadline(
    child: &mut std::process::Child,
    deadline: Option<Instant>,
) -> std::io::Result<Option<std::process::ExitStatus>> {
    let Some(deadline) = deadline else {
        return child.wait().map(Some);
    };

    // `Child` has no portable timed wait, so poll on a short backoff: cheap for
    // fast blocks, and bounded for slow ones.
    let mut interval = Duration::from_millis(1);
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        std::thread::sleep(interval.min(remaining));
        interval = (interval * 2).min(Duration::from_millis(50));
    }
}

/// Hashes the resolved runner command line and expanded source for cache
/// invalidation.
///
/// `argv` is the fully resolved command (executable plus every argument), not
/// the runner's name, so reconfiguring a runner invalidates its cached results.
fn content_hash(argv: Option<&[String]>, content: &str) -> String {
    let mut hasher = Sha256::new();
    match argv {
        Some(argv) => {
            for arg in argv {
                hasher.update(arg.as_bytes());
                hasher.update([0u8]);
            }
        }
        // An unresolvable runner still needs a stable, distinct identity.
        None => hasher.update(b"<unresolved>\0"),
    }
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
        let c = Config {
            namespace_default: NamespaceDefault::None,
            ..Default::default()
        };
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
    fn content_hash_changes_with_content_and_runner_argv() {
        let python = vec!["python3".to_string()];
        let bash = vec!["bash".to_string()];
        let a = content_hash(Some(&python), "print(1)");
        let b = content_hash(Some(&python), "print(2)");
        let c = content_hash(Some(&bash), "print(1)");
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_eq!(a, content_hash(Some(&python), "print(1)"));
    }

    #[test]
    fn content_hash_changes_when_runner_arguments_change() {
        // A runner reconfigured with different arguments must not serve the
        // result cached under the old command line.
        let plain = vec!["python3".to_string()];
        let optimised = vec!["python3".to_string(), "-O".to_string()];
        assert_ne!(
            content_hash(Some(&plain), "print(1)"),
            content_hash(Some(&optimised), "print(1)")
        );
    }

    #[test]
    fn content_hash_argv_boundaries_are_unambiguous() {
        // ["ab", "c"] and ["a", "bc"] are different commands.
        let a = vec!["ab".to_string(), "c".to_string()];
        let b = vec!["a".to_string(), "bc".to_string()];
        assert_ne!(content_hash(Some(&a), "x"), content_hash(Some(&b), "x"));
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
