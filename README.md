# entangled-rs

This is a rust translation of Johannes Hidding's [entangled](https://github.com/entangled/entangled), a literate programming engine. It extracts code from markdown files (`tangle`) and synchronize changes back (`stitch`). 

To quote the original project's explanation:

> Entangled makes writing literate programs easier by keeping code blocks in markdown up-to-date with generated source files. By monitoring the tangled source files, any change in the master document or source files is reflected in the other. In practice this means:

> Write well documented code using Markdown.
> Use any programming language you like (or are forced to use).
> Keep debugging and using other IDE features without change.
> Generate a report in PDF or HTML from the same source (see examples at Entangled homepage).

## Overview

entangled-rs allows you to write documentation and code together in markdown files. Code blocks are extracted ("tangled") into source files, and changes to those files can be synchronized back ("stitched") into the markdown.


    # My Program
    
    ```python #main file=hello.py
    print("Hello, World!")
    ```

Running `entangled tangle` produces `hello.py` with the code block contents.

## Features

- **Tangle**: Extract code blocks from markdown into source files
- **Stitch**: Update markdown when tangled files are modified
- **Sync**: Bidirectional synchronization between markdown and code
- **Watch**: Monitor files for changes and sync automatically
- **References**: Code blocks can reference other blocks with `<<refname>>`
- **Annotations**: Generated files include markers for round-trip editing
- **40+ Languages**: Built-in comment style configurations
- **Conflict Detection**: Warns when files are modified externally

## Installation

### Pre-built Binaries

Download the latest release for your platform from [GitHub Releases](https://github.com/entangled/entangled-rs/releases):

| Platform | Archive |
|----------|---------|
| Linux (x86_64) | `entangled-v*-x86_64-unknown-linux-gnu.tar.gz` |
| Linux (aarch64) | `entangled-v*-aarch64-unknown-linux-gnu.tar.gz` |
| macOS (x86_64) | `entangled-v*-x86_64-apple-darwin.tar.gz` |
| macOS (Apple Silicon) | `entangled-v*-aarch64-apple-darwin.tar.gz` |
| Windows (x86_64) | `entangled-v*-x86_64-pc-windows-msvc.zip` |

### Using Cargo

```bash
cargo install entangled-cli
```

### From Source

```bash
git clone https://github.com/entangled/entangled-rs
cd entangled-rs
cargo install --path entangled-cli
```

### Python Bindings

```bash
pip install pyentangled
```

The Python CLI mirrors the Rust CLI:

```bash
pyentangled init
pyentangled tangle
pyentangled stitch --diff
pyentangled sync --dry-run
pyentangled locate output.py:10
pyentangled status --json
pyentangled config
```

See [Python Bindings API](#python-bindings-api) for library usage.

## Quick Start

1. Create a markdown file with code blocks:

    # Hello World

    ```python #main file=hello.py
    #!/usr/bin/env python3
    <<imports>>
    
    def main():
        <<body>>
    
    if __name__ == "__main__":
        main()
    ```

    ```python #imports
    import sys
    ```
    
    ```python #body
    print("Hello from Entangled!")
    ```

2. Create `entangled.toml`:

```toml
version = "2.0"
namespace_default = "none"
```

3. Run tangle:

```bash
entangled tangle
```

4. Check the generated file:

```bash
cat hello.py
```

## CLI Reference

### Commands

| Command | Description |
|---------|-------------|
| `tangle` | Extract code from markdown files |
| `stitch` | Update markdown from modified code files |
| `sync` | Synchronize markdown and code files |
| `watch` | Watch for changes and sync automatically |
| `status` | Show status of tracked files |
| `reset` | Reset the file database |
| `init` | Initialize a new entangled project |
| `locate` | Map a tangled file line back to its markdown source |

### Global Options

| Option | Description |
|--------|-------------|
| `-c, --config <FILE>` | Configuration file path |
| `-C, --directory <DIR>` | Working directory |
| `-s, --style <STYLE>` | Code block syntax style (overrides config) |
| `-v, --verbose` | Verbose output |
| `-q, --quiet` | Suppress normal output |
| `-h, --help` | Print help |
| `-V, --version` | Print version |

Available styles: `entangled-rs`, `pandoc`, `quarto`, `knitr`

### Tangle Options

```bash
entangled tangle [OPTIONS] [FILES...]
```

| Option | Description |
|--------|-------------|
| `-f, --force` | Force overwrite modified files |
| `-n, --dry-run` | Show what would be done |
| `-d, --diff` | Show unified diffs of what would change |

### Stitch Options

```bash
entangled stitch [OPTIONS] [FILES...]
```

| Option | Description |
|--------|-------------|
| `-f, --force` | Force overwrite modified files |
| `-n, --dry-run` | Show what would be done |
| `-d, --diff` | Show unified diffs of what would change |

### Sync Options

```bash
entangled sync [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `-f, --force` | Force overwrite modified files |
| `-n, --dry-run` | Show what would be done |
| `-d, --diff` | Show unified diffs of what would change |

### Locate Options

```bash
entangled locate <FILE:LINE>
```

Maps a line in a tangled output file back to its markdown source location. Useful for navigating from compiler errors to the originating documentation.

### Watch Options

```bash
entangled watch [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `-d, --debounce <MS>` | Debounce delay in milliseconds (default: 100) |

## Code Block Syntax

Entangled supports multiple code block syntax styles to work with different document formats.

### Supported Styles

| Style | File Extension | Example |
|-------|----------------|---------|
| `entangled-rs` | `.md` (default) | `` ```python #name file=out.py `` |
| `pandoc` | `.md` (configured) | `` ``` {.python #name file=out.py} `` |
| `quarto` | `.qmd` | `` ```{python} `` with `#\|` comments |
| `knitr` | `.Rmd` | `` ```{python, label=name, file=out.py} `` |

Style is determined automatically by file extension:
- `.qmd` files always use Quarto style
- `.Rmd` files always use Knitr style
- `.md` files use the configured default (or `entangled-rs` if not set)

### entangled-rs Style (Default)

The native style uses space-separated properties:

````markdown
```python #main file=output.py
print("Hello")
```
````

| Property | Description |
|----------|-------------|
| `language` | Language identifier (e.g., `python`, `rust`) |
| `#name` | Reference name for the block |
| `file=path` | Output file path (makes block a "target") |

### Pandoc Style

The original Entangled/Pandoc style uses curly braces with dot-prefixed language:

````markdown
``` {.python #main file=output.py}
print("Hello")
```
````

### Quarto Style

Quarto style uses simple braces for language and `#|` comments for options:

````markdown
```{python}
#| label: main
#| file: output.py
print("Hello")
```
````

By default, `#|` lines are stripped from tangled output. Set `strip_quarto_options = false` in config to preserve them.

### Knitr Style

RMarkdown/knitr style uses comma-separated options:

````markdown
```{python, label=main, file=output.py}
print("Hello")
```
````

### References

Reference other blocks using `<<refname>>`:

````markdown
```python #main file=app.py
<<imports>>
<<functions>>
```

```python #imports
import os
```

```python #functions
def hello():
    pass
```
````

References are expanded recursively with proper indentation preservation.

### Multiple Blocks with Same Name

Blocks with the same name are concatenated:

````markdown
```python #setup
import sys
```

```python #setup
import os
```
````

Results in:
```python
import sys
import os
```

## Configuration

Create `entangled.toml` (or `.entangled.toml`) in your project root. Both file names are recognized and searched for in the current directory and its parents.

```toml
# Configuration version
version = "2.0"

# Glob patterns for source markdown files
source_patterns = ["**/*.md", "**/*.qmd", "**/*.Rmd"]

# Optional output directory prefix for tangled files
# output_dir = "src"

# Code block syntax style for .md files
# Options: "entangled-rs" (default), "pandoc", "quarto", "knitr"
style = "entangled-rs"

# Strip #| comment lines from tangled output (Quarto style)
strip_quarto_options = true

# How to annotate output files
# Options: "standard", "naked", "supplemental"
annotation = "standard"

# Default namespace for code block IDs
# Options: "file" (prefix with filename), "none"
namespace_default = "file"

# File database location
filedb_path = ".entangled/filedb.json"

# Watch configuration
[watch]
debounce_ms = 100

# Hook configuration
[hooks]
shebang = true        # Extract shebangs from code and re-add after tangling
spdx_license = true   # Extract SPDX license headers and re-add after tangling

# Custom language definitions
[[languages]]
name = "mylang"
comment = "##"
identifiers = ["ml", "myl"]
```

### Style Options

| Option | Description |
|--------|-------------|
| `style` | Default style for `.md` files |
| `strip_quarto_options` | Remove `#\|` lines from output (default: true) |

Note: `.qmd` and `.Rmd` files always use their native styles regardless of config.

### Annotation Methods

| Method | Description |
|--------|-------------|
| `standard` | Add `# ~/~ begin/end` markers |
| `naked` | No annotations, raw code only |
| `supplemental` | Annotations for documentation output |

### Output Directory

When `output_dir` is set, all tangled file paths are prefixed with the specified directory. For example, with `output_dir = "src"`, a code block with `file=main.py` would be written to `src/main.py`.

### Namespace Default

| Value | Behavior |
|-------|----------|
| `file` | IDs prefixed with filename: `file.md#name` |
| `none` | IDs used as-is: `name` |

### Hooks

Hooks process code blocks during tangling. Enable them in the `[hooks]` config section:

| Hook | Config Key | Description |
|------|-----------|-------------|
| Shebang | `hooks.shebang = true` | Strips `#!/...` lines from markdown code blocks and re-inserts them at the top of the tangled output file |
| SPDX License | `hooks.spdx_license = true` | Strips `// SPDX-License-Identifier: ...` headers from markdown and re-inserts them at the top of tangled output |

Hooks are useful when you want the shebang or license header to appear in the final file but not clutter every code block in the documentation.

## Annotation Format

Generated files include markers for round-trip editing:

```python
# ~/~ begin <<main[0]>>
def main():
    # ~/~ begin <<body[0]>>
    print("Hello!")
    # ~/~ end
# ~/~ end
```

The format is:
- `# ~/~ begin <<name[index]>>` - Start of block
- `# ~/~ end` - End of block

Comment prefix varies by language (`//`, `--`, `/* */`, etc.).

## Project Structure

This project is organized as a Cargo workspace:

| Crate | Type | Edition | Description |
|-------|------|---------|-------------|
| `entangled` | Library | 2021 | Core library with no CLI dependencies |
| `entangled-cli` | Binary | 2021 | Command-line interface |
| `pyentangled` | Python | 2024 | Python bindings and CLI with full command parity (PyO3/maturin) |

### Rust Version Requirements

- `entangled` and `entangled-cli` use Rust edition 2021 and should compile with any recent stable Rust toolchain.
- `pyentangled` uses Rust edition 2024, requiring **Rust 1.85 or later**. This crate is excluded from default workspace builds (`cargo build` / `cargo test` skip it). Build it with `cd pyentangled && maturin develop`.

## Documentation

- [Architecture Overview](docs/architecture.md) - System design and module organization
- [CLI Comparison](docs/cli-comparison.md) - Comparison of Rust and Python CLIs
- [Benchmarks](docs/benchmarks.md) - Performance comparison of implementations

## Library API

### Basic Usage

```rust
use entangled::interface::Context;
use entangled::interface::tangle_documents;

// Create context from current directory
let mut ctx = Context::from_current_dir()?;

// Run tangle
let transaction = tangle_documents(&ctx)?;
transaction.execute(&mut ctx.filedb)?;
ctx.save_filedb()?;
```

### Core Types

#### Config

```rust
use entangled::Config;
use entangled::config::{AnnotationMethod, NamespaceDefault};

let mut config = Config::default();
config.annotation = AnnotationMethod::Naked;
config.namespace_default = NamespaceDefault::None;
config.source_patterns = vec!["docs/**/*.md".to_string()];
```

#### Context

```rust
use entangled::Context;
use std::path::PathBuf;

// With custom config
let ctx = Context::new(config, PathBuf::from("."))?;

// From current directory (reads entangled.toml)
let ctx = Context::from_current_dir()?;
```

#### ReferenceMap

```rust
use entangled::model::{ReferenceMap, CodeBlock, ReferenceName};

let mut refs = ReferenceMap::new();

// Insert blocks
let id = refs.insert(block);

// Lookup by name
let blocks = refs.get_by_name(&ReferenceName::new("main"));

// Get all targets
for target in refs.targets() {
    println!("{}", target.display());
}
```

#### Tangle

```rust
use entangled::model::{tangle_ref, ReferenceMap, ReferenceName};
use entangled::config::{Comment, Markers};

// Naked tangle (no annotations)
let output = tangle_ref(&refs, &name, None, None)?;

// Annotated tangle
let comment = Comment::line("#");
let markers = Markers::default();
let output = tangle_ref(&refs, &name, Some(&comment), Some(&markers))?;
```

### Parsing

```rust
use entangled::readers::{parse_markdown, ParsedDocument};
use entangled::Config;

let content = std::fs::read_to_string("doc.md")?;
let config = Config::default();
let doc = parse_markdown(&content, Some(Path::new("doc.md")), &config)?;

// Access parsed blocks
for block in doc.refs.blocks() {
    println!("{}: {}", block.id, block.source);
}
```

### Transactions

```rust
use entangled::io::{Transaction, FileDB};

let mut tx = Transaction::new();
tx.write("output.py", "print('hello')");
tx.create("new_file.rs", "fn main() {}");

let mut db = FileDB::new();
tx.execute(&mut db)?;
```

### Hooks

```rust
use entangled::hooks::{Hook, HookRegistry, ShebangHook, SpdxLicenseHook};

let mut registry = HookRegistry::new();
registry.add(ShebangHook::new());
registry.add(SpdxLicenseHook::new());

// Hooks process blocks during tangle
let result = registry.run_post_tangle(&content, &block)?;
```

## Python Bindings API

### Basic Usage

```python
from pyentangled import Context, tangle_documents, execute_transaction

ctx = Context.from_current_dir()
tx = tangle_documents(ctx)
if not tx.is_empty():
    execute_transaction(tx, ctx)
    ctx.save_filedb()
```

### Configuration

```python
from pyentangled import Config, Context

cfg = Config()
cfg.style = "pandoc"
cfg.annotation = "naked"
cfg.hooks_shebang = True
cfg.source_patterns = ["docs/**/*.md"]

ctx = Context(config=cfg, base_dir="/path/to/project")
```

Available Config properties: `style`, `annotation`, `namespace_default`, `source_patterns`, `output_dir`, `hooks_shebang`, `hooks_spdx_license`, `filedb_path`, `strip_quarto_options`, `watch_debounce_ms`.

### File-Specific Operations

```python
from pyentangled import tangle_files, stitch_files

# Tangle only specific source files
tx = tangle_files(ctx, ["chapter1.md", "chapter2.md"])

# Stitch only specific source files
tx = stitch_files(ctx, ["chapter1.md"])
```

### Diffs and Dry Runs

```python
tx = tangle_documents(ctx)
for diff in tx.diffs():
    print(diff)
```

### Source Location Mapping

```python
from pyentangled import locate_source

result = locate_source(ctx, "output.py", 10)
if result:
    print(f"{result['source_file']}:{result['source_line']}")
```

### Document Parsing

```python
from pyentangled import Document, tangle_ref

doc = Document.parse(markdown_content)
for block in doc.blocks():
    print(f"{block.name}: {block.language}, {block.line_count()} lines")

output = tangle_ref(doc, "main", annotate=False)
```

## Built-in Languages

Entangled includes comment style configurations for 40+ languages:

| Language      | Comment   | Aliases        |
| ------------- | --------- | -------------- |
| Python        | `#`       | py, python3    |
| Rust          | `//`      | rs             |
| JavaScript    | `//`      | js             |
| TypeScript    | `//`      | ts             |
| C/C++         | `//`      | c, cpp, h, hpp |
| Java          | `//`      |                |
| Go            | `//`      |                |
| Ruby          | `#`       | rb             |
| Bash          | `#`       | sh, shell, zsh |
| Haskell       | `--`      | hs             |
| OCaml         | `(* *)`   | ml             |
| HTML          | `<!-- -->`| htm            |
| CSS           | `/* */`   |                |
| SQL           | `--`      |                |
| YAML          | `#`       | yml            |
| TOML          | `#`       |                |
| Lua           | `--`      |                |
| ...           |           |                |

## File Database

Entangled tracks file states in `.entangled/filedb.json`:

```json
{
  "version": "1.0",
  "files": {
    "output.py": {
      "stat": {
        "mtime": "2024-01-15T10:30:00Z",
        "size": 256
      },
      "hexdigest": "abc123..."
    }
  }
}
```

This enables conflict detection when files are modified externally.

## Migrating from Python Entangled

entangled-rs is designed as a drop-in replacement for the [Python Entangled](https://github.com/entangled/entangled) project.

### What stays the same

- **Configuration format**: `entangled.toml` files are compatible. The same keys (`version`, `source_patterns`, `annotation`, `namespace_default`, `languages`, `watch`, `hooks`) are recognized.
- **File database**: `.entangled/filedb.json` uses the same format. You can switch between implementations without resetting.
- **Annotation markers**: The `# ~/~ begin/end` format is identical, so tangled files produced by either implementation are interchangeable.
- **Code block syntax**: All four styles (entangled, Pandoc, Quarto, Knitr) are supported.

### What's different

- **Performance**: 5-42x faster than the Python implementation (see [benchmarks](docs/benchmarks.md)).
- **Default style**: entangled-rs defaults to its own native style (`#name file=path`). Set `style = "pandoc"` in config to match the Python default.
- **Additional commands**: `init`, `locate`, `status`, and `reset` are new.
- **Additional flags**: `--diff`, `--quiet`, `--dry-run` (on sync) are new.
- **Hook activation**: Hooks (`shebang`, `spdx_license`) must be explicitly enabled in config. The `build` and `brei` hooks from Python Entangled are not yet implemented.
- **No daemon mode**: The Python version supports `entangled daemon`. Use `entangled watch` instead (equivalent behavior).

## License

MIT License
