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

### From Source

```bash
git clone https://github.com/entangled/entangled-rs
cd entangled-rs
cargo install --path .
```

### Using Cargo

```bash
cargo install entangled
```

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

### Global Options

| Option | Description |
|--------|-------------|
| `-c, --config <FILE>` | Configuration file path |
| `-C, --directory <DIR>` | Working directory |
| `-v, --verbose` | Verbose output |
| `-h, --help` | Print help |
| `-V, --version` | Print version |

### Tangle Options

```bash
entangled tangle [OPTIONS] [FILES...]
```

| Option | Description |
|--------|-------------|
| `-f, --force` | Force overwrite modified files |
| `-n, --dry-run` | Show what would be done |

### Watch Options

```bash
entangled watch [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `-d, --debounce <MS>` | Debounce delay in milliseconds (default: 100) |

## Code Block Syntax

### Basic Syntax

````markdown
```language #name file=output.py
code here
```
````

| Property | Description |
|----------|-------------|
| `language` | Language identifier (e.g., `python`, `rust`) |
| `#name` | Reference name for the block |
| `file=path` | Output file path (makes block a "target") |

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

Create `entangled.toml` in your project root:

```toml
# Configuration version
version = "2.0"

# Glob patterns for source markdown files
source_patterns = ["**/*.md"]

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

# Custom language definitions
[[languages]]
name = "mylang"
comment = "##"
identifiers = ["ml", "myl"]
```

### Annotation Methods

| Method | Description |
|--------|-------------|
| `standard` | Add `# ~/~ begin/end` markers |
| `naked` | No annotations, raw code only |
| `supplemental` | Annotations for documentation output |

### Namespace Default

| Value | Behavior |
|-------|----------|
| `file` | IDs prefixed with filename: `file.md#name` |
| `none` | IDs used as-is: `name` |

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

## Library API

### Basic Usage

```rust
use entangled::{Context, Config};
use entangled::commands::{tangle, TangleOptions};

// Create context from current directory
let mut ctx = Context::from_current_dir()?;

// Run tangle
tangle(&mut ctx, TangleOptions::default())?;
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

## Comparison with Python Entangled

This Rust implementation is a translation of the [Python Entangled](https://github.com/entangled/entangled) project with:

- Full feature parity for core functionality
- Compatible configuration format (`entangled.toml`)
- Compatible file database format (`.entangled/filedb.json`)
- Same annotation marker format (`# ~/~ begin/end`)
- Improved performance

## License

MIT License
