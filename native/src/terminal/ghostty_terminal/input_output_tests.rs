use super::*;
use crate::terminal::test_helpers::assert_invariants;

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
        lf_b.map(|cell| cell.codepoint),
        Some('b' as u32),
        "a\\nb: 'b' must be at row1 col0"
    );
    assert_eq!(
        crlf_b.map(|cell| cell.codepoint),
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
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    terminal_under_test.flush();
    terminal_under_test.pty_write(b"a\nb");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
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
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    terminal_under_test.flush();
    // 块 1 在 OSC 内结束（无 ST/BEL）：上游保持字符串状态等待续接。
    terminal_under_test.pty_write(b"\x1b]52;c;abc");
    terminal_under_test.flush();
    // 块 2 先闭合 OSC，再写正常文本：续接内容被 OSC 消费，不渲染。
    terminal_under_test.pty_write(b"def\x07OK");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
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
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    terminal_under_test.flush();
    terminal_under_test.pty_write(b"\x1b]0;title\x1b\\");
    terminal_under_test.flush();
    // If chunk 1 had wrongly ended in string mode, 'hello' would be
    // swallowed as OSC payload and never render.
    terminal_under_test.pty_write(b"hello");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    assert_eq!(
        snap.cells[0].codepoint, 'h' as u32,
        "'h' must render at row0 col0 — complete ST must exit string mode"
    );
    assert_invariants(&snap);
}

/// With a live terminal, `take_snapshot` returns a
/// sensible snapshot whose dimensions match the terminal.
#[test]
fn take_snapshot_returns_dims_when_alive() {
    let terminal_under_test = GhosttyTerminal::new(24, 80, 1000).expect("terminal");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    assert_eq!(snap.rows, 24, "snapshot rows must match terminal");
    assert_eq!(snap.cols, 80, "snapshot cols must match terminal");
    assert!(
        snap.cells.len() >= (24 * 80) as usize,
        "snapshot must carry cells"
    );
    assert_invariants(&snap);
}

/// search_all_in_scrollback returns all occurrences of a query
#[test]
fn search_all_in_scrollback_finds_all_matches() {
    let mut terminal_under_test = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    terminal_under_test.vt_write(b"hello world\n");
    terminal_under_test.vt_write(b"hello again\n");
    terminal_under_test.vt_write(b"goodbye\n");
    terminal_under_test.flush();
    let results = terminal_under_test.search_all_in_scrollback("hello", true);
    assert_eq!(results.len(), 2, "must find 'hello' in both lines");
    for search_match in &results {
        assert!(search_match.row < 3, "match row must be valid");
        assert!(
            search_match.start_col < search_match.end_col,
            "start_col must precede end_col"
        );
    }
}

/// 相邻匹配不跳过：`aaaa` 搜 `aa` 须返回 2 个不重叠匹配（列 0-2 与 2-4）。
#[test]
fn search_all_in_scrollback_finds_adjacent_matches() {
    let mut terminal_under_test = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    terminal_under_test.vt_write(b"aaaa\n");
    terminal_under_test.flush();
    let results = terminal_under_test.search_all_in_scrollback("aa", true);
    assert_eq!(
        results.len(),
        2,
        "adjacent 'aa' in 'aaaa' must yield 2 matches"
    );
    assert_eq!((results[0].start_col, results[0].end_col), (0, 2));
    assert_eq!((results[1].start_col, results[1].end_col), (2, 4));
}

/// search_all_in_scrollback with case-insensitive matching
#[test]
fn search_all_in_scrollback_case_insensitive() {
    let mut terminal_under_test = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    terminal_under_test.vt_write(b"HELLO world\n");
    terminal_under_test.vt_write(b"hello again\n");
    terminal_under_test.flush();
    let results = terminal_under_test.search_all_in_scrollback("hello", false);
    assert_eq!(results.len(), 2, "must find 'hello' case-insensitively");
}

/// search_all_in_scrollback empty query returns nothing
#[test]
fn search_all_in_scrollback_empty_query() {
    let terminal_under_test = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    let results = terminal_under_test.search_all_in_scrollback("", true);
    assert!(results.is_empty(), "empty query must return no matches");
}

/// 对标上游 searchSpansSoftWrap：跨软换行边界的匹配必须命中。
/// 20 列终端写 18 个 x + needle：needle 横跨换行点。
#[test]
fn search_all_in_scrollback_spans_soft_wrap() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    // 18 个 x + needle = 24 字符，在 20 列网格上折成
    // "xxxxxxxxxxxxxxxxxxxx" / "xxxxneedle" 两个物理行。
    let token = format!("{}needle", "x".repeat(18));
    terminal_under_test.vt_write(token.as_bytes());
    terminal_under_test.flush();
    let results = terminal_under_test.search_all_in_scrollback("needle", true);
    // 命中按物理行拆分：列区间必须落在各自行内（旧实现把整段记在行首行并给出
    // 越过 20 列网格的 end_col，高亮实际不可见）。
    assert_eq!(
        results,
        vec![
            SearchMatch {
                row: 0,
                start_col: 18,
                end_col: 20,
            },
            SearchMatch {
                row: 1,
                start_col: 0,
                end_col: 4,
            },
        ],
        "soft-wrapped needle must be split per physical row (got {results:?})"
    );
}

/// search_all_in_scrollback no matches returns empty
#[test]
fn search_all_in_scrollback_no_matches() {
    let mut terminal_under_test = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    terminal_under_test.vt_write(b"abc def\n");
    terminal_under_test.flush();
    let results = terminal_under_test.search_all_in_scrollback("xyz", true);
    assert!(results.is_empty(), "no-match query must return empty vec");
}

/// 对标上游 searchCountsPastTheNavigableCap：超上限时保留最新命中。
/// 小规模验证截断方向：多行同词，返回顺序旧→新且首个非最旧。
#[test]
fn search_all_in_scrollback_keeps_newest_order() {
    let mut terminal_under_test = GhosttyTerminal::new(10, 80, 100).expect("terminal");
    for index in 0..8 {
        terminal_under_test.vt_write(format!("hit{index:02}\n").as_bytes());
    }
    terminal_under_test.flush();
    let results = terminal_under_test.search_all_in_scrollback("hit", true);
    assert_eq!(results.len(), 8, "all hits must be found");
    let rows: Vec<u32> = results.iter().map(|matched| matched.row).collect();
    let mut sorted = rows.clone();
    sorted.sort_unstable();
    assert_eq!(rows, sorted, "results must stay oldest-first");
}

/// Regression: search must not panic on multi-byte (CJK) lines — byte
/// slicing used to land mid-character (start = abs_col + 1), which panics
/// deterministically on CJK text.
#[test]
fn search_all_in_scrollback_cjk_no_panic() {
    let mut terminal_under_test = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    terminal_under_test.vt_write("你好世界 hello 中文测试\n".as_bytes());
    terminal_under_test.flush();
    // Query after a multi-byte char; overlap stepping must stay on char
    // boundaries. (Ghostty reads wide-char rows with interleaved spaces, e.g. "你 好 世 界  hello 中 文 测 试",
    // so an ASCII query is the reliable probe here.)
    let results = terminal_under_test.search_all_in_scrollback("hello", true);
    assert_eq!(results.len(), 1, "must find ASCII query on CJK line");
    // Case-insensitive path over the same CJK row: must not panic and
    // must still find the ASCII query.
    let lower = terminal_under_test.search_all_in_scrollback("HELLO", false);
    assert!(
        lower.iter().any(|search_match| search_match.row == 0),
        "case-insensitive query must find the match on row 0"
    );
}

#[test]
fn search_returns_character_columns_not_byte_offsets() {
    // "你" is 3 UTF-8 bytes but 1 character column. A match after it must
    // report character columns so the renderer's CellData.col highlight
    // aligns (byte offsets would be wider and shifted on CJK rows).
    let mut terminal_under_test = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    terminal_under_test.vt_write("你hello\n".as_bytes());
    terminal_under_test.flush();
    let results = terminal_under_test.search_all_in_scrollback("hello", true);
    assert_eq!(results.len(), 1, "must find ASCII query after CJK char");
    let search_match = &results[0];
    // Ghostty reads wide-char rows with an interleaved fill space
    // ("你 hello"), so character-column counting ("你"=1 char + 1 fill
    // space = 2) matches the grid column where 'h' starts.
    assert_eq!(
        search_match.start_col, 2,
        "start_col must be char column, got {}",
        search_match.start_col
    );
    assert_eq!(
        search_match.end_col, 7,
        "end_col must be char column, got {}",
        search_match.end_col
    );
}

/// 对标上游大小写折叠：非 ASCII 字母不敏感匹配（拉丁/捷克/西里尔/希腊），
/// 且重音不等价（cafe 不得命中 café）。
#[test]
fn search_all_in_scrollback_unicode_case_folding() {
    let mut terminal_under_test = GhosttyTerminal::new(6, 80, 100).expect("terminal");
    terminal_under_test.vt_write("Café café CAFÉ\n".as_bytes());
    terminal_under_test.vt_write("Čau čau\n".as_bytes());
    terminal_under_test.vt_write("Я я\n".as_bytes());
    terminal_under_test.vt_write("Σ σ\n".as_bytes());
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test
            .search_all_in_scrollback("café", false)
            .len(),
        3,
        "café 不敏感须命中三行变体"
    );
    assert_eq!(
        terminal_under_test
            .search_all_in_scrollback("café", true)
            .len(),
        1,
        "café 敏感仅命中全小写"
    );
    assert_eq!(
        terminal_under_test
            .search_all_in_scrollback("čau", false)
            .len(),
        2,
        "čau 不敏感须命中大小写"
    );
    assert_eq!(
        terminal_under_test
            .search_all_in_scrollback("я", false)
            .len(),
        2,
        "西里尔不敏感须命中大小写"
    );
    assert_eq!(
        terminal_under_test
            .search_all_in_scrollback("σ", false)
            .len(),
        2,
        "希腊不敏感须命中大小写"
    );
    assert!(
        terminal_under_test
            .search_all_in_scrollback("cafe", false)
            .is_empty(),
        "无重音不得命中重音文本"
    );
}
