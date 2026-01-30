"""Command-line interface for Entangled.

This module provides a Python-based CLI for the Entangled literate programming
engine, using the Rust library through Python bindings.

Uses only Python standard library (no external dependencies).
"""

from __future__ import annotations

import argparse
import logging
import os
import sys
import time
from pathlib import Path
from typing import Optional, Sequence

from pyentangled._core import (
    Config,
    Context,
    Document,
    execute_transaction,
    stitch_documents,
    sync_documents,
    tangle_documents,
)


def setup_logging(verbose: bool) -> None:
    """Configure logging based on verbosity."""
    level = logging.DEBUG if verbose else logging.INFO
    logging.basicConfig(
        level=level,
        format="%(levelname)s: %(message)s",
    )


def get_context(
    config_path: Optional[str],
    directory: Optional[str],
) -> Context:
    """Create a Context from CLI options."""
    base_dir = directory or os.getcwd()

    if config_path:
        config = Config.from_file(config_path)
    else:
        config = Config.from_dir(base_dir)

    return Context(config=config, base_dir=base_dir)


def cmd_tangle(args: argparse.Namespace) -> int:
    """Execute the tangle command."""
    try:
        context = get_context(args.config, args.directory)

        if args.files:
            print(
                f"Warning: File filtering not yet implemented, processing all files "
                f"(ignoring {len(args.files)} specified files)",
                file=sys.stderr,
            )

        transaction = tangle_documents(context)

        if transaction.is_empty():
            print("No files to tangle.")
            return 0

        if args.dry_run:
            print(f"Would perform {len(transaction)} actions:")
            for desc in transaction.describe():
                print(f"  {desc}")
            return 0

        execute_transaction(transaction, context, force=args.force)
        context.save_filedb()

        print(f"Tangled {len(transaction)} files.")
        return 0

    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


def cmd_stitch(args: argparse.Namespace) -> int:
    """Execute the stitch command."""
    try:
        context = get_context(args.config, args.directory)

        if args.files:
            print(
                f"Warning: File filtering not yet implemented, processing all files "
                f"(ignoring {len(args.files)} specified files)",
                file=sys.stderr,
            )

        transaction = stitch_documents(context)

        if transaction.is_empty():
            print("No files to stitch.")
            return 0

        if args.dry_run:
            print(f"Would perform {len(transaction)} actions:")
            for desc in transaction.describe():
                print(f"  {desc}")
            return 0

        execute_transaction(transaction, context, force=args.force)
        context.save_filedb()

        print(f"Stitched {len(transaction)} files.")
        return 0

    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


def cmd_sync(args: argparse.Namespace) -> int:
    """Execute the sync command."""
    try:
        context = get_context(args.config, args.directory)

        sync_documents(context, force=args.force)

        print("Synchronization complete.")
        return 0

    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


def cmd_watch(args: argparse.Namespace) -> int:
    """Execute the watch command using polling (stdlib only)."""
    try:
        context = get_context(args.config, args.directory)
        debounce_seconds = args.debounce / 1000.0

        print(f"Watching for changes (debounce: {args.debounce}ms)...")
        print("Press Ctrl+C to stop.")

        # Initial sync
        try:
            sync_documents(context)
        except Exception as e:
            print(f"Initial sync error: {e}", file=sys.stderr)

        # Track file modification times
        file_mtimes: dict[Path, float] = {}
        base_path = Path(context.base_dir)
        extensions = {".md", ".py", ".rs", ".js", ".ts", ".go", ".java", ".c", ".cpp", ".h"}

        def get_watched_files() -> dict[Path, float]:
            """Get all watched files and their modification times."""
            mtimes = {}
            for ext in extensions:
                for path in base_path.rglob(f"*{ext}"):
                    if ".entangled" not in path.parts:
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
                changed = False

                # Check for modifications
                for path, mtime in current_mtimes.items():
                    old_mtime = file_mtimes.get(path)
                    if old_mtime is None or mtime > old_mtime:
                        changed = True
                        break

                # Check for new files
                if not changed:
                    for path in current_mtimes:
                        if path not in file_mtimes:
                            changed = True
                            break

                if changed:
                    now = time.time()
                    if now - last_sync >= debounce_seconds:
                        last_sync = now
                        file_mtimes = current_mtimes
                        try:
                            new_context = get_context(args.config, args.directory)
                            sync_documents(new_context)
                            print("Synced after file change.")
                        except Exception as e:
                            print(f"Sync error: {e}", file=sys.stderr)

            except KeyboardInterrupt:
                print("\nStopped watching.")
                return 0

    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


def cmd_status(args: argparse.Namespace) -> int:
    """Execute the status command."""
    try:
        context = get_context(args.config, args.directory)

        source_files = context.source_files()
        print(f"Source files: {len(source_files)}")

        if args.status_verbose:
            for f in source_files:
                print(f"  {f}")

        # Load documents and collect targets
        targets = []
        for path in source_files:
            try:
                doc = Document.load(path, context)
                targets.extend(doc.targets())
            except Exception:
                pass

        print(f"\nTarget files: {len(targets)}")

        if args.status_verbose:
            for t in targets:
                print(f"  {t}")

        print(f"\nTracked files in database: {context.tracked_file_count()}")
        return 0

    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


def cmd_reset(args: argparse.Namespace) -> int:
    """Execute the reset command."""
    try:
        context = get_context(args.config, args.directory)

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
        print(f"Error: {e}", file=sys.stderr)
        return 1


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
        "-v", "--verbose",
        action="store_true",
        help="Verbose output",
    )
    parser.add_argument(
        "-V", "--version",
        action="version",
        version="%(prog)s 0.1.0",
    )

    subparsers = parser.add_subparsers(dest="command", metavar="COMMAND")

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
        "files",
        nargs="*",
        metavar="FILE",
        help="Specific files to tangle",
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
        "files",
        nargs="*",
        metavar="FILE",
        help="Specific files to stitch",
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
    p_sync.set_defaults(func=cmd_sync)

    # watch
    p_watch = subparsers.add_parser(
        "watch",
        help="Watch for changes and sync automatically",
    )
    p_watch.add_argument(
        "-d", "--debounce",
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
    p_status.set_defaults(func=cmd_status)

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

    setup_logging(args.verbose)

    if args.command is None:
        parser.print_help()
        return 0

    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
