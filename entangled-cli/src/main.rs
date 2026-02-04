//! Entangled CLI - Literate Programming Engine

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber::EnvFilter;

mod commands;

use entangled::interface::Context;
use entangled::Style;

/// Code block syntax style for CLI argument.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliStyle {
    /// Native entangled-rs style: ```python #main file=out.py
    EntangledRs,
    /// Original Pandoc/entangled style: ``` {.python #main file=out.py}
    Pandoc,
    /// Quarto style: ```{python} with #| label: main inside block
    Quarto,
    /// RMarkdown/knitr style: ```{python, label=main, file=out.py}
    Knitr,
}

impl From<CliStyle> for Style {
    fn from(cli_style: CliStyle) -> Self {
        match cli_style {
            CliStyle::EntangledRs => Style::EntangledRs,
            CliStyle::Pandoc => Style::Pandoc,
            CliStyle::Quarto => Style::Quarto,
            CliStyle::Knitr => Style::Knitr,
        }
    }
}

#[derive(Parser)]
#[command(name = "entangled")]
#[command(author, version, about = "Literate programming engine", long_about = None)]
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

    /// Code block syntax style (overrides config file)
    #[arg(short, long, global = true, value_enum)]
    style: Option<CliStyle>,

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

        /// Specific files to stitch
        #[arg(value_name = "FILE")]
        files: Vec<PathBuf>,
    },

    /// Synchronize markdown and code files
    Sync {
        /// Force overwrite even if files have been modified
        #[arg(short, long)]
        force: bool,
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
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // Set up logging
    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    // Determine working directory
    let base_dir = cli
        .directory
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));

    // Read configuration from file or use defaults
    let mut config = match cli.config {
        Some(ref path) => entangled::config::read_config_file(path).unwrap_or_default(),
        None => entangled::config::read_config(&base_dir).unwrap_or_default(),
    };

    // Override style if specified on command line
    if let Some(cli_style) = cli.style {
        config.style = cli_style.into();
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
            files,
        } => {
            let options = commands::TangleOptions {
                force,
                dry_run,
                files,
            };
            commands::tangle(&mut ctx, options)
        }

        Commands::Stitch {
            force,
            dry_run,
            files,
        } => {
            let options = commands::StitchOptions {
                force,
                dry_run,
                files,
            };
            commands::stitch(&mut ctx, options)
        }

        Commands::Sync { force } => {
            let options = commands::SyncOptions { force };
            commands::sync(&mut ctx, options)
        }

        Commands::Watch { debounce } => {
            let options = commands::WatchOptions {
                debounce_ms: debounce,
            };
            commands::watch(&mut ctx, options)
        }

        Commands::Status { verbose } => {
            let options = commands::StatusOptions { verbose };
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
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::FAILURE
        }
    }
}
