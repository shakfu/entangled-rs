# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

#### Multi-Style Code Block Syntax Support
- **Style enum**: Support for four code block syntax styles:
  - `entangled-rs` (default): Native style with `python #name file=path`
  - `pandoc`: Original entangled style with `{.python #name file=path}`
  - `quarto`: Quarto/Jupyter style with `{python}` and `#|` comments inside block
  - `knitr`: RMarkdown style with `{python, label=name, file=path}`
- **Automatic style detection by file extension**:
  - `.qmd` files use Quarto style
  - `.Rmd` files use Knitr style
  - `.md` files use configured default style
- **CLI `--style` flag**: Override configured style from command line
- **Config options**:
  - `style`: Set default style for `.md` files
  - `strip_quarto_options`: Control whether `#|` lines are removed from tangled output (default: true)
- **New parsing functions**:
  - `Properties::parse_pandoc()` for Pandoc-style info strings
  - `Properties::parse_knitr()` for knitr-style comma-separated options
  - `extract_quarto_options()` for extracting `#|` comment options from content

#### Stitch Implementation
- Full bidirectional editing: tangled file changes are synchronized back to markdown
- Tracks block locations with YAML header offset correction
- Reads annotated tangled files and compares each block against its markdown source
- Skips blocks containing `<<reference>>` patterns (only leaf blocks are stitched)
- Groups changes by source file and applies them bottom-to-top to preserve line numbers
- Naked annotation mode correctly skipped (no markers to parse)

#### CLI Commands and Options
- `entangled init` command creates `entangled.toml` template, `.entangled/` directory, and `.gitignore` entry
- `entangled config` command prints effective resolved configuration as TOML
- `entangled locate <FILE:LINE>` maps tangled file lines back to markdown source
- `entangled status --json` outputs structured JSON with source files, targets, and tracked count
- `--diff` / `-d` flag on tangle, stitch, and sync shows unified diffs of proposed changes
- `--dry-run` / `-n` flag on sync (was already on tangle/stitch)
- `--quiet` / `-q` global flag suppresses normal output
- `NO_COLOR` environment variable support (disables ANSI colors)
- Descriptive `long_about` in `--help` output

#### Python Bindings
- `pyentangled` crate with PyO3 bindings exposing Config, Context, Document, CodeBlock, Transaction
- Core functions: tangle_documents, tangle_files, stitch_documents, stitch_files, execute_transaction, sync_documents, locate_source, tangle_ref
- Config getters/setters: style, output_dir, hooks_shebang, hooks_spdx_license, filedb_path, strip_quarto_options, watch_debounce_ms
- Transaction.diffs() method for unified diff output
- locate_source() returns dict with source_file, source_line, block_id (or None for annotation lines)
- Python CLI (`pyentangled`) with full command parity:
  - Commands: init, tangle, stitch, sync, watch, status, locate, config, reset
  - Global flags: --style/-s, --quiet/-q, --verbose/-v
  - Per-command flags: --diff/-d, --dry-run/-n, --force/-f, --json (status)
  - File filtering via tangle_files()/stitch_files() (no longer stub)
  - Watch derives extensions from source_files() instead of hardcoded set
- Python test suite with 60 tests covering the full API

#### CI/CD
- GitHub Actions CI pipeline (`.github/workflows/ci.yml`): fmt, clippy, tests on ubuntu/macos/windows, pyentangled tests with Python 3.9 + 3.13
- Release workflow (`.github/workflows/release.yml`): cross-platform binaries for linux (x86_64, aarch64), macOS (x86_64, aarch64), Windows (x86_64); creates GitHub Release with artifacts
- Automated crates.io publishing in release workflow (requires `CARGO_REGISTRY_TOKEN` secret)
- PyPI publishing workflow (`.github/workflows/pypi.yml`): builds sdist + wheels via maturin, publishes via OIDC trusted publisher

### Changed

#### Project Structure
- **Workspace refactoring**: Split single crate into a Cargo workspace with three crates:
  - `entangled` - Core library crate with no CLI dependencies
  - `entangled-cli` - CLI binary crate (binary still named `entangled`)
  - `pyentangled` - Python bindings via PyO3/maturin (edition 2024, excluded from default builds)
- Moved CLI-specific dependencies (`clap`, `tracing-subscriber`) to CLI crate only
- Moved `commands` module from library to CLI crate
- Library can now be used programmatically without pulling in CLI dependencies
- `Style` derives `clap::ValueEnum` conditionally behind optional `clap` feature flag (eliminates `CliStyle` duplication)

#### Installation
- CLI installation now uses `cargo install entangled-cli` or `cargo install --path entangled-cli`

#### Error Handling
- Config parse errors with explicit `--config`: hard error with message
- Config auto-discovery parse failures: `tracing::warn!` and fallback to defaults
- `FileDB::load` distinguishes "file not found" from "file exists but corrupt"
- `.unwrap()` on internal lookups replaced with `EntangledError::Other` descriptive errors
- `unreachable!()` in markdown parser replaced with proper error return
- Unmatched `END` markers in annotated code emit `tracing::warn!`
- `FileConflict` error message now suggests `--force`
- Distinct exit codes: 1=conflict, 2=config/parse, 3=I/O, 4=reference, 5=other

#### Performance
- `builtin_languages()` is now a `Lazy` static (was allocating on every call)
- `ReferenceMap` stores `Arc<CodeBlock>` (pointer copy instead of deep clone during tangle)
- `CycleDetector` uses `HashSet` for O(1) membership checks (was linear scan)
- `ConfigUpdate::merge_into` takes `self` by value (moves instead of cloning)
- Atomic write uses PID + counter for unique temp filenames (safe under parallel execution)

#### API Improvements
- `sync_documents()` takes `force` parameter, eliminating duplicated stitch-then-tangle logic
- `get_target_name` takes `&Path` instead of `&PathBuf`
- `output_dir()` returns `Option<&Path>` instead of `Option<&PathBuf>`
- Properties parsing returns `crate::errors::Result` with `EntangledError::InvalidProperty`
- `#[must_use]` on pure constructors: `ReferenceMap::new()`, `CycleDetector::new()`, `Transaction::new()`, `FileDB::new()`, `HookRegistry::new()`, `Config::new()`, `ParsedDocument::new()`
- `Debug`/`Clone` derives on `Context`, `Document`, `HookRegistry`, `ParsedDocument`, `AnnotatedBlock`
- `Context::source_files_filtered()` for file-specific tangle/stitch operations

#### Code Quality
- Shared `test_utils` module with `make_block` helpers (was duplicated across 5 test modules)
- Shared `helpers::run_transaction()` for tangle/stitch command pattern (was duplicated)
- Properties parser split into `properties/mod.rs`, `properties/knitr.rs`, `properties/quarto.rs`
- Dead config fields removed: `HooksConfig.quarto_attributes`, `BuildHookConfig`, `BreiHookConfig`
- Dead types removed: `Content`, `PlainText`, `RawContent`
- Unknown config keys absorbed via `#[serde(flatten)] extra: HashMap`

#### Watch Command
- File extensions derived dynamically from `source_patterns` and registered languages
- `WatchConfig.exclude` patterns applied via glob matching
- `WatchConfig.include` directories watched alongside base directory

### Fixed
- `WatchConfig::default()` now returns `debounce_ms: 100` (was 0 due to `#[derive(Default)]` on u64; serde default and programmatic default are now consistent)

#### Configuration
- Default `source_patterns` now includes `**/*.qmd` and `**/*.Rmd`
- Hooks (`shebang`, `spdx_license`) wired to `Context::new()` from config

## [0.1.0]

### Added

#### Core Features
- **Tangle command**: Extract code blocks from markdown files into source files
- **Stitch command**: Update markdown files when tangled code is modified
- **Sync command**: Bidirectional synchronization between markdown and code
- **Watch command**: File system monitoring with automatic sync on changes
- **Status command**: Display status of tracked files and targets
- **Reset command**: Clear file database and optionally delete tangled files

#### Code Block Processing
- Property parsing with nom parser combinators
  - Language identifiers (e.g., `python`, `rust`)
  - Named blocks with `#name` syntax
  - File targets with `file=path` attribute
  - Custom attributes with `key=value` syntax
- Reference expansion with `<<refname>>` syntax
- Recursive reference resolution with cycle detection
- Indentation preservation during reference expansion
- Multiple blocks with same name (concatenation)

#### Annotation System
- Standard annotation format: `# ~/~ begin <<ref[n]>>` / `# ~/~ end`
- Support for different comment styles per language
- Three annotation methods: standard, naked, supplemental
- Comment style detection based on language

#### Configuration
- TOML configuration file support (`entangled.toml`)
- Namespace default options (file-based or none)
- Configurable source file patterns (glob)
- Custom language definitions
- Watch debounce configuration
- File database path configuration

#### Language Support
- 40+ built-in language configurations with appropriate comment styles
- C-family: C, C++, Java, JavaScript, TypeScript, Rust, Go, Swift, Kotlin, Scala, C#
- Shell-style: Python, Ruby, Perl, Bash, R, Julia, YAML, TOML, Make, Dockerfile
- Lisp-family: Lisp, Scheme, Clojure, Racket
- ML-family: Haskell, Elm, OCaml, F#
- Web: HTML, CSS, SCSS
- Other: Lua, Nim, Zig, D, PHP, PowerShell, TeX, Fortran, Ada, VHDL, Verilog

#### I/O System
- File caching with virtual filesystem for testing
- SHA256 content hashing for change detection
- JSON-based file database (`.entangled/filedb.json`)
- Atomic file writes via temp file + rename
- Transaction system with conflict detection
- Create, Write, Delete actions with rollback capability

#### Readers
- Markdown parsing with code fence extraction
- Support for backtick and tilde fences
- Fence length matching (longer fences can contain shorter)
- YAML frontmatter extraction
- Annotated code parsing for stitch operations
- Nested annotation handling

#### Hooks
- Extensible hook system for code block processing
- Shebang extraction hook (`#!/usr/bin/env`)
- SPDX license header extraction hook

#### API
- Library crate with public API for programmatic use
- Context management with config, hooks, and filesystem
- Document orchestration for tangle/stitch operations
- Reference map with dual-index lookup (by ID and by name)

### Technical Details

#### Dependencies
- `clap` 4.x - CLI argument parsing with derive macros
- `nom` 8.x - Parser combinators for property parsing
- `serde` - Serialization for config and file database
- `toml` - Configuration file parsing
- `regex` - Pattern matching for references and annotations
- `sha2` - Content hashing
- `notify` 7.x - File system event monitoring
- `chrono` - Timestamp handling
- `indexmap` - Insertion-order preserving maps
- `thiserror` - Error type derivation
- `tracing` - Logging and diagnostics
- `tokio` - Async runtime (for watch command)

#### Compatibility
- Configuration format compatible with Python Entangled
- File database format compatible with Python Entangled
- Annotation marker format compatible with Python Entangled

### Notes

This is the initial release of the Rust translation of the Entangled literate programming engine. The implementation provides full feature parity with the core functionality of the Python version while offering improved performance through Rust's zero-cost abstractions.

[0.1.0]: https://github.com/entangled/entangled-rs/releases/tag/v0.1.0
