# CLI Comparison: entangled vs pyentangled

This document compares the two CLI implementations provided by Entangled:

- **entangled** - Native Rust CLI (`entangled-cli` crate)
- **pyentangled** - Python CLI with Rust bindings (`pyentangled` package)

## Overview

| Aspect | entangled (Rust) | pyentangled (Python) |
|--------|------------------|----------------------|
| Framework | clap | argparse (stdlib) |
| Binary name | `entangled` | `pyentangled` |
| Dependencies | clap, tracing-subscriber | None (stdlib only) |
| File watching | notify crate (native events) | Polling (stdlib) |

## Global Options

Both CLIs support identical global options:

| Option | Description |
|--------|-------------|
| `-c, --config FILE` | Configuration file path |
| `-C, --directory DIR` | Working directory |
| `-v, --verbose` | Verbose output |
| `-V, --version` | Show version |
| `-h, --help` | Show help |

## Commands

### tangle

Extract code from markdown files.

```
entangled tangle [OPTIONS] [FILES...]
pyentangled tangle [OPTIONS] [FILES...]
```

| Option | Description |
|--------|-------------|
| `-f, --force` | Force overwrite modified files |
| `-n, --dry-run` | Show what would be done |
| `[FILES...]` | Specific files to tangle (not yet implemented) |

### stitch

Update markdown from modified code files.

```
entangled stitch [OPTIONS] [FILES...]
pyentangled stitch [OPTIONS] [FILES...]
```

| Option | Description |
|--------|-------------|
| `-f, --force` | Force overwrite modified files |
| `-n, --dry-run` | Show what would be done |
| `[FILES...]` | Specific files to stitch (not yet implemented) |

### sync

Synchronize markdown and code files (stitch then tangle).

```
entangled sync [OPTIONS]
pyentangled sync [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `-f, --force` | Force overwrite modified files |

### watch

Watch for changes and sync automatically.

```
entangled watch [OPTIONS]
pyentangled watch [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `-d, --debounce MS` | Debounce delay in milliseconds (default: 100) |

### status

Show status of files.

```
entangled status [OPTIONS]
pyentangled status [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `-v, --verbose` | Show detailed output |

### reset

Reset the file database.

```
entangled reset [OPTIONS]
pyentangled reset [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--delete-files` | Also delete tangled files |
| `-f, --force` | Don't ask for confirmation |

## Implementation Differences

### Watch Command

The watch implementations differ in how they detect file changes:

**Rust (entangled)**:
- Uses the `notify` crate for native file system events
- More efficient, lower CPU usage
- Instant detection of changes

**Python (pyentangled)**:
- Uses polling with `time.sleep()` and `Path.rglob()`
- No external dependencies (stdlib only)
- Polls every 500ms by default
- Watches common extensions: `.md`, `.py`, `.rs`, `.js`, `.ts`, `.go`, `.java`, `.c`, `.cpp`, `.h`

### Logging

**Rust**:
- Uses `tracing-subscriber` with `EnvFilter`
- Supports `RUST_LOG` environment variable

**Python**:
- Uses `logging.basicConfig()`
- Simple INFO/DEBUG levels based on `-v` flag

### File Filtering

Both CLIs accept file arguments for `tangle` and `stitch` commands, but file filtering is not yet implemented. A warning is printed when files are specified.

## When to Use Which

**Use `entangled` (Rust CLI) when**:
- You want the most efficient file watching
- You're not using Python in your project
- You want a single binary with no runtime dependencies

**Use `pyentangled` (Python CLI) when**:
- You're already using Python and want to integrate with your workflow
- You want to use the Python API programmatically
- You prefer pip/uv installation over cargo

## Examples

Both CLIs work identically:

```bash
# Extract code from markdown
entangled tangle
pyentangled tangle

# Update markdown from code changes
entangled stitch
pyentangled stitch

# Bidirectional sync
entangled sync
pyentangled sync

# Watch for changes
entangled watch
pyentangled watch

# Show status
entangled status -v
pyentangled status -v

# Reset and delete generated files
entangled reset --delete-files
pyentangled reset --delete-files
```
