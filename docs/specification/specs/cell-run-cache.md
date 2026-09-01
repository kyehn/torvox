# Spec: CellRun Cache (Run-Length Merging)

> Status: Implemented | Since: v7 (reference-adoption-v6)

## Purpose

Reduce GPU draw calls by merging consecutive cells with identical visual
properties (character, foreground, background, attributes) into single runs.
This is the termlib CellRun pattern adapted for wgpu rendering.

## Design

### CellRun structure

```rust
pub struct CellRun {
    pub column: u16,       // starting column
    pub length: u16,       // number of cells in this run
    pub cell: CellData,    // shared visual properties
}
```

### Algorithm

```rust
fn build_row_runs(cell_row: &[CellData]) -> Vec<CellRun> {
    // Walk cells left-to-right
    // If current cell matches run-ending cell → extend run
    // Otherwise → start new run
    // "Match" = same char + same fg + same bg + same attributes
}
```

### Key invariants

1. **Column-sequential**: Runs are always emitted in column order; no reordering.
2. **Coverage**: Every cell in the row is covered by exactly one run.
3. **Identity merge**: Two adjacent cells merge only when all visual properties
   are byte-identical — no fuzzy matching.
4. **Measurable savings**: In typical terminal output (plain text, code), runs
   average 5-15 cells, reducing draw calls by 50-80%.

### Files

- `native/src/render/cell_builder.rs` — `CellRun` struct, `build_row_runs()`
- termlib origin: adapted from the termlib CellRun pattern

### Test contract

- 5 test cases covering: uniform row (1 run), alternating (N runs),
  mixed content, empty row, single-cell runs
