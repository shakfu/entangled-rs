# Plan: an `entangled-core` crate

Status: deferred 2026-10-07. Nothing is implemented.

Why it is deferred:
- leo-rs keeps `@entangled` for reading and writing, and calls the
  `entangled` CLI to tangle and check.
- In-process checks and a tangled view do not justify the costs: the crate
  split, and two markdown readers in leolib kept in step.

Revisit if one of these happens:
- leo-rs needs tangled output inside the editor, such as a tangled view or
  checks on save;
- requiring the CLI becomes a problem for users.

## Goal

Split the parts of entangled-rs that do not touch the filesystem into a
crate with few dependencies:
- the block model, reading markdown, references, tangling, annotations and
  checks go into the new crate;
- `entangled-rs` keeps configuration files, the file database, file I/O,
  the CLI, weave and eval, and depends on the core.

leo-rs (`leolib`) then links the core to tangle and check its `@entangled`
documents in-process, without the `entangled` CLI and without `toml`,
`serde_json`, `sha2` or `chrono`.

Non-goals:
- leo-rs does not write generated files. Writing them stays with the CLI,
  because the CLI keeps `.entangled/filedb.json`. A file written without
  it makes the CLI's next tangle report a conflict.
- The public API of `entangled-rs` stays the same. Paths such as
  `entangled::model::ReferenceMap` stay valid through re-exports.

## Where each module goes

| Module | Lines | Goes to | Coupling to remove |
|-|-|-|-|
| `model/` (blocks, ids, names, properties, `tangle.rs`) | 2,066 | core | `tangle.rs` uses `config::{Comment, Markers, REF_PATTERN, annotation_*}`; `reference_map.rs` uses `indexmap` |
| `readers/` (markdown, code = the stitch reader, delimiters, YAML header) | 1,610 | core | `markdown.rs` takes `&Config`; `code.rs` uses `tracing` |
| `hooks/` (shebang, SPDX) | 540 | core | none |
| `config/{language,templates,markers,annotation_method,namespace_default}.rs` and `style.rs` | 875 | core | serde derives |
| `text_location.rs` | 94 | core | none |
| `check_refs`, `split_block_findings` (from `check/`) | part of 382 | core | serde derive on `Finding` |
| `errors.rs` | 95 | split | the core gets an error type without `toml`/`serde_json` variants |
| `config/{mod,config_data,config_update}.rs` | 654 | `entangled-rs` | none |
| `io/`, `interface/`, `status/`, `graph/`, `eval/`, `weave/` | 5,488 | `entangled-rs` | none |

Line counts include tests and come from `wc -l`.

## Dependencies of the core

The target is `regex`, `once_cell` and `nom`. `nom` 8 depends only on
`memchr`, which `regex` already uses, so it adds one crate. leolib already
depends on `regex` and `once_cell`.

| Dependency | Used for | Plan |
|-|-|-|
| `indexmap` | `ReferenceMap`'s insertion order | replace with `Vec` plus `HashMap` |
| `thiserror` | `EntangledError` | hand-written `Display` and `Error` for the core's error type |
| `tracing` | one log call in `readers/code.rs` | remove, or return it as a warning |
| `serde` | derives on settings types and `Finding` | optional `serde` feature, enabled by `entangled-rs` |

## Changes

1. **Readers take options, not `Config`.** A new
   `ReadOptions { style, namespace_default, strip_quarto_options }` struct,
   with `Config::read_options()` in `entangled-rs` to build it.
2. **Tangling takes its settings.** Tangling takes `Markers`, an
   `AnnotationMethod` and a language lookup (built-ins plus overrides)
   instead of reading `Config`.
3. **Errors.** The core gets `CoreError` with the variants it raises:
   - `ReferenceNotFound`
   - `CycleDetected`
   - `DuplicateReference`
   - `UnknownLanguage`
   - `Parse`
   - `Other`

   `EntangledError` wraps it with `#[from]`. The CLI and pyentangled only
   use `to_string()` and the `Other`, `Config` and `Io` variants, which
   stay.
4. **Re-exports.** `entangled-rs` re-exports the core's modules at their
   current paths, e.g. `pub use entangled_core::model;`.

## Phases (entangled-rs)

Each phase ends with `make test`, `make clippy` and `make test-pyentangled`
passing.

- **E1. Create the crate and move the modules unchanged.** Use `git mv`,
  and re-export from `entangled-rs`. The core keeps every current
  dependency for now.
  - Exit: all tests pass with no test edited;
  - `cargo semver-checks` reports no API change in `entangled-rs`.
- **E2. Decouple.** Make changes 1 to 3.
  - Exit: the core does not mention `Config`.
- **E3. Trim the dependencies.** Replace `indexmap`, `thiserror` and
  `tracing`, and put `serde` behind a feature.
  - Exit: `cargo tree -p entangled-core -e normal` lists only `regex`,
    `once_cell`, `nom` and their dependencies.
- **E4. Release.** `entangled-core` 0.1.0, then `entangled-rs` 0.5.0. The
  API is unchanged, but the crate's structure changed, so this is a minor
  version.

## Phases (leo-rs)

- **L1. Depend on the core.**
  - `leolib` gets `entangled-core` as a normal dependency.
  - The `entangled-rs` dev-dependency goes; the cross-check test uses the
    core's reader instead.
- **L2. A reference map of the outline.** Build the core's map from the
  outline, not from disk:
  - write each `@entangled` tree to a string with the existing writer;
  - parse that string with the core's markdown reader;
  - map block i to fence node i, in document order.

  The cross-check test already shows the two orders agree. leolib keeps
  its own tree reader: the core's reader returns blocks, not nodes, and it
  does not reproduce the file byte for byte.
- **L3. In-process check.** `:entangled-check` runs `check_refs` and
  `split_block_findings` on that map. Each finding goes to `:messages`
  with the node it is in. Clicking a finding in leogui goes to that node.
  The CLI is no longer needed for this.
- **L4. Tangled view.** For the selected fence node, or the `file=` block
  it belongs to, show the expanded code:
  - in leogui, a right panel like the rendered view;
  - in leotui, `:entangled-show` opens it read-only.
- **L5. Settings.** Read the scalar keys of `entangled.toml` with a small
  line parser in leolib:
  - `namespace_default`
  - `annotation`
  - `style`
  - `strip_quarto_options`
  - `split_blocks`

  If the file has a `[[languages]]` or `[hooks]` table, say that the view
  may differ from the CLI's output.

`:entangled-tangle` keeps calling the CLI.

## Risks

- **Two markdown readers in leolib.** leolib's tree reader and the core's
  block reader could disagree on an edge case. The cross-check test covers
  entangled-rs's examples and one document per style. L1 adds `demo/` and
  `tests/data/entangled/` to it.
- **The core's error type.** Code outside this repository that matches on
  `EntangledError::ReferenceNotFound` would need
  `EntangledError::Core(CoreError::ReferenceNotFound)`. Neither the CLI nor
  pyentangled does this. If that compatibility matters, E2 can keep
  `EntangledError`'s variants and convert into them.
- **Behaviour change hidden by the move.** E1 moves code without changing
  it, and its exit criterion forbids test edits. Any behaviour change
  waits for E2 or E3, where the tests are run against it.

## Open questions

1. **Crate name.** `entangled-core` or `entangled-rs-core`? Check with
   `cargo search entangled-core` first. `entangled` is taken on crates.io
   by an unrelated crate.
2. **leotui.** Does it need the tangled view (L4), or only leogui?
3. **leo-rs settings.** Is a scalar-key reader for `entangled.toml` (L5)
   acceptable, or should leo-rs use defaults only?
