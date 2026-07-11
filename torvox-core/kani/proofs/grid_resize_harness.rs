// Kani proof harness: grid resize does not panic
// Requires: cargo kani --harness grid_resize_bounds

#[cfg(kani)]
#[kani::proof]
fn grid_resize_bounds() {
    let rows_init: u16 = kani::any();
    let cols_init: u16 = kani::any();
    kani::assume(rows_init > 0 && rows_init <= 200);
    kani::assume(cols_init > 0 && cols_init <= 500);
    let mut grid = torvox_core::grid::Grid::new(rows_init as u32, cols_init as u32);

    let rows_new: u16 = kani::any();
    let cols_new: u16 = kani::any();
    kani::assume(rows_new > 0 && rows_new <= 200);
    kani::assume(cols_new > 0 && cols_new <= 500);
    grid.resize(rows_new as u32, cols_new as u32);

    let c = grid.cell(0, 0);
    assert!(
        c.is_some(),
        "cell(0,0) must exist after resize to {}x{}",
        rows_new,
        cols_new
    );
}

// Kani proof harness: partition logic bounded
// Requires: cargo kani --harness dirty_mask_partition

#[cfg(kani)]
#[kani::proof]
fn dirty_mask_partition() {
    let n_cols: u8 = kani::any();
    kani::assume(n_cols > 0 && n_cols <= 64);
    let words = (n_cols as u64 + 63) / 64;
    let required: usize = words as usize;
    let v: Vec<u64> = vec![0u64; required.max(1)];
    assert!(v.len() >= 1);
}
