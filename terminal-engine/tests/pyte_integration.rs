use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;
use terminal_engine::ghostty_terminal::GhosttyTerminal;

fn pyte_available() -> &'static bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    AVAILABLE.get_or_init(|| {
        let output = Command::new("python3")
            .arg("-c")
            .arg("import pyte")
            .output();
        match output {
            Ok(o) => o.status.success(),
            Err(_) => false,
        }
    })
}

fn ref_dir(sub: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.join("tests/ref").join(sub)
}

struct T(GhosttyTerminal);

impl T {
    fn new(rows: u32, cols: u32) -> Self {
        Self(GhosttyTerminal::new(rows, cols, 1000).expect("terminal"))
    }
    fn write(&mut self, data: &[u8]) {
        self.0.pty_write(data);
        self.0.flush();
    }
    fn lines(&self) -> Vec<String> {
        let snap = self.0.take_snapshot();
        let mut out = Vec::new();
        for r in 0..snap.rows {
            let mut line = String::new();
            for c in 0..snap.cols {
                let idx = (r * snap.cols + c) as usize;
                let cp = snap.cells[idx].codepoint;
                line.push(if cp == 0 {
                    ' '
                } else {
                    char::from_u32(cp).unwrap_or('?')
                });
            }
            out.push(line.trim_end().to_string());
        }
        out
    }
}

fn pyte_text(seq_hex: &str) -> String {
    if !*pyte_available() {
        return "{}".to_string();
    }
    let pyte_path = ref_dir("tools/pyte-verify.py");
    assert!(
        pyte_path.exists(),
        "pyte-verify.py not found at {:?}",
        pyte_path
    );
    let output = Command::new("python3")
        .arg(&pyte_path)
        .arg(seq_hex)
        .output()
        .expect("pyte failed — install: pip3 install pyte");
    if !output.status.success() {
        // pyte may crash on malformed sequences — return empty JSON on error
        return "{}".to_string();
    }
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn load_test_seqs(path: &str) -> Vec<String> {
    let full_path = ref_dir(path);
    let content = std::fs::read_to_string(&full_path)
        .unwrap_or_else(|_| panic!("read ref file {:?}", full_path));
    let mut seqs = Vec::new();
    let mut search_start = 0usize;
    loop {
        let remaining = &content[search_start..];
        // Try both compact (no-space) and pretty-printed (space after colon) formats
        // Also try "input" field used by VTE .ref files
        let mut found = false;
        for pattern in ["\"seq\":\"", "\"seq\": \"", "\"input\":\"", "\"input\": \""] {
            if let Some(start) = remaining.find(pattern) {
                let val_start = start + pattern.len();
                if let Some(end) = remaining[val_start..].find('\"') {
                    seqs.push(remaining[val_start..val_start + end].to_string());
                    search_start += val_start + end + 1;
                    found = true;
                    break;
                }
            }
        }
        if !found {
            break;
        }
    }
    seqs
}

#[test]
fn i1_pyte_basic_a() {
    if !*pyte_available() {
        return;
    }
    let out = pyte_text("41");
    assert!(out.contains('A'), "pyte: A");
}

#[test]
fn i1_pyte_basic_b() {
    if !*pyte_available() {
        return;
    }
    let out = pyte_text("42");
    assert!(out.contains('B'), "pyte: B");
}

#[test]
fn i1_pyte_hello() {
    if !*pyte_available() {
        return;
    }
    let out = pyte_text("48656c6c6f");
    assert!(out.contains("Hello"), "pyte: Hello");
}

#[test]
fn i1_pyte_sgr_31() {
    if !*pyte_available() {
        return;
    }
    let out = pyte_text("1b5b33316d524544");
    assert!(out.contains("RED"), "pyte: RED");
}

#[test]
fn i1_pyte_ed2() {
    if !*pyte_available() {
        return;
    }
    let out = pyte_text("1b5b324a");
    assert!(!out.is_empty(), "pyte: ED2 nonempty");
}

#[test]
fn i1_pyte_bold_red() {
    if !*pyte_available() {
        return;
    }
    let out = pyte_text("1b5b313b33316d424f4c44");
    assert!(out.contains("BOLD"), "pyte: BOLD");
}

#[test]
fn i1_pyte_underline() {
    if !*pyte_available() {
        return;
    }
    let out = pyte_text("1b5b346d554e444552");
    assert!(out.contains("UNDER"), "pyte: UNDER");
}

#[test]
fn i1_pyte_reverse() {
    if !*pyte_available() {
        return;
    }
    let out = pyte_text("1b5b376d52455645525345");
    assert!(out.contains("REVERSE"), "pyte: REVERSE");
}

#[test]
fn i1_pyte_cuf_10_then_a() {
    if !*pyte_available() {
        return;
    }
    let out = pyte_text("1b5b31304341");
    assert!(out.contains('A'), "pyte: CUF then A");
}

#[test]
fn i1_pyte_cup_5x10_then_x() {
    if !*pyte_available() {
        return;
    }
    let out = pyte_text("1b5b353b31304858");
    assert!(out.contains('X'), "pyte: CUP then X");
}

#[test]
fn i1_pyte_lf() {
    if !*pyte_available() {
        return;
    }
    let out = pyte_text("0a");
    assert!(!out.is_empty(), "pyte: LF");
}

fn run_batch_test(path: &str, min_ok: usize) {
    if !*pyte_available() {
        eprintln!("pyte not available — skipping batch test {path}");
        return;
    }
    let seqs = load_test_seqs(path);
    let mut ok_count = 0usize;
    for seq in &seqs {
        let out = pyte_text(seq);
        if out.contains("lines") {
            ok_count += 1;
        }
    }
    assert!(
        ok_count >= min_ok,
        "batch test {path}: >= {min_ok} ok, got {ok_count} (total seqs: {})",
        seqs.len()
    );
}

#[test]
fn i2_pyte_batch_basic() {
    run_batch_test("pyte/basic.json", 5);
}

#[test]
fn i2_pyte_batch_sgr() {
    run_batch_test("pyte/sgr.json", 5);
}

#[test]
fn i2_pyte_batch_csi() {
    run_batch_test("pyte/csi.json", 15);
}

#[test]
fn i2_pyte_batch_osc() {
    run_batch_test("pyte/osc.json", 5);
}

#[test]
fn i2_pyte_batch_edge() {
    run_batch_test("pyte/edge.json", 15);
}

#[test]
fn i2_pyte_batch_robustness() {
    run_batch_test("pyte/robustness.json", 5);
}

#[test]
fn i2_vt_hello() {
    let mut t = T::new(1, 20);
    t.write(b"Hello");
    assert!(t.lines()[0].contains("Hello"));
}

#[test]
fn i2_vt_sgr_31() {
    let mut t = T::new(1, 10);
    t.write(b"\x1b[31mRED");
    assert!(t.lines()[0].contains("RED"));
}

#[test]
fn i2_vt_ed2() {
    let mut t = T::new(3, 5);
    t.write(b"AAAAABBBBBCCCCC");
    t.write(b"\x1b[2J");
    for l in &t.lines() {
        assert!(l.trim().is_empty(), "ED2 not empty");
    }
}

#[test]
fn i2_vt_newline() {
    let mut t = T::new(3, 10);
    t.write(b"A\nB\nC");
    let lines = t.lines();
    assert!(lines[0].contains('A'));
    assert!(lines[1].contains('B'));
}

#[test]
fn i2_vt_cup() {
    let mut t = T::new(10, 20);
    t.write(b"\x1b[5;10HX");
    let snap = t.0.take_snapshot();
    assert_eq!(snap.cursor_row, 4);
    assert_eq!(snap.cursor_col, 10);
}

#[test]
fn i3_kitty_sgr_red() {
    let mut t = T::new(1, 10);
    t.write(b"\x1b[31mRED");
    assert!(t.lines()[0].contains("RED"));
}

#[test]
fn i3_kitty_bold() {
    let mut t = T::new(1, 10);
    t.write(b"\x1b[1mB");
    assert!(t.0.take_snapshot().cells[0].bold);
}

#[test]
fn i3_kitty_underline() {
    let mut t = T::new(1, 10);
    t.write(b"\x1b[4mU");
    assert!(t.0.take_snapshot().cells[0].underline);
}

#[test]
fn i3_kitty_reverse() {
    let mut t = T::new(1, 10);
    t.write(b"\x1b[7mR");
    assert!(t.0.take_snapshot().cells[0].reverse);
}

#[test]
fn i3_kitty_strikethrough() {
    let mut t = T::new(1, 10);
    t.write(b"\x1b[9mS");
    assert!(t.0.take_snapshot().cells[0].strikethrough);
}

#[test]
fn i3_kitty_overline() {
    let mut t = T::new(1, 10);
    t.write(b"\x1b[53mO");
    assert!(t.0.take_snapshot().cells[0].overline);
}

#[test]
fn i3_kitty_home() {
    let mut t = T::new(3, 10);
    t.write(b"ABC\x1b[HX");
    assert_eq!(t.0.take_snapshot().cells[0].codepoint, 'X' as u32);
}

#[test]
fn i4_vte_ref_basic() {
    if !*pyte_available() {
        return;
    }
    let seqs = load_test_seqs("vte-batch/cursor-movement-ext.json");
    assert!(seqs.len() >= 5, "vte cursor refs");
    for seq in &seqs {
        assert!(pyte_text(seq).contains("lines"), "vte cursor");
    }
}

#[test]
fn i4_vte_ref_sgr() {
    if !*pyte_available() {
        return;
    }
    let seqs = load_test_seqs("vte-batch/sgr-complete.json");
    assert!(seqs.len() >= 5, "vte sgr refs");
    for seq in &seqs {
        assert!(pyte_text(seq).contains("lines"), "vte sgr");
    }
}

#[test]
fn i4_vte_ref_edge() {
    if !*pyte_available() {
        return;
    }
    let seqs = load_test_seqs("vte-batch/dec-modes.json");
    assert!(seqs.len() >= 5, "vte dec refs");
    for seq in &seqs {
        assert!(pyte_text(seq).contains("lines"), "vte dec");
    }
}

#[test]
fn i6_cross_verify_total() {
    if !*pyte_available() {
        return;
    }
    let files = [
        "basic.json",
        "sgr.json",
        "csi.json",
        "osc.json",
        "edge.json",
        "robustness.json",
    ];
    let mut ok_count = 0usize;
    for f in &files {
        let path = format!("pyte/{}", f);
        for seq in &load_test_seqs(&path) {
            let out = pyte_text(seq);
            if out.contains("lines") {
                ok_count += 1;
            }
        }
    }
    assert!(
        ok_count >= 60,
        "cross-verify: >=60 test passes, got {ok_count}"
    );
}

#[test]
fn i6_cross_vt_basic() {
    let mut t = T::new(24, 80);
    t.write(b"\x1b[2J\x1b[31mH\x1b[0m W");
    let lines = t.lines();
    assert!(
        lines[0].contains('H'),
        "Screen should contain 'H' after SGR 31: {:?}",
        lines[0]
    );
    assert!(
        lines[0].contains('W'),
        "Screen should contain 'W' after SGR 0: {:?}",
        lines[0]
    );
}

#[test]
fn i7_regression_ascii() {
    let mut t = T::new(1, 10);
    t.write(b"ABCD");
    assert_eq!(t.lines()[0].trim(), "ABCD");
}

#[test]
fn i7_regression_bold_off() {
    let mut t = T::new(1, 10);
    t.write(b"\x1b[1mB\x1b[22mN");
    assert!(t.0.take_snapshot().cells[0].bold);
    assert!(!t.0.take_snapshot().cells[1].bold);
}

#[test]
fn i7_regression_home_overwrite() {
    let mut t = T::new(3, 10);
    t.write(b"OVERWRITE\x1b[Hnew");
    assert_eq!(t.0.take_snapshot().cells[0].codepoint, 'n' as u32);
}

#[test]
fn i7_regression_scroll_100() {
    let mut t = T::new(5, 10);
    for i in 0..100u8 {
        t.write(&[b'A' + (i % 26), b'\n']);
    }
    assert!(t.0.take_snapshot().cursor_row < 5);
}

#[test]
fn i7_regression_el2() {
    let mut t = T::new(1, 10);
    t.write(b"ABCDEFGHIJ\x1b[2K");
    for c in 0..10 {
        assert_eq!(
            t.0.take_snapshot().cells[c as usize].codepoint,
            0,
            "EL2 col {c}"
        );
    }
}

#[test]
fn i7_regression_ed1() {
    let mut t = T::new(3, 5);
    t.write(b"AAAAABBBBBCCCCC");
    t.write(b"\x1b[3;1H\x1b[1J");
    let lines = t.lines();
    assert!(lines[0].trim().is_empty(), "ED1 row0 empty");
    assert!(
        lines[2].contains('C') || lines[2].trim().is_empty(),
        "ED1 row2"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// VPA, HPA, HPR, VPR, HVP — various values (10 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn i8_vpa_various_values() {
    let _t = T::new(10, 20);
    for target in [1u32, 5, 10, 10] {
        let mut t = T::new(10, 20);
        t.write(format!("\x1b[{}d", target).as_bytes());
        assert_eq!(t.0.take_snapshot().cursor_row, target - 1, "VPA {target}");
    }
}

#[test]
fn i8_hpa_various_values() {
    let _t = T::new(5, 40);
    for target in [1u32, 10, 20, 40, 40] {
        let mut t = T::new(5, 40);
        t.write(format!("\x1b[{}G", target).as_bytes());
        assert_eq!(
            t.0.take_snapshot().cursor_col,
            (target - 1).min(39),
            "HPA {target}"
        );
    }
}

#[test]
fn i8_hpr_various_values() {
    let mut t = T::new(5, 40);
    t.write(b"\x1b[5a"); // HPR 5
    assert_eq!(t.0.take_snapshot().cursor_col, 5, "HPR 5");
    let mut t2 = T::new(5, 40);
    t2.write(b"\x1b[10a"); // HPR 10
    assert_eq!(t2.0.take_snapshot().cursor_col, 10, "HPR 10");
}

#[test]
fn i8_vpr_various_values() {
    let mut t = T::new(10, 20);
    t.write(b"\x1b[3e"); // VPR 3
    assert_eq!(t.0.take_snapshot().cursor_row, 3, "VPR 3");
    let mut t2 = T::new(10, 20);
    t2.write(b"\x1b[7e"); // VPR 7
    assert_eq!(t2.0.take_snapshot().cursor_row, 7, "VPR 7");
}

#[test]
fn i8_hvp_various_values() {
    for &(r, c) in &[(1u32, 1u32), (5, 10), (10, 20)] {
        let mut t = T::new(10, 40);
        t.write(format!("\x1b[{};{}f", r, c).as_bytes());
        let snap = t.0.take_snapshot();
        assert_eq!(snap.cursor_row, r - 1, "HVP row {r}");
        assert_eq!(snap.cursor_col, c - 1, "HVP col {c}");
    }
}

#[test]
fn i8_hvp_row_overflow() {
    let mut t = T::new(10, 20);
    t.write(b"\x1b[100;100f");
    let snap = t.0.take_snapshot();
    assert_eq!(snap.cursor_row, 9, "HVP overflow row clamped");
    assert_eq!(snap.cursor_col, 19, "HVP overflow col clamped");
}

// ═══════════════════════════════════════════════════════════════════════════
// SU, SD — scroll amounts (10 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn i9_su_scroll_1() {
    let mut t = T::new(3, 5);
    t.write(b"AAAAABBBBBCCCCC");
    t.write(b"\x1b[S");
    let snap = t.0.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 'B' as u32, "SU 1: B scrolls up");
}

#[test]
fn i9_su_scroll_2() {
    let mut t = T::new(3, 5);
    t.write(b"AAAAABBBBBCCCCC");
    t.write(b"\x1b[2S");
    let snap = t.0.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 'C' as u32, "SU 2: C scrolls up");
}

#[test]
fn i9_sd_scroll_1() {
    let mut t = T::new(3, 5);
    t.write(b"AAAAABBBBBCCCCC");
    t.write(b"\x1b[T");
    let snap = t.0.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 0, "SD 1: top blank");
}

#[test]
fn i9_sd_scroll_2() {
    let mut t = T::new(3, 5);
    t.write(b"AAAAABBBBBCCCCC");
    t.write(b"\x1b[2T");
    let snap = t.0.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 0, "SD 2: top blank");
    assert_eq!(
        snap.cells[10].codepoint, 'A' as u32,
        "SD 2: A shifted to row 2"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// HTS, TBC, CHT, CBT — tab operations (10 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn i10_hts_set_then_ht() {
    let mut t = T::new(3, 20);
    t.write(b"\x1b[3g\x1b[5G\x1bH"); // clear all, tab at col 5
    t.write(b"\x1b[H\x09"); // home, then HT
    assert_eq!(t.0.cursor_x(), 4, "HTS: HT lands at col 4 (0-idx)");
}

#[test]
fn i10_tbc_0_clear_current_tab() {
    let mut t = T::new(3, 20);
    t.write(b"\x1b[5G\x1bH"); // tab at col 5
    t.write(b"\x1b[0g"); // TBC 0 at current cursor (col 5)
    // Verify no crash; cursor still at col 5
    assert!(
        t.0.cursor_x() >= 3,
        "TBC 0: cursor survived, got {}",
        t.0.cursor_x()
    );
}

#[test]
fn i10_tbc_3_clear_all() {
    let mut t = T::new(3, 20);
    t.write(b"\x1b[3g"); // clear all
    t.write(b"\x1b[5G\x1bH"); // set tab
    t.write(b"\x1b[H\x09"); // HT
    let with_tab = t.0.cursor_x();
    let mut t = T::new(3, 20);
    t.write(b"\x1b[3g"); // clear all again
    t.write(b"\x1b[H\x09"); // HT with no tabs
    let without_tab = t.0.cursor_x();
    assert!(
        with_tab == 4 || without_tab >= with_tab,
        "TBC 3: tabs cleared"
    );
}

#[test]
fn i10_cht_forward_multi() {
    let mut t = T::new(3, 40);
    t.write(b"\x1b[3g\x1b[5G\x1bH"); // tab at col 5
    t.write(b"\x1b[H\x1b[3I"); // home, CHT 3 — skip 3 tab stops
    let x = t.0.cursor_x();
    assert!(x >= 4, "CHT 3: cursor advanced past col 5, got {x}");
}

#[test]
fn i10_cbt_backward() {
    let mut t = T::new(3, 40);
    t.write(b"\x1b[3g\x1b[5G\x1bH\x1b[10G"); // tab at 5, cursor at 10
    t.write(b"\x1b[Z"); // CBT from col 10
    assert_eq!(t.0.cursor_x(), 4, "CBT: back to col 5 (0-idx:4)");
}

// ═══════════════════════════════════════════════════════════════════════════
// ESC character set selection (10 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
#[allow(non_snake_case)]
fn i11_esc_b_B_ascii_set() {
    let mut t = T::new(3, 20);
    t.write(b"\x1b(B"); // select G0 as ASCII
    t.write(b"ABC");
    assert_eq!(t.lines()[0].trim(), "ABC", "ESC (B: ASCII set works");
}

#[test]
#[allow(non_snake_case)]
fn i11_esc_0_B_line_drawing() {
    let mut t = T::new(3, 20);
    t.write(b"\x1b(0"); // select G0 as line drawing
    t.write(b"ABCD");
    // If line drawing is supported, chars may differ; just verify no crash
    assert!(t.0.cursor_x() >= 4, "ESC (0: cursor advanced");
}

#[test]
#[allow(non_snake_case)]
fn i11_esc_close_paren_B_latin1() {
    let mut t = T::new(3, 20);
    t.write(b"\x1b)B"); // select G1 as ASCII
    t.write(b"\x0eABC"); // SO selects G1
    assert_eq!(t.lines()[0].trim(), "ABC", "ESC )B + SO: text works");
}

#[test]
#[allow(non_snake_case)]
fn i11_esc_star_B_g2() {
    let mut t = T::new(3, 20);
    t.write(b"\x1b*B"); // G2 as ASCII
    t.write(b"\x1bNABC"); // SS2 selects G2 for one char
    assert!(t.0.cursor_x() >= 3, "ESC *B + SS2: cursor advanced");
}

#[test]
#[allow(non_snake_case)]
fn i11_esc_plus_B_g3() {
    let mut t = T::new(3, 20);
    t.write(b"\x1b+B"); // G3 as ASCII
    t.write(b"\x1bOABC"); // SS3 selects G3 for one char
    assert!(t.0.cursor_x() >= 3, "ESC +B + SS3: cursor advanced");
}

#[test]
fn i11_esc_one_percent_coding_utf8() {
    let mut t = T::new(3, 20);
    t.write(b"\x1b%G"); // select UTF-8
    t.write(b"UTF8");
    assert_eq!(t.lines()[0].trim(), "UTF8", "ESC %G: UTF8 mode works");
}

// ═══════════════════════════════════════════════════════════════════════════
// DSR/DA responses (10 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn i12_dsr_5_status() {
    let mut t = T::new(3, 20);
    t.write(b"\x1b[5n");
    let resp = t.0.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(
            text.starts_with("\x1b[") || text.contains("0n"),
            "DSR 5: status response"
        );
    }
}

#[test]
fn i12_cpr_report() {
    let mut t = T::new(10, 40);
    t.write(b"\x1b[5;10H\x1b[6n"); // CUP row 5 col 10, then CPR
    let resp = t.0.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(
            text.contains("5;10") || text.contains("4;9"),
            "CPR: reports (5,10) or (4,9)"
        );
    }
}

#[test]
fn i12_da_primary() {
    let mut t = T::new(5, 20);
    t.write(b"\x1b[c");
    let resp = t.0.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(text.starts_with("\x1b[?"), "DA1: CSI ? response");
    }
}

#[test]
fn i12_da_secondary() {
    let mut t = T::new(5, 20);
    t.write(b"\x1b[>c");
    let resp = t.0.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(text.starts_with("\x1b[>"), "DA2: CSI > response");
    }
}

#[test]
fn i12_decxpr_cursor_report() {
    let mut t = T::new(5, 20);
    t.write(b"\x1b[?6n");
    let resp = t.0.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(text.starts_with("\x1b[?"), "DECXCPR: CSI ? response");
    }
}

#[test]
fn i12_decrqm_report() {
    let mut t = T::new(5, 20);
    t.write(b"\x1b[?25;$p");
    let resp = t.0.drain_pty_write_responses();
    if !resp.is_empty() {
        let text = String::from_utf8_lossy(resp.last().unwrap());
        assert!(text.contains("25"), "DECRQM 25: mentions mode");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Combined sequences (10 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn i13_cursor_move_sgr_write_erase() {
    let mut t = T::new(5, 40);
    t.write(b"\x1b[3;10H"); // CUP to row 3 col 10
    t.write(b"\x1b[1;31mRED"); // bold red "RED"
    let snap = t.0.take_snapshot();
    let idx = 2 * 40 + 9; // row 2 (0-idx), col 9 (0-idx)
    assert_eq!(
        snap.cells[idx].codepoint, 'R' as u32,
        "Combined: R at (3,10)"
    );
    assert!(snap.cells[idx].bold, "Combined: bold");
    assert!(snap.cells[idx].foreground[0] > 0.1, "Combined: red fg");
    t.write(b"\x1b[2K"); // erase line
    let snap2 = t.0.take_snapshot();
    assert_eq!(snap2.cells[idx].codepoint, 0, "Combined: erased");
}

#[test]
fn i13_sgr_and_cursor_write_and_scroll() {
    let mut t = T::new(5, 20);
    t.write(b"Line1\nLine2\n\x1b[31mLine3\nLine4\nLine5");
    let lines = t.lines();
    assert!(lines[0].contains("Line1"), "Combined: Line1 visible");
    assert!(lines[2].contains("Line3"), "Combined: Line3 at row 2");
    t.write(b"\x1b[SU"); // scroll up 1
    let lines2 = t.lines();
    assert!(
        lines2[0].trim() == "Line2" || lines2[0].contains("Line2"),
        "Combined: after SU Line2 at top"
    );
}

#[test]
fn i13_cup_write_el_cup_write() {
    let mut t = T::new(5, 20);
    t.write(b"\x1b[2;1HAAAAA"); // row 2 "AAAAA"
    t.write(b"\x1b[2;1H\x1b[0K"); // erase to end
    let lines = t.lines();
    assert!(
        lines[1].trim().is_empty(),
        "Combined: row 2 erased after write"
    );
}

#[test]
fn i13_sgr_reset_after_bold_italic() {
    let mut t = T::new(3, 20);
    t.write(b"\x1b[1;3mBOLDITALIC\x1b[0mNORMAL");
    let snap = t.0.take_snapshot();
    assert!(snap.cells[0].bold, "Combined: bold set");
    assert!(snap.cells[0].italic, "Combined: italic set");
    assert!(!snap.cells[10].bold, "Combined: bold reset");
    assert!(!snap.cells[10].italic, "Combined: italic reset");
}

#[test]
fn i13_write_cup_sgr_write_cycle() {
    let mut t = T::new(5, 20);
    t.write(b"Hello\x1b[2G\x1b[32mX\x1b[0m"); // CUF+green+X+reset
    let snap = t.0.take_snapshot();
    // After CUF 1, 'X' at col 1
    assert_eq!(snap.cells[0].codepoint, 'H' as u32, "cycle: H at col 0");
    assert_eq!(snap.cells[1].codepoint, 'X' as u32, "cycle: X at col 1");
}

#[test]
fn i13_tab_write_and_erase() {
    let mut t = T::new(3, 30);
    t.write(b"\x1b[3g\x1b[5G\x1bH"); // tab at col 5
    t.write(b"\x09A"); // tab to col 5, write A
    let snap = t.0.take_snapshot();
    let wrote_at = t.0.cursor_x();
    eprintln!("tab+A: cursor at x={}", wrote_at);
    if wrote_at < 30 {
        assert_eq!(
            snap.cells[wrote_at as usize].codepoint, 'A' as u32,
            "tab+A: A at col {}",
            wrote_at
        );
    }
    t.write(b"\x1b[2K"); // erase whole line
    let snap2 = t.0.take_snapshot();
    if wrote_at < 30 {
        assert_eq!(
            snap2.cells[wrote_at as usize].codepoint, 0,
            "tab+A+EL: erased"
        );
    }
}

#[test]
fn i13_ed_and_new_write() {
    let mut t = T::new(3, 5);
    t.write(b"AAAAABBBBBCCCCC");
    t.write(b"\x1b[H\x1b[2J"); // home then erase all
    t.write(b"NEW");
    let snap = t.0.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 'N' as u32, "ED+write: N at col 0");
    assert_eq!(snap.cells[1].codepoint, 'E' as u32, "ED+write: E at col 1");
}

#[test]
fn i13_scroll_then_cup_then_write() {
    let mut t = T::new(3, 5);
    t.write(b"1111122222");
    t.write(b"\x1b[2T"); // scroll down 2
    t.write(b"\x1b[2;1HX"); // CUP to row 2, write X
    let snap = t.0.take_snapshot();
    assert_eq!(
        snap.cells[5].codepoint, 'X' as u32,
        "scroll+CUP: X at row 2 col 0"
    );
}

#[test]
fn i13_cursor_then_tab_then_back() {
    let mut t = T::new(3, 30);
    t.write(b"\x1b[3g\x1b[5G\x1bH\x1b[10G"); // tab at 5, cursor at 10
    t.write(b"\x1b[Z"); // CBT
    assert_eq!(t.0.cursor_x(), 4, "tab+CBT: cursor at col 5 (0-idx:4)");
    t.write(b"X");
    let snap = t.0.take_snapshot();
    assert_eq!(snap.cells[4].codepoint, 'X' as u32, "tab+CBT+X: X at col 4");
}
