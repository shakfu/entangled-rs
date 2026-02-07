"""Tests for pyentangled Python bindings."""

import os
import tempfile
from pathlib import Path

import pytest

from pyentangled import (
    Config,
    Context,
    Document,
    Transaction,
    CodeBlock,
    tangle_documents,
    tangle_files,
    stitch_documents,
    stitch_files,
    execute_transaction,
    sync_documents,
    locate_source,
    tangle_ref,
)


# --- Config ---


class TestConfig:
    def test_default(self):
        cfg = Config()
        assert cfg.annotation == "standard"
        assert cfg.namespace_default == "file"
        assert "**/*.md" in cfg.source_patterns

    def test_source_patterns_getter_setter(self):
        cfg = Config()
        cfg.source_patterns = ["docs/**/*.md"]
        assert cfg.source_patterns == ["docs/**/*.md"]

    def test_annotation_getter_setter(self):
        cfg = Config()
        cfg.annotation = "naked"
        assert cfg.annotation == "naked"

    def test_annotation_invalid(self):
        cfg = Config()
        with pytest.raises(ValueError):
            cfg.annotation = "bogus"

    def test_namespace_default_getter_setter(self):
        cfg = Config()
        cfg.namespace_default = "none"
        assert cfg.namespace_default == "none"

    def test_namespace_default_invalid(self):
        cfg = Config()
        with pytest.raises(ValueError):
            cfg.namespace_default = "bogus"

    def test_from_dir_missing(self):
        with tempfile.TemporaryDirectory() as d:
            cfg = Config.from_dir(d)
            # Falls back to defaults
            assert cfg.annotation == "standard"

    def test_from_file(self):
        with tempfile.TemporaryDirectory() as d:
            cfg_path = Path(d) / "entangled.toml"
            cfg_path.write_text('annotation = "naked"\n')
            cfg = Config.from_file(str(cfg_path))
            assert cfg.annotation == "naked"

    def test_repr(self):
        cfg = Config()
        r = repr(cfg)
        assert "Config(" in r
        assert "standard" in r

    def test_style_getter_default(self):
        cfg = Config()
        assert cfg.style == "entangled-rs"

    def test_style_setter(self):
        cfg = Config()
        cfg.style = "pandoc"
        assert cfg.style == "pandoc"
        cfg.style = "quarto"
        assert cfg.style == "quarto"
        cfg.style = "knitr"
        assert cfg.style == "knitr"
        cfg.style = "entangled-rs"
        assert cfg.style == "entangled-rs"

    def test_style_invalid(self):
        cfg = Config()
        with pytest.raises(ValueError):
            cfg.style = "bogus"

    def test_output_dir_default(self):
        cfg = Config()
        assert cfg.output_dir is None

    def test_output_dir_setter(self):
        cfg = Config()
        cfg.output_dir = "build/output"
        assert cfg.output_dir == "build/output"
        cfg.output_dir = None
        assert cfg.output_dir is None

    def test_hooks_shebang_default(self):
        cfg = Config()
        assert cfg.hooks_shebang is False

    def test_hooks_shebang_setter(self):
        cfg = Config()
        cfg.hooks_shebang = True
        assert cfg.hooks_shebang is True
        cfg.hooks_shebang = False
        assert cfg.hooks_shebang is False

    def test_hooks_spdx_license_default(self):
        cfg = Config()
        assert cfg.hooks_spdx_license is False

    def test_hooks_spdx_license_setter(self):
        cfg = Config()
        cfg.hooks_spdx_license = True
        assert cfg.hooks_spdx_license is True

    def test_filedb_path_default(self):
        cfg = Config()
        assert cfg.filedb_path == ".entangled/filedb.json"

    def test_filedb_path_setter(self):
        cfg = Config()
        cfg.filedb_path = "custom/db.json"
        assert cfg.filedb_path == "custom/db.json"

    def test_strip_quarto_options_default(self):
        cfg = Config()
        assert cfg.strip_quarto_options is True

    def test_strip_quarto_options_setter(self):
        cfg = Config()
        cfg.strip_quarto_options = False
        assert cfg.strip_quarto_options is False

    def test_watch_debounce_ms_default(self):
        cfg = Config()
        assert cfg.watch_debounce_ms == 100

    def test_watch_debounce_ms_setter(self):
        cfg = Config()
        cfg.watch_debounce_ms = 500
        assert cfg.watch_debounce_ms == 500


# --- Context ---


class TestContext:
    def test_default_for_dir(self):
        with tempfile.TemporaryDirectory() as d:
            ctx = Context.default_for_dir(d)
            assert ctx.base_dir == d
            assert ctx.tracked_file_count() == 0

    def test_with_config(self):
        with tempfile.TemporaryDirectory() as d:
            cfg = Config()
            ctx = Context(config=cfg, base_dir=d)
            assert ctx.base_dir == d

    def test_source_files_empty(self):
        with tempfile.TemporaryDirectory() as d:
            ctx = Context.default_for_dir(d)
            assert ctx.source_files() == []

    def test_source_files_finds_markdown(self):
        with tempfile.TemporaryDirectory() as d:
            (Path(d) / "test.md").write_text("# Hello\n")
            ctx = Context.default_for_dir(d)
            files = ctx.source_files()
            assert len(files) == 1
            assert "test.md" in files[0]

    def test_resolve_path(self):
        with tempfile.TemporaryDirectory() as d:
            ctx = Context.default_for_dir(d)
            resolved = ctx.resolve_path("output.py")
            assert resolved == str(Path(d) / "output.py")

    def test_tracked_files_empty(self):
        with tempfile.TemporaryDirectory() as d:
            ctx = Context.default_for_dir(d)
            assert ctx.tracked_files() == []

    def test_clear_filedb(self):
        with tempfile.TemporaryDirectory() as d:
            ctx = Context.default_for_dir(d)
            ctx.clear_filedb()
            assert ctx.tracked_file_count() == 0

    def test_repr(self):
        with tempfile.TemporaryDirectory() as d:
            ctx = Context.default_for_dir(d)
            r = repr(ctx)
            assert "Context(" in r


# --- Document ---


SIMPLE_MD = """\
```python #main file=hello.py
print('hello')
```
"""

MULTI_BLOCK_MD = """\
```python #main file=hello.py
<<greet>>
```

```python #greet
print('hello')
```
"""


class TestDocument:
    def test_parse_simple(self):
        doc = Document.parse(SIMPLE_MD)
        assert len(doc) == 1
        blocks = doc.blocks()
        assert len(blocks) == 1
        assert blocks[0].source == "print('hello')"

    def test_parse_with_references(self):
        doc = Document.parse(MULTI_BLOCK_MD)
        assert len(doc) == 2

    def test_load_from_file(self):
        with tempfile.TemporaryDirectory() as d:
            md_path = Path(d) / "test.md"
            md_path.write_text(SIMPLE_MD)
            ctx = Context.default_for_dir(d)
            doc = Document.load(str(md_path), ctx)
            assert len(doc) >= 1

    def test_blocks(self):
        doc = Document.parse(SIMPLE_MD)
        blocks = doc.blocks()
        assert isinstance(blocks[0], CodeBlock)

    def test_get_by_name(self):
        doc = Document.parse(SIMPLE_MD)
        blocks = doc.get_by_name("main")
        assert len(blocks) == 1
        assert blocks[0].name == "main"

    def test_get_by_name_missing(self):
        doc = Document.parse(SIMPLE_MD)
        blocks = doc.get_by_name("nonexistent")
        assert blocks == []

    def test_targets(self):
        doc = Document.parse(SIMPLE_MD)
        targets = doc.targets()
        assert len(targets) == 1
        assert targets[0] == "hello.py"

    def test_repr(self):
        doc = Document.parse(SIMPLE_MD)
        r = repr(doc)
        assert "Document(" in r


# --- CodeBlock ---


class TestCodeBlock:
    def test_properties(self):
        doc = Document.parse(SIMPLE_MD)
        block = doc.blocks()[0]
        assert block.name == "main"
        assert block.language == "python"
        assert block.source == "print('hello')"
        assert block.target == "hello.py"
        assert not block.is_empty()
        assert block.line_count() == 1

    def test_id_format(self):
        doc = Document.parse(SIMPLE_MD)
        block = doc.blocks()[0]
        assert "main" in block.id

    def test_repr(self):
        doc = Document.parse(SIMPLE_MD)
        block = doc.blocks()[0]
        r = repr(block)
        assert "CodeBlock(" in r


# --- tangle_ref ---


class TestTangleRef:
    def test_naked(self):
        doc = Document.parse(SIMPLE_MD)
        result = tangle_ref(doc, "main", annotate=False)
        assert result == "print('hello')"

    def test_annotated(self):
        doc = Document.parse(SIMPLE_MD)
        result = tangle_ref(doc, "main", annotate=True)
        assert "~/~ begin" in result
        assert "print('hello')" in result
        assert "~/~ end" in result

    def test_with_references(self):
        doc = Document.parse(MULTI_BLOCK_MD)
        result = tangle_ref(doc, "main", annotate=False)
        assert "print('hello')" in result

    def test_not_found(self):
        doc = Document.parse(SIMPLE_MD)
        with pytest.raises(RuntimeError):
            tangle_ref(doc, "nonexistent", annotate=False)


# --- tangle_documents / execute_transaction ---


class TestTangleDocuments:
    def test_tangle_empty(self):
        with tempfile.TemporaryDirectory() as d:
            ctx = Context.default_for_dir(d)
            tx = tangle_documents(ctx)
            assert tx.is_empty()
            assert len(tx) == 0

    def test_tangle_produces_transaction(self):
        with tempfile.TemporaryDirectory() as d:
            (Path(d) / "test.md").write_text(SIMPLE_MD)
            ctx = Context.default_for_dir(d)
            tx = tangle_documents(ctx)
            assert not tx.is_empty()
            assert len(tx) >= 1

    def test_transaction_describe(self):
        with tempfile.TemporaryDirectory() as d:
            (Path(d) / "test.md").write_text(SIMPLE_MD)
            ctx = Context.default_for_dir(d)
            tx = tangle_documents(ctx)
            descs = tx.describe()
            assert len(descs) >= 1
            assert any("hello.py" in desc for desc in descs)

    def test_execute_creates_file(self):
        with tempfile.TemporaryDirectory() as d:
            (Path(d) / "test.md").write_text(SIMPLE_MD)
            ctx = Context.default_for_dir(d)
            tx = tangle_documents(ctx)
            execute_transaction(tx, ctx)
            ctx.save_filedb()
            output = Path(d) / "hello.py"
            assert output.exists()
            assert "print('hello')" in output.read_text()

    def test_transaction_repr(self):
        with tempfile.TemporaryDirectory() as d:
            ctx = Context.default_for_dir(d)
            tx = tangle_documents(ctx)
            r = repr(tx)
            assert "Transaction(" in r

    def test_transaction_diffs(self):
        with tempfile.TemporaryDirectory() as d:
            (Path(d) / "test.md").write_text(SIMPLE_MD)
            ctx = Context.default_for_dir(d)
            tx = tangle_documents(ctx)
            diffs = tx.diffs()
            assert isinstance(diffs, list)
            # For a new file creation, there should be at least one diff
            assert len(diffs) >= 1
            assert any("hello.py" in diff for diff in diffs)


# --- tangle_files ---


class TestTangleFiles:
    def test_tangle_specific_file(self):
        with tempfile.TemporaryDirectory() as d:
            md_path = Path(d) / "test.md"
            md_path.write_text(SIMPLE_MD)
            ctx = Context.default_for_dir(d)
            tx = tangle_files(ctx, [str(md_path)])
            assert not tx.is_empty()
            execute_transaction(tx, ctx)
            ctx.save_filedb()
            output = Path(d) / "hello.py"
            assert output.exists()
            assert "print('hello')" in output.read_text()

    def test_tangle_no_files(self):
        with tempfile.TemporaryDirectory() as d:
            ctx = Context.default_for_dir(d)
            tx = tangle_files(ctx, [])
            assert tx.is_empty()


# --- stitch_documents ---


class TestStitchDocuments:
    def test_stitch_no_changes(self):
        with tempfile.TemporaryDirectory() as d:
            (Path(d) / "test.md").write_text(SIMPLE_MD)
            ctx = Context.default_for_dir(d)
            # Tangle first
            tx = tangle_documents(ctx)
            execute_transaction(tx, ctx)
            ctx.save_filedb()
            # Stitch should find no changes
            tx2 = stitch_documents(ctx)
            assert tx2.is_empty()


# --- stitch_files ---


class TestStitchFiles:
    def test_stitch_specific_file(self):
        with tempfile.TemporaryDirectory() as d:
            md_path = Path(d) / "test.md"
            md_path.write_text(SIMPLE_MD)
            ctx = Context.default_for_dir(d)
            # Tangle first
            tx = tangle_documents(ctx)
            execute_transaction(tx, ctx)
            ctx.save_filedb()
            # Stitch specific file should find no changes
            tx2 = stitch_files(ctx, [str(md_path)])
            assert tx2.is_empty()


# --- sync_documents ---


class TestSyncDocuments:
    def test_sync(self):
        with tempfile.TemporaryDirectory() as d:
            (Path(d) / "test.md").write_text(SIMPLE_MD)
            ctx = Context.default_for_dir(d)
            sync_documents(ctx)
            output = Path(d) / "hello.py"
            assert output.exists()
            assert "print('hello')" in output.read_text()


# --- locate_source ---


class TestLocateSource:
    def test_locate_content_line(self):
        with tempfile.TemporaryDirectory() as d:
            md_path = Path(d) / "test.md"
            md_path.write_text(SIMPLE_MD)
            ctx = Context.default_for_dir(d)
            tx = tangle_documents(ctx)
            execute_transaction(tx, ctx)
            ctx.save_filedb()

            output_path = str(Path(d) / "hello.py")
            # Line 2 is the content line (line 1 is the annotation begin marker)
            result = locate_source(ctx, output_path, 2)
            assert result is not None
            assert "source_file" in result
            assert "source_line" in result
            assert "block_id" in result
            assert "test.md" in result["source_file"]

    def test_locate_annotation_line(self):
        with tempfile.TemporaryDirectory() as d:
            md_path = Path(d) / "test.md"
            md_path.write_text(SIMPLE_MD)
            ctx = Context.default_for_dir(d)
            tx = tangle_documents(ctx)
            execute_transaction(tx, ctx)
            ctx.save_filedb()

            output_path = str(Path(d) / "hello.py")
            # Line 1 is the annotation begin marker -- should return None
            result = locate_source(ctx, output_path, 1)
            assert result is None
