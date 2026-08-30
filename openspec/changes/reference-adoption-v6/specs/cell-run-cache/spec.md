# Spec: cell-run-cache

## Purpose

Optimize JNI cell data transfer by merging consecutive cells with identical formatting into runs.

## Requirements

- REQ-C1: Detect consecutive cells with same fg_color, bg_color, flags
- REQ-C2: Merge into CellRun with start_col and length
- REQ-C3: Only merge within a single row (no cross-row runs)
- REQ-C4: Preserve incremental rendering path (dirty row cache)
- REQ-C5: Reduce total CellData entries for homogeneous text

## Test Cases

| ID | Input | Expected |
|----|-------|----------|
| TC-C1 | "hello" (same format) | 1 CellRun (length=5) |
| TC-C2 | "he\x1b[31mllo" (color change) | 2 CellRuns |
| TC-C3 | "ab\ncd" (newline) | 2 CellRuns (one per row) |
| TC-C4 | Empty row | 0 CellRuns |

## Traceability

- Source: termlib `CellRun.kt:25-74`
- Gap: torvox builds per-cell instances, no run merging
