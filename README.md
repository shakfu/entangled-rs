# entangled-rs

This is a rust translation of Johannes Hidding's [entangled](https://github.com/entangled/entangled), a literate programming engine. It extracts code from markdown files (`tangle`) and synchronize changes back (`stitch`).

To quote the original project's explanation:

> Entangled makes writing literate programs easier by keeping code blocks in markdown up-to-date with generated source files. By monitoring the tangled source files, any change in the master document or source files is reflected in the other. In practice this means:

> Write well documented code using Markdown. > Use any programming language you like (or are forced to use). > Keep debugging and using other IDE features without change. > Generate a report in PDF or HTML from the same source (see examples at Entangled homepage).

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

- **Weave**: Render documents to self-contained HTML (with clickable code cross-references), clean markdown, or PDF/docx/LaTeX/EPUB via pandoc

- **Eval**: Execute runnable code blocks and capture their output for reproducible reports (cached, shown in woven output)

- **Check**: Validate references, output targets, and cycles -- a CI/pre-commit gate

- **Graph**: Emit the block dependency graph as Graphviz DOT or Mermaid

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

The Python CLI covers every Rust command except `weave`:

```bash
pyentangled init
pyentangled tangle
pyentangled stitch --diff
pyentangled sync --dry-run
pyentangled locate output.py:10
pyentangled status --json
pyentangled check
pyentangled graph --format mermaid
pyentangled eval
pyentangled config
```

`weave` is Rust-CLI-only: its Pandoc integration and output-path handling live in the native CLI. Use `entangled weave` for rendering.

See [Python Bindings API](#python-bindings-api) for library usage.

## Quick Start

The fastest way to see the whole loop is to scaffold a runnable example:

```bash
entangled init --example   # writes entangled.toml and a starter hello.md
entangled tangle           # extract hello.py
entangled eval             # run the demo block (prints 55)
entangled weave -o hello.html   # render with clickable cross-references
entangled check            # validate references and targets
```

Or set it up by hand:

1. Create a markdown file with code blocks:

## Hello World

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

### CLI Reference

#### Commands

| Command | Description |
|---------|-------------|
| `tangle` | Extract code from markdown files |
| `stitch` | Update markdown from modified code files |
| `sync` | Synchronize markdown and code files |
| `watch` | Watch for changes and sync automatically |
| `weave` | Render documents to HTML, markdown, or (via pandoc) PDF/docx/etc. |
| `eval` | Execute runnable code blocks and cache their output |
| `check` | Validate references, targets, and cycles (CI-friendly) |
| `graph` | Emit the block dependency graph (DOT or Mermaid) |
| `status` | Show status of tracked files |
| `reset` | Reset the file database |
| `init` | Initialize a new entangled project (`--example` scaffolds a starter doc) |
| `locate` | Map a tangled file line back to its markdown source |

#### Global Options

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

#### Tangle Options

```bash
entangled tangle [OPTIONS] [-g PATTERN]... [FILES...]
```

| Option | Description |
|--------|-------------|
| `-f, --force` | Force overwrite modified files |
| `-n, --dry-run` | Show what would be done |
| `-d, --diff` | Show unified diffs of what would change |
| `-g, --glob <PATTERN>` | Filter source files by glob pattern (repeatable) |

#### Stitch Options

```bash
entangled stitch [OPTIONS] [-g PATTERN]... [FILES...]
```

| Option | Description |
|--------|-------------|
| `-f, --force` | Force overwrite modified files |
| `-n, --dry-run` | Show what would be done |
| `-d, --diff` | Show unified diffs of what would change |
| `-g, --glob <PATTERN>` | Filter source files by glob pattern (repeatable) |

#### Sync Options

```bash
entangled sync [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `-f, --force` | Force overwrite modified files |
| `-n, --dry-run` | Show what would be done |
| `-d, --diff` | Show unified diffs of what would change |

#### Locate Options

```bash
entangled locate <FILE:LINE>
```

Maps a line in a tangled output file back to its markdown source location. Useful for navigating from compiler errors to the originating documentation.

#### Watch Options

```bash
entangled watch [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `-d, --debounce <MS>` | Debounce delay in milliseconds (default: 100) |

#### Weave Options

```bash
entangled weave [OPTIONS] [-g PATTERN]... [FILES...]
```

| Option | Description |
|--------|-------------|
| `-t, --to <FORMAT>` | Output target: `html` (default), `markdown`, `quarto`, or any pandoc format (`pdf`, `latex`, `docx`, `epub`, ...) |
| `-o, --output <PATH>` | Output file path (single input only; `-` writes text targets to stdout) |
| `--fragment` | Emit an HTML fragment instead of a standalone document |
| `--pandoc <PATH>` | Path to the pandoc executable (default: `pandoc`) |
| `-g, --glob <PATTERN>` | Filter source files by glob pattern (repeatable) |

#### Eval Options

```bash
entangled eval [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `-f, --force` | Re-run every block even if a fresh cached result exists |
| `-n, --dry-run` | Report which blocks would run without executing them |

#### Check Options

```bash
entangled check [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--json` | Emit findings as JSON |
| `--strict` | Treat warnings as errors |

`check` reports **dangling references** (a `<<name>>` with no defining block), **target collisions** (two block names writing the same `file=`), **reference cycles**, and **orphan blocks** (never referenced or tangled). It exits non-zero when any error is found, so it works as a pre-commit or CI gate:

```bash
entangled check || exit 1
```

#### Graph Options

```bash
entangled graph [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `-f, --format <FORMAT>` | `dot` (default) or `mermaid` |
| `-o, --output <PATH>` | Output file (stdout if omitted) |

Emits the block reference-dependency graph. Tangle roots (blocks with a `file=` target) and dangling references are styled distinctly:

```bash
entangled graph -o graph.dot && dot -Tsvg graph.dot -o graph.svg
entangled graph --format mermaid   # paste into a Markdown mermaid block
```

### Weaving (documentation output)

Where `tangle` produces the machine-readable half of a literate program, `weave` produces the human-readable half: a typeset document that interleaves your prose with the code blocks. This is the counterpart that lets a single markdown source serve as both the program and its documentation.

Weaving is a two-layer design:

1. A **transform** rewrites Entangled-flavored markdown into a renderer-agnostic form. This is where the literate-programming value lives: each code block gets a caption (its name and target file), same-named blocks get continuation markers (`2/3`), each `<<reference>>` becomes a cross-reference, and every block records which other blocks it is *used in*.

2. **Backends** render the transformed document. HTML and clean markdown are produced natively (no external tools); every other format is generated by piping the clean markdown through [pandoc](https://pandoc.org).

```bash
# Self-contained HTML (offline, theme-aware, with clickable cross-references)
entangled weave README.md -o book.html

# Clean, portable markdown or a Quarto document (no pandoc needed)
entangled weave README.md --to markdown -o clean.md
entangled weave README.md --to quarto   -o report.qmd

# PDF / Word / LaTeX / EPUB via pandoc (must be installed)
entangled weave README.md --to pdf  -o report.pdf
entangled weave README.md --to docx -o report.docx

# Weave every source file to a sibling .html
entangled weave
```

The native HTML backend is fully self-contained: prose is rendered with `pulldown-cmark`, code is syntax-highlighted server-side with `syntect`, styles are embedded, `<<references>>` become intra-document links to the block that defines them, and each block shows a "used in" footer linking back to its callers. It adapts to light and dark themes via `prefers-color-scheme` and requires no network access.

Syntax highlighting is enabled by the default `highlight` cargo feature. Build with `--no-default-features` to drop the `syntect` dependency for a smaller binary; code then falls back to plain `language-xxx`-classed blocks.

Only the native targets (`html`, `markdown`, `quarto`) work without pandoc. Pandoc-backed formats additionally require pandoc on the `PATH` (or `--pandoc`), and `pdf` requires a LaTeX engine as usual.

### Reproducible output (executable blocks)

A code block marked with an `eval` attribute is *runnable*. `entangled eval` expands its references, pipes the resulting source to a configured runner (interpreter) on standard input, and captures the output. `weave` then renders that output beneath the block, giving reproducible reports and tutorials where the shown results are guaranteed to match the code.

````markdown
```python #answer eval=python
print(6 * 7)
```
````

```bash
entangled eval            # runs the block, prints and caches "42"
entangled weave -o out.html   # renders the block with a "42" output panel
```

Key points:

- **Explicit and opt-in.** Because it runs arbitrary code, execution happens *only* on `entangled eval` -- never during tangle, stitch, or weave.

- **Cached and reproducible.** Results are stored in `.entangled/eval-cache.json` keyed by block name and a hash of the expanded source. A block is re-run only when its code (or runner) changes; use `--force` to re-run everything.

- **The `eval` value names the runner.** `eval=python`, `eval=sh`, `eval=node`, etc. The special value `eval=true` uses the block's own language as the runner.

- **Failures are captured, not fatal.** A non-zero exit or missing interpreter is recorded (and shown as an error panel in weave) without aborting the run.

Built-in runners include `python`, `sh`/`bash`, `node`, `ruby`, `perl`, `lua`, `php`, `r`, and `deno`. Add or override them in `entangled.toml`:

```toml
[eval.runners]
# runner name -> command argv; the block source is piped on stdin
python = ["python3"]
sage = ["sage", "-python"]
```

### Code Block Syntax

Entangled supports multiple code block syntax styles to work with different document formats.

#### Supported Styles

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

#### entangled-rs Style (Default)

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

#### Pandoc Style

The original Entangled/Pandoc style uses curly braces with dot-prefixed language:

````markdown
``` {.python #main file=output.py}
print("Hello")
```
````

#### Quarto Style

Quarto style uses simple braces for language and `#|` comments for options:

````markdown
```{python}
#| label: main
#| file: output.py
print("Hello")
```
````

By default, `#|` lines are stripped from tangled output. Set `strip_quarto_options = false` in config to preserve them.

#### Knitr Style

RMarkdown/knitr style uses comma-separated options:

````markdown
```{python, label=main, file=output.py}
print("Hello")
```
````

#### References

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

#### Multiple Blocks with Same Name

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

### Configuration

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
# Options: "standard", "naked", "bare", "supplemental"
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

#### Style Options

| Option | Description |
|--------|-------------|
| `style` | Default style for `.md` files |
| `strip_quarto_options` | Remove `#\|` lines from output (default: true) |

Note: `.qmd` and `.Rmd` files always use their native styles regardless of config.

#### Annotation Methods

| Method | Description |
|--------|-------------|
| `standard` | Add `# ~/~ begin/end` markers (supports stitch) |
| `naked` | No annotations, raw code only (one-way) |
| `bare` | Blank lines between block boundaries (one-way) |
| `supplemental` | Annotations for documentation output (supports stitch) |

#### Output Directory

When `output_dir` is set, all *relative* tangled file paths are prefixed with the specified directory, which is itself resolved relative to the project root. For example, with `output_dir = "src"`, a code block with `file=main.py` is written to `src/main.py`. Absolute targets are unaffected.

The same resolution is used by `tangle`, `stitch`, `status` and the file database, so a project can turn `output_dir` on without any command losing track of its files.

#### Generated File Safety

`file=` targets come from documents, which Entangled treats as untrusted input. By default a target that resolves outside the project directory -- through `..` or an absolute path -- is rejected before anything is written:

```toml
# Opt in when a project deliberately generates files outside its own tree.
allow_external_targets = true
```

#### Namespace Default

| Value | Behavior |
|-------|----------|
| `file` | IDs prefixed with filename: `file.md#name` |
| `none` | IDs used as-is: `name` |

#### Hooks

Hooks process code blocks during tangling. Enable them in the `[hooks]` config section:

| Hook | Config Key | Description |
|------|-----------|-------------|
| Shebang | `hooks.shebang = true` | Strips a `#!/...` line from a target's first code block and re-inserts it, once, at the top of the tangled output file |
| SPDX License | `hooks.spdx_license = true` | Strips `SPDX-License-Identifier: ...` header lines from a target's first code block and re-inserts them, once, at the top of tangled output |

Hooks are useful when you want the shebang or license header to appear in the final file but not clutter every code block in the documentation.

Headers are lifted out *before* reference expansion and emitted exactly once, above the annotation markers, in the order the hooks are listed above. Both hooks compose, so an SPDX line beneath a shebang is still recognised. Stitching puts the header back into the markdown block it came from, so the round trip is lossless.

### Annotation Format

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

With `annotation = "bare"`, markers are replaced by blank lines, giving clean output with breathing room between blocks. With `annotation = "naked"`, markers are omitted entirely. Both modes are one-way (no stitch support).

### Project Structure

This project is organized as a Cargo workspace:

| Crate | Type | Edition | Description |
|-------|------|---------|-------------|
| `entangled` | Library | 2021 | Core library with no CLI dependencies |
| `entangled-cli` | Binary | 2021 | Command-line interface |
| `pyentangled` | Python | 2024 | Python bindings and CLI covering every Rust command except `weave` (PyO3/maturin) |

#### Rust Version Requirements

- `entangled` and `entangled-cli` use Rust edition 2021 and should compile with any recent stable Rust toolchain.

- `pyentangled` uses Rust edition 2024, requiring **Rust 1.85 or later**. This crate is excluded from default workspace builds (`cargo build` / `cargo test` skip it). Build it with `cd pyentangled && maturin develop`.

### Documentation

- [Architecture Overview](docs/architecture.md) - System design and module organization

- [CLI Comparison](docs/cli-comparison.md) - Comparison of Rust and Python CLIs

- [Benchmarks](docs/benchmarks.md) - Performance comparison of implementations

### Library API

#### Basic Usage

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

#### Core Types

##### Config

```rust
use entangled::Config;
use entangled::config::{AnnotationMethod, NamespaceDefault};

let mut config = Config::default();
config.annotation = AnnotationMethod::Naked;
config.namespace_default = NamespaceDefault::None;
config.source_patterns = vec!["docs/**/*.md".to_string()];
```

##### Context

```rust
use entangled::Context;
use std::path::PathBuf;

// With custom config
let ctx = Context::new(config, PathBuf::from("."))?;

// From current directory (reads entangled.toml)
let ctx = Context::from_current_dir()?;
```

##### ReferenceMap

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

##### Tangle

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

#### Parsing

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

#### Transactions

```rust
use entangled::io::{Transaction, FileDB};

let mut tx = Transaction::new();
tx.write("output.py", "print('hello')");
tx.create("new_file.rs", "fn main() {}");

let mut db = FileDB::new();
tx.execute(&mut db)?;
```

#### Hooks

```rust
use entangled::hooks::{Hook, HookRegistry, ShebangHook, SpdxLicenseHook};

let mut registry = HookRegistry::new();
registry.add(ShebangHook::new());
registry.add(SpdxLicenseHook::new());

// Hooks process blocks during tangle
let result = registry.run_post_tangle(&content, &block)?;
```

### Python Bindings API

#### Basic Usage

```python
from pyentangled import Context, tangle_documents, execute_transaction

ctx = Context.from_current_dir()
tx = tangle_documents(ctx)
if not tx.is_empty():
    execute_transaction(tx, ctx)
    ctx.save_filedb()
```

#### Configuration

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

#### File-Specific Operations

```python
from pyentangled import tangle_files, stitch_files

# Tangle only specific source files
tx = tangle_files(ctx, ["chapter1.md", "chapter2.md"])

# Stitch only specific source files
tx = stitch_files(ctx, ["chapter1.md"])
```

#### Diffs and Dry Runs

```python
tx = tangle_documents(ctx)
for diff in tx.diffs():
    print(diff)
```

#### Source Location Mapping

```python
from pyentangled import locate_source

result = locate_source(ctx, "output.py", 10)
if result:
    print(f"{result['source_file']}:{result['source_line']}")
```

#### Document Parsing

```python
from pyentangled import Document, tangle_ref

doc = Document.parse(markdown_content)
for block in doc.blocks():
    print(f"{block.name}: {block.language}, {block.line_count()} lines")

output = tangle_ref(doc, "main", annotate=False)
```

### Built-in Languages

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

### File Database

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

### Migrating from Python Entangled

entangled-rs is designed as a drop-in replacement for the [Python Entangled](https://github.com/entangled/entangled) project.

#### What stays the same

- **Configuration format**: `entangled.toml` files are compatible. The same keys (`version`, `source_patterns`, `annotation`, `namespace_default`, `languages`, `watch`, `hooks`) are recognized.

- **File database**: `.entangled/filedb.json` uses the same format. You can switch between implementations without resetting.

- **Annotation markers**: The `# ~/~ begin/end` format is identical, so tangled files produced by either implementation are interchangeable.

- **Code block syntax**: All four styles (entangled, Pandoc, Quarto, Knitr) are supported.

#### What's different

- **Performance**: 5-42x faster than the Python implementation (see [benchmarks](docs/benchmarks.md)).

- **Default style**: entangled-rs defaults to its own native style (`#name file=path`). Set `style = "pandoc"` in config to match the Python default.

- **Additional commands**: `init`, `locate`, `status`, and `reset` are new.

- **Additional flags**: `--diff`, `--quiet`, `--dry-run` (on sync) are new.

- **Hook activation**: Hooks (`shebang`, `spdx_license`) must be explicitly enabled in config. The `build` and `brei` hooks from Python Entangled are not yet implemented.

- **No daemon mode**: The Python version supports `entangled daemon`. Use `entangled watch` instead (equivalent behavior).

### License

MIT License
