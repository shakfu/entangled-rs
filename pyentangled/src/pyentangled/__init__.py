"""Entangled - Literate Programming Engine.

This package provides Python bindings for the Entangled literate programming
system. It allows extracting code from markdown files (tangling) and updating
markdown from code changes (stitching).

Example:
    >>> from pyentangled import Context, tangle_documents, execute_transaction
    >>> ctx = Context.from_current_dir()
    >>> tx = tangle_documents(ctx)
    >>> if not tx.is_empty():
    ...     execute_transaction(tx, ctx)
    ...     ctx.save_filedb()
"""

from importlib.metadata import PackageNotFoundError, version as _package_version

from pyentangled._core import (
    Config,
    Context,
    Transaction,
    CodeBlock,
    Document,
    EntangledError,
    tangle_documents,
    tangle_files,
    stitch_documents,
    stitch_files,
    execute_transaction,
    sync_documents,
    locate_source,
    tangle_ref,
    collect_status,
    check_documents,
    graph_documents,
    eval_documents,
)

__all__ = [
    "Config",
    "Context",
    "Transaction",
    "CodeBlock",
    "Document",
    "EntangledError",
    "tangle_documents",
    "tangle_files",
    "stitch_documents",
    "stitch_files",
    "execute_transaction",
    "sync_documents",
    "locate_source",
    "tangle_ref",
    "collect_status",
    "check_documents",
    "graph_documents",
    "eval_documents",
    "main",
]

try:
    # Single source of truth: the version declared in pyproject.toml and baked
    # into the installed distribution. Hard-coding it here is how it came to
    # report 0.1.0 while the package and crate were both at 0.2.0.
    __version__ = _package_version("pyentangled")
except PackageNotFoundError:  # running from a source tree, not installed
    __version__ = "0.0.0+unknown"


def main() -> int:
    """CLI entry point."""
    from pyentangled.cli import main as cli_main
    return cli_main()
