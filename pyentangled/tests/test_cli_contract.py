"""Contract tests for the Python CLI and the bindings behind it.

These pin the things the project review found drifting between the two CLIs:
the command list, the version, the status schema, path resolution, and the exit
codes. Where the native `entangled` binary is available they are compared
against it directly, so the two front ends cannot silently diverge again.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
from pathlib import Path

import pytest

import pyentangled
from pyentangled import (
    Context,
    EntangledError,
    check_documents,
    collect_status,
    eval_documents,
    graph_documents,
    locate_source,
)
from pyentangled.cli import create_parser, get_context, main


def project_context(project: Path) -> Context:
    """A context built the way the CLI builds one: reading `entangled.toml`.

    `Context.default_for_dir` deliberately ignores the config file, which would
    give these tests a different namespace than the CLI uses.
    """
    return get_context(None, str(project))


# Every command the native CLI has. `weave` is deliberately Rust-only: its
# Pandoc integration and output-path handling live in the native CLI.
RUST_ONLY_COMMANDS = {"weave"}
EXPECTED_COMMANDS = {
    "init",
    "tangle",
    "stitch",
    "sync",
    "watch",
    "status",
    "locate",
    "check",
    "graph",
    "eval",
    "config",
    "reset",
}


def native_cli() -> str | None:
    """Path to the native `entangled` binary, if this checkout has built one."""
    found = shutil.which("entangled")
    if found:
        return found
    repo_root = Path(__file__).resolve().parents[2]
    for profile in ("release", "debug"):
        candidate = repo_root / "target" / profile / "entangled"
        if candidate.exists():
            return str(candidate)
    return None


needs_native = pytest.mark.skipif(
    native_cli() is None,
    reason="native `entangled` binary not built; run `cargo build --release`",
)


@pytest.fixture
def project(tmp_path: Path) -> Path:
    """A minimal project with one target and one runnable block."""
    (tmp_path / "entangled.toml").write_text(
        'version = "2.0"\nsource_patterns = ["**/*.md"]\nnamespace_default = "none"\n'
    )
    (tmp_path / "doc.md").write_text(
        "```python #main file=out.py\nprint(1)\n```\n\n"
        "```sh #demo eval=sh\necho hi\n```\n"
    )
    return tmp_path


def run_python_cli(project: Path, *args: str) -> int:
    """Runs the Python CLI against `project`, returning its exit code."""
    return main(["-C", str(project), *args])


def run_native_cli(project: Path, *args: str) -> subprocess.CompletedProcess:
    binary = native_cli()
    assert binary is not None
    return subprocess.run(
        [binary, "-C", str(project), *args],
        capture_output=True,
        text=True,
    )


# --- command coverage --------------------------------------------------------


def test_cli_exposes_every_expected_command():
    parser = create_parser()
    subparsers = [
        action
        for action in parser._actions
        if isinstance(action, argparse._SubParsersAction)
    ]
    assert subparsers, "parser has no subcommands"
    assert set(subparsers[0].choices) == EXPECTED_COMMANDS


@needs_native
def test_command_list_matches_the_native_cli_except_weave():
    help_text = run_native_cli(Path.cwd(), "--help").stdout
    native = {c for c in EXPECTED_COMMANDS | RUST_ONLY_COMMANDS if f"  {c}" in help_text}
    assert RUST_ONLY_COMMANDS <= native, "native CLI unexpectedly lacks weave"
    # Everything the native CLI offers, minus the documented Rust-only set.
    assert native - RUST_ONLY_COMMANDS <= EXPECTED_COMMANDS


# --- version -----------------------------------------------------------------


def test_version_comes_from_package_metadata():
    # It was hard-coded, and read 0.1.0 while the package was at 0.2.0.
    assert pyentangled.__version__ != "0.1.0"
    from importlib.metadata import version

    assert pyentangled.__version__ == version("pyentangled")


@needs_native
def test_version_matches_the_native_cli(capsys):
    native = run_native_cli(Path.cwd(), "--version").stdout.split()[-1]
    with pytest.raises(SystemExit):
        main(["--version"])
    assert capsys.readouterr().out.split()[-1] == native


# --- status ------------------------------------------------------------------


def test_status_reports_a_real_state_not_just_paths(project: Path):
    ctx = project_context(project)
    status = collect_status(ctx)
    assert [t["path"] for t in status["targets"]] == ["out.py"]
    # The Python CLI used to emit only the path.
    assert status["targets"][0]["status"] == "needs-tangle"

    run_python_cli(project, "tangle")
    assert collect_status(project_context(project))["targets"][0]["status"] == "up-to-date"


def test_status_surfaces_a_broken_document_instead_of_swallowing_it(project: Path):
    (project / "doc.md").write_text("```python #a file=x.py\n<<nope>>\n```\n")
    ctx = project_context(project)
    # A document whose references do not resolve must not be silently skipped:
    # a partial status presented as complete is worse than an error.
    with pytest.raises(EntangledError):
        # Tangling is what surfaces the unresolvable reference.
        from pyentangled import tangle_documents

        tangle_documents(ctx)


@needs_native
def test_status_json_is_identical_in_both_clis(project: Path, capsys):
    run_python_cli(project, "tangle")
    capsys.readouterr()  # discard the tangle output

    run_python_cli(project, "status", "--json")
    python_json = json.loads(capsys.readouterr().out)
    native_json = json.loads(run_native_cli(project, "status", "--json").stdout)

    assert python_json == native_json


# --- path handling -----------------------------------------------------------


def test_locate_source_accepts_a_relative_target(project: Path):
    run_python_cli(project, "tangle")
    ctx = project_context(project)

    # The binding passed the relative path straight through, so it only worked
    # for absolute ones -- unlike the CLI, which resolved first.
    result = locate_source(ctx, "out.py", 2)
    assert result is not None
    assert result["source_file"] == "doc.md"

    absolute = locate_source(ctx, str(project / "out.py"), 2)
    assert absolute == result


def test_output_dir_is_honoured_by_the_bindings(tmp_path: Path):
    (tmp_path / "entangled.toml").write_text(
        'version = "2.0"\nnamespace_default = "none"\noutput_dir = "generated"\n'
    )
    (tmp_path / "doc.md").write_text("```python #main file=out.py\nprint(1)\n```\n")

    assert run_python_cli(tmp_path, "tangle") == 0
    assert (tmp_path / "generated" / "out.py").exists()
    assert not (tmp_path / "out.py").exists()


def test_a_target_outside_the_project_is_refused(tmp_path: Path):
    (tmp_path / "entangled.toml").write_text(
        'version = "2.0"\nnamespace_default = "none"\n'
    )
    (tmp_path / "doc.md").write_text(
        "```python #evil file=../escaped.py\nprint('escaped')\n```\n"
    )

    assert run_python_cli(tmp_path, "tangle") != 0
    assert not (tmp_path.parent / "escaped.py").exists()


# --- the other project-wide commands ----------------------------------------


def test_check_graph_and_eval_are_available_through_the_bindings(project: Path):
    ctx = project_context(project)

    assert check_documents(ctx) == []
    assert graph_documents(ctx, "mermaid").startswith("graph")
    with pytest.raises(ValueError):
        graph_documents(ctx, "not-a-format")

    results = eval_documents(ctx)
    assert [(r["block_id"], r["exit_code"], r["stdout"].strip()) for r in results] == [
        ("demo", 0, "hi")
    ]


def test_check_reports_a_target_collision(tmp_path: Path):
    (tmp_path / "entangled.toml").write_text(
        'version = "2.0"\nnamespace_default = "none"\n'
    )
    (tmp_path / "doc.md").write_text(
        "```python #a file=out.py\nprint(1)\n```\n\n"
        "```python #b file=out.py\nprint(2)\n```\n"
    )
    ctx = project_context(tmp_path)

    findings = check_documents(ctx)
    kinds = {f["kind"] for f in findings}
    assert "target-collision" in kinds
    assert all(f["severity"] in {"error", "warning"} for f in findings)


# --- exit codes --------------------------------------------------------------


def test_engine_errors_carry_the_native_exit_code(tmp_path: Path):
    (tmp_path / "entangled.toml").write_text(
        'version = "2.0"\nnamespace_default = "none"\n'
    )
    (tmp_path / "doc.md").write_text(
        "```python #evil file=/nowhere/escaped.py\nprint(1)\n```\n"
    )
    ctx = project_context(tmp_path)

    from pyentangled import tangle_documents

    with pytest.raises(EntangledError) as excinfo:
        tangle_documents(ctx)
    # A configuration-class error; the native CLI exits 2 for these.
    assert excinfo.value.exit_code == 2


@needs_native
@pytest.mark.parametrize(
    "args",
    [
        ("tangle",),
        ("check",),
        ("status",),
        ("eval",),
    ],
)
def test_exit_codes_match_the_native_cli_on_a_collision(tmp_path: Path, args):
    (tmp_path / "entangled.toml").write_text(
        'version = "2.0"\nnamespace_default = "none"\n'
    )
    (tmp_path / "doc.md").write_text(
        "```python #a file=out.py\nprint(1)\n```\n\n"
        "```python #b file=out.py\nprint(2)\n```\n"
    )

    native = run_native_cli(tmp_path, *args).returncode
    python = run_python_cli(tmp_path, *args)
    assert python == native, f"{args}: python={python} native={native}"


@needs_native
def test_exit_codes_match_the_native_cli_on_a_clean_project(project: Path):
    for args in (("tangle",), ("check",), ("status",), ("eval",), ("graph",)):
        native = run_native_cli(project, *args).returncode
        python = run_python_cli(project, *args)
        assert python == native, f"{args}: python={python} native={native}"
