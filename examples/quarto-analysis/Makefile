# Penguin Morphology Analysis — Makefile
#
# Literate data-science workflow:  the Quarto document (report.qmd) is the
# single source of truth.  Entangled tangles it into Python scripts;
# Quarto renders the final HTML/PDF report.

ENTANGLED ?= ../../target/release/entangled
PYTHON    ?= python3
QUARTO    ?= quarto

# Source document
QMD := report.qmd

# Tangled scripts (derived from file= labels in the .qmd)
SCRIPTS := analysis.py load_data.py

# ── Core literate-programming targets ────────────────────────────────

.PHONY: tangle stitch sync watch status reset clean help run render report all

## Tangle: extract Python scripts from the Quarto document
tangle:
	$(ENTANGLED) tangle

## Stitch: propagate script edits back into the Quarto document
stitch:
	$(ENTANGLED) stitch

## Sync: bidirectional sync (stitch then tangle)
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

# ── Analysis targets ─────────────────────────────────────────────────

## Run: tangle and execute the analysis pipeline
run: tangle
	$(PYTHON) analysis.py

## Render: tangle, then render the Quarto report to HTML
render: tangle
	$(QUARTO) render $(QMD)

## Report: alias for render
report: render

## All: full pipeline — tangle, run analysis, render report
all: run render

# ── Housekeeping ─────────────────────────────────────────────────────

## Clean: remove tangled scripts, figures, rendered output, and caches
clean:
	rm -f $(SCRIPTS)
	rm -rf figures/
	rm -rf report_files/ report.html report.pdf
	rm -rf .entangled
	rm -rf __pycache__

## Help: list available targets
help:
	@echo "Targets:"
	@echo "  tangle   Extract Python scripts from report.qmd"
	@echo "  stitch   Update report.qmd from edited scripts"
	@echo "  sync     Bidirectional sync (stitch + tangle)"
	@echo "  watch    Continuously sync on file changes"
	@echo "  status   Show file sync status"
	@echo "  reset    Wipe database and tangled files, then re-tangle from scratch"
	@echo "  run      Tangle and execute analysis.py"
	@echo "  render   Tangle and render Quarto report to HTML"
	@echo "  all      Full pipeline (run + render)"
	@echo "  clean    Remove all generated files and caches"
