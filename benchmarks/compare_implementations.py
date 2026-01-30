#!/usr/bin/env python3
"""
Benchmark comparing three Entangled implementations:
- entangled-cli (Rust)
- pyentangled (Python + Rust bindings)
- entangled (Python original)

Usage:
    python benchmarks/compare_implementations.py [--sizes 10,50,100] [--iterations 5]
"""

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Optional


@dataclass
class BenchmarkResult:
    """Result of a single benchmark run."""
    implementation: str
    num_blocks: int
    duration_ms: float
    success: bool
    error: Optional[str] = None


def generate_markdown(num_blocks: int, lines_per_block: int = 10) -> str:
    """Generate a markdown file with the specified number of code blocks."""
    lines = ["# Benchmark Document\n"]

    # Main block that references all others
    lines.append("```python #main file=output.py")
    for i in range(num_blocks):
        lines.append(f"<<block{i}>>")
    lines.append("```\n")

    # Referenced blocks
    for i in range(num_blocks):
        lines.append(f"```python #block{i}")
        for j in range(lines_per_block):
            lines.append(f"print('Block {i} line {j}')")
        lines.append("```\n")

    return "\n".join(lines)


def generate_config(implementation: str = "rust") -> str:
    """Generate entangled.toml configuration compatible with the implementation."""
    if implementation == "entangled":
        # Python entangled v2.x config format
        return """version = "2.0"
watch_list = ["*.md"]
"""
    else:
        # Rust entangled / pyentangled config format
        return """version = "2.0"
source_patterns = ["*.md"]
namespace_default = "none"
annotation = "naked"
"""


def find_command(name: str) -> Optional[str]:
    """Find a command in PATH or common locations."""
    # Check PATH first
    result = shutil.which(name)
    if result:
        return result

    # Check common locations
    common_paths = [
        Path.home() / ".cargo" / "bin" / name,
        Path("/usr/local/bin") / name,
    ]

    for path in common_paths:
        if path.exists():
            return str(path)

    return None


def run_tangle(cmd: list[str], workdir: Path, timeout: float = 60.0) -> tuple[float, bool, Optional[str]]:
    """Run a tangle command and return (duration_ms, success, error)."""
    start = time.perf_counter()
    try:
        result = subprocess.run(
            cmd,
            cwd=workdir,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        duration = (time.perf_counter() - start) * 1000

        if result.returncode != 0:
            return duration, False, result.stderr or result.stdout
        return duration, True, None

    except subprocess.TimeoutExpired:
        duration = (time.perf_counter() - start) * 1000
        return duration, False, "Timeout"
    except Exception as e:
        duration = (time.perf_counter() - start) * 1000
        return duration, False, str(e)


def benchmark_implementation(
    name: str,
    cmd: list[str],
    workdir: Path,
    iterations: int,
    num_blocks: int,
) -> list[BenchmarkResult]:
    """Benchmark a single implementation."""
    results = []

    for i in range(iterations):
        # Clean up any previous output
        output_file = workdir / "output.py"
        entangled_dir = workdir / ".entangled"
        if output_file.exists():
            output_file.unlink()
        if entangled_dir.exists():
            shutil.rmtree(entangled_dir)

        duration, success, error = run_tangle(cmd, workdir)
        results.append(BenchmarkResult(
            implementation=name,
            num_blocks=num_blocks,
            duration_ms=duration,
            success=success,
            error=error,
        ))

    return results


def print_results(all_results: dict[str, list[BenchmarkResult]], sizes: list[int]) -> None:
    """Print benchmark results in a table format."""
    # Header
    print("\n" + "=" * 80)
    print("BENCHMARK RESULTS: Tangle Operation")
    print("=" * 80)

    # Get all implementations that have results
    implementations = list(all_results.keys())

    # Print table header
    header = f"{'Blocks':<10}"
    for impl in implementations:
        header += f"{impl:<20}"
    print(header)
    print("-" * (10 + 20 * len(implementations)))

    # Print results by size
    for size in sizes:
        row = f"{size:<10}"
        for impl in implementations:
            results = [r for r in all_results[impl] if r.num_blocks == size and r.success]
            if results:
                avg_ms = sum(r.duration_ms for r in results) / len(results)
                row += f"{avg_ms:>8.2f} ms        "
            else:
                # Check for errors
                errors = [r for r in all_results[impl] if r.num_blocks == size and not r.success]
                if errors:
                    row += f"{'ERROR':<20}"
                else:
                    row += f"{'N/A':<20}"
        print(row)

    print("-" * (10 + 20 * len(implementations)))

    # Print speedup comparison if we have entangled (Python) as baseline
    if "entangled" in implementations and len(implementations) > 1:
        print("\nSpeedup vs Python entangled:")
        print("-" * 50)

        for size in sizes:
            py_results = [r for r in all_results["entangled"] if r.num_blocks == size and r.success]
            if not py_results:
                continue

            py_avg = sum(r.duration_ms for r in py_results) / len(py_results)

            row = f"{size:<10}"
            for impl in implementations:
                if impl == "entangled":
                    row += f"{'1.00x (baseline)':<20}"
                else:
                    results = [r for r in all_results[impl] if r.num_blocks == size and r.success]
                    if results:
                        avg_ms = sum(r.duration_ms for r in results) / len(results)
                        speedup = py_avg / avg_ms if avg_ms > 0 else 0
                        row += f"{speedup:>8.1f}x           "
                    else:
                        row += f"{'N/A':<20}"
            print(row)

    print()


def main():
    parser = argparse.ArgumentParser(description="Benchmark Entangled implementations")
    parser.add_argument(
        "--sizes",
        type=str,
        default="10,50,100,200",
        help="Comma-separated list of block counts to test (default: 10,50,100,200)",
    )
    parser.add_argument(
        "--iterations",
        type=int,
        default=5,
        help="Number of iterations per benchmark (default: 5)",
    )
    parser.add_argument(
        "--rust-cli",
        type=str,
        default=None,
        help="Path to entangled-cli binary (default: auto-detect)",
    )
    parser.add_argument(
        "--pyentangled-venv",
        type=str,
        default=None,
        help="Path to pyentangled venv (default: ./pyentangled/.venv)",
    )
    parser.add_argument(
        "--python-entangled",
        type=str,
        default=None,
        help="Path to Python entangled (default: pip installed or ~/projects/personal/entangled)",
    )
    args = parser.parse_args()

    sizes = [int(s.strip()) for s in args.sizes.split(",")]

    # Find implementations
    implementations = {}

    # 1. Rust CLI (entangled-cli)
    rust_cli = args.rust_cli
    if not rust_cli:
        # Try to find in target/release first, then target/debug
        project_root = Path(__file__).parent.parent
        for build_type in ["release", "debug"]:
            candidate = project_root / "target" / build_type / "entangled"
            if candidate.exists():
                rust_cli = str(candidate)
                break
        if not rust_cli:
            rust_cli = find_command("entangled")

    if rust_cli and Path(rust_cli).exists():
        implementations["entangled-cli"] = [rust_cli, "tangle"]
        print(f"Found entangled-cli: {rust_cli}")
    else:
        print("Warning: entangled-cli not found. Building...")
        project_root = Path(__file__).parent.parent
        result = subprocess.run(
            ["cargo", "build", "--release", "-p", "entangled-cli"],
            cwd=project_root,
            capture_output=True,
        )
        if result.returncode == 0:
            rust_cli = str(project_root / "target" / "release" / "entangled")
            implementations["entangled-cli"] = [rust_cli, "tangle"]
            print(f"Built entangled-cli: {rust_cli}")
        else:
            print("Warning: Could not build entangled-cli")

    # 2. pyentangled
    pyentangled_venv = args.pyentangled_venv
    if not pyentangled_venv:
        project_root = Path(__file__).parent.parent
        pyentangled_venv = project_root / "pyentangled" / ".venv"
    else:
        pyentangled_venv = Path(pyentangled_venv)

    pyentangled_bin = pyentangled_venv / "bin" / "pyentangled"
    if pyentangled_bin.exists():
        implementations["pyentangled"] = [str(pyentangled_bin), "tangle"]
        print(f"Found pyentangled: {pyentangled_bin}")
    else:
        # Try using python -m
        python_bin = pyentangled_venv / "bin" / "python"
        if python_bin.exists():
            implementations["pyentangled"] = [str(python_bin), "-m", "pyentangled", "tangle"]
            print(f"Found pyentangled via: {python_bin} -m pyentangled")
        else:
            print(f"Warning: pyentangled not found at {pyentangled_venv}")

    # 3. Python entangled (original)
    python_entangled = args.python_entangled
    if not python_entangled:
        # Check common location first (most reliable)
        home_entangled = Path.home() / "projects" / "personal" / "entangled"
        if (home_entangled / ".venv" / "bin" / "entangled").exists():
            python_entangled = str(home_entangled / ".venv" / "bin" / "entangled")
        else:
            # Check if installed via pip and in PATH
            entangled_path = find_command("entangled")
            if entangled_path:
                python_entangled = entangled_path

    if python_entangled and Path(python_entangled).exists():
        implementations["entangled"] = [python_entangled, "tangle"]
        print(f"Found entangled: {python_entangled}")
    else:
        print("Warning: Python entangled not found")

    if not implementations:
        print("Error: No implementations found to benchmark")
        sys.exit(1)

    print(f"\nBenchmarking {len(implementations)} implementations:")
    for name, cmd in implementations.items():
        print(f"  - {name}: {' '.join(cmd)}")
    print(f"\nSizes: {sizes}")
    print(f"Iterations: {args.iterations}")

    # Run benchmarks
    all_results: dict[str, list[BenchmarkResult]] = {name: [] for name in implementations}

    with tempfile.TemporaryDirectory() as tmpdir:
        workdir = Path(tmpdir)

        for size in sizes:
            print(f"\nBenchmarking with {size} blocks...")

            # Generate test markdown (same for all implementations)
            md_content = generate_markdown(size)
            (workdir / "test.md").write_text(md_content)

            for name, cmd in implementations.items():
                # Generate implementation-specific config
                config_content = generate_config(name)
                (workdir / "entangled.toml").write_text(config_content)

                print(f"  Running {name}...", end=" ", flush=True)
                results = benchmark_implementation(
                    name, cmd, workdir, args.iterations, size
                )
                all_results[name].extend(results)

                successful = [r for r in results if r.success]
                if successful:
                    avg = sum(r.duration_ms for r in successful) / len(successful)
                    print(f"{avg:.2f} ms (avg of {len(successful)}/{len(results)})")
                else:
                    print(f"FAILED: {results[0].error if results else 'unknown'}")

    # Print summary
    print_results(all_results, sizes)


if __name__ == "__main__":
    main()
