# Benchmark Results

Comparison of three Entangled implementations on tangle operations.

## Implementations

| Implementation | Language | Description |
|----------------|----------|-------------|
| `entangled-cli` | Rust | Native CLI, release build |
| `pyentangled` | Python + Rust | Python CLI with Rust bindings (PyO3), release build |
| `entangled` | Python | Original Python implementation (v2.4.2) |

## Test Setup

- **Operation**: `tangle` command (dry run)
- **Workload**: Markdown file with N code blocks, each targeting a separate file
- **Iterations**: 5 per size (after warmup)
- **Platform**: macOS (Apple Silicon)
- **Build**: Release mode for both Rust implementations

## Results

### Absolute Times (ms)

| Blocks | entangled-cli | pyentangled | Python entangled |
|--------|---------------|-------------|------------------|
| 10     | 5.8 ms        | 32.5 ms     | 202 ms           |
| 50     | 4.8 ms        | 33.9 ms     | 201 ms           |
| 100    | 4.8 ms        | 31.8 ms     | 201 ms           |
| 500    | 6.7 ms        | 33.1 ms     | 203 ms           |
| 1000   | 8.6 ms        | 35.3 ms     | 213 ms           |
| 5000   | 24.4 ms       | 53.0 ms     | 240 ms           |
| 10000  | 50.5 ms       | 72.7 ms     | 277 ms           |

### Speedup vs Python entangled

| Blocks | entangled-cli | pyentangled |
|--------|---------------|-------------|
| 10     | 35x           | 6.2x        |
| 50     | 42x           | 5.9x        |
| 100    | 42x           | 6.3x        |
| 500    | 30x           | 6.1x        |
| 1000   | 25x           | 6.0x        |
| 5000   | 10x           | 4.5x        |
| 10000  | 5.5x          | 3.8x        |

## Analysis

### entangled-cli (Rust)

- **Fastest implementation** across all workload sizes
- **5-42x faster** than Python entangled
- Scales linearly: ~5ms base + ~4.5us per block
- Minimal startup overhead (~5ms)

### pyentangled (Python + Rust bindings)

- **4-6x faster** than Python entangled
- ~30ms Python overhead on top of Rust execution time
- Overhead components:
  - Python interpreter startup
  - Module imports (argparse, pathlib, etc.)
  - PyO3 data marshalling
- Good choice for Python integration

### entangled (Python)

- **Near-constant time** (~200ms) for small documents
- Dominated by Python startup and import time
- Starts scaling at ~1000+ blocks
- At 10000 blocks: 277ms (only ~75ms for actual work)

## Performance Breakdown

```
entangled-cli:  [~5ms startup] + [~4.5us/block]
pyentangled:    [~30ms Python startup] + [~4.5us/block]
entangled:      [~200ms Python startup] + [~7.5us/block]
```

## Recommendations

| Use Case | Recommended Implementation |
|----------|---------------------------|
| Production / CI pipelines | `entangled-cli` (Rust) |
| Python integration / scripting | `pyentangled` |
| Large documents (1000+ blocks) | `entangled-cli` (Rust) |
| Any performance-critical workload | `entangled-cli` (Rust) |

## Important Notes

**Build Mode Matters**: pyentangled must be built in release mode for fair comparison:

```bash
# Debug build (slow!) - default for maturin develop
maturin develop

# Release build (fast!) - use this
maturin develop --release
```

Debug builds can be 5-10x slower than release builds.

## Running the Benchmark

```bash
# Build release versions first
cargo build --release -p entangled-cli
cd pyentangled && maturin develop --release && cd ..

# Run benchmark
python3 benchmarks/compare_implementations.py --sizes 10,50,100,500,1000,5000,10000 --iterations 5
```

Options:
- `--sizes`: Comma-separated list of block counts
- `--iterations`: Number of iterations per benchmark
- `--rust-cli`: Path to entangled-cli binary
- `--pyentangled-venv`: Path to pyentangled venv
- `--python-entangled`: Path to Python entangled

## Notes

- Results may vary based on hardware and system load
- First run may show cold cache effects
- Python entangled uses different config format (watch_list vs source_patterns)
