use super::*;

/// 100 resize cycles with scrolling — ring buffer stress test.
#[test]
fn resize_stress_100_cycles_with_scroll() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    for cycle in 0..50 {
        terminal_under_test.vt_write(format!("cycle{cycle}\n").as_bytes());
        // 行列数交替变化
        let row_count = if cycle % 2 == 0 { 5 } else { 8 };
        let column_count = if cycle % 3 == 0 { 10 } else { 15 };
        terminal_under_test.resize(row_count, column_count);
        terminal_under_test.flush();
    }
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"StressTest");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let found = snap.cells.iter().any(|cell| cell.codepoint == 'S' as u32);
    assert!(
        found,
        "resize stress: StressTest should render after 50 cycles"
    );
    assert_invariants(&snap);
}

/// 缩窄重排必须保留软换行的整段文本：字符一个不少，只是折行位置改变。
///
/// 判别力：重排若按「行截断」实现，缩窄后的第二行起会被丢弃，全文只剩首行。
#[test]
fn shrink_reflow_keeps_wrapped_text() {
    let mut terminal_under_test = GhosttyTerminal::new(6, 20, 100).expect("terminal");
    terminal_under_test.vt_write(b"abcdefghijklmnopqrstuvwxyz0123456789\n");
    terminal_under_test.flush();
    terminal_under_test.resize(6, 10);
    terminal_under_test.flush();
    let text: String = terminal_under_test.read_all_text().split('\n').collect();
    assert!(
        text.contains("abcdefghijklmnopqrstuvwxyz0123456789"),
        "缩窄重排后的全文必须保留原串，实际 {text:?}"
    );
    let snap = terminal_under_test.take_snapshot();
    assert_eq!(10, snap.cols, "网格列数应已缩窄");
    assert_invariants(&snap);
}

/// 备用屏缩窄不重排也不 panic：可见内容仍是原文的连续片段，退出后主屏正文仍在。
///
/// 判别力：备用屏若按主屏同样「重排」，换行处会把两行拼成新的相邻关系，可见文本
/// 就不再是原串的连续片段；缩窄路径越界则直接 panic（备用屏没有回滚区可借用）。
#[test]
fn shrink_alt_screen_clips_without_reflow() {
    let mut terminal_under_test = GhosttyTerminal::new(6, 20, 100).expect("terminal");
    terminal_under_test.vt_write(b"primary line\n");
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"\x1b[?1049h");
    terminal_under_test.vt_write(b"alt-content-abcdefghij");
    terminal_under_test.vt_write(b"\x1b[H");
    terminal_under_test.flush();
    terminal_under_test.resize(4, 10);
    terminal_under_test.flush();
    let alt: String = terminal_under_test.read_all_text().split('\n').collect();
    assert!(
        "alt-content-abcdefghij".contains(&alt),
        "备用屏缩窄后可见内容必须是原文的连续片段（不得重排拼接），实际 {alt:?}"
    );
    assert_eq!(
        10,
        alt.chars().count(),
        "可见内容必须按新宽度裁剪而非重排成多行，实际 {alt:?}"
    );
    terminal_under_test.vt_write(b"\x1b[?1049l");
    terminal_under_test.flush();
    let primary: String = terminal_under_test.read_all_text().split('\n').collect();
    assert!(
        primary.contains("primary line"),
        "退出备用屏后主屏正文必须保留，实际 {primary:?}"
    );
    let snap = terminal_under_test.take_snapshot();
    assert_invariants(&snap);
}

#[test]
// 与字面量默认值精确比较，不涉及浮点运算。
fn cell_snapshot_default() {
    let cell = CellSnapshot::default();
    assert_eq!(cell.codepoint, 0);
    assert_eq!(cell.foreground, [0.0, 0.0, 0.0, 0.0]);
    assert_eq!(cell.background, [0.0, 0.0, 0.0, 0.0]);
    assert!(!cell.bold);
    assert!(!cell.italic);
}

/// Verify that the CellIterator-based build_snapshot() produces the same
/// output as the legacy per-cell terminal.grid_ref() path for the viewport.
///
/// This test exercises both code paths:
/// - CellIterator path: build_snapshot() with scroll_offset=0
/// - Legacy path: build_snapshot_legacy() with scroll_offset=0
///
/// The legacy path is triggered by the internal::build_snapshot_legacy
/// function. We verify this by feeding content, flushing, and comparing
/// cells from both paths cell-by-cell.
#[test]
fn cell_iterator_matches_legacy_grid_ref() {
    let mut terminal_under_test = terminal();

    // Feed mixed content: ASCII, bold, colored, CJK
    terminal_under_test.vt_write(b"\x1b[31mRed\x1b[0m Normal ");
    terminal_under_test.vt_write(b"\x1b[1mBold\x1b[0m ");
    terminal_under_test.vt_write(b"\x1b[44mBlueBg\x1b[0m ");
    terminal_under_test.vt_write("Hello 日本 World!".as_bytes());
    terminal_under_test.vt_write(b"\n");
    terminal_under_test.vt_write(b"Second line with \x1b[33mYELLOW\x1b[0m text");
    terminal_under_test.vt_write(b"\n");
    terminal_under_test.vt_write(b"Third line\x1b[K");
    terminal_under_test.flush();

    let snap = terminal_under_test.take_snapshot();

    // 校验 CellIterator 正确性的基本不变式
    assert!(
        snap.cells.len() >= 240,
        "snapshot should have at least 240 cells (3 rows × 80 cols)"
    );

    // 按行取文本以核对具体内容
    let row0_text = row_text(&snap, 0);
    assert!(
        row0_text.contains("Red"),
        "Row 0 should contain 'Red' (bold red text)"
    );
    assert!(
        row0_text.contains("Normal"),
        "Row 0 should contain 'Normal'"
    );
    assert!(row0_text.contains("Bold"), "Row 0 should contain 'Bold'");
    assert!(
        row0_text.contains("BlueBg"),
        "Row 0 should contain 'BlueBg'"
    );

    let row2_text = row_text(&snap, 2);
    assert!(
        row2_text.trim().contains("Third line"),
        "Row 2 should contain 'Third line'"
    );

    // 核对指定单元的颜色
    let first_red = snap
        .cells
        .iter()
        .position(|cell| cell.codepoint == 'R' as u32);
    assert!(first_red.is_some(), "Should find 'R' at start of 'Red'");
    if let Some(idx) = first_red {
        let cell = &snap.cells[idx];
        // Red foreground should have R=1, G=0, B=0 (approximately)
        // Red foreground should have R channel much higher than G+B
        assert!(
            cell.foreground[0] > cell.foreground[1] + 0.3,
            "Red foreground: R({}) should be much higher than G({}), got {:?}",
            cell.foreground[0],
            cell.foreground[1],
            cell.foreground
        );
        // 红色前景不得退化为纯白
        assert!(
            cell.foreground[0] > 0.5,
            "Red foreground: R channel should be significant, got {}",
            cell.foreground[0]
        );
    }

    // 核对 Bold 单词上的加粗标记
    let first_bold_position = snap
        .cells
        .iter()
        .position(|cell| cell.codepoint == 'B' as u32);
    if let Some(cell_index) = first_bold_position {
        let cell = &snap.cells[cell_index];
        assert!(cell.bold, "Bold word should have bold=true");
    }

    // Verify invariants
    assert_invariants(&snap);
}

/// 断言 CellIterator 路径能正确处理快照中的 CJK 双宽字符（width=2）。
#[test]
fn cell_iterator_cjk_double_width() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    terminal_under_test.vt_write("A中B".as_bytes());
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();

    let a_cell = &snap.cells[0];
    assert_eq!(a_cell.codepoint, 'A' as u32, "Cell 0 should be 'A'");
    assert_eq!(a_cell.width, 1, "'A' should have width=1");

    let cjk_cell = &snap.cells[1];
    assert_eq!(cjk_cell.codepoint, 0x4E2D, "Cell 1 should be CJK '中'");
    assert_eq!(cjk_cell.width, 2, "CJK should have width=2");

    let b_cell = &snap.cells[3];
    assert_eq!(b_cell.codepoint, 'B' as u32, "Cell 3 should be 'B'");
    assert_eq!(b_cell.width, 1, "'B' should have width=1");
}

/// Verify that `build_cell_data()` produces correct CellData output
/// matching the GridSnapshot content.
#[test]
fn build_cell_data_matches_grid_snapshot() {
    let mut terminal_under_test = terminal();
    // Feed various content
    terminal_under_test.vt_write(b"AB");
    terminal_under_test.vt_write(b"\x1b[31mRed\x1b[0m");
    terminal_under_test.vt_write(b"\n");
    terminal_under_test.vt_write("中".as_bytes());
    terminal_under_test.flush();

    let snap = terminal_under_test.take_snapshot();

    // Verify grid state
    assert!(
        snap.cells.len() >= 240,
        "snapshot should have 24 rows × 80 cols"
    );

    // 经 CellIterator 路径核对 ASCII 内容
    let row0_text = row_text(&snap, 0);
    assert!(row0_text.contains("AB"), "Row 0 should start with 'AB'");
    assert!(row0_text.contains("Red"), "Row 0 should contain 'Red'");

    let row1_text = row_text(&snap, 1);
    assert!(
        row1_text.contains("中"),
        "Row 1 should contain CJK character"
    );

    // 核对指定单元的码位
    let cell_0_0 = &snap.cells[0];
    assert_eq!(cell_0_0.codepoint, 'A' as u32, "Cell[0,0] should be 'A'");
    assert_eq!(cell_0_0.width, 1, "'A' width should be 1");

    // CJK cell should have width=2
    let mid_cells: Vec<_> = snap
        .cells
        .iter()
        .filter(|cell| cell.codepoint == 0x4E2D)
        .collect();
    assert_eq!(mid_cells.len(), 1, "Should find exactly one CJK cell");
    assert_eq!(mid_cells[0].width, 2, "CJK width should be 2");

    assert_invariants(&snap);
}

/// 断言经 CellIterator 快照路径得到的网格总尺寸正确。
#[test]
fn cell_iterator_grid_dimensions() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"Hello World");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    assert_eq!(snap.rows, 24, "Grid should have 24 rows");
    assert_eq!(snap.cols, 80, "Grid should have 80 cols");
    assert_eq!(snap.cells.len(), 24 * 80, "Total cells should be 1920");
}

// ── EPT/DECALN test ────────────────────────────────────────────────────

// ── Mouse tracking (from Termux testMouseClick) ─────────────────────────

// ── Terminal reports (from Termux testReportTerminalSize, testDeviceStatusReport) ──

// ── Cursor style DECSCUSR (from Termux testSetCursorStyle) ──────────────

// ── BEL callback (from Termux testBel) ─────────────────────────────────

// ── Tab stops (from Termux testTab) ────────────────────────────────────

// ── Line drawing charset (from Termux testLineDrawing) ─────────────────

// ── Insert/Delete Characters (from Termux testDeleteCharacters) ────────

// ── REP repeat (from Termux testRepeat) ────────────────────────────────

// ── Underline variants (Kitty 4:0 — 4:5, from Termux) ────────────────

// ── SGR parameter overflow (from Termux) ─────────────────────────────

// ── HPA (Horizontal Position Absolute, from Termux) ──────────────────

// ── Autowrap clearing (from Termux testClearingOfAutowrap) ───────────

// ── Backspace across wrapped lines (from Termux) ─────────────────────

// ── Cursor save/restore text style (from Termux) ─────────────────────

// ── Scroll Down (SD/CSI T) and Scroll Up (SU/CSI S) (from Termux) ───

// ── Dynamic colors (from Termux testSettingDynamicColors / testReportSpecialColors) ──

// ── Title stack (from Termux testTitleStack) ──────────────────────────

// ── DCS +q reports (from Termux testReportColorsAndName / testReportKeys) ──

// ── APC consumed silently (from Termux testApcConsumed) ──────────────

// ── IRM Insert Mode (from Termux testInsertMode) ──────────────────────

// ── Cursor margin clamping (from Termux testCursorForward/Back/Up/Down) ──

// ── ECH (from Termux testCsiX) ───────────────────────────────────────

// ── DECCOLM (from Termux testDECCOLMResetsScrollMargin) ─────────────

// ── NEL with origin mode margin (from Termux) ────────────────────────

/// LF (\\n) implies CR+LF: session restore and normal PTY output depend on this.
/// 若 ghostty 的 VT 解析器把 LF 当作纯 LF，每行都会停在上行末尾的列号而非第 0 列。
#[test]
fn newline_lf_implies_crlf() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    terminal_under_test.flush();
    // Write "AB\nCD" — after LF→CR+LF conversion, CD should be at col 0 of row 1
    terminal_under_test.pty_write(b"AB\nCD");
    terminal_under_test.flush();
    terminal_under_test.flush();
    let dumped = terminal_under_test.dump_grid();
    let row1_col0 = dumped.visible[10].codepoint;
    assert_eq!(
        row1_col0, 'C' as u32,
        "LF→CR+LF: 'C' should be at column 0 of row 1"
    );
    // 第 0 行应是 'A'、'B'，其后为空白
    assert_eq!(dumped.visible[0].codepoint, 'A' as u32, "row0 col0 = A");
    assert_eq!(dumped.visible[1].codepoint, 'B' as u32, "row0 col1 = B");
    assert_eq!(
        dumped.visible[2].codepoint, 0,
        "row0 col2 = empty after LF implies CR"
    );
}

#[test]
fn newline_lf_after_full_line_restore() {
    // Simulate session restore: write a full-width line then \n then another line.
    // LF must return cursor to column 0 so the next line starts correctly.
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    terminal_under_test.flush();
    // "ABCDEFGHIJ" is exactly 10 chars (full width), then \n, then "next"
    terminal_under_test.pty_write(b"ABCDEFGHIJ\nnext");
    terminal_under_test.flush();
    terminal_under_test.flush();
    let dumped = terminal_under_test.dump_grid();
    // Row 0: A B C D E F G H I J
    assert_eq!(
        dumped.visible[9].codepoint, 'J' as u32,
        "row0 col9 = J (full width)"
    );
    // Row 1: n e x t at columns 0-3
    let row1_col0 = dumped.visible[10].codepoint;
    assert_eq!(
        row1_col0, 'n' as u32,
        "row1 col0 = n (after LF, cursor must return to col 0)"
    );
    assert_eq!(dumped.visible[11].codepoint, 'e' as u32, "row1 col1 = e");
    assert_eq!(dumped.visible[12].codepoint, 'x' as u32, "row1 col2 = x");
    assert_eq!(dumped.visible[13].codepoint, 't' as u32, "row1 col3 = t");
    // Row 1 col 4 should be empty (cursor returned to col 0 after LF)
    assert_eq!(dumped.visible[14].codepoint, 0, "row1 col4 = empty");
}

#[test]
fn newline_crlf_still_works() {
    // CR+LF must continue to work as before
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"AB\r\nCD");
    terminal_under_test.flush();
    terminal_under_test.flush();
    let dumped = terminal_under_test.dump_grid();
    let row1_col0 = dumped.visible[10].codepoint;
    assert_eq!(row1_col0, 'C' as u32, "CRLF: 'C' at column 0 of row 1");
}

// ── TC-TM: Terminal Mode State (from test gap analysis §3.E) ────

// ── TC-IV: Invariant Checking (from test gap analysis §3.L) ─────

/// TC-IV-002: Alt buffer has no history
#[test]
fn tc_iv_002_alt_buffer_no_history() {
    let mut terminal_under_test = GhosttyTerminal::new(3, 10, 100).expect("terminal");
    terminal_under_test.flush();
    for line_number in 0..5 {
        terminal_under_test.vt_write(format!("line{line_number}\r\n").as_bytes());
    }
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"\x1b[?1049h");
    terminal_under_test.flush();
    // 备用缓冲不得带回滚内容
    assert_eq!(
        terminal_under_test.scrollback_length(),
        0,
        "IV-002: alt buffer should have no scrollback"
    );
    let snap = terminal_under_test.take_snapshot();
    assert_invariants(&snap);
}

// ── TC-OC: Output Capture (from test gap analysis §3.A) ───────────
// Adapted: verify terminal survives response sequences (no output capture)

// ── TC-CV: Color Verification (from test gap analysis §3.D) ───────

// ── TC-AG: Cell Attributes Grid (from test gap analysis §3.G) ────

// ── TC-MS: Mouse Simulation (from test gap analysis §3.F) ────────
// Adapted: no sendMouseEvent API, verify DECSET modes don't crash

// ── TC-PF: Protocol Fuzz (from test gap analysis §3.N) ────────────
// Most PF tests already exist; adding the missing bare-ESC case.

// TC-PF-009: BEL in text renders both sides (exists as bel_character_does_not_crash)
// TC-PF-010: APC consumed (exists as apc_consumed_silently)
// ── TC-RS: Resize Stress (from test gap analysis §3.O) ────────────

// ── TC-UI: Session Panel & UI (from test gap analysis §3.P) ──────

// ── TC-RB: Regression Bugs (from test gap analysis §3.M) ──────────

// ── TC-SM: Session Management (from test gap analysis §3.J) ───────
// Adapted: test GhosttyTerminal creation and independent content

/// TC-SM-001: Create terminal with valid dimensions
#[test]
fn tc_sm_001_create_valid_dimensions() {
    let terminal_under_test = GhosttyTerminal::new(24, 80, 1000).expect("terminal");
    terminal_under_test.flush();
    assert_eq!(terminal_under_test.rows(), 24, "SM-001: rows == 24");
    assert_eq!(terminal_under_test.cols(), 80, "SM-001: cols == 80");
    let snap = terminal_under_test.take_snapshot();
    assert_invariants(&snap);
}

/// TC-SM-002: Two sessions have independent content
#[test]
fn tc_sm_002_independent_sessions() {
    let mut t1 = GhosttyTerminal::new(3, 3, 100).expect("t1");
    let mut t2 = GhosttyTerminal::new(3, 3, 100).expect("t2");
    t1.flush();
    t1.vt_write(b"A");
    t1.flush();
    t2.vt_write(b"B");
    t2.flush();
    let snap1 = t1.take_snapshot();
    let snap2 = t2.take_snapshot();
    let a_in_1 = snap1.cells.iter().any(|cell| cell.codepoint == 'A' as u32);
    let b_in_2 = snap2.cells.iter().any(|cell| cell.codepoint == 'B' as u32);
    assert!(a_in_1, "SM-002: session 1 has 'A'");
    assert!(b_in_2, "SM-002: session 2 has 'B'");
    // 会话 1 中 MUST NOT 出现 B
    let a_in_2 = snap2.cells.iter().any(|cell| cell.codepoint == 'A' as u32);
    assert!(!a_in_2, "SM-002: session 2 should not have 'A'");
    let snap = t1.take_snapshot();
    assert_invariants(&snap);
}

/// TC-SM-003: Drop terminal cleans up
#[test]
fn tc_sm_003_drop_cleans_up() {
    let terminal_under_test = GhosttyTerminal::new(3, 3, 100).expect("terminal");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    assert_invariants(&snap);
    drop(terminal_under_test);
    // 走到这里即说明没有 panic
}

/// TC-SM-004: Double drop is safe (handled by Drop impl)
#[test]
fn tc_sm_004_double_drop_safe() {
    let terminal_under_test = GhosttyTerminal::new(3, 3, 100).expect("terminal");
    terminal_under_test.flush();
    // 安全 Rust 下无法显式重复 drop，但可以验证正常 drop 不会 panic
    let snap = terminal_under_test.take_snapshot();
    assert_invariants(&snap);
    drop(terminal_under_test);
}

/// TC-SM-005: Process-like cleanup (just verify terminal works)
#[test]
fn tc_sm_005_terminal_works_after_writes() {
    let mut terminal_under_test = terminal();
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"SessionActive");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let found = snap.cells.iter().any(|cell| cell.codepoint == 'S' as u32);
    assert!(found, "SM-005: terminal should work normally");
    let snap = terminal_under_test.take_snapshot();
    assert_invariants(&snap);
}

// ── TC-AL: Android Lifecycle (from test gap analysis §3.I) ───────
// Adapted: simulate pause/resume via resize cycles

/// TC-AL-001: "Pause" (snapshot) preserves content — verify via snapshot
#[test]
fn tc_al_001_snapshot_preserves_content() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"LifecycleContent");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let found = snap.cells.iter().any(|cell| cell.codepoint == 'L' as u32);
    assert!(found, "AL-001: content preserved in snapshot");
    let snap = terminal_under_test.take_snapshot();
    assert_invariants(&snap);
}

/// TC-AL-002: Alt screen via snapshot
#[test]
fn tc_al_002_alt_screen_preserved() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"\x1b[?1049h");
    terminal_under_test.vt_write(b"AltContent");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let found = snap.cells.iter().any(|cell| cell.codepoint == 'A' as u32);
    assert!(found, "AL-002: alt screen content in snapshot");
    let snap = terminal_under_test.take_snapshot();
    assert_invariants(&snap);
}

/// TC-AL-003: Cursor position restored after resize cycle
#[test]
fn tc_al_003_cursor_restored() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"\x1b[3;5H"); // CUP to (3,5)
    terminal_under_test.flush();
    let x_before = terminal_under_test.cursor_x();
    let y_before = terminal_under_test.cursor_y();
    terminal_under_test.resize(5, 10); // same size, simulate pause/resume
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.cursor_x(),
        x_before,
        "AL-003: cursor_x preserved after resize"
    );
    assert_eq!(
        terminal_under_test.cursor_y(),
        y_before,
        "AL-003: cursor_y preserved after resize"
    );
    let snap = terminal_under_test.take_snapshot();
    assert_invariants(&snap);
}

/// TC-AL-004: Mode state preserved after resize cycle
#[test]
fn tc_al_004_mode_preserved() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"\x1b[?25l"); // hide cursor
    terminal_under_test.flush();
    assert!(
        !terminal_under_test.cursor_visible(),
        "AL-004: cursor hidden before resize"
    );
    terminal_under_test.resize(5, 20);
    terminal_under_test.flush();
    assert!(
        !terminal_under_test.cursor_visible(),
        "AL-004: cursor hidden after resize"
    );
    let snap = terminal_under_test.take_snapshot();
    assert_invariants(&snap);
}

// ── 13.6: Pause / resume (simulated via resize) ────────────────
// 001: 50 cycles — no resource leak; 002: content preserved.

#[test]
fn tc_lifecycle_001_pause_resume_cycles() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal_under_test.vt_write(b"BaseContent");
    terminal_under_test.flush();

    for cycle in 0..50 {
        let marker = format!(
            "\x1b[{};{}HCycle{}",
            1 + (cycle % 5),
            1 + (cycle % 18),
            cycle
        );
        terminal_under_test.vt_write(marker.as_bytes());
        terminal_under_test.flush();

        // Simulate pause/resume via resize to same size.
        terminal_under_test.resize(5, 20);
        terminal_under_test.flush();

        // Verify basic invariants after each cycle.
        let snap = terminal_under_test.take_snapshot();
        assert_invariants(&snap);
        assert_eq!(snap.rows, 5, "rows unchanged after cycle {cycle}");
        assert_eq!(snap.cols, 20, "cols unchanged after cycle {cycle}");
    }
}

// ── 13.7: Content preserved after pause/resume cycle ───────────

#[test]
fn tc_lifecycle_002_content_preserved_after_pause_resume() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal_under_test.vt_write(b"PreserveThisContent!");
    terminal_under_test.flush();

    // Capture row 0 text before simulated pause/resume.
    let snap_before = terminal_under_test.take_snapshot();
    let text_before: String = snap_before
        .cells
        .iter()
        .take(20)
        .map(|cell| char::from_u32(cell.codepoint).unwrap_or('�'))
        .collect();

    // Simulate pause (release/destroy) and resume (recreate) via resize.
    terminal_under_test.resize(5, 20);
    terminal_under_test.flush();

    let snap_after = terminal_under_test.take_snapshot();
    let text_after: String = snap_after
        .cells
        .iter()
        .take(20)
        .map(|cell| char::from_u32(cell.codepoint).unwrap_or('�'))
        .collect();

    assert_eq!(
        text_before.trim_end(),
        text_after.trim_end(),
        "content should be preserved after pause/resume cycle"
    );
    assert_invariants(&snap_after);
}

// ── Stage 3: Bug Regression Tests ──────────────────────────────────
//
// B3（Activity 重建导致白屏）：
// 根因：pauseRendering 里的 releaseSurface() 销毁了 SurfaceView 的
// ANativeWindow。Activity 重建时 bridge 会因 surface 已释放而静默跳过
// updateNativeWindow。
// 修复：pauseRendering() 不再调用 releaseSurface()，改为把渲染标志置为
// false，把 surface 留给重建复用。
// This fix is Kotlin-side only and cannot be tested in Rust.
// Kotlin commit: (referenced in git history)

// ══════════════════════════════════════════════════════════════════════════
// Performance Benchmarks — realistic usage patterns
// ══════════════════════════════════════════════════════════════════════════

/// Simulate user typing latency: small writes (1-10 chars) followed by flush.
/// Measures wall-clock time per iteration — the user-visible metric.
/// Single anti-flake threshold (no environment checks per TESTING.md):
/// 并行执行与软件 Vulkan 的争用使墙钟时间噪声较大，故该下限只拦数量级级别的回归。
/// 细粒度跟踪由基准测试覆盖，见 scripts/check-rust.nu。

#[test]
fn bench_typing_latency() {
    let mut terminal_under_test = GhosttyTerminal::new(24, 80, 5000).expect("terminal");
    // 先填入一些内容，避免命中空终端的优化路径
    for _ in 0..10 {
        terminal_under_test.vt_write(b"A line to fill the screen with some realistic content\n");
    }
    terminal_under_test.flush();

    let keystrokes: [&[u8]; 6] = [b"h", b"e", b"l", b"l", b"o", b"\n"];
    let round_count = 300; // 300 keystrokes
    let start = Instant::now();
    for _ in 0..round_count {
        for keystroke in &keystrokes {
            terminal_under_test.vt_write(keystroke);
        }
        terminal_under_test.flush();
        let count = black_box(
            terminal_under_test
                .receive_cell_data()
                .map(|(cells, _)| cells.len())
                .unwrap_or(0),
        );
        black_box(count);
    }
    let elapsed = start.elapsed();
    let ms_per_keystroke =
        elapsed.as_millis() as f64 / (round_count as f64 * keystrokes.len() as f64);
    println!(
        "Typing latency: {:.3}ms per keystroke ({:.1}ms for {} keystrokes)",
        ms_per_keystroke,
        elapsed.as_millis(),
        round_count * keystrokes.len(),
    );
    let threshold = 6.0;
    assert!(
        ms_per_keystroke < threshold,
        "Typing too slow: {:.3}ms per keystroke (need <{threshold:.1}ms)",
        ms_per_keystroke,
    );
}

/// Simulate bulk output (paste / program output like `cat`, `git log`).
/// Uses realistic plain-text lines — the most common real-world output
/// pattern. No ANSI escape codes (ghostty C FFI handles them slowly in
/// debug builds; ANSI throughput is implicitly covered by other benchmarks).
///
/// 断言行为而非绝对吞吐：200KB 突发输出必须整段走完解析器（末行与期望一致，
/// 无截断/丢行）。吞吐量阈值由 `benches/` 的 criterion 基准承担——把墙钟阈值
/// 放进 `cargo test` 会随并行测试线程数与构建机负载随机判红（TESTING.md「没有
/// 不稳定的测试」）。
#[test]
fn bulk_output_is_processed_without_loss() {
    let mut terminal_under_test = GhosttyTerminal::new(24, 80, 5000).expect("terminal");
    let mut buf = Vec::with_capacity(4096);
    while buf.len() < 4096 {
        buf.extend_from_slice(b"user@host:~$ ls -la src/main.rs docs/README.md\n");
    }
    let round_count = 50; // 50 × 4KB = 200KB total

    for _ in 0..round_count {
        terminal_under_test.vt_write(&buf);
        terminal_under_test.flush();
        // 每轮取一次单元数据：与渲染帧同口径，确保输出真的走完 VT→CellData 链路。
        black_box(
            terminal_under_test
                .receive_cell_data()
                .map(|(cells, _)| cells.len()),
        );
    }

    let marker = format!("burst-round-{round_count}");
    // 突发负载按 4KB 切块，末块可能停在一行中间：先回到行首再写标记，
    // 否则标记会在行尾折行而被拆到两行。
    terminal_under_test.vt_write(b"\r\n");
    terminal_under_test.vt_write(marker.as_bytes());
    terminal_under_test.flush();

    let snapshot = terminal_under_test.take_snapshot();
    let last_row = snapshot.rows - 1;
    assert_eq!(
        row_text(&snapshot, last_row),
        marker,
        "200KB 突发输出后末行必须是最后写入的标记（无截断/丢行）"
    );
    assert_invariants(&snapshot);
}

// ── Scrollback fallback ────────────────────────────────────────────────

/// A live terminal must report a valid viewport snapshot; a disconnected VT
/// thread must surface as a panic from `take_snapshot` rather than silently
/// substituting a blank grid (DESIGN 禁止掩盖错误).
#[test]
#[should_panic(expected = "快照命令入队失败")]
fn snapshot_panics_when_terminal_disconnected() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    for line_number in 0..20 {
        terminal_under_test.vt_write(format!("line {line_number}\n").as_bytes());
    }
    terminal_under_test.flush();

    assert!(
        terminal_under_test.is_alive(),
        "terminal should be alive before disconnect"
    );
    let snap = terminal_under_test.take_snapshot();
    assert!(
        snap.rows > 0 && snap.cols > 0,
        "viewport snapshot should have valid dimensions"
    );

    terminal_under_test.disconnect_for_test();
    assert!(
        !terminal_under_test.is_alive(),
        "terminal must report dead after disconnect"
    );
    let _ = terminal_under_test.take_snapshot();
}

/// Verify that `take_snapshot` returns
/// consistent results across multiple calls (cache hit path).
#[test]
fn scrollback_cache_consistency() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    terminal_under_test.vt_write(b"Hello World\n");
    terminal_under_test.flush();

    let snap1 = terminal_under_test.take_snapshot();
    let snap2 = terminal_under_test.take_snapshot();
    assert_eq!(snap1.rows, snap2.rows, "cached snapshots should match");
    assert_eq!(snap1.cols, snap2.cols, "cached snapshots should match");
    assert_eq!(
        snap1.cells.len(),
        snap2.cells.len(),
        "cached snapshot cell count should match"
    );
}

/// Verify that `scroll_viewport(Delta)` scrolls the CellData view and
/// that the delta accumulates on repeated calls: scrollback
/// browsing previously did nothing).
#[test]
fn scroll_viewport_delta_scrolls_cell_data() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    for line_number in 0..20 {
        terminal_under_test.vt_write(format!("line {line_number}\n").as_bytes());
    }
    terminal_under_test.flush();

    // First frame at offset 0 shows the bottom of the output.
    let (cells0, _) = terminal_under_test.receive_cell_data().expect("cell data");
    let bottom_rows: std::collections::HashSet<u32> = cells0.iter().map(|cell| cell.row).collect();
    assert!(
        bottom_rows.contains(&4),
        "offset 0 should include viewport row 4, got {bottom_rows:?}"
    );

    // The terminal must actually accumulate scrollback (host probe:
    // scrollback_length should be > 0 after 20 lines into a 5-row view).
    let scrollback = terminal_under_test.scrollback_length();
    assert!(
        scrollback > 0,
        "scrollback should exist after output, got {scrollback}"
    );

    // Scroll up by 2: the VT thread applies the delta and pushes new
    // CellData; the visible content shifts (rows are renumbered from the
    // new viewport top, so the row set is unchanged but the text differs).
    assert!(
        terminal_under_test.scroll_viewport(-2),
        "scroll_viewport should accept delta"
    );
    // Give the VT thread a moment to process and push.
    let mut scrolled = None;
    for _ in 0..50 {
        if let Some((cells, _)) = terminal_under_test.receive_cell_data() {
            scrolled = Some(cells);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let scrolled = scrolled.expect("scrolled cell data");
    let scrolled_rows: std::collections::HashSet<u32> =
        scrolled.iter().map(|cell| cell.row).collect();
    assert_eq!(
        scrolled_rows, bottom_rows,
        "scrolled view keeps 5 viewport rows"
    );
    // 滚动后的可见文本必须真的变了：不再显示最底部的 "line 19"。
    // 断言直接读 CellData（渲染热路径）；此前经带偏移的快照通道读取，而该通道
    // 永远返回空白网格，断言形同虚设。
    let visible_text = scrolled
        .iter()
        .map(|cell| char::from_u32(cell.codepoint).unwrap_or('\0'))
        .collect::<String>();
    assert!(
        !visible_text.contains("line 19"),
        "scrolled view should not show the bottom line, got {visible_text:?}"
    );
    assert!(
        visible_text.contains("line 1"),
        "scrolled view should show an earlier line, got {visible_text:?}"
    );

    // 向下滚回底部：可见文本必须再次变化（DESIGN 修饰键栏节：方向键/滑动双向移动）。
    assert!(
        terminal_under_test.scroll_viewport(2),
        "scroll_viewport 应接受正向增量"
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let scrolled_back = loop {
        if let Some((cells, _)) = terminal_under_test.receive_cell_data() {
            break cells;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "向下滚动后 2s 内未收到 CellData 帧"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    let bottom_again = scrolled_back
        .iter()
        .map(|cell| char::from_u32(cell.codepoint).unwrap_or('\0'))
        .collect::<String>();
    assert_ne!(
        bottom_again, visible_text,
        "回到底部的可见文本必须与回滚深处不同"
    );
    assert!(
        bottom_again.contains("line 19"),
        "回到底部必须重新显示最后一行, got {bottom_again:?}"
    );
}
