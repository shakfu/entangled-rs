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

from pyentangled._core import (
    Config,
    Context,
    Transaction,
    CodeBlock,
    Document,
    tangle_documents,
    stitch_documents,
    execute_transaction,
    sync_documents,
    tangle_ref,
)

__all__ = [
    "Config",
    "Context",
    "Transaction",
    "CodeBlock",
    "Document",
    "tangle_documents",
    "stitch_documents",
    "execute_transaction",
    "sync_documents",
    "tangle_ref",
    "main",
]

__version__ = "0.1.0"


def main() -> int:
    """CLI entry point."""
    from pyentangled.cli import main as cli_main
    return cli_main()
