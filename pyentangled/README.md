# pyentangled

Python bindings for the Entangled literate programming engine.

## Overview

`pyentangled` provides Python bindings for Entangled, allowing you to:

- **Tangle**: Extract code from markdown files to source files
- **Stitch**: Update markdown files when code changes
- **Sync**: Bidirectional synchronization between markdown and code
- **Watch**: Automatically sync on file changes

## Installation

### From source (requires Rust toolchain)

```bash
cd pyentangled
maturin develop
```

Or build a wheel:

```bash
maturin build --release
pip install target/wheels/pyentangled-*.whl
```

## CLI Usage

```bash
# Extract code from markdown
pyentangled tangle

# Update markdown from code changes
pyentangled stitch

# Bidirectional sync
pyentangled sync

# Watch for changes
pyentangled watch

# Show status
pyentangled status

# Reset file database
pyentangled reset
```

### Options

```
pyentangled [OPTIONS] COMMAND

Options:
  -c, --config FILE    Configuration file path
  -C, --directory DIR  Working directory
  -v, --verbose        Verbose output
  -V, --version        Show version

Commands:
  tangle   Extract code from markdown files
  stitch   Update markdown from modified code files
  sync     Synchronize markdown and code files
  watch    Watch for changes and sync automatically
  status   Show status of files
  reset    Reset the file database
```

## Python API

```python
from pyentangled import (
    Config,
    Context,
    Document,
    tangle_documents,
    stitch_documents,
    execute_transaction,
    sync_documents,
    tangle_ref,
)

# Create a context from current directory
ctx = Context.from_current_dir()

# Or with custom config
config = Config()
config.source_patterns = ["docs/**/*.md"]
ctx = Context(config=config, base_dir="/path/to/project")

# Tangle documents
tx = tangle_documents(ctx)
if not tx.is_empty():
    print(f"Tangling {len(tx)} files:")
    for desc in tx.describe():
        print(f"  {desc}")
    execute_transaction(tx, ctx)
    ctx.save_filedb()

# Stitch documents
tx = stitch_documents(ctx)
if not tx.is_empty():
    execute_transaction(tx, ctx)
    ctx.save_filedb()

# Or sync (stitch then tangle)
sync_documents(ctx)
```

### Working with Documents

```python
from pyentangled import Document, Context

ctx = Context.from_current_dir()

# Load a document
doc = Document.load("README.md", ctx)

# Get all code blocks
for block in doc.blocks():
    print(f"Block: {block.id}")
    print(f"  Language: {block.language}")
    print(f"  Target: {block.target}")
    print(f"  Lines: {block.line_count()}")

# Get blocks by name
blocks = doc.get_by_name("main")

# Get target files
targets = doc.targets()

# Parse markdown directly
doc = Document.parse(markdown_content, path="example.md")
```

### Tangling References

```python
from pyentangled import Document, tangle_ref

doc = Document.parse("""
```python file=example.py
def hello():
    <<greeting>>
```

```python #greeting
print("Hello, World!")
```
""")

# Tangle a specific reference
code = tangle_ref(doc, "file=example.py", annotate=True)
print(code)
```

## Configuration

Create `entangled.toml` in your project root:

```toml
source_patterns = ["docs/**/*.md", "*.md"]
annotation = "standard"  # or "naked", "supplemented"
namespace_default = "file"  # or "none"
```

## Requirements

- Python 3.9+
- No runtime dependencies (stdlib only)

## Development

```bash
# Install dev dependencies
pip install -e ".[dev]"

# Run tests
pytest

# Build
maturin develop
```

## License

MIT
