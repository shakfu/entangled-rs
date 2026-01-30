# Entangled Architecture

This document provides an architectural overview of the Entangled literate programming system.

## Workspace Structure

The project is organized as a Cargo workspace with three crates:

```
entangled-rs/
  Cargo.toml              # Workspace definition
  entangled/              # Core library
  entangled-cli/          # Command-line interface
  pyentangled/            # Python bindings
```

| Crate | Type | Purpose |
|-------|------|---------|
| `entangled` | Library | Core literate programming engine |
| `entangled-cli` | Binary | Native CLI (clap-based) |
| `pyentangled` | cdylib | Python bindings (PyO3) |

**Default members**: `entangled`, `entangled-cli` (pyentangled requires `maturin develop`)

## Core Library Modules

```
entangled/src/
  lib.rs              # Public exports
  config/             # Configuration management
  errors.rs           # Error types
  hooks/              # Extension hooks
  interface/          # High-level API
  io/                 # File I/O and transactions
  model/              # Core data structures
  readers/            # Markdown/code parsing
  text_location.rs    # Source location tracking
```

### Module Overview

| Module | Purpose |
|--------|---------|
| `config` | Load and validate entangled.toml configuration |
| `model` | Code blocks, references, and tangling algorithms |
| `readers` | Parse markdown and annotated code files |
| `io` | File operations, transactions, conflict detection |
| `interface` | High-level orchestration (Context, Document) |
| `hooks` | Pre/post-tangle processing hooks |
| `errors` | Unified error types |

## Key Types

### Configuration (`config/`)

```
Config
  source_patterns: Vec<String>    # Glob patterns for markdown files
  annotation: AnnotationMethod    # Standard, Naked, or Supplemental
  namespace_default: NamespaceDefault
  filedb_path: PathBuf
  languages: Vec<Language>        # Custom language definitions

AnnotationMethod
  Standard      # Add source reference comments
  Naked         # No annotations
  Supplemental  # Annotations for weaved output

Language
  name: String
  identifiers: Vec<String>
  comment: Comment                # Line or block comment style
```

### Data Model (`model/`)

```
CodeBlock
  id: ReferenceId                 # Unique identifier
  language: Option<String>        # Language identifier
  source: String                  # Code content
  target: Option<PathBuf>         # Output file (if file target)
  location: TextLocation          # Position in source markdown

ReferenceId
  name: ReferenceName             # Symbolic name
  count: usize                    # Instance number (for duplicates)

ReferenceName
  name: String                    # Base name
  namespace: Option<String>       # Optional namespace prefix
  # Supports: "name", "namespace::name", "file:path/to/file.py"

ReferenceMap
  blocks: IndexMap<ReferenceId, CodeBlock>  # Insertion-ordered storage
  name_index: HashMap<ReferenceName, Vec<ReferenceId>>
  targets: HashMap<PathBuf, ReferenceName>
```

### File I/O (`io/`)

```
Transaction
  actions: Vec<Box<dyn Action>>   # Pending file operations

Action (trait)
  Create(path, content)           # Create new file
  Write(path, content)            # Overwrite file
  Delete(path)                    # Remove file

FileDB
  files: HashMap<PathBuf, FileData>
  # Tracks SHA256 hashes and modification times

FileCache (trait)
  read(path) -> String
  glob(pattern) -> Vec<PathBuf>
  stat(path) -> FileData
```

### Interface (`interface/`)

```
Context
  config: Config
  hooks: HookRegistry
  file_cache: Arc<dyn FileCache>
  filedb: FileDB
  base_dir: PathBuf

Document
  path: PathBuf
  parsed: ParsedDocument
    refs: ReferenceMap
    frontmatter: Option<String>
```

### Hooks (`hooks/`)

```
Hook (trait)
  pre_tangle(block) -> PreTangleResult
  post_tangle(content, block) -> PostTangleResult

HookRegistry
  hooks: Vec<Box<dyn Hook>>
  run_pre_tangle(block) -> Result
  run_post_tangle(content, block) -> Result

Built-in hooks:
  ShebangHook       # Adds shebangs to executable scripts
  SpdxLicenseHook   # Adds SPDX license headers
```

## Data Flow

### Tangle (Markdown to Code)

```
1. Load Configuration
   Context::from_current_dir()
     -> Finds entangled.toml
     -> Loads FileDB from .entangled/filedb.json

2. Collect References
   tangle_documents(&context)
     -> context.source_files()         # Find **/*.md
     -> For each markdown file:
          Document::load(path, context)
            -> parse_markdown(content, config)
            -> Extract CodeBlocks into ReferenceMap

3. Expand References
   For each target in ReferenceMap.targets():
     -> tangle_ref(refs, name, comment, markers)
          -> Recursively expand <<refname>> patterns
          -> Preserve indentation
          -> Add annotation markers (if not naked)
     -> hooks.run_post_tangle(content)

4. Execute Transaction
   transaction.execute(&mut filedb)
     -> Check for conflicts (external modifications)
     -> Write files atomically
     -> Update FileDB with new hashes
     -> Save filedb.json
```

### Stitch (Code to Markdown)

```
1. Load source markdown files
   -> Extract all code blocks into ReferenceMap

2. Read tangled files
   -> read_annotated_code(content)
   -> Parse annotation markers to extract blocks

3. Compare blocks
   -> Detect changes between source and tangled versions

4. Update markdown
   -> Generate Transaction with markdown updates
```

### Sync (Bidirectional)

```
1. Stitch first
   -> Apply code changes back to markdown

2. Tangle second
   -> Extract updated markdown to code files

3. Save state
   -> Update FileDB
```

## Reference Expansion Algorithm

The tangling algorithm recursively expands reference patterns:

```
Input markdown:
  ```python #main file=output.py
  <<imports>>
  <<body>>
  ```

  ```python #imports
  import sys
  ```

  ```python #body
  print("hello")
  ```

Expansion process:
  1. Start with "main" reference
  2. Find line "<<imports>>" -> expand recursively
  3. Find line "<<body>>" -> expand recursively
  4. Preserve indentation from parent block

Output (with annotations):
  # ~/~ begin <<main[0]>>
  import sys
  print("hello")
  # ~/~ end
```

**Cycle Detection**: A `CycleDetector` tracks the expansion stack to prevent infinite loops (e.g., `a -> b -> a`).

## Conflict Detection

The FileDB enables safe concurrent editing:

```
1. On tangle:
   - Compute SHA256 of new content
   - Check if file exists and hash differs from DB
   - If external modification detected:
     - Normal mode: Error with conflict message
     - Force mode: Overwrite anyway
   - Update DB with new hash and mtime

2. On stitch:
   - Compare tangled file content with source blocks
   - Detect if code was modified externally
```

## Annotation Format

Annotations mark block boundaries in tangled output:

```python
# ~/~ begin <<reference-name[0]>>
code content here
# ~/~ end
```

- Comment style matches the language (e.g., `//`, `#`, `/* */`)
- Reference ID includes instance count for disambiguation
- Markers are configurable via `Markers` config

## Type Relationships

```
Context
  Config
    AnnotationMethod
    Language[]
    Markers
  HookRegistry
    Hook[]
  FileDB
    FileData[]
  FileCache

Document
  ParsedDocument
    ReferenceMap
      CodeBlock[]
        ReferenceId
          ReferenceName

Transaction
  Action[]
```

## CLI Layer

The CLI (`entangled-cli`) provides commands that use the library:

| Command | Library Function |
|---------|-----------------|
| `tangle` | `tangle_documents()` + `transaction.execute()` |
| `stitch` | `stitch_documents()` + `transaction.execute()` |
| `sync` | `sync_documents()` |
| `watch` | Monitor + auto `sync_documents()` |
| `status` | Read `Context` state |
| `reset` | Clear `FileDB` |

## Python Bindings

The `pyentangled` crate provides Python access via PyO3:

```
pyentangled._core (compiled Rust module)
  Config          -> PyConfig wrapper
  Context         -> PyContext wrapper
  Document        -> PyDocument wrapper
  CodeBlock       -> PyCodeBlock wrapper
  Transaction     -> PyTransaction wrapper
  tangle_documents()
  stitch_documents()
  execute_transaction()
  sync_documents()
  tangle_ref()

pyentangled (Python package)
  __init__.py     # Re-exports from _core
  cli.py          # Python CLI (argparse, stdlib only)
  _core.pyi       # Type stubs for IDE support
```

## Design Principles

1. **Separation of Concerns**: Library is independent of CLI
2. **Transaction Safety**: File operations are atomic with rollback
3. **Conflict Detection**: FileDB tracks state for safe concurrent editing
4. **Extensibility**: Hook system for custom processing
5. **Round-trip Editing**: Annotations enable bidirectional sync
6. **No Runtime Dependencies** (Python CLI): Uses only stdlib
