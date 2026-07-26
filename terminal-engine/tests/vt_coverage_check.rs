use terminal_engine::ghostty_terminal::GhosttyTerminal;

fn term() -> GhosttyTerminal {
    GhosttyTerminal::new(24, 80, 1000).expect("terminal create")
}

fn assert_seq_runs_safely(t: &mut GhosttyTerminal, seq: &[u8], label: &str) {
    t.vt_write(seq);
    t.flush();
    let snap = t.take_snapshot();
    assert!(snap.rows > 0, "{label}: rows > 0");
    assert!(snap.cols > 0, "{label}: cols > 0");
    assert_eq!(
        snap.cells.len() as u32,
        snap.rows * snap.cols,
        "{label}: cell count"
    );
    assert!(snap.cursor_row < snap.rows, "cursor_row out of bounds");
    assert!(snap.cursor_col < snap.cols, "cursor_col out of bounds");
}

fn c0_sequences() -> Vec<(String, Vec<u8>)> {
    let mut v = Vec::new();
    for c in 0x00u8..=0x1f {
        v.push((format!("C0 0x{c:02X}"), vec![c]));
    }
    v
}

fn csi_final_sequences() -> Vec<(String, Vec<u8>)> {
    let mut v = Vec::new();
    for fb in 0x40u8..=0x7e {
        let seq = vec![0x1b, b'[', fb];
        v.push((format!("CSI {}", fb as char), seq));
    }
    v
}

fn sgr_sequences() -> Vec<(String, Vec<u8>)> {
    let mut v = Vec::new();
    for p in 0..=109 {
        let seq = format!("\x1b[{}mX", p).into_bytes();
        v.push((format!("SGR {p}"), seq));
    }
    for p in &[38u8, 48] {
        for color in &[0, 16, 32, 64, 128, 192, 231, 255] {
            let seq = format!("\x1b[{};5;{}mX", p, color).into_bytes();
            v.push((format!("SGR {};5;{}", p, color), seq));
        }
    }
    for rgb in &[
        "38;2;255;0;0",
        "38;2;0;255;0",
        "48;2;255;0;0",
        "58;2;128;128;128",
    ] {
        let seq = format!("\x1b[{}mX", rgb).into_bytes();
        v.push((format!("SGR {}", rgb), seq));
    }
    v
}

fn dec_private_mode_sequences() -> Vec<(String, Vec<u8>)> {
    let mode_nums: Vec<u16> = vec![
        1, 2, 3, 4, 5, 6, 7, 8, 9, 12, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 28, 29, 30,
        31, 33, 34, 35, 36, 37, 38, 40, 41, 42, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56,
        57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79,
        80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101,
        102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119,
        120, 1000, 1001, 1002, 1003, 1004, 1005, 1006, 1007, 1010, 1011, 1015, 1016, 1034, 1035,
        1036, 1037, 1039, 1040, 1041, 1042, 1043, 1044, 1045, 1046, 1047, 1048, 1049, 2004, 2005,
        2026,
    ];
    let mut v = Vec::new();
    for &n in &mode_nums {
        v.push((
            format!("DECSET ?{n}h"),
            format!("\x1b[?{}h", n).into_bytes(),
        ));
        v.push((
            format!("DECRST ?{n}l"),
            format!("\x1b[?{}l", n).into_bytes(),
        ));
    }
    v
}

fn osc_sequences() -> Vec<(String, Vec<u8>)> {
    let mut v = Vec::new();
    for n in 0u16..=120 {
        let seq = format!("\x1b]{};Test\x1b\\", n).into_bytes();
        v.push((format!("OSC {n}"), seq));
    }
    v
}

fn xtwinops_sequences() -> Vec<(String, Vec<u8>)> {
    let mut v = Vec::new();
    for n in 1u8..=24 {
        v.push((
            format!("XTWINOPS {n}t"),
            format!("\x1b[{}t", n).into_bytes(),
        ));
    }
    for sub in &[
        "3;0", "4;12;40", "8;24;80", "9;0", "9;1", "9;2", "9;3", "10;1", "10;2", "20;Label",
        "21;Title",
    ] {
        let seq = format!("\x1b[{}t", sub).into_bytes();
        v.push((format!("XTWINOPS {sub}"), seq));
    }
    v
}

fn dsr_sequences() -> Vec<(String, Vec<u8>)> {
    let mut v = Vec::new();
    for n in 0u8..=26 {
        let seq = format!("\x1b[{}n", n).into_bytes();
        v.push((format!("DSR {n}"), seq));
    }
    v.push(("DA1".to_string(), b"\x1b[c".to_vec()));
    v.push(("DA2".to_string(), b"\x1b[>c".to_vec()));
    v.push(("DA3".to_string(), b"\x1b[=c".to_vec()));
    v
}

fn keyboard_sequences() -> Vec<(String, Vec<u8>)> {
    let mut v = Vec::new();
    for key in &[9u16, 13, 27, 127, 32, 65, 97, 1000, 1001] {
        v.push((
            format!("CSI {}u", key),
            format!("\x1b[{}u", key).into_bytes(),
        ));
        for modif in &[2u16, 3, 4, 5, 8] {
            v.push((
                format!("CSI {};{}u", key, modif),
                format!("\x1b[{};{}u", key, modif).into_bytes(),
            ));
        }
    }
    for fn_key in 1u16..=35 {
        v.push((
            format!("DECFNK {}~", fn_key),
            format!("\x1b[{}~", fn_key).into_bytes(),
        ));
    }
    v.push(("Kitty push >1u".to_string(), b"\x1b[>1u".to_vec()));
    v.push(("Kitty push >2u".to_string(), b"\x1b[>2u".to_vec()));
    v.push(("Kitty query ?u".to_string(), b"\x1b[?u".to_vec()));
    v
}

fn mouse_sequences() -> Vec<(String, Vec<u8>)> {
    let mut v = Vec::new();
    for btn in 0u8..=2 {
        for col in 0u8..=2 {
            for row in 0u8..=2 {
                v.push((
                    format!("SGR mouse {btn};{col};{row}"),
                    format!("\x1b[<{};{};{}M", btn, col, row).into_bytes(),
                ));
            }
        }
    }
    v
}

// =========================================================================
// Coverage tests
// =========================================================================

#[test]
fn coverage_c0() {
    for (l, s) in c0_sequences() {
        assert_seq_runs_safely(&mut term(), &s, &l);
    }
}

#[test]
fn coverage_csi() {
    for (l, s) in csi_final_sequences() {
        assert_seq_runs_safely(&mut term(), &s, &l);
    }
}

#[test]
fn coverage_sgr() {
    for (l, s) in sgr_sequences() {
        assert_seq_runs_safely(&mut term(), &s, &l);
    }
}

#[test]
fn coverage_dec_modes() {
    for (l, s) in dec_private_mode_sequences() {
        assert_seq_runs_safely(&mut term(), &s, &l);
    }
}

#[test]
fn coverage_osc() {
    for (l, s) in osc_sequences() {
        assert_seq_runs_safely(&mut term(), &s, &l);
    }
}

#[test]
fn coverage_xtwinops() {
    for (l, s) in xtwinops_sequences() {
        assert_seq_runs_safely(&mut term(), &s, &l);
    }
}

#[test]
fn coverage_dsr() {
    for (l, s) in dsr_sequences() {
        assert_seq_runs_safely(&mut term(), &s, &l);
    }
}

#[test]
fn coverage_keyboard() {
    for (l, s) in keyboard_sequences() {
        assert_seq_runs_safely(&mut term(), &s, &l);
    }
}

#[test]
fn coverage_mouse() {
    for (l, s) in mouse_sequences() {
        assert_seq_runs_safely(&mut term(), &s, &l);
    }
}

#[test]
fn coverage_total_count() {
    let total = c0_sequences().len()
        + csi_final_sequences().len()
        + sgr_sequences().len()
        + dec_private_mode_sequences().len()
        + osc_sequences().len()
        + xtwinops_sequences().len()
        + dsr_sequences().len()
        + keyboard_sequences().len()
        + mouse_sequences().len();
    eprintln!("COVERAGE: {} unique VT sequences exercised", total);
    assert!(total >= 500, "Expected >=500 sequences, got {total}");
}
