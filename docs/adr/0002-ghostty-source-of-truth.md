# 2. Ghostty as the Single Source of Terminal State

## Status

Accepted

Date: 2026-07-27

## Requirement IDs

FR-01, NFR-01, NFR-03

## Context

The original torvox maintained a **complete parallel data model** of terminal
state in `terminal-core/`: `Grid`, `Cell`, `Attrs`, `Color`, `Selection`,
`Cursor`, `DirtyMask` — replicating what Ghostty's C library already manages
internally. This was ~11 KLOC of Rust that duplicated Ghostty's production C
code.

Ghostty's `libghostty-vt` (via `libghostty-rs`) exposes all terminal state
through:

- **`RenderState`** — batched viewport snapshot (covers visible grid,
  scrollback, cursor, colors, palette)
- **`RowIterator` / `CellIterator`** — bulk access to per-row and per-cell
  data (fg/bg colors, graphemes, style, selection)
- **`Screen::GridRef`** — random access for scrollback browsing

These APIs provide everything needed for rendering and interaction without
replicating the data model.

## Decision

Ghostty (via `libghostty-rs`) is the **single source of truth** for all
terminal state:

- The `terminal-core/` crate is removed in its entirety
- Grid, cell, selection, cursor data flow directly from Ghostty C API through
  `libghostty-rs` into flat render buffers — no Rust data model in between
- `terminal.rs` in the new `native/` crate is a thin (~300 line) wrapper
  around `libghostty-vt` C API calls via `libghostty-rs`
- Scrollback browsing uses Ghostty's native `scrollback_rows` API, not a
  local copy
- Selection is managed by Ghostty's `ghostty_terminal_selection_*` C API
  exposed through `libghostty-rs`

## Alternatives Considered

### Keep terminal-core but delegate to Ghostty for syncing

- **Rejected**: Either Ghostty owns the state or we do. Hybrid ownership
  creates synchronization bugs. The C API has no "export to GridSnapshot" that
  stays in sync — every snapshot is a point-in-time copy.

### Keep terminal-core for no_std safety guarantees

- **Rejected** (ADR-0006): no_std is removed. The safety value of re-verifying
  something Ghostty already handles is near zero. Ghostty itself is
  production C code with extensive fuzzing.

## Consequences

### Positive

- Removes ~11 KLOC of redundant code (core + tests)
- Eliminates all sync bugs between Ghostty state and Rust state
- Ghostty C API additions (new VT features) are automatically available
  through `libghostty-rs` with no Rust data model changes
- Selection, cursor movement, and scrollback are handled by Ghostty's
  proven C implementation

This decision enables ADR-0003 (bridge simplification — no grid data needs
to cross the FFI boundary because Ghostty owns it) and drives ADR-0004
(thread model — the `!Send` constraint of Ghostty C types requires
per-session threads).

### Negative

- Rendering code must now go through `CellIterator` instead of random-access
  `GridSnapshot` — this is a meaningful code change in the renderer
- Scrollback row count queries require FFI call (was local field)
- Tests that validated the Rust data model no longer apply (they tested
  torvox's implementation, not correct terminal behavior)

## Compliance

- No `use ghostty_rs` or `use terminal_core` types in the rendering path —
  only `libghostty_rs::render::*` iterators. (`libghostty_rs` types are
  expected and required — they are the replacement API.)
- `cargo geiger` no longer needs to check `terminal-core` for unsafe

## Status Note (Jul 2026)

This decision has been fully implemented. `terminal-core` (the separate data-model crate) was deleted in Phase 3 (commit 74002cb). Ghostty is now the single source of truth for all terminal state. Cell data flows through `CellIterator` → `CellData` → `CellInstance` pipeline. The `#![no_std]` constraint is also removed since `terminal-core` no longer exists.
