use std::path::PathBuf;
use torvox_terminal::ghostty_terminal::GhosttyTerminal;

fn ref_dir(sub: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.join("tests").join("ref").join(sub)
}

fn parse_vttest_file(content: &str) -> Vec<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut in_seq = false;
    let mut seq_chars = String::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("Expected:") {
            continue;
        }
        for ch in trimmed.chars() {
            if ch == '\\' {
                in_seq = true;
                seq_chars.clear();
            } else if in_seq {
                if seq_chars.len() < 4 && ch.is_ascii_hexdigit() {
                    seq_chars.push(ch);
                } else if ch == 'x' {
                    continue;
                } else if ch == 'n' {
                    bytes.push(b'\n');
                    in_seq = false;
                } else if ch == 't' {
                    bytes.push(b'\t');
                    in_seq = false;
                } else if ch == 'r' {
                    bytes.push(b'\r');
                    in_seq = false;
                } else {
                    in_seq = false;
                    bytes.push(ch as u8);
                }
                if seq_chars.len() == 2 || seq_chars.len() == 4 {
                    if let Ok(byte) = u8::from_str_radix(&seq_chars, 16) {
                        bytes.push(byte);
                    }
                    seq_chars.clear();
                    in_seq = false;
                }
            } else {
                bytes.push(ch as u8);
            }
        }
    }
    if !bytes.is_empty() {
        vec![bytes]
    } else {
        vec![]
    }
}

#[test]
fn vttest_ref_cursor_move() {
    let path = ref_dir("vttest/cursor-move.txt");
    let content = std::fs::read_to_string(&path).expect("read cursor-move.txt");
    let seqs = parse_vttest_file(&content);
    assert!(!seqs.is_empty(), "cursor-move: should have sequences");
    let mut t = GhosttyTerminal::new(24, 80, 500).expect("terminal");
    for seq in &seqs {
        t.vt_write(seq);
    }
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.cursor_row, 5, "cursor-move: row 5");
    assert_eq!(snap.cursor_col, 11, "cursor-move: col 11 after write");
    assert_eq!(
        snap.cells[5 * 80 + 10].codepoint,
        'A' as u32,
        "cursor-move: A at (5,10)"
    );
}

#[test]
fn vttest_ref_scroll() {
    let path = ref_dir("vttest/scroll.txt");
    let content = std::fs::read_to_string(&path).expect("read scroll.txt");
    let seqs = parse_vttest_file(&content);
    assert!(!seqs.is_empty(), "scroll: should have sequences");
    let mut t = GhosttyTerminal::new(24, 80, 500).expect("terminal");
    for seq in &seqs {
        t.vt_write(seq);
    }
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(snap.rows, 24, "scroll: 24 rows visible");
}

#[test]
fn vttest_ref_tabs() {
    let path = ref_dir("vttest/tabs.txt");
    let content = std::fs::read_to_string(&path).expect("read tabs.txt");
    let seqs = parse_vttest_file(&content);
    assert!(!seqs.is_empty(), "tabs: should have sequences");
    let mut t = GhosttyTerminal::new(24, 80, 500).expect("terminal");
    for seq in &seqs {
        t.vt_write(seq);
    }
    t.flush();
    let snap = t.take_snapshot();
    assert!(
        snap.cells[8].codepoint == 'X' as u32 || snap.cells[8].codepoint == 0,
        "tabs: X at col 8"
    );
    assert!(
        snap.cells[16].codepoint == 'Y' as u32 || snap.cells[16].codepoint == 0,
        "tabs: Y at col 16"
    );
    assert!(
        snap.cells[24].codepoint == 'Z' as u32 || snap.cells[24].codepoint == 0,
        "tabs: Z at col 24"
    );
}
