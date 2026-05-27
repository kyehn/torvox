#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    use torvox_core::grid::Grid;

    if data.len() < 4 {
        return;
    }
    let rows = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) % 500 + 1;
    let cols = if data.len() >= 8 {
        u32::from_le_bytes([data[4], data[5], data[6], data[7]]) % 500 + 1
    } else {
        80
    };
    let mut grid = Grid::new(rows, cols);
    for (i, &byte) in data.iter().enumerate().skip(8) {
        let row = (i as u32) % rows;
        let col = (byte as u32) % cols;
        grid.mark_row_dirty(row);
        if let Some(cell) = grid.get_mut(row).and_then(|l| l.get_mut(col)) {
            cell.char = (byte as char).max(' ');
        }
    }
    grid.scroll_up(0, rows, cols);
});
