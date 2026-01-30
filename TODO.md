# TODO

Pending implementations and improvements for entangled-rs.

## CLI Features

### File Filtering for Tangle/Stitch

The `tangle` and `stitch` commands accept file arguments but filtering is not yet implemented. Currently, specifying files logs a warning and all files are processed anyway.

**Current behavior:**
```bash
entangled tangle foo.md bar.md  # Warns and processes all markdown files
```

**Expected behavior:**
```bash
entangled tangle foo.md bar.md  # Only processes foo.md and bar.md
```

**Implementation notes:**
- The `TangleOptions::files` and `StitchOptions::files` fields exist
- Need to filter `ctx.source_files()` results before processing
- Should validate that specified files exist and match source patterns
- Consider supporting glob patterns in file arguments

**Files to modify:**
- `entangled-cli/src/commands/tangle.rs`
- `entangled-cli/src/commands/stitch.rs`
- Possibly `entangled/src/interface/document.rs` if filtering should happen at library level

## Library Improvements

### Parameterized sync_documents

The library's `sync_documents` function doesn't support force mode. The CLI works around this by calling `stitch_documents` and `tangle_documents` directly.

**Option A:** Add force parameter to `sync_documents`:
```rust
pub fn sync_documents(ctx: &mut Context, force: bool) -> Result<()>
```

**Option B:** Return transactions instead of executing:
```rust
pub fn sync_documents(ctx: &mut Context) -> Result<(Transaction, Transaction)>
```

**Files to modify:**
- `entangled/src/interface/document.rs`
