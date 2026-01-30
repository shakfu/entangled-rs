"""Type stubs for pyentangled._core Rust bindings."""

from typing import Optional, Sequence

class Config:
    """Configuration for Entangled."""

    def __init__(self) -> None: ...
    @staticmethod
    def from_dir(path: str) -> Config:
        """Load configuration from a directory (looks for entangled.toml)."""
        ...
    @staticmethod
    def from_file(path: str) -> Config:
        """Load configuration from a specific file."""
        ...
    @property
    def source_patterns(self) -> list[str]:
        """Get source patterns."""
        ...
    @source_patterns.setter
    def source_patterns(self, patterns: list[str]) -> None:
        """Set source patterns."""
        ...
    @property
    def annotation(self) -> str:
        """Get annotation method as string."""
        ...
    @annotation.setter
    def annotation(self, value: str) -> None:
        """Set annotation method from string."""
        ...
    @property
    def namespace_default(self) -> str:
        """Get namespace default as string."""
        ...
    @namespace_default.setter
    def namespace_default(self, value: str) -> None:
        """Set namespace default from string."""
        ...
    def __repr__(self) -> str: ...

class Transaction:
    """A transaction representing pending file operations."""

    def is_empty(self) -> bool:
        """Check if transaction is empty."""
        ...
    def __len__(self) -> int:
        """Get number of actions in transaction."""
        ...
    def describe(self) -> list[str]:
        """Get descriptions of all actions."""
        ...
    def __repr__(self) -> str: ...

class Context:
    """Context for Entangled operations."""

    def __init__(
        self,
        config: Optional[Config] = None,
        base_dir: Optional[str] = None,
    ) -> None: ...
    @staticmethod
    def from_current_dir() -> Context:
        """Create context from current directory."""
        ...
    @staticmethod
    def default_for_dir(path: str) -> Context:
        """Create context with default config for a specific directory."""
        ...
    @property
    def base_dir(self) -> str:
        """Get the base directory."""
        ...
    def source_files(self) -> list[str]:
        """Get source files matching the configuration patterns."""
        ...
    def resolve_path(self, path: str) -> str:
        """Resolve a relative path against the base directory."""
        ...
    def save_filedb(self) -> None:
        """Save the file database."""
        ...
    def tracked_file_count(self) -> int:
        """Get number of tracked files in the database."""
        ...
    def tracked_files(self) -> list[str]:
        """Get list of tracked files."""
        ...
    def clear_filedb(self) -> None:
        """Clear the file database."""
        ...
    def __repr__(self) -> str: ...

class CodeBlock:
    """A code block extracted from a markdown document."""

    @property
    def id(self) -> str:
        """Get the block's reference ID as string."""
        ...
    @property
    def name(self) -> str:
        """Get the block's reference name."""
        ...
    @property
    def language(self) -> Optional[str]:
        """Get the language identifier."""
        ...
    @property
    def source(self) -> str:
        """Get the source content."""
        ...
    @property
    def target(self) -> Optional[str]:
        """Get the target file path if this is a file target."""
        ...
    def is_empty(self) -> bool:
        """Check if block is empty."""
        ...
    def line_count(self) -> int:
        """Get number of lines in the block."""
        ...
    def __repr__(self) -> str: ...

class Document:
    """A parsed markdown document."""

    path: Optional[str]

    @staticmethod
    def load(path: str, ctx: Context) -> Document:
        """Load a document from a file."""
        ...
    @staticmethod
    def parse(
        content: str,
        path: Optional[str] = None,
        config: Optional[Config] = None,
    ) -> Document:
        """Parse markdown content directly."""
        ...
    def blocks(self) -> list[CodeBlock]:
        """Get all code blocks."""
        ...
    def get_by_name(self, name: str) -> list[CodeBlock]:
        """Get blocks by name."""
        ...
    def targets(self) -> list[str]:
        """Get all target file paths."""
        ...
    def __len__(self) -> int:
        """Get number of code blocks."""
        ...
    def __repr__(self) -> str: ...

def tangle_documents(ctx: Context) -> Transaction:
    """Tangle all documents in the context.

    Returns a Transaction that can be inspected or executed.
    """
    ...

def stitch_documents(ctx: Context) -> Transaction:
    """Stitch all documents in the context.

    Returns a Transaction that can be inspected or executed.
    """
    ...

def execute_transaction(
    transaction: Transaction,
    ctx: Context,
    force: bool = False,
) -> None:
    """Execute a transaction."""
    ...

def sync_documents(ctx: Context, force: bool = False) -> None:
    """Synchronize all documents (stitch then tangle)."""
    ...

def tangle_ref(doc: Document, name: str, annotate: bool = True) -> str:
    """Tangle a reference by name from a reference map."""
    ...
