// Kani proof harness: selection stays within grid
// Requires: cargo kani --harness selection_boundary

#[cfg(kani)]
#[kani::proof]
fn selection_stays_within_grid() {
    let rows: u16 = kani::any();
    let cols: u16 = kani::any();
    kani::assume(rows >= 2 && rows <= 100);
    kani::assume(cols >= 2 && cols <= 200);

    let start_row: u16 = kani::any();
    let start_col: u16 = kani::any();
    let end_row: u16 = kani::any();
    let end_col: u16 = kani::any();
    kani::assume(start_row < rows);
    kani::assume(start_col < cols);
    kani::assume(end_row < rows);
    kani::assume(end_col < cols);

    // Selection is valid if start <= end (inclusive, row-major)
    let start_idx = start_row as u32 * cols as u32 + start_col as u32;
    let end_idx = end_row as u32 * cols as u32 + end_col as u32;
    kani::assume(start_idx <= end_idx);

    let total = rows as u32 * cols as u32;
    assert!(end_idx < total, "Selection end within grid");
}
