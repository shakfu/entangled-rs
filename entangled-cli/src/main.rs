//! Entangled CLI - Literate Programming Engine

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod commands;

use entangled::interface::Context;
use entangled::Style;

#[derive(Parser)]
#[command(name = "entangled")]
#[command(
    author,
    version,
    about = "Literate programming engine",
    long_about = "\
Literate programming engine that keeps code and documentation in sync.\n\n\
  tangle  - extract code from markdown files into source files\n\
  stitch  - update markdown from modified source files\n\
  sync    - bidirectional sync (stitch then tangle)\n\
  watch   - auto-sync on file changes"
)]
struct Cli {
    /// Configuration file path
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    /// Working directory
    #[arg(short = 'C', long, global = true)]
    directory: Option<PathBuf>,

    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Suppress normal output
    #[arg(short, long, global = true)]
    quiet: bool,

    /// Code block syntax style (overrides config file)
    #[arg(short, long, global = true, value_enum)]
    style: Option<Style>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Extract code from markdown files
    Tangle {
        /// Force overwrite even if files have been modified externally
        #[arg(short, long)]
        force: bool,

        /// Dry run - show what would be done without doing it
        #[arg(short = 'n', long)]
        dry_run: bool,

        /// Show unified diffs of what would change
        #[arg(short, long)]
        diff: bool,

        /// Glob patterns to filter source files
        #[arg(short = 'g', long = "glob")]
        glob: Vec<String>,

        /// Specific files to tangle
        #[arg(value_name = "FILE")]
        files: Vec<PathBuf>,
    },

    /// Update markdown from modified code files
    Stitch {
        /// Force overwrite even if files have been modified
        #[arg(short, long)]
        force: bool,

        /// Dry run - show what would be done without doing it
        #[arg(short = 'n', long)]
        dry_run: bool,

        /// Show unified diffs of what would change
        #[arg(short, long)]
        diff: bool,

        /// Glob patterns to filter source files
        #[arg(short = 'g', long = "glob")]
        glob: Vec<String>,

        /// Specific files to stitch
        #[arg(value_name = "FILE")]
        files: Vec<PathBuf>,
    },

    /// Synchronize markdown and code files
    Sync {
        /// Force overwrite even if files have been modified
        #[arg(short, long)]
        force: bool,

        /// Dry run - show what would be done without doing it
        #[arg(short = 'n', long)]
        dry_run: bool,

        /// Show unified diffs of what would change
        #[arg(short, long)]
        diff: bool,
    },

    /// Watch for changes and sync automatically
    Watch {
        /// Debounce delay in milliseconds
        #[arg(short, long, default_value = "100")]
        debounce: u64,
    },

    /// Show status of files
    Status {
        /// Show verbose output
        #[arg(short, long)]
        verbose: bool,

        /// Output machine-readable JSON
        #[arg(long)]
        json: bool,
    },

    /// Reset the file database
    Reset {
        /// Also delete tangled files
        #[arg(long)]
        delete_files: bool,

        /// Don't ask for confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Show effective resolved configuration
    Config,

    /// Initialize a new entangled project
    Init,

    /// Map a tangled file line back to its markdown source
    Locate {
        /// Location in format file:line (e.g., output.py:42)
        #[arg(value_name = "FILE:LINE")]
        location: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // Set up logging
    let filter = if cli.quiet {
        EnvFilter::new("error")
    } else if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    // Respect NO_COLOR convention (https://no-color.org/)
    let no_color = std::env::var_os("NO_COLOR").is_some();

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_ansi(!no_color)
        .init();

    // Determine working directory
    let base_dir = cli
        .directory
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));

    // Handle init before context creation (no config needed)
    if matches!(cli.command, Commands::Init) {
        return match commands::init(&base_dir) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("Error: {}", e);
                ExitCode::FAILURE
            }
        };
    }

    // Read configuration from file or use defaults
    let mut config = match cli.config {
        Some(ref path) => {
            // Explicit --config: parse failure is a hard error
            match entangled::config::read_config_file(path) {
                Ok(cfg) => cfg,
                Err(e) => {
                    eprintln!("Error reading config file {}: {}", path.display(), e);
                    return ExitCode::FAILURE;
                }
            }
        }
        None => {
            // Auto-discovery: warn on parse failure, fall back to defaults
            match entangled::config::read_config(&base_dir) {
                Ok(cfg) => cfg,
                Err(e) => {
                    tracing::warn!("Failed to parse config file, using defaults: {}", e);
                    entangled::Config::default()
                }
            }
        }
    };

    // Override style if specified on command line
    if let Some(style) = cli.style {
        config.style = style;
    }

    // Create context
    let mut ctx = match Context::new(config, base_dir) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Error initializing: {}", e);
            return ExitCode::FAILURE;
        }
    };

    // Execute command
    let result = match cli.command {
        Commands::Tangle {
            force,
            dry_run,
            diff,
            glob,
            files,
        } => {
            let options = commands::TangleOptions {
                force,
                dry_run,
                diff,
                quiet: cli.quiet,
                glob,
                files,
            };
            commands::tangle(&mut ctx, options)
        }

        Commands::Stitch {
            force,
            dry_run,
            diff,
            glob,
            files,
        } => {
            let options = commands::StitchOptions {
                force,
                dry_run,
                diff,
                quiet: cli.quiet,
                glob,
                files,
            };
            commands::stitch(&mut ctx, options)
        }

        Commands::Sync {
            force,
            dry_run,
            diff,
        } => {
            let options = commands::SyncOptions {
                force,
                dry_run,
                diff,
                quiet: cli.quiet,
            };
            commands::sync(&mut ctx, options)
        }

        Commands::Watch { debounce } => {
            let options = commands::WatchOptions {
                debounce_ms: debounce,
            };
            commands::watch(&mut ctx, options)
        }

        Commands::Status { verbose, json } => {
            let options = commands::StatusOptions { verbose, json };
            commands::status(&ctx, options)
        }

        Commands::Reset {
            delete_files,
            force,
        } => {
            let options = commands::ResetOptions {
                delete_files,
                force,
            };
            commands::reset(&mut ctx, options)
        }

        Commands::Config => commands::config(&ctx),

        Commands::Locate { location } => {
            let (file, line) = match location.rsplit_once(':') {
                Some((f, l)) => match l.parse::<usize>() {
                    Ok(n) if n > 0 => (PathBuf::from(f), n),
                    _ => {
                        eprintln!(
                            "Invalid line number in '{}'. Expected format: file:line",
                            location
                        );
                        return ExitCode::FAILURE;
                    }
                },
                None => {
                    eprintln!("Expected format: file:line (e.g., output.py:42)");
                    return ExitCode::FAILURE;
                }
            };
            let options = commands::LocateOptions { file, line };
            commands::locate(&ctx, options)
        }

        Commands::Init => unreachable!("handled before context creation"),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::from(e.exit_code())
        }
    }
}
