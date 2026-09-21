use super::*;
use crate::terminal::test_helpers::assert_invariants;
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
    let mut lf = GhosttyTerminal::new(5, 10, 100).expect("terminal lf");
    lf.flush();
    lf.pty_write(b"a\nb");
    lf.flush();

    let mut crlf = GhosttyTerminal::new(5, 10, 100).expect("terminal crlf");
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
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("terminal");
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

// ── R: pty_write string-mode termination across chunk boundaries ──

/// 分片 OSC 由上游跨调用重组：块 1 未闭合时块 2 作为字符串续接被消费，
/// 不得渲染；ST 闭合后后续文本正常渲染（此前 ST 自动闭合会截断合法跨块 OSC）。
#[test]
fn pty_write_split_osc_reassembled_across_chunks() {
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    t.flush();
    // 块 1 在 OSC 内结束（无 ST/BEL）：上游保持字符串状态等待续接。
    t.pty_write(b"\x1b]52;c;abc");
    t.flush();
    // 块 2 先闭合 OSC，再写正常文本：续接内容被 OSC 消费，不渲染。
    t.pty_write(b"def\x07OK");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cells[0].codepoint, 'O' as u32,
        "续接内容必须被 OSC 消费，OK 应从 row0 col0 渲染"
    );
    assert_eq!(snap.cells[1].codepoint, 'K' as u32, "OK 的 K 应在 col1");
    assert_invariants(&snap);
}

/// A chunk that ends with a complete ST-terminated OSC must not leave the
/// next chunk inside string mode.
#[test]
fn pty_write_complete_st_resets_string_mode() {
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    t.flush();
    t.pty_write(b"\x1b]0;title\x1b\\");
    t.flush();
    // If chunk 1 had wrongly ended in string mode, 'hello' would be
    // swallowed as OSC payload and never render.
    t.pty_write(b"hello");
    t.flush();
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cells[0].codepoint, 'h' as u32,
        "'h' must render at row0 col0 — complete ST must exit string mode"
    );
    assert_invariants(&snap);
}

/// With a live terminal, `take_snapshot_with_scroll` returns a
/// sensible snapshot whose dimensions match the terminal.
#[test]
fn take_snapshot_with_scroll_returns_dims_when_alive() {
    let t = GhosttyTerminal::new(24, 80, 1000).expect("terminal");
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

/// When scroll_offset > 0, build_snapshot falls back to GridSnapshot::fallback()
/// because RenderState doesn't expose scrollback. The fallback snapshot must
/// have the correct rows/cols with one CellSnapshot per cell.
#[test]
fn scrollback_fallback_uses_fallback_snapshot() {
    let t = GhosttyTerminal::new(24, 80, 1000).expect("terminal");
    t.flush();
    let snap = t.take_snapshot_with_scroll(1);
    assert_eq!(snap.rows, 24, "fallback snapshot rows must match terminal");
    assert_eq!(snap.cols, 80, "fallback snapshot cols must match terminal");
    assert_eq!(
        snap.cells.len(),
        (24 * 80) as usize,
        "fallback snapshot must have one CellSnapshot per cell"
    );
}

// ── RK1–RK4: keyboard encoder correctness ─────────────────────

/// RK1: `utf8` is the produced char ('A'), distinct from the
/// unshifted codepoint ('a'). With SHIFT stripped (shift changed
/// the char), the encoder emits the bare printable 'A'.
#[test]
fn key_encode_shift_a_uses_utf8_char() {
    let mut t = GhosttyTerminal::new(24, 80, 1000).expect("terminal");
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
    let mut t = GhosttyTerminal::new(24, 80, 1000).expect("terminal");
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
    let t = GhosttyTerminal::new(24, 80, 1000).expect("terminal");
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
    let mut t = GhosttyTerminal::new(24, 80, 1000).expect("terminal");
    enable_kitty(&mut t);
    let shift = key::Mods::SHIFT.bits();
    let first = t.key_encode(29, shift, 0, 0x41, 0x61).expect("encode");
    let second = t.key_encode(29, shift, 0, 0x41, 0x61).expect("encode");
    let third = t.key_encode(29, shift, 0, 0x41, 0x61).expect("encode");
    assert_eq!(first, second, "encoder reuse must be stable (1st vs 2nd)");
    assert_eq!(second, third, "encoder reuse must be stable (2nd vs 3rd)");
}

/// 对标上游 ctrlKeyEncoding/escapeAndEnterEncoding：Ctrl+C 发 0x03，
/// ESC 发 0x1B，回车发 0x0D。
#[test]
fn key_encode_ctrl_c_escape_enter_basics() {
    let t = GhosttyTerminal::new(24, 80, 1000).expect("terminal");
    let ctrl = key::Mods::CTRL.bits();
    // Android C 键码 31，unicode 0x03（C0 控制字符走逻辑键路径）。
    let ctrl_c = t.key_encode(31, ctrl, 0, 0x03, 0).expect("encode");
    assert_eq!(ctrl_c, vec![0x03], "Ctrl+C must emit 0x03 (got {ctrl_c:?})");
    // ESC 键码 111。
    let esc = t.key_encode(111, 0, 0, 0x1B, 0).expect("encode");
    assert_eq!(esc, vec![0x1B], "ESC must emit 0x1B (got {esc:?})");
    // 回车键码 66。
    let enter = t.key_encode(66, 0, 0, 0x0D, 0x0D).expect("encode");
    assert_eq!(enter, vec![0x0D], "Enter must emit 0x0D (got {enter:?})");
}

/// P1-S3: search_all_in_scrollback returns all occurrences of a query
#[test]
fn search_all_in_scrollback_finds_all_matches() {
    let mut t = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    t.vt_write(b"hello world\n");
    t.vt_write(b"hello again\n");
    t.vt_write(b"goodbye\n");
    t.flush();
    let results = t.search_all_in_scrollback("hello", true);
    assert_eq!(results.len(), 2, "must find 'hello' in both lines");
    for m in &results {
        assert!(m.row < 3, "match row must be valid");
        assert!(m.start_col < m.end_col, "start_col must precede end_col");
    }
}

/// 相邻匹配不跳过：`aaaa` 搜 `aa` 须返回 2 个不重叠匹配（列 0-2 与 2-4）。
#[test]
fn search_all_in_scrollback_finds_adjacent_matches() {
    let mut t = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    t.vt_write(b"aaaa\n");
    t.flush();
    let results = t.search_all_in_scrollback("aa", true);
    assert_eq!(
        results.len(),
        2,
        "adjacent 'aa' in 'aaaa' must yield 2 matches"
    );
    assert_eq!((results[0].start_col, results[0].end_col), (0, 2));
    assert_eq!((results[1].start_col, results[1].end_col), (2, 4));
}

/// P1-S3: search_all_in_scrollback with case-insensitive matching
#[test]
fn search_all_in_scrollback_case_insensitive() {
    let mut t = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    t.vt_write(b"HELLO world\n");
    t.vt_write(b"hello again\n");
    t.flush();
    let results = t.search_all_in_scrollback("hello", false);
    assert_eq!(results.len(), 2, "must find 'hello' case-insensitively");
}

/// P1-S3: search_all_in_scrollback empty query returns nothing
#[test]
fn search_all_in_scrollback_empty_query() {
    let t = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    let results = t.search_all_in_scrollback("", true);
    assert!(results.is_empty(), "empty query must return no matches");
}

/// 对标上游 searchSpansSoftWrap：跨软换行边界的匹配必须命中。
/// 20 列终端写 18 个 x + needle：needle 横跨换行点。
#[test]
fn search_all_in_scrollback_spans_soft_wrap() {
    let mut t = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    let token = format!("{}needle", "x".repeat(18));
    t.vt_write(token.as_bytes());
    t.flush();
    let results = t.search_all_in_scrollback("needle", true);
    assert_eq!(
        results.len(),
        1,
        "soft-wrapped needle must be found (got {results:?})"
    );
}

/// P1-S3: search_all_in_scrollback no matches returns empty
#[test]
fn search_all_in_scrollback_no_matches() {
    let mut t = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    t.vt_write(b"abc def\n");
    t.flush();
    let results = t.search_all_in_scrollback("xyz", true);
    assert!(results.is_empty(), "no-match query must return empty vec");
}

/// 对标上游 searchCountsPastTheNavigableCap：超上限时保留最新命中。
/// 小规模验证截断方向：多行同词，返回顺序旧→新且首个非最旧。
#[test]
fn search_all_in_scrollback_keeps_newest_order() {
    let mut t = GhosttyTerminal::new(10, 80, 100).expect("terminal");
    for index in 0..8 {
        t.vt_write(format!("hit{index:02}\n").as_bytes());
    }
    t.flush();
    let results = t.search_all_in_scrollback("hit", true);
    assert_eq!(results.len(), 8, "all hits must be found");
    let rows: Vec<u32> = results.iter().map(|matched| matched.row).collect();
    let mut sorted = rows.clone();
    sorted.sort_unstable();
    assert_eq!(rows, sorted, "results must stay oldest-first");
}

/// key_encode_submit returns a Some(receiver) for a valid key and the
/// receiver produces the expected encoded bytes (same semantic as key_encode).
#[test]
fn key_encode_submit_returns_receiver() {
    let mut t = GhosttyTerminal::new(24, 80, 1000).expect("terminal");
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
    let mut t = GhosttyTerminal::new(24, 80, 1000).expect("terminal");
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
    let t = GhosttyTerminal::new(24, 80, 1000).expect("terminal");
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

/// Regression: search must not panic on multi-byte (CJK) lines — byte
/// slicing used to land mid-character (start = abs_col + 1), which panics
/// deterministically on CJK text.
#[test]
fn search_all_in_scrollback_cjk_no_panic() {
    let mut t = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    t.vt_write("你好世界 hello 中文测试\n".as_bytes());
    t.flush();
    // Query after a multi-byte char; overlap stepping must stay on char
    // boundaries. (Ghostty reads wide-char rows with interleaved spaces, e.g. "你 好 世 界  hello 中 文 测 试",
    // so an ASCII query is the reliable probe here.)
    let results = t.search_all_in_scrollback("hello", true);
    assert_eq!(results.len(), 1, "must find ASCII query on CJK line");
    // Case-insensitive path over the same CJK row: must not panic and
    // must still find the ASCII query.
    let lower = t.search_all_in_scrollback("HELLO", false);
    assert!(
        lower.iter().any(|m| m.row == 0),
        "case-insensitive query must find the match on row 0"
    );
}

#[test]
fn search_returns_character_columns_not_byte_offsets() {
    // "你" is 3 UTF-8 bytes but 1 character column. A match after it must
    // report character columns so the renderer's CellData.col highlight
    // aligns (byte offsets would be wider and shifted on CJK rows).
    let mut t = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    t.vt_write("你hello\n".as_bytes());
    t.flush();
    let results = t.search_all_in_scrollback("hello", true);
    assert_eq!(results.len(), 1, "must find ASCII query after CJK char");
    let m = &results[0];
    // Ghostty reads wide-char rows with an interleaved fill space
    // ("你 hello"), so character-column counting ("你"=1 char + 1 fill
    // space = 2) matches the grid column where 'h' starts.
    assert_eq!(
        m.start_col, 2,
        "start_col must be char column, got {}",
        m.start_col
    );
    assert_eq!(
        m.end_col, 7,
        "end_col must be char column, got {}",
        m.end_col
    );
}

/// 对标上游大小写折叠：非 ASCII 字母不敏感匹配（拉丁/捷克/西里尔/希腊），
/// 且重音不等价（cafe 不得命中 café）。
#[test]
fn search_all_in_scrollback_unicode_case_folding() {
    let mut t = GhosttyTerminal::new(6, 80, 100).expect("terminal");
    t.vt_write("Café café CAFÉ\n".as_bytes());
    t.vt_write("Čau čau\n".as_bytes());
    t.vt_write("Я я\n".as_bytes());
    t.vt_write("Σ σ\n".as_bytes());
    t.flush();
    assert_eq!(
        t.search_all_in_scrollback("café", false).len(),
        3,
        "café 不敏感须命中三行变体"
    );
    assert_eq!(
        t.search_all_in_scrollback("café", true).len(),
        1,
        "café 敏感仅命中全小写"
    );
    assert_eq!(
        t.search_all_in_scrollback("čau", false).len(),
        2,
        "čau 不敏感须命中大小写"
    );
    assert_eq!(
        t.search_all_in_scrollback("я", false).len(),
        2,
        "西里尔不敏感须命中大小写"
    );
    assert_eq!(
        t.search_all_in_scrollback("σ", false).len(),
        2,
        "希腊不敏感须命中大小写"
    );
    assert!(
        t.search_all_in_scrollback("cafe", false).is_empty(),
        "无重音不得命中重音文本"
    );
}
