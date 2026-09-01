# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1]

### Fixed

- **Tangling no longer discards code on a target collision.** Two differently named blocks writing one `file=` target silently kept only the last one. `tangle` and `sync` now refuse the whole operation, naming every collided target, before any file is written. Continuation blocks (several blocks sharing one *name*) are unaffected.

- **Block identity is now project-wide.** Per-document IDs restarted at zero in every file, so a `#part` block in `a.md` and in `b.md` were both `part[0]`: one replaced the other and the survivor was expanded twice. The default file namespace also keyed on the file name alone, so `chapter/a.md` and `other/a.md` collided; it now uses the whole project-relative path.

- **A bare `<<reference>>` resolves inside its own document's namespace**, so the default `namespace_default = "file"` is usable at all. Reference syntax also accepts `#`, so `<<other/a.md#part>>` can be written.

- **Header hooks emit exactly once.** `shebang` and `spdx_license` re-added their header without removing it, producing duplicate shebangs, and the pre-tangle half was never called. Headers are now lifted out before reference expansion and emitted once above the annotations; the hooks compose, and the header survives a stitch round trip.

- **Evaluated blocks run in the project directory** rather than inheriting the caller's, so `-C` and library use behave like running in the project.

- **Evaluation no longer deadlocks or hangs.** stdin is written while output is drained (a child filling its 64 KiB stdout pipe used to block the parent forever), and a new `[eval] timeout_secs` (default 60) bounds each block.

- **The evaluation cache is keyed on the resolved command line**, not the runner's name, so changing `[eval.runners]` invalidates stale results.

- **`output_dir` is implemented.** It was parsed and documented but never applied. One resolver now serves tangle, stitch, status and the file database.

- **Generated files cannot escape the project.** `file=` targets that resolve outside the project directory (via `..` or an absolute path) are rejected; set `allow_external_targets = true` to opt in.

- **Transactions are genuinely all-or-nothing.** Actions were applied sequentially, so a failure part-way through left the filesystem and the file database inconsistent. Everything is now staged and backed up before any commit, a failed commit rolls back, and the database advances only on success.

- **CRLF documents survive weave and stitch.** A byte-offset bug in YAML frontmatter splitting emitted a stray `-` line and shifted every code block's line number; stitch also rewrote CRLF files as LF.

- **HTML anchors are unique.** Names that normalise to the same slug (`a.b` and `a-b`), or to nothing at all, shared one HTML id.

- **Weave uses the same names as tangle** (namespace included) and marks a reference defined in another source document as external rather than missing.

- **State files are written atomically**, so overlapping processes cannot truncate `filedb.json` or the evaluation cache; an unparsable database is quarantined rather than silently replaced with an empty one.

- **Watch mode** notices deleted source files and source types created after it started (Python), ignores events for its own generated output (Rust), and reports an initial-sync failure explicitly instead of looking like a clean start.

- **Python parity.** The CLI gains `check`, `graph` and `eval`; `status` uses the shared Rust computation (same values and JSON schema) and no longer swallows document errors; `__version__` is derived from package metadata instead of being hard-coded (it read `0.1.0` at version `0.2.0`); and `locate_source` resolves relative paths through `Context`; and engine errors now raise `pyentangled.EntangledError` carrying the same `exit_code` the native CLI exits with, instead of every failure collapsing to 1.

### Security

- Updated `bytes` (RUSTSEC-2026-0007), `crossbeam-epoch` (RUSTSEC-2026-0204) and PyO3 to 0.29 (RUSTSEC-2026-0176, RUSTSEC-2026-0177). `cargo audit` now runs in CI, with the accepted unmaintained transitive crates documented in `.cargo/audit.toml`.

### Added

- `[eval] timeout_secs` -- wall-clock limit per evaluated block (default 60, `0` disables).

- `allow_external_targets` -- opt in to generating files outside the project.

- `entangled::status` -- the shared status computation behind both CLIs.

- `entangled::interface::analyze_project` -- one validated, project-wide model of every code block, used by tangle, stitch, locate, status and weave.

### Changed

- Clippy runs with `--all-targets` in CI, and the Python test job now covers Windows and uses `uv`. It also builds the native CLI first, so the new cross-CLI contract tests (command list, version, status JSON, exit codes) run rather than skipping themselves.

## [0.2.0]

### Added

#### Weave: documentation output

- **`entangled weave` command** renders literate documents to human-readable output, closing the second half of the literate-programming loop (tangle produces code; weave produces the typeset document).

- **Two-layer design**: a renderer-agnostic transform (`entangled::weave`) plus pluggable backends. The transform annotates each code block with a caption (name and target file), continuation markers for same-named blocks, resolved `<<reference>>` cross-references, and a "used by" back-reference set.

- **Native HTML backend**: self-contained, offline, theme-aware (`prefers-color-scheme`) output. Prose is rendered with `pulldown-cmark`; `<<references>>` become intra-document links to the defining block, and each block shows a "used in" footer. No external tools required.

- **Syntax highlighting**: code blocks are highlighted server-side with `syntect` (pure-Rust `fancy-regex` backend) using class-based spans and an embedded light/dark theme stylesheet. Gated behind the default-on `highlight` cargo feature; building with `--no-default-features` drops the dependency and falls back to plain `language-xxx`-classed code. Reference lines are left unhighlighted so their cross-reference links are preserved.

- **Native clean-markdown backend**: emits portable Pandoc/Quarto-ready markdown with Entangled attributes replaced by readable captions. Drives the `markdown` and `quarto` (`.qmd`) targets.

- **Pandoc passthrough**: any other `--to` format (`pdf`, `latex`, `docx`, `epub`, ...) is produced by piping the clean markdown through `pandoc`.

- **Options**: `--to/-t`, `--output/-o` (with `-` for stdout on text targets), `--fragment` (HTML body only), `--pandoc <path>`, plus `-g/--glob` and file filters matching the other commands.

- **Library API**: `weave_document`, `weave_to_html`, `weave_to_markdown`, `WovenDocument`, and `HtmlOptions` are exported from the `entangled` crate.

#### Executable code blocks / reproducible output

- **`entangled eval` command** executes runnable code blocks -- those marked with an `eval=<runner>` attribute -- by expanding their references and piping the source to a configured interpreter on stdin, capturing stdout/stderr/exit code.

- **Reproducible caching**: results are stored in `.entangled/eval-cache.json` keyed by block name and a hash of the expanded source, so a block re-runs only when its code or runner changes. `--force` re-runs all; `--dry-run` reports runnable blocks without executing.

- **Safety**: execution is opt-in and happens only on `eval` -- never during tangle, stitch, or weave. Per-block failures (non-zero exit, unknown runner, expansion error) are captured rather than aborting the run.

- **Weave integration**: `weave` renders a captured-output panel beneath each runnable block (success or error), turning a document into a reproducible report. New `weave_document_with_outputs` and `BlockOutput` API.

- **Configurable runners**: built-ins for `python`, `sh`/`bash`, `node`, `ruby`, `perl`, `lua`, `php`, `r`, and `deno`; overridable/extendable via an `[eval.runners]` config table. `eval=true` uses the block's own language.

- **Library API**: `eval_documents`, `EvalResult`, `EvalCache`, `EvalOptions`, and `EvalConfig` are exported from the `entangled` crate.

#### Authoring safety, discoverability, and visualization

- **`entangled check` command**: validates the project and reports dangling references (`<<name>>` with no defining block), output-target collisions (two block names writing the same `file=`), reference cycles, and orphan blocks (never referenced or tangled). Exits non-zero on any error, so it works as a pre-commit/CI gate. Supports `--json` and `--strict` (warnings as errors). Runnable `eval=` blocks are correctly excluded from the orphan check.

- **`entangled graph` command**: emits the block reference-dependency graph as Graphviz DOT (`--format dot`, default) or Mermaid (`--format mermaid`), to stdout or `-o <file>`. Tangle roots and dangling references are styled distinctly.

- **`entangled init --example`**: scaffolds a runnable starter document (`hello.md`) plus a matching config, exercising the full tangle -> eval -> weave -> check loop so a new user sees payoff on the first run.

- **Shared analysis helper**: `combined_reference_map` builds one cross-file reference map, now reused by tangle, eval, check, and graph.

- **Library API**: `check_documents`, `Finding`, `Severity`, `graph_documents`, and `GraphFormat` are exported from the `entangled` crate.

#### Bare Annotation Mode

- New `annotation = "bare"` mode: replaces sentinel comments with blank lines between block boundaries, giving clean output with visual separation

- `tangle_bare()` function in tangle engine with blank-line collapse post-processing

- One-way only (no stitch), like `naked` mode

- Python bindings support `"bare"` in Config getter/setter

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

- Python CLI (`pyentangled`):

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

- `Context::source_files_glob()` for glob-based file filtering

- `-g` / `--glob` option on `tangle` and `stitch` commands for filtering source files by glob pattern (e.g., `entangled tangle -g "docs/*.md"`)

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

- **`source_files_filtered` path matching**: explicit-file filters (e.g. `tangle FILE`, `stitch FILE`, `weave FILE`) previously failed with "not a source file" because base-dir-joined filter paths were compared against base-dir-relative source paths. Both sides are now resolved to absolute form before comparison, so relative and absolute filters match correctly.

- **Clippy lints**: resolved two warnings flagged by newer clippy toolchains (`sort_by` -> `sort_by_key` with `Reverse` in the stitch splice; manual loop counter -> `enumerate()` in the YAML header reader). No behavior change.

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

- Four annotation methods: standard, naked, bare, supplemental

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
