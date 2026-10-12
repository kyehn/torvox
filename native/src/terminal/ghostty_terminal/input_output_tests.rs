use super::*;
use crate::terminal::test_helpers::{assert_invariants, search_viewport_matches};

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

/// 以完整 ST 结尾的 OSC 分块 MUST NOT 让下一分块仍处于字符串模式。
#[test]
fn pty_write_complete_st_resets_string_mode() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    terminal_under_test.flush();
    terminal_under_test.pty_write(b"\x1b]0;title\x1b\\");
    terminal_under_test.flush();
    // 若分块 1 错误地停在字符串模式，`hello` 会被当作 OSC 负载吞掉且永不渲染。
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

/// 搜索须找出查询的全部出现。
#[test]
fn search_viewport_matches_find_all_matches() {
    let mut terminal_under_test = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    terminal_under_test.vt_write(b"hello world\n");
    terminal_under_test.vt_write(b"hello again\n");
    terminal_under_test.vt_write(b"goodbye\n");
    terminal_under_test.flush();
    let results = search_viewport_matches(&terminal_under_test, "hello");
    assert_eq!(results.len(), 2, "must find 'hello' in both lines");
    for search_match in &results {
        assert!(search_match.row < 3, "match row must be valid");
        assert!(
            search_match.start_col < search_match.end_col,
            "start_col must precede end_col"
        );
    }
}

/// 相邻命中逐个报出：`aaaa` 搜 `aa` 命中 3 次（上游允许重叠命中，
/// 高亮因此覆盖整行，而非隔列跳读）。
#[test]
fn search_viewport_matches_find_adjacent_matches() {
    let mut terminal_under_test = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    terminal_under_test.vt_write(b"aaaa\n");
    terminal_under_test.flush();
    let results = search_viewport_matches(&terminal_under_test, "aa");
    assert_eq!(
        results,
        vec![
            SearchMatch {
                row: 0,
                start_col: 0,
                end_col: 2,
            },
            SearchMatch {
                row: 0,
                start_col: 1,
                end_col: 3,
            },
            SearchMatch {
                row: 0,
                start_col: 2,
                end_col: 4,
            },
        ],
        "相邻重叠命中须逐个上报（got {results:?}）"
    );
}

/// 上游匹配：ASCII 字母不区分大小写。
#[test]
fn search_is_case_insensitive_for_ascii_letters() {
    let mut terminal_under_test = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    terminal_under_test.vt_write(b"HELLO world\n");
    terminal_under_test.vt_write(b"hello again\n");
    terminal_under_test.flush();
    let results = search_viewport_matches(&terminal_under_test, "hello");
    assert_eq!(results.len(), 2, "must find 'hello' case-insensitively");
}

/// 空 needle 清除搜索：无命中。
#[test]
fn search_empty_needle_returns_nothing() {
    let terminal_under_test = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    let results = search_viewport_matches(&terminal_under_test, "");
    assert!(results.is_empty(), "empty query must return no matches");
}

/// 对标上游 searchSpansSoftWrap：跨软换行边界的匹配必须命中。
/// 20 列终端写 18 个 x + needle：needle 横跨换行点。
#[test]
fn search_viewport_matches_span_soft_wrap() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    // 18 个 x + needle = 24 字符，在 20 列网格上折成
    // "xxxxxxxxxxxxxxxxxxxx" / "xxxxneedle" 两个物理行。
    let token = format!("{}needle", "x".repeat(18));
    terminal_under_test.vt_write(token.as_bytes());
    terminal_under_test.flush();
    let results = search_viewport_matches(&terminal_under_test, "needle");
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

/// 无命中返回空列表。
#[test]
fn search_viewport_matches_no_matches_returns_empty() {
    let mut terminal_under_test = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    terminal_under_test.vt_write(b"abc def\n");
    terminal_under_test.flush();
    let results = search_viewport_matches(&terminal_under_test, "xyz");
    assert!(results.is_empty(), "no-match query must return empty vec");
}

/// 对标上游 searchCountsPastTheNavigableCap：超上限时保留最新命中。
/// 小规模验证覆盖面：多行同词，每个命中行都必须在视口命中里出现。
#[test]
fn search_viewport_matches_cover_every_hit_row() {
    let mut terminal_under_test = GhosttyTerminal::new(10, 80, 100).expect("terminal");
    for index in 0..8 {
        terminal_under_test.vt_write(format!("hit{index:02}\n").as_bytes());
    }
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.search_set_needle("hit"),
        8,
        "命中总数须为 8"
    );
    let results = search_viewport_matches(&terminal_under_test, "hit");
    let mut rows: Vec<u32> = results.iter().map(|matched| matched.row).collect();
    rows.sort_unstable();
    assert_eq!(
        rows,
        (0..8).collect::<Vec<u32>>(),
        "视口命中须逐行覆盖全部命中"
    );
}

/// Regression: search must not panic on multi-byte (CJK) lines — byte
/// slicing used to land mid-character (start = abs_col + 1), which panics
/// deterministically on CJK text.
#[test]
fn search_viewport_matches_cjk_no_panic() {
    let mut terminal_under_test = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    terminal_under_test.vt_write("你好世界 hello 中文测试\n".as_bytes());
    terminal_under_test.flush();
    // Query after a multi-byte char; overlap stepping must stay on char
    // boundaries.
    let results = search_viewport_matches(&terminal_under_test, "hello");
    assert_eq!(results.len(), 1, "must find ASCII query on CJK line");
    // Case-insensitive path over the same CJK row: must not panic and
    // must still find the ASCII query.
    let lower = search_viewport_matches(&terminal_under_test, "HELLO");
    assert!(
        lower.iter().any(|search_match| search_match.row == 0),
        "case-insensitive query must find the match on row 0"
    );
    // 同一行上的 CJK 查询同样命中：宽字符尾格不再被填充为空格。
    assert_eq!(
        search_viewport_matches(&terminal_under_test, "中文测试").len(),
        1,
        "CJK 查询必须在同一行命中"
    );
}

#[test]
fn search_returns_character_columns_not_byte_offsets() {
    // "你" is 3 UTF-8 bytes but one wide cell spanning 2 grid columns. A match
    // after it must report grid columns so the renderer's CellData.col
    // highlight aligns (byte offsets would be wider and shifted on CJK rows).
    let mut terminal_under_test = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    terminal_under_test.vt_write("你hello\n".as_bytes());
    terminal_under_test.flush();
    let results = search_viewport_matches(&terminal_under_test, "hello");
    assert_eq!(results.len(), 1, "must find ASCII query after CJK char");
    let search_match = &results[0];
    // 宽字符占两列，故 `h` 起于网格列 2、止于 7。
    assert_eq!(
        search_match.start_col, 2,
        "start_col must be the grid column after the wide cell, got {}",
        search_match.start_col
    );
    assert_eq!(
        search_match.end_col, 7,
        "end_col must be the grid column past `hello`, got {}",
        search_match.end_col
    );
}

/// Regression: 宽字符尾格曾被填充为空格（"你好" → "你 好"），CJK 多字查询永不命中。
#[test]
fn search_matches_adjacent_cjk_characters() {
    let mut terminal_under_test = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    terminal_under_test.vt_write("你好世界\n".as_bytes());
    terminal_under_test.flush();

    for query in ["你好", "好世", "世界", "你好世界"] {
        let results = search_viewport_matches(&terminal_under_test, query);
        assert_eq!(
            results.len(),
            1,
            "查询 {query} 必须命中，实际 {:?}",
            results
        );
    }
}

/// 命中跨多个宽字符时，高亮须覆盖它们占据的全部网格列。
#[test]
fn search_highlight_spans_the_full_width_of_matched_wide_cells() {
    let mut terminal_under_test = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    terminal_under_test.vt_write("a你好b\n".as_bytes());
    terminal_under_test.flush();

    let results = search_viewport_matches(&terminal_under_test, "你好");
    assert_eq!(results.len(), 1, "必须命中，实际 {:?}", results);
    // `a` 占列 0；两个宽字符占列 1..4；`b` 在列 5。
    assert_eq!(results[0].start_col, 1, "起点须为第一个宽字符的起始列");
    assert_eq!(results[0].end_col, 5, "终点须覆盖两个宽字符的全部四列");

    let ascii = search_viewport_matches(&terminal_under_test, "b");
    assert_eq!(ascii[0].start_col, 5, "宽字符之后的 ASCII 须落在列 5");
}

/// 单个宽字符的命中覆盖两列：尾格不另成一条命中。
#[test]
fn search_single_wide_character_covers_both_columns() {
    let mut terminal_under_test = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    terminal_under_test.vt_write("x你y\n".as_bytes());
    terminal_under_test.flush();

    let results = search_viewport_matches(&terminal_under_test, "你");
    assert_eq!(results.len(), 1, "尾格不得另成命中，实际 {:?}", results);
    assert_eq!(results[0].start_col, 1);
    assert_eq!(results[0].end_col, 3);
}

/// 行内真实空格是用户写下的内容，必须可被查询命中（与未用单元的填充空格区分）。
#[test]
fn search_matches_real_spaces_between_cjk_characters() {
    let mut terminal_under_test = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    terminal_under_test.vt_write("你 好\n".as_bytes());
    terminal_under_test.flush();

    let results = search_viewport_matches(&terminal_under_test, "你 好");
    assert_eq!(results.len(), 1, "真实空格必须参与匹配，实际 {:?}", results);
    assert_eq!(results[0].start_col, 0);
    // 「你」列 0..2，空格列 2..3，「好」列 3..5。
    assert_eq!(results[0].end_col, 5);
}

/// 上游匹配语义：字节精确，仅 ASCII 字母不区分大小写。
///
/// 故 `Café` 命中（首字母 ASCII），`CAFÉ` 不命中（重音字母大小写不折叠）；
/// `cafe` 仍不命中 `café`（字节本就不同）。断言的是上游口径，不是本仓另造规则。
#[test]
fn search_folds_case_for_ascii_letters_only() {
    let mut terminal_under_test = GhosttyTerminal::new(10, 80, 100).expect("terminal");
    for line in ["Café", "café", "CAFÉ", "Čau", "čau", "Я", "я", "Σ", "σ"] {
        terminal_under_test.vt_write(format!("{line}\n").as_bytes());
    }
    terminal_under_test.flush();
    assert_eq!(
        search_viewport_matches(&terminal_under_test, "café").len(),
        2,
        "首字母为 ASCII：CAFÉ 之外的两行须命中"
    );
    assert_eq!(
        search_viewport_matches(&terminal_under_test, "čau").len(),
        1,
        "非 ASCII 首字母不折叠：Čau 行不得命中，仅小写行命中"
    );
    assert_eq!(
        search_viewport_matches(&terminal_under_test, "я").len(),
        1,
        "西里尔字母不折叠：仅小写行命中"
    );
    assert_eq!(
        search_viewport_matches(&terminal_under_test, "σ").len(),
        1,
        "希腊字母不折叠：仅小写行命中"
    );
    assert!(
        search_viewport_matches(&terminal_under_test, "cafe").is_empty(),
        "无重音不得命中重音文本"
    );
}
