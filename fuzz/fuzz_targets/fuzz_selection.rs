#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    use terminal_core::selection::{Selection, SelectionAnchor, SelectionMode};

    if data.len() < 9 {
        return;
    }
    let mut pos = 0;
    let read_u16 = |pos: &mut usize, data: &[u8]| -> Option<u16> {
        if *pos + 2 > data.len() {
            return None;
        }
        let v = u16::from_le_bytes([data[*pos], data[*pos + 1]]);
        *pos += 2;
        Some(v)
    };

    let mode_byte = data[pos];
    pos += 1;
    let mode = match mode_byte % 4 {
        0 => SelectionMode::Char,
        1 => SelectionMode::Word,
        2 => SelectionMode::Line,
        _ => SelectionMode::Block,
    };

    let start_row = read_u16(&mut pos, data).unwrap_or(0) as u32;
    let start_col = read_u16(&mut pos, data).unwrap_or(0) as u32;
    let end_row = read_u16(&mut pos, data).unwrap_or(0) as u32;
    let end_col = read_u16(&mut pos, data).unwrap_or(0) as u32;

    let s = Selection::new(
        SelectionAnchor {
            row: start_row,
            col: start_col,
        },
        SelectionAnchor {
            row: end_row,
            col: end_col,
        },
        mode,
    );

    let _ = s.is_ordered();
    let _ = s.ordered();
    let _ = s.contains(0, 0);
});
