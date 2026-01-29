//! Watch command implementation.

use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Duration;

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};

use crate::errors::{EntangledError, Result};
use crate::interface::{sync_documents, Context};

/// Options for the watch command.
#[derive(Debug, Clone, Default)]
pub struct WatchOptions {
    /// Debounce delay in milliseconds.
    pub debounce_ms: u64,
}

/// Executes the watch command.
pub fn watch(ctx: &mut Context, options: WatchOptions) -> Result<()> {
    let debounce = if options.debounce_ms > 0 {
        options.debounce_ms
    } else {
        ctx.config.watch.debounce_ms
    };

    println!("Watching for changes (debounce: {}ms)...", debounce);
    println!("Press Ctrl+C to stop.");

    // Initial sync
    if let Err(e) = sync_documents(ctx) {
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

    // Event loop
    loop {
        match rx.recv() {
            Ok(event) => {
                // Check if event is relevant (markdown or tangled files)
                let paths: Vec<&PathBuf> = event.paths.iter().collect();
                let relevant = paths.iter().any(|p| {
                    p.extension()
                        .map(|e| e == "md" || e == "py" || e == "rs" || e == "js")
                        .unwrap_or(false)
                });

                if relevant {
                    tracing::debug!("File changed: {:?}", paths);
                    if let Err(e) = sync_documents(ctx) {
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
