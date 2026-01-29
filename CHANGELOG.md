# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
