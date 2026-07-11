#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    use torvox_core::grid::Grid;

    if data.is_empty() {
        return;
    }
    let mut pos = 0;
    let read_u16 = |pos: &mut usize, data: &[u8]| -> Option<u16> {
        if *pos + 2 > data.len() {
            return None;
        }
        let v = u16::from_le_bytes([data[*pos], data[*pos + 1]]);
        *pos += 2;
        Some(v.max(1))
    };

    let Some(rows) = read_u16(&mut pos, data) else {
        return;
    };
    let Some(cols) = read_u16(&mut pos, data) else {
        return;
    };
    let rows = (rows % 512) + 1;
    let cols = (cols % 512) + 1;
    let max_scrollback = 10_000;

    let mut g = Grid::with_scrollback(rows as u32, cols as u32, max_scrollback);

    while pos < data.len() {
        let op = data[pos] % 8;
        pos += 1;
        match op {
            0 => {
                if let (Some(r), Some(c)) = (read_u16(&mut pos, data), read_u16(&mut pos, data)) {
                    if let (Some(nr), Some(nc)) =
                        (read_u16(&mut pos, data), read_u16(&mut pos, data))
                    {
                        let nr = (nr % 512) + 1;
                        let nc = (nc % 512) + 1;
                        g.resize(nr as u32, nc as u32);
                        g.fill_cells(r as u32, 'X', c as u32, nr.min(nc) as u32);
                    }
                } else {
                    break;
                }
            }
            1 => {
                if let (Some(r), Some(c)) = (read_u16(&mut pos, data), read_u16(&mut pos, data)) {
                    if let Some(count) = read_u16(&mut pos, data) {
                        g.scroll_up(r as u32, c as u32, count as u32);
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            2 => {
                if let (Some(r), Some(c)) = (read_u16(&mut pos, data), read_u16(&mut pos, data)) {
                    if let Some(count) = read_u16(&mut pos, data) {
                        g.scroll_down(r as u32, c as u32, count as u32);
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            3 => {
                g.clear_scrollback();
            }
            4 => {
                if g.rows() > 0 && g.cols() > 0 {
                    g.clear_cells(0, 0, g.cols().saturating_sub(1));
                }
            }
            5 => {
                g.mark_clean();
            }
            6 => {
                let r = (data.get(pos).copied().unwrap_or(0) as u32) % g.rows().max(1);
                g.mark_row_dirty(r);
                pos += 1;
            }
            7 => {
                if g.cols() > 0 {
                    g.fill_cells(0, ' ', 0, g.cols().saturating_sub(1));
                }
            }
            _ => unreachable!(),
        }
    }

    let _ = g.rows();
    let _ = g.cols();
    let _ = g.scrollback_length();
});
