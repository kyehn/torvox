use super::*;
use crate::test_helpers::assert_invariants;
use flume::bounded;
use libghostty_vt::key::{self};

/// Enable the Kitty keyboard protocol so the encoder reports
/// explicit mods (required to observe SHIFT stripping, RK2).
fn enable_kitty(t: &mut GhosttyTerminal) {
    t.vt_write(b"\x1b[?u"); // query supported flags
    t.flush();
    t.vt_write(b"\x1b[>1u"); // enable progressive enhancement (level 1+)
    t.flush();
}

// ── R3: pty_write LF→CRLF idempotency ──────────────────

/// `pty_write` converts a bare LF to CRLF, but must NOT insert a
/// second CR when the LF is already preceded by a CR. Both
/// `a\nb` and `a\r\nb` must reach the same cell layout.
#[test]
fn pty_write_lf_crlf_idempotent() {
    let mut lf = GhosttyTerminal::new(5, 10, 100).expect("term lf");
    lf.flush();
    lf.pty_write(b"a\nb");
    lf.flush();

    let mut crlf = GhosttyTerminal::new(5, 10, 100).expect("term crlf");
    crlf.flush();
    crlf.pty_write(b"a\r\nb");
    crlf.flush();

    // 'b' must land at row 1, column 0 in BOTH terminals — proving
    // the already-present CR was not doubled into an extra line break.
    let lf_snap = lf.take_snapshot();
    let crlf_snap = crlf.take_snapshot();
    let lf_b = lf_snap.cells.get(lf_snap.cols as usize);
    let crlf_b = crlf_snap.cells.get(crlf_snap.cols as usize);
    assert_eq!(
        lf_b.map(|c| c.codepoint),
        Some('b' as u32),
        "a\\nb: 'b' must be at row1 col0"
    );
    assert_eq!(
        crlf_b.map(|c| c.codepoint),
        Some('b' as u32),
        "a\\r\\nb: 'b' must be at row1 col0 (no double CR)"
    );
    assert_eq!(
        lf_snap.cells[(lf_snap.cols + 1) as usize].codepoint,
        0,
        "a\\nb: row1 col1 must stay empty (cursor advanced past 'b')"
    );
    assert_eq!(
        crlf_snap.cells[(crlf_snap.cols + 1) as usize].codepoint,
        0,
        "a\\r\\nb: row1 col1 must stay empty (no spurious CR)"
    );
    assert_invariants(&lf_snap);
    assert_invariants(&crlf_snap);
}

/// A bare LF must still be promoted to CRLF (regression: the
/// transform must fire for `a\nb`, placing 'b' on row 1).
#[test]
fn pty_write_lf_is_promoted_to_crlf() {
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
    t.flush();
    t.pty_write(b"a\nb");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cells[snap.cols as usize].codepoint, 'b' as u32,
        "LF must advance to next row (CRLF); 'b' at row1 col0"
    );
    assert_invariants(&snap);
}

// ── R7: take_snapshot_with_scroll routes through recv_or_fallback ─

/// `recv_or_fallback` returns the channel value when the terminal thread
/// is alive and responds (the normal path used by
/// `take_snapshot_with_scroll`).
#[test]
fn recv_or_fallback_returns_value_when_present() {
    let (tx, rx) = bounded(1);
    tx.send(99u32).expect("send");
    let result = GhosttyTerminal::recv_or_fallback(rx, 7u32, "unit");
    assert_eq!(result, 99, "recv_or_fallback must return the sent value");
}

/// `recv_or_fallback` returns the fallback when the channel is
/// disconnected (terminal thread dead — the "no surface" path
/// `take_snapshot_with_scroll` must take when the surface is gone).
#[test]
fn recv_or_fallback_returns_fallback_when_disconnected() {
    let (tx, rx) = bounded::<u32>(1);
    drop(tx); // simulate a dead terminal thread
    let result = GhosttyTerminal::recv_or_fallback(rx, 42u32, "unit");
    assert_eq!(
        result, 42,
        "recv_or_fallback must return the fallback when disconnected"
    );
}

/// With a live terminal, `take_snapshot_with_scroll` returns a
/// sensible snapshot whose dimensions match the terminal.
#[test]
fn take_snapshot_with_scroll_returns_dims_when_alive() {
    let t = GhosttyTerminal::new(24, 80, 1000).expect("term");
    t.flush();
    let snap = t.take_snapshot_with_scroll(0);
    assert_eq!(snap.rows, 24, "snapshot rows must match terminal");
    assert_eq!(snap.cols, 80, "snapshot cols must match terminal");
    assert!(
        snap.cells.len() >= (24 * 80) as usize,
        "snapshot must carry cells"
    );
    assert_invariants(&snap);
}

// ── RK1–RK4: keyboard encoder correctness ─────────────────────

/// RK1: `utf8` is the produced char ('A'), distinct from the
/// unshifted codepoint ('a'). With SHIFT stripped (shift changed
/// the char), the encoder emits the bare printable 'A'.
#[test]
fn key_encode_shift_a_uses_utf8_char() {
    let mut t = GhosttyTerminal::new(24, 80, 1000).expect("term");
    enable_kitty(&mut t);
    let shift = key::Mods::SHIFT.bits();
    let out = t.key_encode(29, shift, 0, 0x41, 0x61).expect("encode");
    assert!(
        out.contains(&0x41),
        "output must contain 'A' (utf8): {out:?}"
    );
    assert!(
        !out.contains(&0x61),
        "output must NOT contain 'a' (unshifted): {out:?}"
    );
    assert_eq!(
        out,
        vec![0x41],
        "Shift+A with stripped shift emits bare 'A': {out:?}"
    );
}

/// RK2: SHIFT is only stripped when it changed the printed char.
/// For Enter, the shifted and unshifted char are both 0x0d, so
/// SHIFT is RETAINED and the Kitty encoder emits a CSI sequence
/// (proving the strip is conditional, not blanket).
#[test]
fn key_encode_shift_enter_keeps_shift() {
    let mut t = GhosttyTerminal::new(24, 80, 1000).expect("term");
    enable_kitty(&mut t);
    let shift = key::Mods::SHIFT.bits();
    let out = t.key_encode(66, shift, 0, 0x0d, 0x0d).expect("encode");
    assert!(
        out.starts_with(b"\x1b["),
        "Shift+Enter must emit a CSI sequence (shift retained): {out:?}"
    );
}

/// RK3: pure control keys must pass `utf8 = NULL` so the encoder
/// uses the logical key. The base behaviour (Kitty progressive
/// enhancement intentionally NOT enabled here) is that Ctrl+A still
/// reaches the PTY as the control byte 0x01 — the encoder must NOT
/// silently drop the key, and must NOT embed the C0 byte as a utf8
/// codepoint (the malformed `1;5u` form).
#[test]
fn key_encode_ctrl_a_passes_null_utf8() {
    let t = GhosttyTerminal::new(24, 80, 1000).expect("term");
    let ctrl = key::Mods::CTRL.bits();
    let out = t.key_encode(29, ctrl, 0, 0x01, 0).expect("encode");
    assert!(
        !out.is_empty(),
        "Ctrl+A must produce output (control byte 0x01), not be dropped: {out:?}"
    );
    assert!(
        out.contains(&0x01),
        "Ctrl+A must emit the control byte 0x01: {out:?}"
    );
    let rendered = String::from_utf8_lossy(&out);
    assert!(
        !rendered.contains("1;5u"),
        "Ctrl+A must NOT pass the C0 byte (1) as a utf8 codepoint: {out:?}"
    );
}

/// RK4: the encoder/event are stored once on `GhosttyTerminal`
/// and reused. Repeated encodes of the same key must produce
/// identical output (no per-call state loss from re-allocation).
#[test]
fn key_encode_encoder_reused_stable() {
    let mut t = GhosttyTerminal::new(24, 80, 1000).expect("term");
    enable_kitty(&mut t);
    let shift = key::Mods::SHIFT.bits();
    let first = t.key_encode(29, shift, 0, 0x41, 0x61).expect("encode");
    let second = t.key_encode(29, shift, 0, 0x41, 0x61).expect("encode");
    let third = t.key_encode(29, shift, 0, 0x41, 0x61).expect("encode");
    assert_eq!(first, second, "encoder reuse must be stable (1st vs 2nd)");
    assert_eq!(second, third, "encoder reuse must be stable (2nd vs 3rd)");
}

/// P1-S3: search_all_in_scrollback returns all occurrences of a query
#[test]
fn search_all_in_scrollback_finds_all_matches() {
    let mut t = GhosttyTerminal::new(3, 80, 100).expect("term");
    t.vt_write(b"hello world\n");
    t.vt_write(b"hello again\n");
    t.vt_write(b"goodbye\n");
    t.flush();
    let results = t.search_all_in_scrollback("hello", true, false);
    assert!(!results.is_empty(), "must find 'hello'");
    assert_eq!(results.len(), 2, "must find 'hello' in both lines");
    for m in &results {
        assert!(m.row < 3, "match row must be valid");
        assert!(m.start_col < m.end_col, "start_col must precede end_col");
    }
}

/// P1-S3: search_all_in_scrollback with case-insensitive matching
#[test]
fn search_all_in_scrollback_case_insensitive() {
    let mut t = GhosttyTerminal::new(3, 80, 100).expect("term");
    t.vt_write(b"HELLO world\n");
    t.vt_write(b"hello again\n");
    t.flush();
    let results = t.search_all_in_scrollback("hello", false, false);
    assert_eq!(results.len(), 2, "must find 'hello' case-insensitively");
}

/// P1-S3: search_all_in_scrollback empty query returns nothing
#[test]
fn search_all_in_scrollback_empty_query() {
    let t = GhosttyTerminal::new(3, 80, 100).expect("term");
    let results = t.search_all_in_scrollback("", true, false);
    assert!(results.is_empty(), "empty query must return no matches");
}

/// P1-S3: search_all_in_scrollback no matches returns empty
#[test]
fn search_all_in_scrollback_no_matches() {
    let mut t = GhosttyTerminal::new(3, 80, 100).expect("term");
    t.vt_write(b"abc def\n");
    t.flush();
    let results = t.search_all_in_scrollback("xyz", true, false);
    assert!(results.is_empty(), "no-match query must return empty vec");
}

/// P1-S3: fuzzy search returns ALL near-matches per line, not just the closest
#[test]
fn search_all_in_scrollback_fuzzy_finds_multiple_per_line() {
    let mut t = GhosttyTerminal::new(3, 80, 100).expect("term");
    t.vt_write(b"hello helxo heplo\n");
    t.flush();
    let results = t.search_all_in_scrollback("hello", true, true);
    // "hello" at col 0 (exact match), "helxo" at col 6 (1 edit), "heplo" at col 12 (1 edit)
    // With query len=5, max_distance = max(1, 5/3) = 1
    // So all three should match since each is ≤1 edit from "hello"
    assert!(
        results.len() >= 3,
        "fuzzy search should find all three near-matches, found {}",
        results.len()
    );
    // Verify all three positions are within bounds
    for m in &results {
        assert!(m.start_col < m.end_col, "start_col must precede end_col");
        assert!(m.row == 0, "all matches on row 0");
    }
    // Verify the third match is different from the first (not deduped to nearest)
    let positions: std::collections::HashSet<(u32, u32)> =
        results.iter().map(|m| (m.start_col, m.end_col)).collect();
    assert!(
        positions.len() >= 3,
        "fuzzy search should return at least 3 distinct match positions, got {}",
        positions.len()
    );
}

/// key_encode_submit returns a Some(receiver) for a valid key and the
/// receiver produces the expected encoded bytes (same semantic as key_encode).
#[test]
fn key_encode_submit_returns_receiver() {
    let mut t = GhosttyTerminal::new(24, 80, 1000).expect("term");
    enable_kitty(&mut t);
    let shift = key::Mods::SHIFT.bits();
    let rx = t.key_encode_submit(29, shift, 0, 0x41, 0x61);
    assert!(rx.is_some(), "key_encode_submit must return Some receiver");
    let result = rx.unwrap().recv().expect("receiver must produce result");
    assert!(
        result.contains(&0x41),
        "output must contain 'A' (utf8): {result:?}"
    );
    assert!(
        !result.contains(&0x61),
        "output must NOT contain 'a' (unshifted): {result:?}"
    );
}

/// key_encode_submit + waiting on receiver produces the same result as
/// the synchronous key_encode for the same input.
#[test]
fn key_encode_submit_and_key_encode_produce_same_result() {
    let mut t = GhosttyTerminal::new(24, 80, 1000).expect("term");
    enable_kitty(&mut t);
    let shift = key::Mods::SHIFT.bits();
    let rx = t
        .key_encode_submit(29, shift, 0, 0x41, 0x61)
        .expect("key_encode_submit must return receiver");
    let submit_result = rx.recv().expect("receiver must produce result");
    let direct_result = t
        .key_encode(29, shift, 0, 0x41, 0x61)
        .expect("key_encode must produce result");
    assert_eq!(
        submit_result, direct_result,
        "key_encode_submit and key_encode must produce identical output"
    );
}

/// Dropping the receiver before the ghostty thread responds does not
/// cause a panic — ghostty handles the send error gracefully and the
/// terminal remains usable for subsequent requests.
#[test]
fn key_encode_submit_dropped_receiver_does_not_panic() {
    let t = GhosttyTerminal::new(24, 80, 1000).expect("term");
    let rx = t.key_encode_submit(29, 0, 0, 0x61, 0x61);
    drop(rx);
    let result = t
        .key_encode(29, 0, 0, 0x62, 0x62)
        .expect("terminal must remain functional after dropped receiver");
    assert!(
        !result.is_empty(),
        "key_encode after dropped receiver must produce output: {result:?}"
    );
}
