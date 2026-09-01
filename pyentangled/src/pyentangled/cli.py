"""Command-line interface for Entangled.

This module provides a Python-based CLI for the Entangled literate programming
engine, using the Rust library through Python bindings.

Uses only Python standard library (no external dependencies).
"""

from __future__ import annotations

import argparse
import json
import logging
import os
import sys
import time
from pathlib import Path
from typing import Optional, Sequence

from pyentangled import __version__
from pyentangled._core import (
    Config,
    Context,
    Document,
    EntangledError,
    check_documents,
    collect_status,
    eval_documents,
    execute_transaction,
    graph_documents,
    locate_source,
    stitch_documents,
    stitch_files,
    sync_documents,
    tangle_documents,
    tangle_files,
)


# The native CLI exits with a structured code per error class (see
# `EntangledError::exit_code` in the Rust crate); bindings attach that code to
# the raised exception. Reporting a failure identically in both CLIs means using
# it rather than collapsing everything to 1.
GENERIC_FAILURE = 1
# Codes the native CLI produces for conditions this CLI detects itself, rather
# than by catching an engine error (see `EntangledError::exit_code`).
OTHER_FAILURE = 5  # EntangledError::Other


def report_error(e: BaseException) -> int:
    """Prints an engine error and returns the exit code the native CLI uses."""
    print(f"Error: {e}", file=sys.stderr)
    code = getattr(e, "exit_code", GENERIC_FAILURE)
    return code if isinstance(code, int) and 0 < code < 256 else GENERIC_FAILURE


DEFAULT_CONFIG = """\
version = "2.0"

# Glob patterns for source markdown files
source_patterns = ["**/*.md"]

# Code block syntax style for .md files
# Options: "entangled-rs" (default), "pandoc", "quarto", "knitr"
style = "entangled-rs"

# How to annotate output files
# Options: "standard" (default), "naked", "supplemental"
annotation = "standard"

# Default namespace for code block IDs
# Options: "file" (prefix with filename, default), "none"
namespace_default = "file"

# File database location
filedb_path = ".entangled/filedb.json"

# Watch configuration
[watch]
debounce_ms = 100

# Hook configuration
[hooks]
# shebang = true      # Move shebang lines to top of tangled output
# spdx_license = true # Move SPDX license headers to top of tangled output

# Custom language definitions (uncomment to add)
# [[languages]]
# name = "mylang"
# comment = "#"
# identifiers = ["ml", "myl"]
"""


def setup_logging(verbose: bool, quiet: bool) -> None:
    """Configure logging based on verbosity."""
    if quiet:
        level = logging.WARNING
    elif verbose:
        level = logging.DEBUG
    else:
        level = logging.INFO
    logging.basicConfig(
        level=level,
        format="%(levelname)s: %(message)s",
    )


def get_context(
    config_path: Optional[str],
    directory: Optional[str],
    style: Optional[str] = None,
) -> Context:
    """Create a Context from CLI options."""
    base_dir = directory or os.getcwd()

    if config_path:
        config = Config.from_file(config_path)
    else:
        config = Config.from_dir(base_dir)

    if style:
        config.style = style

    return Context(config=config, base_dir=base_dir)


def run_transaction(
    context: Context,
    transaction,
    verb: str,
    *,
    diff: bool = False,
    dry_run: bool = False,
    force: bool = False,
    quiet: bool = False,
) -> int:
    """Run a transaction with common option handling."""
    if transaction.is_empty():
        if not quiet:
            print(f"No files to {verb}.")
        return 0

    if diff:
        for d in transaction.diffs():
            print(d)
        return 0

    if dry_run:
        print(f"Would perform {len(transaction)} actions:")
        for desc in transaction.describe():
            print(f"  {desc}")
        return 0

    execute_transaction(transaction, context, force=force)
    context.save_filedb()

    if not quiet:
        past = {"stitch": "Stitched", "tangle": "Tangled"}.get(verb, "Processed")
        print(f"{past} {len(transaction)} files.")
    return 0


def cmd_init(args: argparse.Namespace) -> int:
    """Execute the init command."""
    try:
        base_dir = Path(args.directory or os.getcwd())
        config_path = base_dir / "entangled.toml"

        if config_path.exists():
            print(f"Error: {config_path} already exists", file=sys.stderr)
            # `init` runs before the native CLI builds a context, and its
            # dispatch uses a plain failure code rather than the error's own.
            return GENERIC_FAILURE

        config_path.write_text(DEFAULT_CONFIG)
        print(f"Created {config_path}")

        db_dir = base_dir / ".entangled"
        if not db_dir.exists():
            db_dir.mkdir(parents=True)
            print(f"Created {db_dir}/")

        # Add .entangled/ to .gitignore
        gitignore_path = base_dir / ".gitignore"
        entry = ".entangled/"
        if gitignore_path.exists():
            content = gitignore_path.read_text()
            if entry not in [line.strip() for line in content.splitlines()]:
                suffix = "" if content.endswith("\n") else "\n"
                gitignore_path.write_text(f"{content}{suffix}{entry}\n")
                print(f"Added {entry} to {gitignore_path}")
        else:
            gitignore_path.write_text(f"{entry}\n")
            print(f"Created {gitignore_path} with {entry}")

        return 0

    except Exception as e:
        return report_error(e)


def cmd_tangle(args: argparse.Namespace) -> int:
    """Execute the tangle command."""
    try:
        context = get_context(args.config, args.directory, args.style)

        if args.files:
            transaction = tangle_files(context, args.files)
        else:
            transaction = tangle_documents(context)

        return run_transaction(
            context, transaction, "tangle",
            diff=args.diff, dry_run=args.dry_run,
            force=args.force, quiet=args.quiet,
        )

    except Exception as e:
        return report_error(e)


def cmd_stitch(args: argparse.Namespace) -> int:
    """Execute the stitch command."""
    try:
        context = get_context(args.config, args.directory, args.style)

        if args.files:
            transaction = stitch_files(context, args.files)
        else:
            transaction = stitch_documents(context)

        return run_transaction(
            context, transaction, "stitch",
            diff=args.diff, dry_run=args.dry_run,
            force=args.force, quiet=args.quiet,
        )

    except Exception as e:
        return report_error(e)


def cmd_sync(args: argparse.Namespace) -> int:
    """Execute the sync command."""
    try:
        context = get_context(args.config, args.directory, args.style)

        if args.diff or args.dry_run:
            stitch_tx = stitch_documents(context)
            tangle_tx = tangle_documents(context)

            if args.diff:
                for d in stitch_tx.diffs():
                    print(d)
                for d in tangle_tx.diffs():
                    print(d)
                return 0

            # dry_run
            stitch_count = len(stitch_tx)
            tangle_count = len(tangle_tx)
            if stitch_count + tangle_count == 0:
                if not args.quiet:
                    print("Nothing to do.")
            else:
                if stitch_count > 0:
                    print(f"Would stitch {stitch_count} files:")
                    for desc in stitch_tx.describe():
                        print(f"  {desc}")
                if tangle_count > 0:
                    print(f"Would tangle {tangle_count} files:")
                    for desc in tangle_tx.describe():
                        print(f"  {desc}")
            return 0

        sync_documents(context, force=args.force)

        if not args.quiet:
            print("Synchronization complete.")
        return 0

    except Exception as e:
        return report_error(e)


def cmd_watch(args: argparse.Namespace) -> int:
    """Execute the watch command using polling (stdlib only)."""
    try:
        context = get_context(args.config, args.directory, args.style)
        debounce_seconds = args.debounce / 1000.0

        if not args.quiet:
            print(f"Watching for changes (debounce: {args.debounce}ms)...")
            print("Press Ctrl+C to stop.")

        # Initial sync. A failure here means the watcher would start from a
        # state that does not match the sources, so it is reported and the
        # watcher keeps running only because the next edit may well fix it --
        # the same policy the native CLI uses.
        try:
            sync_documents(context)
        except Exception as e:
            print(f"Initial sync failed: {e}", file=sys.stderr)
            print("Watching anyway; the next change will retry.", file=sys.stderr)

        # Derive the watch scope from the configured *patterns*, not from the
        # files that happen to exist right now: a project whose first `.qmd`
        # file is created after the watcher starts must still be picked up.
        base_path = Path(context.base_dir)
        patterns = context.source_patterns()
        if not patterns:
            patterns = ["**/*.md"]

        def get_watched_files() -> dict[Path, float]:
            """Every source file matching the configured patterns, with mtimes."""
            mtimes: dict[Path, float] = {}
            for pattern in patterns:
                for path in base_path.glob(pattern):
                    if not path.is_file() or ".entangled" in path.parts:
                        continue
                    try:
                        mtimes[path] = path.stat().st_mtime
                    except OSError:
                        pass
            return mtimes

        file_mtimes = get_watched_files()
        last_sync = time.time()

        while True:
            try:
                time.sleep(0.5)  # Poll interval

                current_mtimes = get_watched_files()

                # Compare both directions. Only checking the current paths
                # would miss a *deleted* source file, leaving its generated
                # output stale on disk indefinitely.
                changed = current_mtimes.keys() != file_mtimes.keys() or any(
                    current_mtimes[path] > file_mtimes[path] for path in current_mtimes
                )

                if changed:
                    now = time.time()
                    if now - last_sync >= debounce_seconds:
                        last_sync = now
                        file_mtimes = current_mtimes
                        try:
                            new_context = get_context(args.config, args.directory, args.style)
                            sync_documents(new_context)
                            if not args.quiet:
                                print("Synced after file change.")
                        except Exception as e:
                            print(f"Sync error: {e}", file=sys.stderr)

            except KeyboardInterrupt:
                print("\nStopped watching.")
                return 0

    except Exception as e:
        return report_error(e)


def cmd_status(args: argparse.Namespace) -> int:
    """Execute the status command."""
    try:
        context = get_context(args.config, args.directory, args.style)

        # The status is computed by the shared Rust implementation, so this
        # reports exactly the values (and JSON schema) the native CLI reports.
        # A document that fails to load raises rather than being skipped: a
        # partial status presented as complete is worse than an error.
        data = collect_status(context)
        source_files = data["source_files"]
        targets = data["targets"]

        if args.json:
            output = {
                "source_files": source_files,
                "targets": [
                    {"path": t["path"], "status": t["status"]} for t in targets
                ],
                "tracked_count": data["tracked_count"],
            }
            print(json.dumps(output, indent=2))
        else:
            print(f"Source files: {len(source_files)}")

            if args.status_verbose:
                for f in source_files:
                    print(f"  {f}")

            print(f"\nTarget files: {len(targets)}")

            if args.status_verbose:
                for t in targets:
                    print(f"  {t['path']} ({t['status']})")

            counts = {"up-to-date": 0, "needs-tangle": 0, "modified": 0, "missing": 0}
            for t in targets:
                counts[t["status"]] = counts.get(t["status"], 0) + 1

            print("\nStatus summary:")
            print(f"  Up to date: {counts['up-to-date']}")
            print(f"  Needs tangle: {counts['needs-tangle']}")
            print(f"  Externally modified: {counts['modified']}")
            print(f"  Missing: {counts['missing']}")

            print(f"\nTracked files in database: {data['tracked_count']}")
        return 0

    except Exception as e:
        return report_error(e)


def cmd_check(args: argparse.Namespace) -> int:
    """Execute the check command."""
    try:
        context = get_context(args.config, args.directory, args.style)
        findings = check_documents(context)

        if args.json:
            print(json.dumps(findings, indent=2))
        else:
            for f in findings:
                if f["file"] and f["line"]:
                    loc = f" ({f['file']}:{f['line']})"
                elif f["file"]:
                    loc = f" ({f['file']})"
                else:
                    loc = ""
                line = f"{f['severity']}[{f['kind']}]: {f['message']}{loc}"
                if f["severity"] == "error":
                    print(line, file=sys.stderr)
                else:
                    print(line)

        errors = sum(1 for f in findings if f["severity"] == "error")
        warnings = len(findings) - errors

        if errors or (args.strict and warnings):
            print(
                f"Error: check failed: {errors} error(s), {warnings} warning(s)",
                file=sys.stderr,
            )
            return OTHER_FAILURE
        if not args.quiet and not args.json:
            print("check passed: no issues found.")
        return 0

    except Exception as e:
        return report_error(e)


def cmd_graph(args: argparse.Namespace) -> int:
    """Execute the graph command."""
    try:
        context = get_context(args.config, args.directory, args.style)
        rendered = graph_documents(context, args.format)

        if args.output:
            resolved = Path(context.resolve_path(args.output))
            resolved.parent.mkdir(parents=True, exist_ok=True)
            resolved.write_text(rendered, encoding="utf-8")
            if not args.quiet:
                print(f"graph: wrote {resolved}")
        else:
            print(rendered, end="")
        return 0

    except Exception as e:
        return report_error(e)


def cmd_eval(args: argparse.Namespace) -> int:
    """Execute the eval command."""
    try:
        context = get_context(args.config, args.directory, args.style)
        results = eval_documents(context, force=args.force, dry_run=args.dry_run)

        if args.json:
            print(json.dumps(results, indent=2))
            return 0

        if not results:
            print("No runnable blocks found (mark a block with `eval=<runner>`).")
            return 0

        failed = 0
        for r in results:
            ok = r["exit_code"] == 0
            if args.dry_run:
                print(f"would run: {r['block_id']} ({r['runner']})")
                continue
            if ok:
                print(f"ok: {r['block_id']} ({r['runner']})")
            else:
                failed += 1
                print(f"FAILED: {r['block_id']} ({r['runner']})", file=sys.stderr)
            for stream in ("stdout", "stderr"):
                for line in r[stream].splitlines():
                    print(f"    {line}", file=sys.stderr if stream == "stderr" else sys.stdout)

        if not args.dry_run:
            print(
                f"{len(results)} block(s) evaluated, "
                f"{len(results) - failed} succeeded, {failed} failed."
            )
        # The native CLI reports failed blocks on stderr but still exits 0;
        # matching that keeps the two CLIs' contracts identical.
        return 0

    except Exception as e:
        return report_error(e)


def cmd_locate(args: argparse.Namespace) -> int:
    """Execute the locate command."""
    try:
        # Parse FILE:LINE
        location = args.location
        if ":" not in location:
            print("Error: Expected FILE:LINE format", file=sys.stderr)
            return 1

        file_part, line_part = location.rsplit(":", 1)
        try:
            line_num = int(line_part)
        except ValueError:
            print(f"Error: Invalid line number: {line_part}", file=sys.stderr)
            return 1

        context = get_context(args.config, args.directory, args.style)
        full_path = context.resolve_path(file_part)

        if not Path(full_path).exists():
            print(f"Error: File not found: {full_path}", file=sys.stderr)
            return OTHER_FAILURE

        result = locate_source(context, full_path, line_num)

        if result is not None:
            print(f"{result['source_file']}:{result['source_line']}")
        else:
            print(
                f"No source mapping for {file_part}:{line_num}",
                file=sys.stderr,
            )

        return 0

    except Exception as e:
        return report_error(e)


def cmd_config(args: argparse.Namespace) -> int:
    """Execute the config command."""
    try:
        base_dir = args.directory or os.getcwd()
        if args.config:
            config = Config.from_file(args.config)
        else:
            config = Config.from_dir(base_dir)
        if args.style:
            config.style = args.style

        print(f"style = \"{config.style}\"")
        print(f"annotation = \"{config.annotation}\"")
        print(f"namespace_default = \"{config.namespace_default}\"")
        print(f"source_patterns = {config.source_patterns}")
        print(f"filedb_path = \"{config.filedb_path}\"")
        print(f"strip_quarto_options = {'true' if config.strip_quarto_options else 'false'}")
        if config.output_dir is not None:
            print(f"output_dir = \"{config.output_dir}\"")
        print()
        print("[hooks]")
        print(f"shebang = {'true' if config.hooks_shebang else 'false'}")
        print(f"spdx_license = {'true' if config.hooks_spdx_license else 'false'}")
        print()
        print("[watch]")
        print(f"debounce_ms = {config.watch_debounce_ms}")
        return 0

    except Exception as e:
        return report_error(e)


def cmd_reset(args: argparse.Namespace) -> int:
    """Execute the reset command."""
    try:
        context = get_context(args.config, args.directory, args.style)

        if args.delete_files:
            tracked = context.tracked_files()

            if not tracked:
                print("No tracked files to delete.")
            else:
                if not args.force:
                    print(f"This will delete {len(tracked)} tracked files:")
                    for f in tracked:
                        print(f"  {f}")

                    response = input("Continue? [y/N] ").strip().lower()
                    if response != "y":
                        print("Aborted.")
                        return 0

                # Delete tracked files
                deleted = 0
                for f in tracked:
                    full_path = Path(context.resolve_path(f))
                    if full_path.exists():
                        full_path.unlink()
                        deleted += 1

                print(f"Deleted {deleted} tracked files.")

        # Clear the database
        context.clear_filedb()
        context.save_filedb()

        # Try to remove .entangled directory
        entangled_dir = Path(context.base_dir) / ".entangled"
        if entangled_dir.exists():
            try:
                filedb_path = entangled_dir / "filedb.json"
                if filedb_path.exists():
                    filedb_path.unlink()
                entangled_dir.rmdir()
            except OSError:
                pass  # Directory not empty

        print("Reset complete. File database cleared.")
        return 0

    except Exception as e:
        return report_error(e)


def create_parser() -> argparse.ArgumentParser:
    """Create the argument parser."""
    parser = argparse.ArgumentParser(
        prog="pyentangled",
        description="Entangled - Literate programming engine (Python bindings)",
    )
    parser.add_argument(
        "-c", "--config",
        metavar="FILE",
        help="Configuration file path",
    )
    parser.add_argument(
        "-C", "--directory",
        metavar="DIR",
        help="Working directory",
    )
    parser.add_argument(
        "-s", "--style",
        metavar="STYLE",
        choices=["entangled-rs", "pandoc", "quarto", "knitr"],
        help="Code block syntax style",
    )
    parser.add_argument(
        "-q", "--quiet",
        action="store_true",
        default=False,
        help="Suppress normal output",
    )
    parser.add_argument(
        "-v", "--verbose",
        action="store_true",
        help="Verbose output",
    )
    parser.add_argument(
        "-V", "--version",
        action="version",
        version=f"%(prog)s {__version__}",
    )

    subparsers = parser.add_subparsers(dest="command", metavar="COMMAND")

    # init
    p_init = subparsers.add_parser(
        "init",
        help="Initialize entangled configuration",
    )
    p_init.set_defaults(func=cmd_init)

    # tangle
    p_tangle = subparsers.add_parser(
        "tangle",
        help="Extract code from markdown files",
    )
    p_tangle.add_argument(
        "-f", "--force",
        action="store_true",
        help="Force overwrite modified files",
    )
    p_tangle.add_argument(
        "-n", "--dry-run",
        action="store_true",
        help="Show what would be done",
    )
    p_tangle.add_argument(
        "-d", "--diff",
        action="store_true",
        help="Show unified diffs of what would change",
    )
    p_tangle.add_argument(
        "files",
        nargs="*",
        metavar="FILE",
        help="Specific source files to tangle",
    )
    p_tangle.set_defaults(func=cmd_tangle)

    # stitch
    p_stitch = subparsers.add_parser(
        "stitch",
        help="Update markdown from modified code files",
    )
    p_stitch.add_argument(
        "-f", "--force",
        action="store_true",
        help="Force overwrite modified files",
    )
    p_stitch.add_argument(
        "-n", "--dry-run",
        action="store_true",
        help="Show what would be done",
    )
    p_stitch.add_argument(
        "-d", "--diff",
        action="store_true",
        help="Show unified diffs of what would change",
    )
    p_stitch.add_argument(
        "files",
        nargs="*",
        metavar="FILE",
        help="Specific source files to stitch",
    )
    p_stitch.set_defaults(func=cmd_stitch)

    # sync
    p_sync = subparsers.add_parser(
        "sync",
        help="Synchronize markdown and code files",
    )
    p_sync.add_argument(
        "-f", "--force",
        action="store_true",
        help="Force overwrite modified files",
    )
    p_sync.add_argument(
        "-n", "--dry-run",
        action="store_true",
        help="Show what would be done",
    )
    p_sync.add_argument(
        "-d", "--diff",
        action="store_true",
        help="Show unified diffs of what would change",
    )
    p_sync.set_defaults(func=cmd_sync)

    # watch
    p_watch = subparsers.add_parser(
        "watch",
        help="Watch for changes and sync automatically",
    )
    p_watch.add_argument(
        "--debounce",
        type=int,
        default=100,
        metavar="MS",
        help="Debounce delay in milliseconds (default: 100)",
    )
    p_watch.set_defaults(func=cmd_watch)

    # status
    p_status = subparsers.add_parser(
        "status",
        help="Show status of files",
    )
    p_status.add_argument(
        "-v", "--verbose",
        dest="status_verbose",
        action="store_true",
        help="Show detailed output",
    )
    p_status.add_argument(
        "--json",
        action="store_true",
        help="Output machine-readable JSON",
    )
    p_status.set_defaults(func=cmd_status)

    # locate
    p_locate = subparsers.add_parser(
        "locate",
        help="Locate markdown source for a tangled file line",
    )
    p_locate.add_argument(
        "location",
        metavar="FILE:LINE",
        help="Target file and line number (e.g. output.py:10)",
    )
    p_locate.set_defaults(func=cmd_locate)

    # check
    p_check = subparsers.add_parser(
        "check",
        help="Validate references, targets and cycles",
    )
    p_check.add_argument(
        "--json",
        action="store_true",
        help="Output findings as JSON",
    )
    p_check.add_argument(
        "--strict",
        action="store_true",
        help="Treat warnings as failures",
    )
    p_check.set_defaults(func=cmd_check)

    # graph
    p_graph = subparsers.add_parser(
        "graph",
        help="Render the reference graph",
    )
    p_graph.add_argument(
        "--format",
        default="dot",
        choices=["dot", "mermaid", "json"],
        help="Output format (default: dot)",
    )
    p_graph.add_argument(
        "-o", "--output",
        metavar="FILE",
        help="Write to FILE instead of stdout",
    )
    p_graph.set_defaults(func=cmd_graph)

    # eval
    p_eval = subparsers.add_parser(
        "eval",
        help="Run blocks marked with eval=<runner>",
    )
    p_eval.add_argument(
        "-f", "--force",
        action="store_true",
        help="Re-run every block, ignoring the cache",
    )
    p_eval.add_argument(
        "-n", "--dry-run",
        action="store_true",
        help="List runnable blocks without executing them",
    )
    p_eval.add_argument(
        "--json",
        action="store_true",
        help="Output results as JSON",
    )
    p_eval.set_defaults(func=cmd_eval)

    # config
    p_config = subparsers.add_parser(
        "config",
        help="Print effective configuration",
    )
    p_config.set_defaults(func=cmd_config)

    # reset
    p_reset = subparsers.add_parser(
        "reset",
        help="Reset the file database",
    )
    p_reset.add_argument(
        "--delete-files",
        action="store_true",
        help="Also delete tangled files",
    )
    p_reset.add_argument(
        "-f", "--force",
        action="store_true",
        help="Don't ask for confirmation",
    )
    p_reset.set_defaults(func=cmd_reset)

    return parser


def main(argv: Optional[Sequence[str]] = None) -> int:
    """Main entry point."""
    parser = create_parser()
    args = parser.parse_args(argv)

    # Set defaults for global flags that subcommands might access
    if not hasattr(args, "quiet"):
        args.quiet = False
    if not hasattr(args, "style"):
        args.style = None

    setup_logging(args.verbose, args.quiet)

    if args.command is None:
        parser.print_help()
        return 0

    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
