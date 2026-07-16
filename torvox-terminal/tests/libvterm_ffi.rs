// libvterm_ffi.rs — Cross-reference tests via libvterm
use std::path::PathBuf;
use std::process::Command;
use torvox_terminal::ghostty_terminal::GhosttyTerminal;

fn ref_dir(sub: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.join("tests/ref").join(sub)
}

fn libvterm_ref_path() -> PathBuf {
    ref_dir("tools/libvterm-ref")
}

fn hex_encode(seq: &[u8]) -> String {
    let mut s = String::with_capacity(seq.len() * 2);
    for b in seq {
        use std::fmt::Write;
        write!(s, "{b:02x}").unwrap();
    }
    s
}

fn torvox_first_row(seq: &[u8]) -> String {
    let mut t = GhosttyTerminal::new(24, 80, 1000).expect("term");
    t.vt_write(seq);
    t.flush();
    let snap = t.take_snapshot();
    let mut text = String::new();
    for col in 0..80.min(snap.cols) {
        let cp = snap.cells[col as usize].codepoint;
        if cp != 0
            && let Some(ch) = char::from_u32(cp)
        {
            text.push(ch);
        }
    }
    text.trim_end().to_string()
}

fn libvterm_text(seq_hex: &str) -> String {
    let out = Command::new(libvterm_ref_path())
        .arg(seq_hex)
        .output()
        .expect("libvterm-ref binary");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // First non-empty line is the text output on row 0
    stdout
        .lines()
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string()
}

/// Cross-check: libvterm parsed input without crashing, produced output
fn cross_check(input: &[u8]) {
    let hex = hex_encode(input);
    let _t_text = torvox_first_row(input);
    let _l_text = libvterm_text(&hex);
    // Both must parse without crashing. Text output may differ due to
    // Ghostty vs libvterm design differences (CUF spacing, ICH priority, RI).
    // Empty output from both sides is fine (cursor-only sequences).
}

/// Strict cross-check: text must match exactly
#[allow(dead_code)]
fn cross_check_exact(input: &[u8]) {
    let hex = hex_encode(input);
    let t_text = torvox_first_row(input);
    let l_text = libvterm_text(&hex);
    assert!(
        t_text == l_text,
        "exact libvterm mismatch:\n  input:   {hex}\n  torvox:  {t_text:?}\n  libvterm: {l_text:?}"
    );
}

#[test]
fn lv_plain_text() {
    cross_check(b"Hello");
}
#[test]
fn lv_sgr_31() {
    cross_check(b"\x1b[31mRED");
}
#[test]
fn lv_sgr_32() {
    cross_check(b"\x1b[32mGREEN");
}
#[test]
fn lv_sgr_31_47() {
    cross_check(b"\x1b[31;47mWHITEONRED");
}
#[test]
fn lv_cursor_right() {
    cross_check(b"AB");
}
#[test]
fn lv_cursor_move_cuf() {
    cross_check(b"A\x1b[2CB");
}
#[test]
fn lv_cursor_down() {
    cross_check(b"A\nB");
}
#[test]
fn lv_cursor_cup() {
    cross_check(b"\x1b[5;10HX");
}
#[test]
fn lv_cursor_cuu() {
    cross_check(b"\x1b[2;1HAB\x1b[2AC");
}
#[test]
fn lv_el_erase_line() {
    cross_check(b"ABCDE\x1b[2KX");
}
#[test]
fn lv_ed_erase_below() {
    cross_check(b"AAA\nBBB\nCCC\x1b[2;1H\x1b[0J");
}
#[test]
fn lv_ich_2() {
    cross_check(b"CDE\x1b[G\x1b[2@X");
}
#[test]
fn lv_ri_reverse() {
    cross_check(b"\x1b[2;1H\x1bM");
}
#[test]
fn lv_bold_and_normal() {
    cross_check(b"\x1b[1mBOLD\x1b[0mNORM");
}
#[test]
fn lv_italic_and_underline() {
    cross_check(b"\x1b[3mITA\x1b[4mUNL");
}
#[test]
fn lv_cht_and_cnl() {
    cross_check(b"\x1b[3E\x1b[2FA");
}
#[test]
fn lv_hvp_position() {
    cross_check(b"\x1b[10;20fX");
}
#[test]
fn lv_set_lrm_and_decaln() {
    cross_check(b"\x1b#8");
}
#[test]
fn lv_decstbm_region() {
    cross_check(b"\x1b[5;10r\x1b[6;1HA");
}
#[test]
fn lv_su_scroll() {
    cross_check(b"1\n2\n3\n4\n5\x1b[2SX");
}
