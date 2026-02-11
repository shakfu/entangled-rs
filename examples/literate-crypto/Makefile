# Literate Crypto — Makefile
#
# Manages a literate-programming workflow where the source of truth is
# docs/*.md and entangled tangles them into Python source files.

ENTANGLED ?= entangled
PYTHON    ?= python3

# Source markdown files
DOCS := $(wildcard docs/*.md)

# Tangled output files (derived from file= attributes in the markdown)
SOURCES := ciphers.py cli.py test_ciphers.py

# ── Core literate-programming targets ────────────────────────────────

.PHONY: tangle stitch sync watch status reset clean help test run

## Tangle: extract source files from the literate documents
tangle:
	$(ENTANGLED) tangle

## Stitch: propagate source-file edits back into the markdown
stitch:
	$(ENTANGLED) stitch

## Sync: stitch then tangle (full bidirectional sync)
sync:
	$(ENTANGLED) sync

## Watch: continuously sync on file changes
watch:
	$(ENTANGLED) watch

## Status: show which files are in sync / modified
status:
	$(ENTANGLED) status -v

## Reset: wipe the entangled database and all tangled files, then re-tangle
reset:
	$(ENTANGLED) reset --delete-files -f
	$(ENTANGLED) tangle

# ── Project targets ──────────────────────────────────────────────────

## Test: tangle first, then run the test suite
test: tangle
	$(PYTHON) -m unittest test_ciphers -v

## Run: tangle and encrypt a demo message
run: tangle
	@echo "--- Caesar (shift 13) ---"
	@echo "HELLO WORLD" | $(PYTHON) cli.py caesar encrypt 13
	@echo "--- Vigenere (key SECRET) ---"
	@$(PYTHON) cli.py vigenere encrypt SECRET "ATTACK AT DAWN"

# ── Housekeeping ─────────────────────────────────────────────────────

## Clean: remove tangled files, entangled database, and Python caches
clean:
	rm -f $(SOURCES)
	rm -rf .entangled
	rm -rf __pycache__ .pytest_cache

## Help: list available targets
help:
	@echo "Targets:"
	@echo "  tangle   Extract source files from docs/*.md"
	@echo "  stitch   Update docs from edited source files"
	@echo "  sync     Bidirectional sync (stitch + tangle)"
	@echo "  watch    Continuously sync on file changes"
	@echo "  status   Show file sync status"
	@echo "  reset    Wipe database and tangled files, then re-tangle from scratch"
	@echo "  test     Tangle and run the test suite"
	@echo "  run      Tangle and run a demo"
	@echo "  clean    Remove all generated files and caches"
