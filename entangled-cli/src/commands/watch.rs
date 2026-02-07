//! Watch command implementation.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::Duration;

use entangled::config::builtin_languages;
use entangled::errors::{EntangledError, Result};
use entangled::interface::{sync_documents, Context};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};

/// Options for the watch command.
#[derive(Debug, Clone, Default)]
pub struct WatchOptions {
    /// Debounce delay in milliseconds.
    pub debounce_ms: u64,
}

/// Collects all relevant file extensions from config and built-in languages.
///
/// This includes extensions from source patterns (e.g. "md", "qmd", "Rmd")
/// and all language identifiers that could be file extensions.
fn relevant_extensions(ctx: &Context) -> HashSet<String> {
    let mut exts = HashSet::new();

    // Extract extensions from source patterns (e.g. "**/*.md" -> "md")
    for pattern in &ctx.config.source_patterns {
        if let Some(ext) = pattern.rsplit('.').next() {
            exts.insert(ext.to_string());
        }
    }

    // Add all language names and identifiers as potential extensions
    for lang in &ctx.config.languages {
        exts.insert(lang.name.clone());
        for id in &lang.identifiers {
            exts.insert(id.clone());
        }
    }
    for lang in builtin_languages() {
        exts.insert(lang.name.clone());
        for id in &lang.identifiers {
            exts.insert(id.clone());
        }
    }

    exts
}

/// Checks whether a path matches any of the exclude patterns.
fn is_excluded(path: &Path, base_dir: &Path, exclude_patterns: &[String]) -> bool {
    let relative = path.strip_prefix(base_dir).unwrap_or(path);
    let rel_str = relative.to_string_lossy();
    for pattern in exclude_patterns {
        if let Ok(glob) = glob::Pattern::new(pattern) {
            if glob.matches(&rel_str) {
                return true;
            }
        }
    }
    false
}

/// Executes the watch command.
pub fn watch(ctx: &mut Context, options: WatchOptions) -> Result<()> {
    let debounce = if options.debounce_ms > 0 {
        options.debounce_ms
    } else {
        ctx.config.watch.debounce_ms
    };

    let exts = relevant_extensions(ctx);
    let exclude_patterns = ctx.config.watch.exclude.clone();
    let base_dir = ctx.base_dir.clone();
    tracing::debug!("Watching for extensions: {:?}", exts);
    if !exclude_patterns.is_empty() {
        tracing::debug!("Exclude patterns: {:?}", exclude_patterns);
    }

    println!("Watching for changes (debounce: {}ms)...", debounce);
    println!("Press Ctrl+C to stop.");

    // Initial sync
    if let Err(e) = sync_documents(ctx, false) {
        eprintln!("Initial sync error: {}", e);
    }

    let (tx, rx) = channel();

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        },
        Config::default().with_poll_interval(Duration::from_millis(debounce)),
    )
    .map_err(|e| EntangledError::Watch(e.to_string()))?;

    // Watch the base directory
    watcher
        .watch(&ctx.base_dir, RecursiveMode::Recursive)
        .map_err(|e| EntangledError::Watch(e.to_string()))?;

    // Also watch any additional include directories
    for dir in &ctx.config.watch.include {
        let include_path = ctx.base_dir.join(dir);
        if include_path.is_dir() {
            watcher
                .watch(&include_path, RecursiveMode::Recursive)
                .map_err(|e| EntangledError::Watch(e.to_string()))?;
            tracing::debug!("Also watching: {}", include_path.display());
        }
    }

    // Event loop
    loop {
        match rx.recv() {
            Ok(event) => {
                let paths: Vec<&PathBuf> = event.paths.iter().collect();

                // Check extension relevance and exclude patterns
                let relevant = paths.iter().any(|p| {
                    let ext_ok = p
                        .extension()
                        .and_then(OsStr::to_str)
                        .map(|e| exts.contains(e))
                        .unwrap_or(false);
                    ext_ok && !is_excluded(p, &base_dir, &exclude_patterns)
                });

                if relevant {
                    tracing::debug!("File changed: {:?}", paths);
                    if let Err(e) = sync_documents(ctx, false) {
                        eprintln!("Sync error: {}", e);
                    }
                }
            }
            Err(e) => {
                return Err(EntangledError::Watch(format!("Watch error: {}", e)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // use super::*;

    // Watch is difficult to test in unit tests due to its blocking nature
    // Integration tests would be more appropriate
}
