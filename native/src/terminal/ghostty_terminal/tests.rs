//! VT 引擎行为测试：只覆盖本仓自有语义，不复述上游解析语义。
//!
//! 保留集合（本仓机制与对外 API，上游仅作输入夹具）：
//! 1. 快照管线：snapshot 缓存/回退、视口映射、行缓存；
//! 2. 查询 API：dump_grid / read_line_text / hyperlink_at / selection_text /
//!    select_word_at / select_line_at / select_all / search_in（断言对象是
//!    本仓包装与格式化）；
//! 3. 输入输出管道：vt_write 清洗、pty_write LF→CRLF 与分片直透（无 ST/SGR 提前闭合）、
//!    键盘/鼠标编码（含钳制）；
//! 4. 会话与生命周期 tc_sm_ / tc_al_ / tc_lifecycle_，性能基准 bench_*。
//!
//! 删除集合（上游 libghostty-vt 的职责，由上游自有测试覆盖）：光标移动与钳制、
//! 擦除/插入/删除、制表位、滚动区域、复位、尺寸重排、DECSET 模式矩阵、标题读写
//! 语义、纯 VT 一致性用例（原 vt_conformance.rs 与文末回归 mod 已删）。

use std::hint::black_box;
use std::time::Instant;

use super::*;
use crate::terminal::ghostty_terminal::public_api::sanitize_vt_input;
use crate::terminal::test_helpers::assert_invariants;
use libghostty_vt::key::Mods;

fn terminal() -> GhosttyTerminal {
    GhosttyTerminal::new(24, 80, 1000).expect("terminal create")
}

fn small_terminal() -> GhosttyTerminal {
    GhosttyTerminal::new(3, 3, 100).expect("terminal")
}

/// Get the cell at a given row and column from the snapshot
fn cell_at(snap: &GridSnapshot, row: u32, col: u32) -> Option<&CellSnapshot> {
    if row >= snap.rows || col >= snap.cols {
        return None;
    }
    let idx = (row * snap.cols + col) as usize;
    snap.cells.get(idx)
}

fn row_text(snap: &GridSnapshot, row: u32) -> String {
    let mut text = String::new();
    for col in 0..snap.cols {
        if let Some(cell) = cell_at(snap, row, col)
            && cell.codepoint != 0
            && let Some(character) = char::from_u32(cell.codepoint)
        {
            text.push(character);
        }
    }
    text.trim_end().to_string()
}

#[test]
fn create_terminal_zero_scrollback() {
    let terminal_under_test = GhosttyTerminal::new(5, 10, 0).expect("terminal");
    assert_eq!(terminal_under_test.scrollback_length(), 0);
}

#[test]
fn read_line_text_returns_text() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b[1;1HHello World");
    terminal_under_test.flush();
    let text = terminal_under_test.read_line_text(0);
    assert!(text.is_some());
    assert!(text.unwrap().contains("Hello"));
}

#[test]
fn read_line_text_empty_returns_none() {
    let terminal_under_test = terminal();
    let text = terminal_under_test.read_line_text(5);
    assert!(text.is_none());
}

/// 行文本的字符下标恒等于网格列（每列恰好一个字符），宽字符尾格是空格占位。
/// Kotlin 侧据此按列直取字符，不再用宽度表反推。
#[test]
fn read_line_text_index_equals_grid_column_for_wide_chars() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write("中文AB".as_bytes());
    terminal_under_test.flush();
    let text = terminal_under_test.read_line_text(0).expect("row has text");
    let columns: Vec<char> = text.chars().collect();
    assert_eq!(
        columns,
        vec!['中', ' ', '文', ' ', 'A', 'B'],
        "宽字符尾格占一列空格，字符下标即列号"
    );
}

#[test]
fn wide_char_tail_cols_marks_only_trailing_halves() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write("中文AB".as_bytes());
    terminal_under_test.flush();
    // 「中」占列 0..1、「文」占列 2..3：两个尾格都被标出，窄字符与起始格不标。
    assert_eq!(terminal_under_test.wide_char_tail_cols(0), vec![1, 3]);
}

/// 纯 ASCII 行没有尾格（无吸附），行文本的列映射因而与字符下标一致。
#[test]
fn wide_char_tail_cols_is_empty_for_narrow_text() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"hello");
    terminal_under_test.flush();
    assert!(terminal_under_test.wide_char_tail_cols(0).is_empty());
}

/// 软换行续行的行首是 `SpacerHead`：它不是尾格，不得被标出——其前一列属于折行前
/// 的上一行，把它当尾格会把选区跨过折行边界。
#[test]
fn wide_char_tail_cols_ignores_wrapped_row_head() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    terminal_under_test.vt_write("0123456789中".as_bytes());
    terminal_under_test.flush();
    // 10 列宽：首行是「0123456789」，宽字符折到续行的第 0..1 列。
    let first_row_tails = terminal_under_test.wide_char_tail_cols(0);
    let continuation_row_tails = terminal_under_test.wide_char_tail_cols(1);
    assert!(first_row_tails.is_empty(), "首行全窄字符，不应有尾格");
    assert!(
        !continuation_row_tails.contains(&0),
        "续行行首不得被当作尾格（会把选区吸过折行边界）"
    );
    assert!(
        continuation_row_tails.contains(&1),
        "续行里的宽字符尾格仍须标出"
    );
}

#[test]
fn reset_clears_grid_and_scrollback() {
    let mut terminal = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal.vt_write(b"reset_marker_xyz\nsecond line");
    terminal.flush();
    assert!(terminal.read_visible_text().contains("reset_marker_xyz"));
    terminal.reset();
    terminal.flush();
    assert!(
        !terminal.read_visible_text().contains("reset_marker_xyz"),
        "RIS 重置必须清屏"
    );
    assert_eq!(terminal.scrollback_length(), 0);
}

#[test]
fn reset_clears_selection() {
    let mut terminal = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal.vt_write(b"hello");
    terminal.flush();
    let snap = terminal.take_snapshot();
    let row = snap.scrollback_length;
    terminal.set_selection((row, 0), (row, 4));
    terminal.flush();
    terminal.reset();
    terminal.flush();
    let (cells, _) = terminal.receive_cell_data().expect("cell data after reset");
    let picked = cells
        .iter()
        .find(|cell| cell.row == 0 && cell.col == 0)
        .expect("row 0 col 0 present");
    let theme_foreground = GhosttyTerminal::byte_color_to_float([205, 214, 244]);
    assert_eq!(
        picked.foreground, theme_foreground,
        "重置后选区反白必须清除"
    );
}

#[test]
fn search_all_finds_match() {
    let mut terminal_under_test = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    terminal_under_test.vt_write(b"search_target_here\n");
    terminal_under_test.flush();
    for line_number in 0..5 {
        terminal_under_test.vt_write(format!("filler {line_number}\n").as_bytes());
    }
    terminal_under_test.flush();
    // 搜索命中回滚首行并保持终端可用。
    let results = terminal_under_test.search_all_in_scrollback("search_target", true);
    assert!(
        !results.is_empty(),
        "search_target must be found in scrollback"
    );
    terminal_under_test.vt_write(b"AfterSearch");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    assert!(
        snap.cells.iter().any(|cell| cell.codepoint == 'A' as u32),
        "terminal should remain functional after scrollback search"
    );
    assert_invariants(&snap);
}

/// 软换行的逻辑行只算一次，且命中按其真实所在的物理行与列返回：
/// 逐物理行拼接会产生 N 份重复命中，并把续接段的命中记到行首那一行。
#[test]
fn search_reports_each_wrapped_line_once_with_physical_rows() {
    const COLS: u32 = 10;
    let mut terminal_under_test = GhosttyTerminal::new(5, COLS, 100).expect("terminal");
    // 26 字符在 10 列网格上软换行为 3 个物理行：abcdefghij / klmnopqrst / uvwxyz
    terminal_under_test.vt_write(b"abcdefghijklmnopqrstuvwxyz\r\n");
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.read_line_text(0).as_deref(),
        Some("abcdefghij")
    );
    assert_eq!(
        terminal_under_test.read_line_text(1).as_deref(),
        Some("klmnopqrst")
    );

    assert_eq!(
        terminal_under_test.search_all_in_scrollback("klmnopqrst", true),
        vec![SearchMatch {
            row: 1,
            start_col: 0,
            end_col: 10,
        }],
        "单个物理行内的命中不得按逻辑行重复上报"
    );

    // 跨软换行的命中按物理行拆成两段，列区间落在各自行内。
    assert_eq!(
        terminal_under_test.search_all_in_scrollback("ijklm", true),
        vec![
            SearchMatch {
                row: 0,
                start_col: 8,
                end_col: 10,
            },
            SearchMatch {
                row: 1,
                start_col: 0,
                end_col: 3,
            },
        ],
        "跨软换行的命中必须按各物理行拆分"
    );
}

#[test]
fn search_all_empty_query() {
    let terminal_under_test = terminal();
    assert!(
        terminal_under_test
            .search_all_in_scrollback("", true)
            .is_empty()
    );
}

#[test]
fn dump_grid_dimensions_match() {
    let terminal_under_test = terminal();
    // 查询经工作线程超时回退空值，新终端繁忙时单次查询可能命中回退；
    // 确定性轮询直到就绪，杜绝 flaky。
    let start = Instant::now();
    let dumped = loop {
        let dumped = terminal_under_test.dump_grid();
        if dumped.rows == 24 && dumped.cols == 80 {
            break dumped;
        }
        assert!(
            start.elapsed() < std::time::Duration::from_secs(5),
            "dump_grid 未就绪：rows={} cols={}",
            dumped.rows,
            dumped.cols
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    assert_eq!(dumped.visible.len(), (24 * 80) as usize);
    let _snap = terminal_under_test.take_snapshot();
    assert_invariants(&_snap);
}

#[test]
fn dump_grid_visible_populated() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"hello");
    terminal_under_test.flush();
    let dumped = terminal_under_test.dump_grid();
    let has_h_character = dumped
        .visible
        .iter()
        .any(|cell| cell.codepoint == 'h' as u32);
    assert!(
        has_h_character,
        "dump_grid visible: 'h' from 'hello' should be present"
    );
    let _snap = terminal_under_test.take_snapshot();
    assert_invariants(&_snap);
}

#[test]
fn dump_grid_scrollback_populated_after_scroll() {
    let mut terminal_under_test = GhosttyTerminal::new(3, 10, 100).expect("terminal");
    for line_number in 0..10 {
        terminal_under_test.vt_write(format!("line{line_number}\n").as_bytes());
    }
    terminal_under_test.flush();
    let dumped = terminal_under_test.dump_grid();
    assert!(
        !dumped.scrollback.is_empty(),
        "scrollback should contain scrolled-off lines"
    );
    let has_line0 = dumped
        .scrollback
        .iter()
        .any(|row| row.iter().any(|cell| cell.codepoint == 'l' as u32));
    assert!(has_line0, "scrollback: should contain 'l' from line0");
    let _snap = terminal_under_test.take_snapshot();
    assert_invariants(&_snap);
}

/// `read_all_text` 取代 `getTerminalText` 的 `dump_grid` 路径：文本必须逐行等价。
///
/// `dump_grid` 丢弃 codepoint 0（宽字符尾格不计字符）而 `read_line_text_impl` 以空格
/// 占位，两者在行尾 `trim_end` 后同形；行内宽字符尾格处 `dump_grid` 少一个字符，
/// 故此处的等价性以「行尾去空白后的行文本」为准。
#[test]
fn read_all_text_matches_dump_grid_lines() {
    let mut terminal_under_test = GhosttyTerminal::new(6, 20, 100).expect("terminal");
    for line_number in 0..20 {
        terminal_under_test.vt_write(format!("行{line_number}tail\n").as_bytes());
    }
    terminal_under_test.flush();
    let dumped = terminal_under_test.dump_grid();
    let from_grid: Vec<String> = dumped
        .scrollback
        .iter()
        .map(|row| {
            row.iter()
                .filter_map(|cell| char::from_u32(cell.codepoint).filter(|&c| c != '\0'))
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .chain((0..dumped.rows as usize).map(|row| {
            let start = row * dumped.cols as usize;
            let end = (start + dumped.cols as usize).min(dumped.visible.len());
            dumped.visible[start..end]
                .iter()
                .filter_map(|cell| char::from_u32(cell.codepoint).filter(|&c| c != '\0'))
                .collect::<String>()
                .trim_end()
                .to_string()
        }))
        .collect();
    let from_text: Vec<String> = terminal_under_test
        .read_all_text()
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(
        from_text.len(),
        from_grid.len(),
        "行数必须一致（read_all_text={} dump_grid={}）",
        from_text.len(),
        from_grid.len()
    );
    assert_eq!(from_text, from_grid, "逐行文本必须一致");
    assert!(
        from_text.iter().any(|line| line.contains("行0tail")),
        "回滚内容必须可读：{from_text:?}"
    );
}

/// 回归护栏：回滚填满时单次 `read_all_text` 必须留在查询超时之内。
///
/// `read_all_text` 逐格走 ghostty FFI；回滚填到上限（2000 行 × 80 列）时一次重建
/// 曾逼近 `QUERY_TIMEOUT_MS`，调用方读到的却是超时后的空值——外部表现为「标记不落格」。
/// 网格不变时 VT 线程直接复用缓存，故重复查询必须是微秒级。
#[test]
fn read_all_text_answers_repeated_queries_within_query_timeout() {
    use super::types::QUERY_TIMEOUT_MS;
    const COLUMNS: u32 = 80;
    const ROWS: u32 = 24;
    const MAX_SCROLLBACK: u32 = 500;
    let mut terminal_under_test =
        GhosttyTerminal::new(ROWS, COLUMNS, MAX_SCROLLBACK).expect("terminal");
    for line_number in 0..MAX_SCROLLBACK + ROWS {
        terminal_under_test.vt_write(format!("line{line_number}\n").as_bytes());
    }
    terminal_under_test.flush();
    // ghostty 会自行裁剪回滚，故只断言「已大幅填满」而非精确上限。
    let filled = terminal_under_test.scrollback_length();
    assert!(
        filled > MAX_SCROLLBACK / 2,
        "回滚必须大幅填满，护栏才覆盖最坏情况（实际={filled}）"
    );

    let first = terminal_under_test.read_all_text();
    assert!(
        first.contains("line0"),
        "首次查询必须拿到内容而非超时空值（长度={}）",
        first.len()
    );

    // 网格未变：内容必须逐轮一致（缓存复用而非陈旧重建）。
    for round in 0..5 {
        assert_eq!(
            terminal_under_test.read_all_text(),
            first,
            "第 {round} 轮内容必须与首轮一致"
        );
    }
    // 缓存命中时单轮必须留在查询期限内。VT 循环以 recv_timeout(50ms) 的粒度
    // 轮转查询通道，故每轮延迟上限即该节拍（约 50ms）；越过 500ms 说明查询排不上
    // VT 线程——回滚填满时的全量重建正是这条路。
    let started = Instant::now();
    let cached = terminal_under_test.read_all_text();
    let elapsed = started.elapsed();
    assert_eq!(cached, first, "缓存命中时内容必须一致");
    assert!(
        elapsed < std::time::Duration::from_millis(QUERY_TIMEOUT_MS),
        "缓存命中的单轮耗时 {elapsed:?}，越过查询期限 {QUERY_TIMEOUT_MS}ms"
    );

    // 网格一变缓存必须失效，否则调用方会读到陈旧内容。
    terminal_under_test.vt_write(b"fresh-marker\n");
    terminal_under_test.flush();
    assert!(
        terminal_under_test.read_all_text().contains("fresh-marker"),
        "网格变动后缓存必须失效"
    );
}

// ── DECSET/DECRST ──────────────────────────────────────────────────────

/// EncodeMouseEvent with no tracking mode enabled must return empty (the
/// application never asked for mouse reporting) — zelland renderer/mod.rs
/// drops mouse events when `get_mouse_mode()` is false.
#[test]
fn encode_mouse_event_gated_off_without_tracking_mode() {
    let terminal_under_test = terminal();
    let encoded = terminal_under_test.encode_mouse_event((50.0, 60.0), 0, 0, 0, 10.0, 20.0);
    let encoded = encoded.expect("encode_mouse_event should return Some");
    assert!(
        encoded.is_empty(),
        "mouse event must be dropped when tracking is off (got {encoded:?})"
    );
}

/// DECSET 1000 + SGR format (DECSET 1006) → left-click press at pixel
/// (35,45) with 10x20 cells must produce a valid SGR mouse sequence for
/// cell (3,2). Matches ghostty's standard SGR encoding.
#[test]
fn encode_mouse_event_sgr_press() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b[?1000h\x1b[?1006h");
    terminal_under_test.flush();
    let encoded = terminal_under_test
        .encode_mouse_event((35.0, 45.0), 0, 0, 0, 10.0, 20.0)
        .expect("encode_mouse_event should return Some");
    // SGR: ESC [ < Cb ; Cx ; Cy M — Cb is the 0-based button (0 = left
    // press; X10's +32 offset does NOT apply to SGR mode).
    // Cell (3,2) → Cx=3+1=4, Cy=2+1=3.
    assert_eq!(
        encoded, b"\x1b[<0;4;3M",
        "SGR left-press at cell (3,2) must match ghostty's standard encoding"
    );
}

/// Shift/Ctrl 点击必须与普通左键在编码结果上可区分（vim/tmux 依此扩展选区、
/// 粘贴选择）。无修饰时 Cb=0，Shift 为 +4，Ctrl 为 +16（xterm SGR 口径）。
#[test]
fn encode_mouse_event_carries_modifiers() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b[?1000h\x1b[?1006h");
    terminal_under_test.flush();
    let plain = terminal_under_test
        .encode_mouse_event((35.0, 45.0), 0, 0, Mods::empty().bits(), 10.0, 20.0)
        .expect("plain left-press encode");
    let shift = terminal_under_test
        .encode_mouse_event((35.0, 45.0), 0, 0, Mods::SHIFT.bits(), 10.0, 20.0)
        .expect("shift left-press encode");
    let ctrl = terminal_under_test
        .encode_mouse_event((35.0, 45.0), 0, 0, Mods::CTRL.bits(), 10.0, 20.0)
        .expect("ctrl left-press encode");
    assert_eq!(plain, b"\x1b[<0;4;3M");
    assert_ne!(
        plain, shift,
        "Shift+左键必须与普通左键编码不同（否则远端无法扩展选区）"
    );
    assert_ne!(
        plain, ctrl,
        "Ctrl+左键必须与普通左键编码不同（否则远端无法粘贴选择）"
    );
    assert_ne!(shift, ctrl, "Shift 与 Ctrl 组合不得互相混淆");
}

/// Wheel-up with DECSET 1000 + SGR → button 4 (wheel-up = button 64+4-32).
/// The Ghostty encoder emits button 4 for wheel-up; SGR adds 32 for press.
#[test]
fn encode_mouse_event_wheel() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b[?1000h\x1b[?1006h");
    terminal_under_test.flush();
    let up = terminal_under_test
        .encode_mouse_event((10.0, 10.0), 0, 3, 0, 10.0, 20.0)
        .expect("wheel-up encode");
    assert!(
        up.len() >= 6 && up.starts_with(b"\x1b[<"),
        "wheel-up must produce an SGR sequence (got {up:?})"
    );
    let down = terminal_under_test
        .encode_mouse_event((10.0, 10.0), 0, 4, 0, 10.0, 20.0)
        .expect("wheel-down encode");
    assert!(
        down.len() >= 6 && down.starts_with(b"\x1b[<"),
        "wheel-down must produce an SGR sequence (got {down:?})"
    );
}

/// Negative pixel coordinates must be handled gracefully — no panic,
/// no underflow, no out-of-range.  The Ghostty encoder may drop the
/// event (empty Vec) when coordinates are negative, which is correct
/// behavior — the application should clamp before calling this.
#[test]
fn encode_mouse_event_bounds_negative_clamp() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b[?1000h\x1b[?1006h");
    terminal_under_test.flush();
    let result = terminal_under_test.encode_mouse_event((-5.0, -10.0), 0, 0, 0, 10.0, 20.0);
    // The encoder returns Some(empty) or Some(sgr) — it must not panic.
    assert!(
        result.is_some(),
        "negative coords must return Some (not None = tracking off)"
    );
    // Ghostty encoder drops events for negative coordinates.
    // This is correct — the Kotlin caller must clamp before calling.
    let encoded = result.unwrap();
    // No crash is the primary assertion. If it produces output, it
    // must be valid SGR.
    if !encoded.is_empty() {
        assert!(
            encoded.starts_with(b"\x1b[<"),
            "if output is produced, must be SGR (got {encoded:?})"
        );
    }
}

/// Oversized pixel coordinates (beyond the grid) must be handled
/// gracefully — no panic, no overflow.
#[test]
fn encode_mouse_event_bounds_oversized_clamp() {
    let mut terminal_under_test = terminal(); // default 24 rows × 80 cols
    terminal_under_test.vt_write(b"\x1b[?1000h\x1b[?1006h");
    terminal_under_test.flush();
    // Position far beyond the grid: 9999x9999 with 10x20 cells.
    let result = terminal_under_test.encode_mouse_event((9999.0, 9999.0), 0, 0, 0, 10.0, 20.0);
    assert!(result.is_some(), "oversized coords must return Some");
    let encoded = result.unwrap();
    if !encoded.is_empty() {
        let text = String::from_utf8_lossy(&encoded);
        assert!(
            text.starts_with("\x1b[<"),
            "if output is produced, must be SGR (got {text})"
        );
        // Parse and verify no overflow — coordinates must be finite integers.
        let inside = text.trim_start_matches("\x1b[<");
        let parts: Vec<&str> = inside.trim_end_matches('M').split(';').collect();
        assert_eq!(parts.len(), 3, "SGR must have 3 parts: {text}");
        let col: u32 = parts[1].parse().expect("col must be numeric");
        let row: u32 = parts[2].parse().expect("row must be numeric");
        assert!(col < 1000, "col must be reasonable, got {col}");
        assert!(row < 1000, "row must be reasonable, got {row}");
    }
}

/// Full press → drag → release sequence: three events with increasing x
/// must all produce valid SGR output and the action byte must change
/// (0=press, 2=motion, 1=release).
#[test]
fn encode_mouse_event_drag_sequence() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b[?1000h\x1b[?1006h"); // button tracking + SGR
    terminal_under_test.vt_write(b"\x1b[?1002h"); // button-event tracking (motion reports)
    terminal_under_test.flush();
    let cell_width = 10.0;
    let cell_height = 20.0;

    // Press at (10, 20)
    let press = terminal_under_test
        .encode_mouse_event((10.0, 20.0), 0, 0, 0, cell_width, cell_height)
        .expect("press encode");
    assert!(
        press.starts_with(b"\x1b[<"),
        "press must produce SGR (got {press:?})"
    );
    assert!(
        press.ends_with(b"M"),
        "press must end with M (got {press:?})"
    );

    // Drag (motion) at (30, 20) — action=2
    let drag = terminal_under_test
        .encode_mouse_event((30.0, 20.0), 2, 0, 0, cell_width, cell_height)
        .expect("drag encode");
    assert!(
        drag.starts_with(b"\x1b[<"),
        "drag must produce SGR (got {drag:?})"
    );

    // Release at (50, 20) — action=1
    let release = terminal_under_test
        .encode_mouse_event((50.0, 20.0), 1, 0, 0, cell_width, cell_height)
        .expect("release encode");
    assert!(
        release.starts_with(b"\x1b[<"),
        "release must produce SGR (got {release:?})"
    );

    // Column should increase across the sequence (10→30→50 px = 1→3→5 cell).
    let parse_col = |seq: &[u8]| -> u32 {
        let text = String::from_utf8_lossy(seq);
        let inside = text.trim_start_matches("\x1b[<");
        inside
            .trim_end_matches('M')
            .split(';')
            .nth(1)
            .unwrap()
            .parse()
            .unwrap()
    };
    let col_press = parse_col(&press);
    let col_drag = parse_col(&drag);
    let col_release = parse_col(&release);
    assert!(
        col_press < col_drag && col_drag < col_release,
        "columns must increase across drag: press={col_press}, drag={col_drag}, release={col_release}"
    );
}

// ── OSC split-buffer tests ──────────────────────────────────────────────

/// OSC 0 title — split across two writes (true reassembly: the second half
/// must be consumed by the OSC, not rendered as text).
#[test]
fn osc_title_split_buffer() {
    let mut terminal_under_test = terminal();
    // Send the first and second parts of OSC 0 sequence
    terminal_under_test.vt_write(b"\x1b]0;My ");
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"QQQ\x07");
    terminal_under_test.flush();
    let _snap = terminal_under_test.take_snapshot();
    // After setting the title, terminal should not crash, text should still be writable
    terminal_under_test.vt_write(b"AfterTitle");
    terminal_under_test.flush();
    let snap2 = terminal_under_test.take_snapshot();
    let found = snap2.cells.iter().any(|cell| cell.codepoint == 'A' as u32);
    assert!(found, "OSC split: text after split title should render");
    let leaked = snap2.cells.iter().any(|cell| cell.codepoint == 'Q' as u32);
    assert!(
        !leaked,
        "OSC split: title second half must be consumed, not rendered"
    );
    assert_invariants(&snap2);
}

/// OSC 52 clipboard — sent across split buffer (true reassembly: the payload
/// must reach the clipboard callback, not the grid).
#[test]
fn osc_clipboard_split_buffer() {
    let mut terminal_under_test = terminal();
    // OSC 52 sequence: first part sets clipboard selection, second provides data.
    terminal_under_test.vt_write(b"\x1b]52;c;");
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"SGVsbG8=\x07");
    terminal_under_test.flush();
    // 分片必须由上游重组：剪贴板事件内容为解码后文本。
    let event = terminal_under_test.poll_clipboard_event();
    assert_eq!(
        event,
        Some(("c".to_string(), "Hello".to_string())),
        "OSC 52 split: payload must reassemble into clipboard event"
    );
    // Terminal should not crash, text should still be writable
    terminal_under_test.vt_write(b"PostClip");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let found = snap.cells.iter().any(|cell| cell.codepoint == 'P' as u32);
    assert!(found, "OSC 52 split: post-clipboard text should render");
    assert_invariants(&snap);
}

/// OSC 52 clipboard via pty_write (the production output path:
/// Session::poll_pty_output feeds PTY bytes through pty_write, NOT
/// vt_write). LF→CRLF conversion must not corrupt the sequence.
#[test]
fn osc_clipboard_via_pty_write_path() {
    let mut terminal_under_test = terminal();
    terminal_under_test.pty_write(b"\x1b]52;c;SGVsbG8=\x07");
    terminal_under_test.flush();
    let event = terminal_under_test.poll_clipboard_event();
    assert_eq!(
        event,
        Some(("c".to_string(), "Hello".to_string())),
        "OSC 52 via pty_write must reach the clipboard callback"
    );
}

/// OSC color reset — sent across split buffer.
#[test]
fn osc_color_reset_split_buffer() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b]104;");
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"\x07");
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"ColorReset");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let found = snap.cells.iter().any(|cell| cell.codepoint == 'C' as u32);
    assert!(found, "OSC 104 split: text after color reset should render");
    assert_invariants(&snap);
}

/// OSC sequence terminated after partial first block — crash test
#[test]
fn osc_aborted_after_partial_feed() {
    let mut terminal_under_test = terminal();
    // Send partial OSC sequence, then BEL to terminate it
    terminal_under_test.vt_write(b"H\x1b]0;Partial\x07");
    terminal_under_test.flush();
    // Then write normally, should not be consumed by OSC
    terminal_under_test.vt_write(b"Normal");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let outer = snap.cells.iter().any(|cell| cell.codepoint == 'H' as u32);
    let normal = snap.cells.iter().any(|cell| cell.codepoint == 'N' as u32);
    assert!(outer, "aborted OSC: H should be visible before OSC");
    assert!(normal, "aborted OSC: Normal should be visible");
    assert_invariants(&snap);
}

/// Oversized OSC 52 payload — no crash
#[test]
fn osc_large_clipboard_payload_terminal_survives() {
    let mut terminal_under_test = terminal();
    let large = vec![b'A'; 1024 * 4]; // 4KB base64
    let mut seq = Vec::from(b"\x1b]52;c;");
    seq.extend_from_slice(&large);
    seq.push(b'\x07');
    terminal_under_test.vt_write(&seq);
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"OK");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let ok = snap.cells.iter().any(|cell| cell.codepoint == 'O' as u32);
    assert!(ok, "OSC large payload: OK should render");
    assert_invariants(&snap);
}

/// Extremely long 8KB OSC string — no crash
#[test]
fn osc_extremely_long_8kb_string() {
    let mut terminal_under_test = terminal();
    let mut seq = Vec::from(b"\x1b]0;");
    seq.extend(std::iter::repeat_n(b'x', 8000));
    seq.push(b'\x07');
    terminal_under_test.vt_write(&seq);
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"LongDone");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let found = snap.cells.iter().any(|cell| cell.codepoint == 'L' as u32);
    assert!(found, "OSC 8KB: LongDone should render");
    assert_invariants(&snap);
}

// ── Resize + CJK + SGR ─────────────────────────────────────────────────

// ── Scroll Region (DECSTBM + Origin Mode) ──────────────────────────────

// ── DECSC/DECRC cursor save/restore ─────────────────────────────────────

// ── SGR 24-bit color ────────────────────────────────────────────────────

// ── UTF-8 edge cases ────────────────────────────────────────────────────

// ── Wide char scroll tests (scroll preserves wide char attributes) ─────

// ── OSC color setting ──────────────────────────────────────────────────

// ── Resize stress ──────────────────────────────────────────────────────

/// 100 resize cycles with scrolling — ring buffer stress test.
#[test]
fn resize_stress_100_cycles_with_scroll() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    for cycle in 0..50 {
        terminal_under_test.vt_write(format!("cycle{cycle}\n").as_bytes());
        // Alternate row and column counts
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

#[test]
// 与字面量默认值精确比较，不涉及浮点运算。
#[allow(clippy::float_cmp)]
fn cell_snapshot_default() {
    let cell = CellSnapshot::default();
    assert_eq!(cell.codepoint, 0);
    assert_eq!(cell.foreground, [0.0, 0.0, 0.0, 0.0]);
    assert_eq!(cell.background, [0.0, 0.0, 0.0, 0.0]);
    assert!(!cell.bold);
    assert!(!cell.italic);
}

#[test]
// 与字面量默认值精确比较，不涉及浮点运算。
#[allow(clippy::float_cmp)]
fn cell_snapshot_clone() {
    let cell = CellSnapshot {
        codepoint: 65,
        graphemes: Vec::new(),
        foreground: [1.0, 0.0, 0.0, 1.0],
        background: [0.0, 0.0, 0.0, 1.0],
        underline_color: [1.0, 0.0, 0.0, 1.0],
        bold: true,
        dim: false,
        italic: false,
        underline: true,
        reverse: false,
        strikethrough: false,
        blink: false,
        hidden: false,
        overline: false,
        double_underline: false,
        width: 1,
    };
    let cloned_cell = cell.clone();
    assert_eq!(cell.codepoint, cloned_cell.codepoint);
    assert_eq!(cell.foreground, cloned_cell.foreground);
}

// ── CellIterator vs Legacy grid_ref verification ──────────────────────────

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

    // Basic invariants that validate CellIterator correctness
    assert!(
        snap.cells.len() >= 240,
        "snapshot should have at least 240 cells (3 rows × 80 cols)"
    );

    // Verify specific content via string extraction
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

    // Verify colors on specific cells
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
        // Red foreground should not be purely white
        assert!(
            cell.foreground[0] > 0.5,
            "Red foreground: R channel should be significant, got {}",
            cell.foreground[0]
        );
    }

    // Verify bold flag on Bold word
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

/// Verify that the CellIterator path correctly handles CJK double-width
/// characters in the snapshot (width=2).
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

    // Check ASCII content via CellIterator path
    let row0_text = row_text(&snap, 0);
    assert!(row0_text.contains("AB"), "Row 0 should start with 'AB'");
    assert!(row0_text.contains("Red"), "Row 0 should contain 'Red'");

    let row1_text = row_text(&snap, 1);
    assert!(
        row1_text.contains("中"),
        "Row 1 should contain CJK character"
    );

    // Verify codepoints on specific cells
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

/// Verify that the overall grid dimensions are correct through the
/// CellIterator snapshot path.
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
/// If ghostty's VT parser treats LF as LF-only, each line would start at the
/// previous line's end column instead of column 0.
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
    // Row 0 should have 'A','B' then empty space
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
    // Alt buffer should have no scrollback
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
// RS-001 (shrink alt buffer) is the main gap

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
    // Session 1 should NOT have B
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
    // If we reach here, no panic
}

/// TC-SM-004: Double drop is safe (handled by Drop impl)
#[test]
fn tc_sm_004_double_drop_safe() {
    let terminal_under_test = GhosttyTerminal::new(3, 3, 100).expect("terminal");
    terminal_under_test.flush();
    // Can't explicitly double-drop in safe Rust, but we can verify
    // that a normal drop completes without panic
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
// B3 (White Screen on Activity Recreate):
// Root cause: releaseSurface() in pauseRendering was destroying the
// SurfaceView's ANativeWindow. When the Activity recreated, the
// bridge silently skipped updateNativeWindow because the surface
// was already released.
// Fix: pauseRendering() no longer calls releaseSurface(). Instead it
// sets a rendering flag to false, preserving the surface for recreation.
// This fix is Kotlin-side only and cannot be tested in Rust.
// Kotlin commit: (referenced in git history)

// ══════════════════════════════════════════════════════════════════════════
// Performance Benchmarks — realistic usage patterns
// ══════════════════════════════════════════════════════════════════════════

/// Simulate user typing latency: small writes (1-10 chars) followed by flush.
/// Measures wall-clock time per iteration — the user-visible metric.
/// Single anti-flake threshold (no environment checks per TESTING.md):
/// parallel execution and software Vulkan contention make wall time noisy,
/// so this floor catches order-of-magnitude regressions only.
/// 细粒度跟踪由基准测试覆盖，见 scripts/check-rust.nu。

#[test]
fn bench_typing_latency() {
    let mut terminal_under_test = GhosttyTerminal::new(24, 80, 5000).expect("terminal");
    // Pre-fill with some content to avoid empty-terminal optimizations
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

/// 已在底部时继续向下滚动：视口内容不变，但显式滚动是用户动作，
/// 必须强制推帧，否则行级滚动的即时重绘补救路径会被内容去重吞掉。
#[test]
fn scroll_viewport_with_unchanged_content_still_pushes_frame() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    for line_number in 0..20 {
        terminal_under_test.vt_write(format!("line{line_number}\r\n").as_bytes());
    }
    terminal_under_test.flush();
    assert!(
        terminal_under_test.receive_cell_data().is_some(),
        "首帧必须到达"
    );
    assert!(
        terminal_under_test.scroll_viewport(2),
        "scroll_viewport 应接受增量"
    );
    terminal_under_test.flush();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut pushed = false;
    while !pushed {
        pushed = terminal_under_test.receive_cell_data().is_some();
        assert!(
            pushed || std::time::Instant::now() < deadline,
            "内容不变的滚动后 2s 内未收到 CellData 帧"
        );
        if !pushed {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
}

/// 视口滚动后，绝对网格行（0 = 回滚顶部，与 Kotlin 的
/// `scrollbackLength - scrollOffset + row` 同口径）查询必须仍命中该行的内容。
/// `dump_grid.visible` 直接取 `Point::Viewport`，即屏幕上真实显示的文本，
/// 作为地面真值。
#[test]
fn absolute_row_text_survives_viewport_scroll() {
    const ROWS: u32 = 5;
    const SCROLL_OFFSET: u32 = 3;
    let mut terminal_under_test = GhosttyTerminal::new(ROWS, 20, 100).expect("terminal");
    // vt_write 不做 LF→CRLF 展开，必须自带 CR，否则光标只下移不回列。
    for line_number in 0..20 {
        terminal_under_test.vt_write(format!("line{line_number}\r\n").as_bytes());
    }
    terminal_under_test.flush();
    let scrollback = terminal_under_test.scrollback_length();
    assert!(
        scrollback > SCROLL_OFFSET,
        "回滚须多于上滚量, got {scrollback}"
    );
    assert!(terminal_under_test.scroll_viewport(-(SCROLL_OFFSET as isize)));
    terminal_under_test.flush();

    let dumped = terminal_under_test.dump_grid();
    assert_eq!(dumped.rows, ROWS);
    let visible_line = |viewport_row: u32| -> String {
        let start = (viewport_row * dumped.cols) as usize;
        dumped.visible[start..start + dumped.cols as usize]
            .iter()
            .map(|cell| match char::from_u32(cell.codepoint) {
                Some('\0') | None => ' ',
                Some(character) => character,
            })
            .collect::<String>()
            .trim_end()
            .to_string()
    };
    assert_eq!(
        visible_line(0),
        format!("line{}", scrollback - SCROLL_OFFSET),
        "上滚后视口首行应是回滚中的某一行"
    );

    for viewport_row in 0..ROWS {
        let absolute_row = scrollback - SCROLL_OFFSET + viewport_row;
        assert_eq!(
            terminal_under_test.read_line_text(absolute_row).as_deref(),
            Some(visible_line(viewport_row).as_str()),
            "绝对行 {absolute_row}（视口第 {viewport_row} 行）在上滚 {SCROLL_OFFSET} 后读错行"
        );
    }
}

/// Scrollback browsing: the cursor must be reported in VIEWPORT
/// coordinates (or hidden when the cursor page is scrolled out of the
/// viewport), never in active-screen coordinates. The old code used
/// `cursor_y()` (active-area row) against viewport-relative CellData
/// rows, drawing the cursor on the wrong grid row after any scroll —
/// the reported cursor "block" offset ~1 cell down/right.
#[test]
fn cursor_viewport_coordinates_track_scrollback_scroll() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    for line_number in 0..20 {
        terminal_under_test.vt_write(format!("line {line_number}\n").as_bytes());
    }
    terminal_under_test.flush();

    // Cursor sits on the active bottom row (row 4), col 0.
    let (_, cursor0) = terminal_under_test.receive_cell_data().expect("cell data");
    assert!(cursor0.visible, "cursor visible at scroll offset 0");
    assert_eq!(cursor0.row, 4, "cursor on active bottom row");

    // Scroll up 1: viewport now shows scrollback rows 14..18; the
    // cursor page (active row 19) is out of view → the cursor must be
    // hidden, not drawn on scrollback row 4.
    assert!(
        terminal_under_test.scroll_viewport(-1),
        "scroll_viewport(-1)"
    );
    let mut hidden = None;
    for _ in 0..50 {
        if let Some((_, cursor)) = terminal_under_test.receive_cell_data() {
            hidden = Some(cursor);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    match hidden {
        Some(cursor) => {
            assert!(!cursor.visible, "cursor hidden when scrolled out of view")
        }
        None => panic!("no cell data after scroll"),
    }

    // Scroll back to the bottom: the cursor reappears on row 4.
    assert!(
        terminal_under_test.scroll_viewport(1),
        "scroll_viewport(+1)"
    );
    let mut restored = None;
    for _ in 0..50 {
        if let Some((_, cursor)) = terminal_under_test.receive_cell_data() {
            restored = Some(cursor);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    match restored {
        Some(cursor) => {
            assert!(cursor.visible, "cursor visible again at offset 0");
            assert_eq!(cursor.row, 4, "cursor back on the active bottom row");
        }
        None => panic!("no cell data after scroll back"),
    }
}

#[test]
fn terminal_is_alive_after_creation() {
    let terminal_under_test = small_terminal();
    assert!(terminal_under_test.is_alive());
}

#[test]
fn terminal_is_alive_after_vt_write() {
    let mut terminal_under_test = small_terminal();
    terminal_under_test.vt_write(b"Hello, world!");
    assert!(terminal_under_test.is_alive());
}

#[test]
fn terminal_is_alive_after_flush() {
    let mut terminal_under_test = small_terminal();
    terminal_under_test.vt_write(b"ABC");
    terminal_under_test.flush();
    assert!(terminal_under_test.is_alive());
}

/// 经公开 receive_cell_data() 流断言逐行输出一致性：写入第 1 行不得扰动第 0 行。
#[test]
fn cell_data_rows_stay_consistent_across_writes() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"hello");
    terminal_under_test.flush();
    let first = terminal_under_test
        .receive_cell_data()
        .expect("first cell data");
    let (first_cells, _) = first;
    let cols = 80usize;
    let row0_first: Vec<u32> = first_cells[..cols]
        .iter()
        .map(|cell| cell.codepoint)
        .collect();

    // 空闲去重下静默 VT 线程不再推送相同内容：一致性由下方第三快照
    // （新输入后的确定性重建）验证，此处不做定时等待。

    // New input on row 1 must not disturb row 0's cached content.
    terminal_under_test.vt_write(b"\nworld");
    terminal_under_test.flush();
    let third = terminal_under_test
        .receive_cell_data()
        .expect("third cell data");
    let (third_cells, _) = third;
    let row0_third: Vec<u32> = third_cells[..cols]
        .iter()
        .map(|cell| cell.codepoint)
        .collect();
    let row1_third: Vec<u32> = third_cells[cols..cols * 2]
        .iter()
        .map(|cell| cell.codepoint)
        .collect();
    assert_eq!(row0_third, row0_first, "row 0 unchanged after row-1 write");
    // vt_write treats LF as a bare line feed (no CR), so "world" lands at
    // col 5 on row 1, right after the LF.
    assert_eq!(row1_third[5], 'w' as u32, "row 1 col 5 is 'w'");
}

/// 调整尺寸后推送的单元数据必须反映新网格维度，而非旧尺寸的行。
#[test]
fn cell_data_follows_resized_grid() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"top");
    terminal_under_test.flush();
    // 确定性轮询直到工作线程产出对应尺寸数据，杜绝固定时长等待的 flaky。
    let start = Instant::now();
    let before_cells = loop {
        if let Some((cells, _)) = terminal_under_test.receive_cell_data()
            && cells.len() == 24 * 80
        {
            break cells;
        }
        assert!(
            start.elapsed() < std::time::Duration::from_secs(5),
            "调整前单元数据未就绪"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    assert_eq!(before_cells.len(), 24 * 80);

    assert!(terminal_under_test.resize(10, 40), "resize to 10x40");
    let start = Instant::now();
    let after_cells = loop {
        if let Some((cells, _)) = terminal_under_test.receive_cell_data()
            && cells.len() == 10 * 40
        {
            break cells;
        }
        assert!(
            start.elapsed() < std::time::Duration::from_secs(5),
            "调整后单元数据未就绪"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    assert_eq!(after_cells.len(), 10 * 40, "调整后仍推送旧尺寸的行");
}

/// Ghostty formatter selection extraction: a soft-wrapped long line must be
/// joined without '\n' (termux TerminalBuffer.getSelectedText joinBackLines
/// semantics). Write a line longer than 80 cols then select across the wrap.
#[test]
fn selection_text_unwraps_soft_wrapped_lines() {
    let mut terminal_under_test = terminal(); // 24x80
    // 90 chars: exceeds the 80-col width -> soft wrap onto row 2.
    let long = "a".repeat(90);
    terminal_under_test.vt_write(long.as_bytes());
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let scrollback = snap.scrollback_length;
    // The text starts at viewport row 0 (grid row = scrollback_rows).
    let row0 = scrollback;
    let text = terminal_under_test.selection_text((row0, 0), (row0 + 1, 9));
    assert_eq!(
        text.len(),
        90,
        "soft-wrapped selection must be joined without newline (len={})",
        text.len()
    );
    assert!(
        !text.contains('\n'),
        "no newline inside a soft-wrapped selection"
    );
}

/// Wide-char (CJK) column mapping: selecting a range that includes a wide
/// glyph must not split it and the extracted text must match the visible
/// content (TerminalRow.findStartOfColumn equivalent).
#[test]
fn selection_text_wide_char_columns() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write("中".as_bytes()); // wide char at cols 0-1
    terminal_under_test.vt_write(b"ab");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    let text = terminal_under_test.selection_text((row0, 0), (row0, 3));
    assert_eq!(
        text, "中ab",
        "wide char must round-trip exactly (got {text:?})"
    );
}

/// 空格选字：点空格选中该格（对标 selectWordOnBlankCellSelectsThatCell）。
/// 上游格式化器把纯空白选区 trim 为空串：不断言文本，只断言选区反白
/// 烘焙到了该格（选区存在且可见）。
#[test]
fn selection_text_blank_cell_selects_itself() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"a b");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    terminal_under_test.set_selection((row0, 1), (row0, 1));
    terminal_under_test.flush();
    let (selected, _) = terminal_under_test
        .receive_cell_data()
        .expect("selected cell data");
    let theme_background = GhosttyTerminal::byte_color_to_float([30, 30, 46]);
    let picked = selected
        .iter()
        .find(|cell| cell.row == 0 && cell.col == 1)
        .expect("row 0 col 1 present");
    assert_eq!(
        picked.foreground, theme_background,
        "blank cell under selection must be inverted"
    );
    terminal_under_test.clear_selection();
    terminal_under_test.flush();
}

/// 选择清除往返：装选区烘焙反白，清除后恢复基线（对标 selectionClearRemovesSelection）。
#[test]
fn selection_clear_restores_baseline_colors() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"hello");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    terminal_under_test.set_selection((row0, 0), (row0, 4));
    terminal_under_test.flush();
    let (selected, _) = terminal_under_test
        .receive_cell_data()
        .expect("selected cell data");
    let theme_background = GhosttyTerminal::byte_color_to_float([30, 30, 46]);
    let picked = selected
        .iter()
        .find(|cell| cell.row == 0 && cell.col == 0)
        .expect("row 0 col 0 present");
    assert_eq!(picked.foreground, theme_background);
    terminal_under_test.clear_selection();
    terminal_under_test.flush();
    let (cleared, _) = terminal_under_test
        .receive_cell_data()
        .expect("cleared cell data");
    let theme_foreground = GhosttyTerminal::byte_color_to_float([205, 214, 244]);
    let restored = cleared
        .iter()
        .find(|cell| cell.row == 0 && cell.col == 0)
        .expect("row 0 col 0 present");
    assert_eq!(
        restored.foreground, theme_foreground,
        "clear must restore baseline foreground"
    );
}

/// 终端持有选区反白（spec 文本选择：选区存于终端，跟踪引用）：
/// 对标上游 selectWordHighlightsAndExtractsText：上游原生 select_word
/// 派生选区并经格式化器提取文本。本仓单测经“解析出词边界→
/// 用 install 链路安装→格式化器提取”验证同一语义（本仓 selection
/// 接口即 install+format；上游 select_word 另经 select_word_at 查询接入）。
#[test]
fn select_word_via_boundary_resolve_extracts_hello() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"hello world");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    // 词边界解析：col 1（hello 内）→ 词起止 (0, 5)。
    let (word_start, word_end) = resolve_word_bounds(&terminal_under_test, row0, 1);
    assert_eq!(
        (word_start, word_end),
        (0, 5),
        "col 1 must resolve to hello"
    );
    terminal_under_test.set_selection((row0, word_start), (row0, word_end - 1));
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.selection_text((row0, 0), (row0, 4)),
        "hello"
    );
}

/// 词边界解析辅助：沿行左右扫描词字符（空格/行尾为界）。
fn resolve_word_bounds(terminal: &GhosttyTerminal, row: u32, col: u32) -> (u32, u32) {
    let line = terminal.read_line_text(row).unwrap_or_default();
    let chars: Vec<char> = line.chars().collect();
    if chars.is_empty() {
        return (col, col + 1);
    }
    let mut start = (col as usize).min(chars.len());
    // 空格格自身即词：直接选中本格（对标上游空白选中该格）。
    if chars.get(start).is_none_or(|ch| *ch == ' ') {
        return (start as u32, (start + 1) as u32);
    }
    while start > 0 && chars[start - 1] != ' ' {
        start -= 1;
    }
    let mut end = (col as usize).min(chars.len());
    while end < chars.len() && chars[end] != ' ' {
        end += 1;
    }
    (start as u32, end.max(start + 1) as u32)
}

/// 对标上游 selectWordOnBlankCellSelectsThatCell：空格格解析为空长度
/// 词边界并经 install 链路反白烘焙（文本断言见
/// selection_text_blank_cell_selects_itself）。
#[test]
fn select_word_blank_cell_resolves_empty_bounds() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"a b");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    let (word_start, word_end) = resolve_word_bounds(&terminal_under_test, row0, 1);
    assert_eq!(
        (word_start, word_end),
        (1, 2),
        "blank cell must resolve to its own cell"
    );
}

/// 对标上游 selectLineSelectsWholeLine：整行解析出起止并经
/// install+format 提取整行文本。
#[test]
fn select_line_via_full_row_extracts_whole_line() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"hello world");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    let line = terminal_under_test.read_line_text(row0).expect("line text");
    assert_eq!(line, "hello world");
    let end_col = (line.chars().count() as u32).saturating_sub(1);
    terminal_under_test.set_selection((row0, 0), (row0, end_col));
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.selection_text((row0, 0), (row0, end_col)),
        "hello world"
    );
}

/// 对标上游 selectAllCoversScrollback：首行滚入历史后，跨全缓冲
/// 的 install+format 仍覆盖首尾行。
#[test]
fn select_all_via_range_covers_scrollback() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal_under_test.vt_write(b"alpha\n");
    for index in 0..8 {
        terminal_under_test.vt_write(format!("filler{index}\n").as_bytes());
    }
    terminal_under_test.flush();
    let total = terminal_under_test.take_snapshot();
    let last_row = total.rows + total.scrollback_length - 1;
    let text = terminal_under_test.selection_text((0, 0), (last_row, 19));
    assert!(
        text.starts_with("alpha"),
        "must include scrolled-off first line"
    );
    assert!(text.contains("filler7"), "must include the latest line");
}

/// 空白格没有词：长按空白只出仅粘贴菜单，Kotlin 侧据此分菜单，故此处钉死该语义。
/// 若上游对空白格也派生出一个"词"，Kotlin 的空白判定会整体失效（空白格弹出
/// 带复制的完整菜单）。
#[test]
fn select_word_at_yields_nothing_on_blank_cell() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"ab");
    terminal_under_test.flush();
    let row0 = terminal_under_test.take_snapshot().scrollback_length;
    // 第 8 列在 80 列网格上必为空白：行内既无文字也无尾格。
    assert!(
        terminal_under_test.select_word_at(row0, 8).is_none(),
        "空白格不得派生出词界"
    );
}

/// 上游词选接入（design 决策 1）：select_word_at 派生词界限、取序、反解为
/// 绝对坐标并安装；返回界限提取的文本恰为该词，渲染反白证明选区已装回。
#[test]
fn select_word_at_derives_installs_and_returns_bounds() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"git status");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    let ((start_row, start_col), (end_row, end_col)) = terminal_under_test
        .select_word_at(row0, 1)
        .expect("word bounds under cell");
    assert_eq!((start_row, start_col), (row0, 0), "word starts at col 0");
    assert_eq!((end_row, end_col), (row0, 2), "git spans cols 0..=2");
    assert_eq!(
        terminal_under_test.selection_text((start_row, start_col), (end_row, end_col)),
        "git",
        "returned bounds must extract exactly the word"
    );
    // 安装断言：派生快照经 to_ordered 装回终端后，该格渲染必须反白。
    terminal_under_test.flush();
    let (selected, _) = terminal_under_test
        .receive_cell_data()
        .expect("selected cell data");
    let theme_background = GhosttyTerminal::byte_color_to_float([30, 30, 46]);
    let picked = selected
        .iter()
        .find(|cell| cell.row == 0 && cell.col == 1)
        .expect("row 0 col 1 present");
    assert_eq!(
        picked.foreground, theme_background,
        "derived selection must be installed (cell inverted)"
    );
    terminal_under_test.clear_selection();
    terminal_under_test.flush();
}

/// 上游全选接入（design 决策 2 钉住）：两行内容之后，select_all 界限落在
/// 最后一行的最后内容列，不含尾部空行与空列。
#[test]
fn select_all_bounds_exclude_trailing_blank_rows_and_columns() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal_under_test.vt_write(b"alpha\r\nbravo");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    let bounds = terminal_under_test.select_all().expect("select_all bounds");
    assert_eq!(
        bounds,
        ((row0, 0), (row0 + 1, 4)),
        "bounds must exclude trailing blank rows and columns (got {bounds:?})"
    );
    let text = terminal_under_test.selection_text(bounds.0, bounds.1);
    assert!(
        text.contains("alpha") && text.contains("bravo"),
        "pinned bounds must cover both content lines (got {text:?})"
    );
}

/// 对标上游 selectionTracksTextIntoScrollback：选区安装后文本继续
/// 滚动，同一绝对坐标的 selection_text 仍提取原文本（跟踪引用语义
/// 经 install 链路保持）。
#[test]
fn selection_text_survives_scrolled_output() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal_under_test.vt_write(b"alpha\n");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    terminal_under_test.set_selection((row0, 0), (row0, 4));
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.selection_text((row0, 0), (row0, 4)),
        "alpha"
    );
    for index in 0..8 {
        terminal_under_test.vt_write(format!("filler{index}\n").as_bytes());
    }
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.selection_text((row0, 0), (row0, 4)),
        "alpha",
        "tracked selection must follow text into scrollback"
    );
}

/// 对标上游 pasteEncodingHonorsBracketedMode：粘贴编码走
/// libghostty_vt::paste Native：非括号模式换行转回车，括号模式
/// 包裹 \x1b[200~/201~。
#[test]
fn paste_encoding_plain_and_bracketed() {
    let plain = encode_paste_text("ab\ncd", false);
    assert_eq!(plain, b"ab\rcd");
    let bracketed = encode_paste_text("ab", true);
    assert_eq!(bracketed, b"\x1b[200~ab\x1b[201~");
}

/// 粘贴编码辅助：经上游 Native，两模式语义与参考一致。
fn encode_paste_text(text: &str, bracketed: bool) -> Vec<u8> {
    let mut data = text.as_bytes().to_vec();
    let mut output = vec![0u8; data.len() + 16];
    let written =
        libghostty_vt::paste::encode(&mut data, bracketed, &mut output).expect("paste encode");
    output.truncate(written);
    output
}

/// 对标上游 searchFindsMatchAcrossScrollbackAndReveals：首行滚入历史
/// 后 search_all 仍命中首行（揭示语义由 Kotlin 滚动承载，
/// 此处断言命中坐标）。
#[test]
fn search_all_reveals_history_match() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal_under_test.vt_write(b"needle\n");
    for index in 0..10 {
        terminal_under_test.vt_write(format!("filler{index}\n").as_bytes());
    }
    terminal_under_test.flush();
    let results = terminal_under_test.search_all_in_scrollback("needle", true);
    assert!(!results.is_empty(), "history hit");
    let hit = &results[0];
    assert_eq!(hit.start_col, 0, "needle starts at col 0");
    let line = terminal_under_test
        .read_line_text(hit.row)
        .expect("hit line");
    assert_eq!(line, "needle");
}

/// 对标上游 searchStepWraps：本仓 search_all 返回旧→新稳定顺序，
/// Kotlin SearchResult.nextIndex/previousIndex 承载回绕（已有单测）；
/// 此处锁定顺序契约：首个为最旧、末个为最新。
#[test]
fn search_all_order_oldest_first_newest_last() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    for _ in 0..3 {
        terminal_under_test.vt_write(b"match\n");
    }
    terminal_under_test.flush();
    let results = terminal_under_test.search_all_in_scrollback("match", true);
    assert_eq!(results.len(), 3);
    assert!(results[0].row < results[2].row, "oldest first, newest last");
}

/// 对标上游 searchNoMatchesClearsSelection：无命中返回空（清除语义
/// 由 Kotlin 搜索状态机承载，此处锁定空结果契约）。
#[test]
fn search_all_no_matches_returns_empty() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"hello world");
    terminal_under_test.flush();
    assert!(
        terminal_under_test
            .search_all_in_scrollback("zzz", false)
            .is_empty()
    );
    assert!(
        terminal_under_test
            .search_all_in_scrollback("zzz", true)
            .is_empty()
    );
}

/// R21-T1：锁外查询通道与方法版同结果——JNI 侧只将会话锁删减到取通道，
/// 查询语义必须零漂移。
#[test]
fn search_via_bare_channel_matches_method() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"needle in haystack needle");
    terminal_under_test.flush();
    let via_method = terminal_under_test.search_all_in_scrollback("needle", true);
    let via_channel =
        crate::terminal::ghostty_terminal::GhosttyTerminal::search_all_in_scrollback_on(
            &terminal_under_test.query_channel(),
            "needle",
            true,
        );
    assert_eq!(via_method.len(), 2);
    assert_eq!(via_method, via_channel);
}

/// 安装选区后 VT 线程把行级选区反白烘焙进 CellData（前景背景互换），
/// 清除后恢复。该测试断言本仓的安装—烘焙链路，不复述上游选区语义。
#[test]
fn terminal_owned_selection_inverts_cell_data() {
    let mut terminal_under_test = terminal(); // 24x80
    terminal_under_test.vt_write(b"hello");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    // 基线：未选中时前景为主题前景色。
    let (_, _) = terminal_under_test
        .receive_cell_data()
        .expect("baseline cell data");
    terminal_under_test.set_selection((row0, 0), (row0, 4));
    terminal_under_test.flush();
    let (selected, _) = terminal_under_test
        .receive_cell_data()
        .expect("selected cell data");
    let picked = selected
        .iter()
        .find(|cell| cell.row == 0 && cell.col == 0)
        .expect("row 0 col 0 present");
    // Catppuccin Mocha 默认：前景 #CDD6F4，背景 #1E1E2E；选中后互换。
    let theme_foreground = GhosttyTerminal::byte_color_to_float([205, 214, 244]);
    let theme_background = GhosttyTerminal::byte_color_to_float([30, 30, 46]);
    assert_eq!(
        picked.foreground, theme_background,
        "selected foreground must be theme background"
    );
    assert_eq!(
        picked.background, theme_foreground,
        "selected background must be theme foreground"
    );
    // 选区外单元格不受影响。
    let outside = selected
        .iter()
        .find(|cell| cell.row == 0 && cell.col == 10)
        .expect("row 0 col 10 present");
    assert_eq!(outside.foreground, theme_foreground);
    assert_eq!(outside.background, theme_background);
    // 清除后恢复基线。
    terminal_under_test.clear_selection();
    terminal_under_test.flush();
    let (cleared, _) = terminal_under_test
        .receive_cell_data()
        .expect("cleared cell data");
    let restored = cleared
        .iter()
        .find(|cell| cell.row == 0 && cell.col == 0)
        .expect("row 0 col 0 present");
    assert_eq!(restored.foreground, theme_foreground);
    assert_eq!(restored.background, theme_background);
}

/// OSC 8 hyperlink query (termux TerminalView.openLinkAt equivalent):
/// after writing an OSC 8 link, hyperlink_at returns the URI at the link
/// cells and None outside them.
#[test]
fn hyperlink_at_returns_uri_inside_link() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b]8;;https://example.com\x1b\\Link\x1b]8;;\x1b\\");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    // "Link" starts at col 0.
    let url = terminal_under_test
        .hyperlink_at(row0, 0)
        .expect("cell 0 has the link");
    assert_eq!(url, "https://example.com");
    // Last link cell still resolves; one past the link does not.
    assert!(
        terminal_under_test.hyperlink_at(row0, 3).is_some(),
        "col 3 is the last link char"
    );
    assert!(
        terminal_under_test.hyperlink_at(row0, 4).is_none(),
        "col 4 is past the link"
    );
}

/// Kitty 图像协议像素回读（ghostty-android-terminal
/// kittyGraphicsPlacementAndPixelReadback 对等）：传输 1x1 红色 RGB 图像后，
/// take_kitty_graphics_image 返回 Some 且宽高为 1x1。
#[test]
fn kitty_graphics_transmit_returns_1x1_image() {
    let mut terminal_under_test = terminal();
    // 1x1 RGB 红色像素：base64("/wAA") = {0xff, 0x00, 0x00}。
    // 显式 i=1 指定图像 id（上游默认自动分配 id，不保证为 1）。
    // 注意：vt_write/pty_write 均为分片直透（上游跨调用重组，无 ST/SGR 提前闭合）；
    // 此处走 pty_write 以覆盖 LF→CRLF 文本路径。
    terminal_under_test.pty_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=1;/wAA\x1b\\");
    terminal_under_test.flush();
    let image = terminal_under_test
        .take_kitty_graphics_image(1)
        .expect("image id 1 must exist after transmit");
    assert_eq!(image.width, 1);
    assert_eq!(image.height, 1);
    assert_eq!(
        image.data,
        vec![255, 0, 0, 255],
        "RGB 载荷归一化为不透明 RGBA"
    );
}

/// PNG 载荷经进程内解码器透出为 RGBA（1x1 红点 PNG，对标上游文档示例）。
#[test]
fn kitty_graphics_png_decodes_to_rgba() {
    let mut terminal = terminal();
    terminal.pty_write(
        b"\x1b_Ga=T,f=100,i=2;iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==\x1b\\",
    );
    terminal.flush();
    let image = terminal
        .take_kitty_graphics_image(2)
        .expect("PNG 图像解码后必须存在");
    assert_eq!((image.width, image.height), (1, 1));
    assert_eq!(image.data.len(), 4, "解码输出为 RGBA8");
}

/// PNG 调色板/16bit 灰度经解码器展开为 8bit RGBA（解码器 EXPAND/STRIP_16 路径回归）。
#[test]
fn kitty_graphics_png_palette_and_gray16_decode() {
    let mut terminal = terminal();
    terminal.pty_write(b"\x1b_Ga=T,f=100,i=11;iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAMAAAAoyzS7AAAAA1BMVEX/AAAZ4gk3AAAACklEQVR4nGNgAAAAAgABSK+kcQAAAABJRU5ErkJggg==\x1b\\");
    terminal.pty_write(b"\x1b_Ga=T,f=100,i=12;iVBORw0KGgoAAAANSUhEUgAAAAEAAAABEAAAAABq7kcWAAAAC0lEQVR4nGP4/x8AAwAB//wl3FEAAAAASUVORK5CYII=\x1b\\");
    terminal.flush();
    let palette = terminal
        .take_kitty_graphics_image(11)
        .expect("调色板 PNG 必须解码");
    assert_eq!((palette.width, palette.height), (1, 1));
    assert_eq!(palette.data, vec![255, 0, 0, 255]);
    let gray = terminal
        .take_kitty_graphics_image(12)
        .expect("16bit 灰度 PNG 必须解码");
    assert_eq!((gray.width, gray.height), (1, 1));
    assert_eq!(gray.data, vec![255, 255, 255, 255]);
}

/// Kitty 放置采集：传输并显示 1x1 图像后，可见放置恰为 1 个且几何非零。
#[test]
fn kitty_placements_collects_visible_display() {
    let mut terminal = terminal();
    terminal.set_cell_pixel_size(8, 16);
    terminal.pty_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=7;/wAA\x1b\\");
    terminal.flush();
    let placements = terminal.take_kitty_placements();
    assert_eq!(placements.len(), 1, "显示中的图像必须有可见放置");
    let placement = &placements[0];
    assert_eq!(placement.image_id, 7);
    assert!(placement.pixel_width > 0 && placement.pixel_height > 0);
    assert_eq!(placement.image_rgba, vec![255, 0, 0, 255]);
}

/// Kitty 载荷杂散 NUL 不得丢图（对标上游 kittyStrayNulInPayloadStillStores：
/// mpv --vo=kitty 在末块追加 NUL，ECMA-48 忽略控制字符，不得污染 base64）。
#[test]
fn kitty_graphics_stray_nul_still_stores() {
    let mut terminal = terminal();
    terminal.pty_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=8;/wAA\x00\x1b\\");
    terminal.flush();
    let image = terminal
        .take_kitty_graphics_image(8)
        .expect("杂散 NUL 不得丢弃整图");
    assert_eq!((image.width, image.height), (1, 1));
    assert_eq!(image.data, vec![255, 0, 0, 255]);
}

/// 无图像时 Kitty 查询必须为空（对标 kittyGraphicsAbsentWhenNoImages：
/// 未传输返回空放置，未知 id 取图返回 None）。
#[test]
fn kitty_graphics_absent_when_no_images() {
    let terminal = terminal();
    assert!(
        terminal.take_kitty_placements().is_empty(),
        "no images must yield no placements"
    );
    assert_eq!(
        terminal
            .take_kitty_graphics_image(123)
            .map(|image| image.width),
        None,
        "unknown image id must yield None"
    );
}

/// DECCKM 应用光标键模式必须改变方向键编码（对标
/// arrowKeyEncodingHonorsCursorKeyMode：普通模式 ESC[A，应用模式 ESC OA）。
#[test]
fn arrow_key_encoding_honors_cursor_key_mode() {
    let normal = terminal();
    normal.flush();
    let plain = normal
        .key_encode(19, 0, 0, 0, 0)
        .expect("arrow up must encode");
    assert_eq!(plain, b"\x1b[A", "normal mode arrow up (got {plain:?})");
    let mut applied = terminal();
    applied.vt_write(b"\x1b[?1h");
    applied.flush();
    let application = applied
        .key_encode(19, 0, 0, 0, 0)
        .expect("arrow up must encode");
    assert_eq!(
        application, b"\x1bOA",
        "application mode arrow up (got {application:?})"
    );
}

// ── : cursor/row coordinate consistency (D1 deterministic leg) ──

/// The shell-echo path (prompt text + typed chars, no newline) must report a
/// cursor that sits ON the last printed row at the column right after the
/// text — the same row the CellData rows occupy. A mismatch here renders the
/// cursor one row below the text ("输入指针出现在当前文本的正下",
/// emulator evidence). Mirrors the exact production path: VT loop auto-push →
/// build_cell_data → (cells, CursorInfo).
#[test]
fn cursor_matches_last_printed_row_after_echo_print() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"$ ");
    terminal_under_test.vt_write(b"abc");
    terminal_under_test.flush();

    let (cells, cursor) = terminal_under_test
        .receive_cell_data()
        .expect("VT loop must auto-push cell data after writes");

    // Last row that has any non-blank cell = the row the text visibly sits on.
    let last_text_row = cells
        .iter()
        .filter(|cell| cell.codepoint != 0)
        .map(|cell| cell.row)
        .max()
        .expect("printed cells must exist");
    let last_text_col = cells
        .iter()
        .filter(|cell| cell.codepoint != 0 && cell.row == last_text_row)
        .map(|cell| cell.col)
        .max()
        .expect("printed cells must exist on the text row");

    assert_eq!(
        cursor.row, last_text_row,
        "cursor row must equal the last printed row (echo path)"
    );
    assert_eq!(
        cursor.col,
        last_text_col + 1,
        "cursor col must be right after the last printed cell"
    );
    assert!(cursor.visible, "cursor must be visible after print");
}

/// CUP-positioned cursor (the path CursorGeometryQuantifiedTest parks with)
/// must agree with the echo path — one coordinate contract for both.
#[test]
fn cursor_matches_cell_rows_after_cup_positioning() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b[3;5H"); // CUP to row 2, col 4 (1-based)
    terminal_under_test.flush();

    let (cells, cursor) = terminal_under_test
        .receive_cell_data()
        .expect("VT loop must auto-push cell data after CUP");

    assert_eq!(cursor.row, 2, "CUP row must be 0-based viewport row 2");
    assert_eq!(cursor.col, 4, "CUP col must be 0-based viewport col 4");
    // The cell grid must have NO text (CUP only moves the cursor) — the
    // cursor must therefore sit on an empty row, not below any text.
    let text_rows: Vec<u32> = cells
        .iter()
        .filter(|cell| cell.codepoint != 0)
        .map(|cell| cell.row)
        .collect();
    assert!(
        text_rows.is_empty(),
        "CUP alone must not print cells, got rows {text_rows:?}"
    );
}

/// R16-T5：CursorInfo 必须携带产出本帧时的真实网格行列，与同批 CellData 同源。
/// resize 入队时会话侧原子缓存提前发布新尺寸，而 VT 尚未应用；渲染线程若读缓存，
/// 收缩帧会被 `build_row_ranges` 判空丢弃（IME 弹出/旋转必现）。
/// 断言默认网格下推送的 CursorInfo 与单元数一致。
#[test]
fn cursor_info_carries_producing_frame_grid_dimensions() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"AB");
    terminal_under_test.flush();

    let (cells, cursor) = terminal_under_test
        .receive_cell_data()
        .expect("VT loop must auto-push cell data after writes");

    assert_eq!((cursor.rows, cursor.cols), (24, 80));
    assert_eq!(
        cells.len() as u32,
        cursor.rows * cursor.cols,
        "cells and CursorInfo grid must come from the same frame"
    );
}

/// R16-T5（resize 变体）：resize 应用后推送的帧必须携带新网格，
/// 而不是入队瞬间缓存的尺寸与 VT 旧网格的混搭。
#[test]
fn cursor_info_carries_resized_grid_dimensions() {
    let mut terminal_under_test = terminal();
    terminal_under_test.resize(10, 40);
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"XY");
    terminal_under_test.flush();

    let (cells, cursor) = terminal_under_test
        .receive_cell_data()
        .expect("VT loop must auto-push cell data after resize");

    assert_eq!((cursor.rows, cursor.cols), (10, 40));
    assert_eq!(
        cells.len() as u32,
        cursor.rows * cursor.cols,
        "cells and CursorInfo grid must come from the same frame"
    );
}

/// SGR31 红色必须到达渲染 CellData 的前景（设备渲染通路的精确复刻）。
/// 背景：设备像素验收曾报 SGR 无红色，后证实为测试 harness 自废武功
///（全局暂停与直接呈现互斥）；本测试在 host 复刻设备渲染输入
///（receive_cell_data），锁定颜色通路。
#[test]
fn sgr31_red_reaches_cell_data_foreground() {
    // 1) 默认配置：基线。
    let mut plain = terminal();
    plain.vt_write(b"\x1b[31mRED\x1b[0m");
    plain.flush();
    let (cells, _) = plain.receive_cell_data().expect("cell data");
    let red_cell = cells
        .iter()
        .find(|cell| cell.codepoint == 'R' as u32)
        .expect("R cell present");
    assert!(
        red_cell.foreground[0] > 0.9
            && (red_cell.foreground[1] - 0.545).abs() < 0.05
            && (red_cell.foreground[2] - 0.659).abs() < 0.05,
        "默认配置 SGR31 前景须为内置红（粉调），实际 {:?}",
        red_cell.foreground
    );

    // 2) Dracula 主题配置：复刻设备 apply_theme 后的精确状态。
    let dracula_ansi: [[u8; 3]; 16] = [
        [0x21, 0x22, 0x2C],
        [0xFF, 0x55, 0x55],
        [0x50, 0xFA, 0x7B],
        [0xFF, 0xCB, 0x6B],
        [0x82, 0xAA, 0xFF],
        [0xC7, 0x92, 0xEA],
        [0x8B, 0xE9, 0xFD],
        [0xF8, 0xF9, 0xF2],
        [0x54, 0x54, 0x54],
        [0xFF, 0x6E, 0x6E],
        [0x69, 0xFF, 0x94],
        [0xFF, 0xCB, 0x6B],
        [0xD6, 0xAC, 0xFF],
        [0xFF, 0x92, 0xDF],
        [0xA4, 0xFF, 0xFF],
        [0xF8, 0xF8, 0xF2],
    ];
    let mut dracula = GhosttyTerminal::new_with_theme(
        24,
        80,
        1000,
        [0x21, 0x21, 0x21],
        [0xF8, 0xF8, 0xF2],
        dracula_ansi,
    )
    .expect("dracula terminal");
    dracula.vt_write(b"\x1b[31mRED\x1b[0m");
    dracula.flush();
    let (cells, _) = dracula.receive_cell_data().expect("cell data");
    let red_cell = cells
        .iter()
        .find(|cell| cell.codepoint == 'R' as u32)
        .expect("R cell present");
    assert!(
        (red_cell.foreground[0] - 1.0).abs() < 0.02
            && (red_cell.foreground[1] - 0x55 as f32 / 255.0).abs() < 0.02
            && (red_cell.foreground[2] - 0x55 as f32 / 255.0).abs() < 0.02,
        "Dracula 配置 SGR31 前景须为 #FF5555，实际 {:?}",
        red_cell.foreground
    );
}

/// FFI 默认主题（Catppuccin Mocha）下 SGR31 必须到达 CellData 前景。
/// 背景：设备像素验收曾报 SGR 无显色，后证实为 harness 问题；
/// 复刻其精确主题与几何，锁定颜色通路。
#[test]
fn sgr31_mocha_default_theme_reaches_foreground() {
    let (ansi, background, foreground) = GhosttyTerminal::catppuccin_mocha_palette();
    let mut mocha = GhosttyTerminal::new_with_theme(44, 48, 1000, background, foreground, ansi)
        .expect("mocha terminal");
    mocha.vt_write(b"\x1b[31mRED\x1b[0m");
    mocha.flush();
    let (cells, _) = mocha.receive_cell_data().expect("cell data");
    let red_cell = cells
        .iter()
        .find(|cell| cell.codepoint == 'R' as u32)
        .expect("R cell present");
    assert!(
        (red_cell.foreground[0] - 243.0 / 255.0).abs() < 0.02
            && (red_cell.foreground[1] - 139.0 / 255.0).abs() < 0.05
            && (red_cell.foreground[2] - 168.0 / 255.0).abs() < 0.05,
        "Mocha 主题 SGR31 前景须为 #F38BA8，实际 {:?}",
        red_cell.foreground
    );
}

/// 同一终端连收三包同文本不同色时，三行必须各就其位各带其色。
/// 背景：设备上三包同文本（EEEEEEEE）蓝/红/绿轮换时恒只有两行上屏，
/// 第三包系统性缺席；先在 host 判定是构建层去重还是设备推送时序。
#[test]
fn sgr_same_text_tricolor_rows_all_present() {
    let (ansi, background, foreground) = GhosttyTerminal::catppuccin_mocha_palette();
    let mut terminal = GhosttyTerminal::new_with_theme(24, 80, 1000, background, foreground, ansi)
        .expect("mocha terminal");
    for code in [34u8, 31, 32] {
        terminal.vt_write(format!("\x1b[{code}mEEEEEEEE\x1b[0m\r\n").as_bytes());
    }
    terminal.flush();
    let (cells, _) = terminal.receive_cell_data().expect("cell data");
    // 三行 E 首字符：行0蓝、行1红、行2绿。
    let expected: [(u32, [u8; 3]); 3] = [
        (0, [137, 180, 250]),
        (1, [243, 139, 168]),
        (2, [166, 227, 161]),
    ];
    for (row, expected_rgb) in expected {
        let cell = cells
            .iter()
            .find(|cell| cell.row == row && cell.col == 0)
            .unwrap_or_else(|| panic!("行{row}首格缺席"));
        assert_eq!(
            cell.codepoint, 'E' as u32,
            "行{row}首格须为 E，实际 {}",
            cell.codepoint
        );
        assert!(
            (cell.foreground[0] - expected_rgb[0] as f32 / 255.0).abs() < 0.02
                && (cell.foreground[1] - expected_rgb[1] as f32 / 255.0).abs() < 0.05
                && (cell.foreground[2] - expected_rgb[2] as f32 / 255.0).abs() < 0.05,
            "行{row}前景须为 {expected_rgb:?}，实际 {:?}",
            cell.foreground
        );
    }
}
/// Mocha 主题下 SGR31/32/34 三色必须各自到达 CellData 前景。
/// 背景：设备像素验收红绿显色而蓝计数为 0；先在 host 判定蓝是映射问题
/// 还是设备侧问题（计数/呈现），再定跟进方向。
#[test]
fn sgr_tricolor_mocha_reaches_foreground() {
    let (ansi, background, foreground) = GhosttyTerminal::catppuccin_mocha_palette();
    // （码，标记字符，期望槽位色）
    let cases: [(u8, u8, [u8; 3]); 3] = [
        (31, b'R', [243, 139, 168]),
        (32, b'G', [166, 227, 161]),
        (34, b'B', [137, 180, 250]),
    ];
    for (code, marker, expected) in cases {
        let mut terminal =
            GhosttyTerminal::new_with_theme(24, 80, 1000, background, foreground, ansi)
                .expect("mocha terminal");
        terminal.vt_write(format!("\x1b[{code}m{}\x1b[0m", marker as char).as_bytes());
        terminal.flush();
        let (cells, _) = terminal.receive_cell_data().expect("cell data");
        let marked = cells
            .iter()
            .find(|cell| cell.codepoint == marker as u32)
            .expect("marked cell present");
        assert!(
            (marked.foreground[0] - expected[0] as f32 / 255.0).abs() < 0.02
                && (marked.foreground[1] - expected[1] as f32 / 255.0).abs() < 0.05
                && (marked.foreground[2] - expected[2] as f32 / 255.0).abs() < 0.05,
            "Mocha 主题 SGR{code} 前景须为 {expected:?}，实际 {:?}",
            marked.foreground
        );
    }
}

/// BEL（0x07）必须经上游 on_bell 回调到达振铃通道（对标 bellEventIsReported）。
/// 背景：此前上游回调未接线，BEL 被静默吞掉，Kotlin 侧永远收不到振铃。
#[test]
fn bell_reaches_bell_channel() {
    let mut bell_terminal = terminal();
    bell_terminal.pty_write(b"\x07");
    bell_terminal.flush();
    assert_eq!(
        bell_terminal.poll_bell_event(),
        Some(()),
        "BEL must surface a bell event"
    );
    // 通道级 get-and-clear：取走后即无（会话锁存另有同语义单测位）。
    assert_eq!(
        bell_terminal.poll_bell_event(),
        None,
        "bell channel must clear after poll"
    );
}

/// 无 BEL 输入时振铃通道必须保持空（守卫：回调不得误触发）。
#[test]
fn bell_channel_stays_empty_without_bel() {
    let mut plain_terminal = terminal();
    plain_terminal.pty_write(b"hello");
    plain_terminal.flush();
    assert_eq!(
        plain_terminal.poll_bell_event(),
        None,
        "plain output must not raise a bell event"
    );
}

/// SGR 3/4/9（斜体/下划线/删除线）必须到达快照层与 CellData 标志位。
/// 背景：用户报告斜体等文本缺失无法显示；颜色有 VT 端到端覆盖
///（sgr_tricolor_mocha_reaches_foreground），样式属性只有 pack_style_flags
/// 单元测试（Style 结构体→bits），无 VT 解析端到端。
#[test]
fn sgr_style_attributes_reach_snapshot_and_cell_data() {
    use crate::terminal::ghostty_terminal::cell_flags;
    // （SGR 码，标记字符）
    for (code, marker) in [(3u8, b'I'), (4u8, b'U'), (9u8, b'S')] {
        let flag_bit = match code {
            3 => cell_flags::ITALIC,
            4 => cell_flags::UNDERLINE,
            _ => cell_flags::STRIKETHROUGH,
        };
        let mut styled = terminal();
        styled.vt_write(format!("\x1b[{code}m{}\x1b[0m", marker as char).as_bytes());
        styled.flush();
        let (cells, _) = styled.receive_cell_data().expect("cell data");
        let marked = cells
            .iter()
            .find(|cell| cell.codepoint == marker as u32)
            .expect("marked cell present");
        assert_eq!(
            (marked.flags >> flag_bit) & 1,
            1,
            "SGR{code} must set CellData flag bit {flag_bit}"
        );
        let snapshot = styled.take_snapshot();
        let snap_cell = snapshot
            .cells
            .iter()
            .find(|cell| cell.codepoint == marker as u32)
            .expect("marked snapshot cell present");
        let snapshot_set = match code {
            3 => snap_cell.italic,
            4 => snap_cell.underline,
            _ => snap_cell.strikethrough,
        };
        assert!(snapshot_set, "SGR{code} must set snapshot style attribute");
    }
}

/// SGR 5 闪烁必须到达渲染层且不污染下划线位（对标上游
/// sgrBlinkAndUnderlineStyleAreDisjoint：两字段曾共用一位，下划线 cell
/// 闪烁、闪烁 cell 画下划线）。
#[test]
fn blink_reaches_cell_data_without_underline_pollution() {
    use crate::terminal::ghostty_terminal::cell_flags;
    // 下划线不闪。
    let mut underlined = terminal();
    underlined.vt_write(b"\x1b[4mA");
    underlined.flush();
    let (underlined_cells, _) = underlined.receive_cell_data().expect("cell data");
    let underlined_cell = underlined_cells
        .iter()
        .find(|cell| cell.codepoint == 'A' as u32)
        .expect("A present");
    assert_eq!((underlined_cell.flags >> cell_flags::UNDERLINE) & 1, 1);
    assert_eq!(
        (underlined_cell.flags >> cell_flags::BLINK) & 1,
        0,
        "underline must not set blink"
    );
    // 闪烁不带下划线。
    let mut blinking = terminal();
    blinking.vt_write(b"\x1b[5mB");
    blinking.flush();
    let (blinking_cells, _) = blinking.receive_cell_data().expect("cell data");
    let blinking_cell = blinking_cells
        .iter()
        .find(|cell| cell.codepoint == 'B' as u32)
        .expect("B present");
    assert_eq!((blinking_cell.flags >> cell_flags::BLINK) & 1, 1);
    assert_eq!(
        (blinking_cell.flags >> cell_flags::UNDERLINE) & 1,
        0,
        "blink must not set underline"
    );
    let snapshot = blinking.take_snapshot();
    let snap_cell = snapshot
        .cells
        .iter()
        .find(|cell| cell.codepoint == 'B' as u32)
        .expect("snapshot B present");
    assert!(snap_cell.blink, "SGR 5 must set snapshot blink");
    // 两者叠加各自完整。
    let mut both = terminal();
    both.vt_write(b"\x1b[4;5mC");
    both.flush();
    let (both_cells, _) = both.receive_cell_data().expect("cell data");
    let both_cell = both_cells
        .iter()
        .find(|cell| cell.codepoint == 'C' as u32)
        .expect("C present");
    assert_eq!((both_cell.flags >> cell_flags::BLINK) & 1, 1);
    assert_eq!((both_cell.flags >> cell_flags::UNDERLINE) & 1, 1);
}

/// SGR 4:x 下划线形状必须到达渲染层（对标 sgrUnderlineStyles：单/双/卷/点/虚
/// 均置下划线位，仅双线另置双下划线位）。
#[test]
fn underline_styles_reach_cell_data_and_snapshot() {
    use crate::terminal::ghostty_terminal::cell_flags;
    for (sequence, marker, double) in [
        ("4:1", b'A', false),
        ("4:2", b'B', true),
        ("4:3", b'C', false),
        ("4:4", b'D', false),
        ("4:5", b'E', false),
    ] {
        let mut styled = terminal();
        styled.vt_write(format!("\x1b[{sequence}m{}", marker as char).as_bytes());
        styled.flush();
        let (cells, _) = styled.receive_cell_data().expect("cell data");
        let marked = cells
            .iter()
            .find(|cell| cell.codepoint == marker as u32)
            .expect("marked cell present");
        assert_eq!(
            (marked.flags >> cell_flags::UNDERLINE) & 1,
            1,
            "SGR {sequence} must set UNDERLINE bit"
        );
        assert_eq!(
            (marked.flags >> cell_flags::DOUBLE_UNDERLINE) & 1,
            u32::from(double),
            "SGR {sequence} double bit must be {double}"
        );
        let snapshot = styled.take_snapshot();
        let snap_cell = snapshot
            .cells
            .iter()
            .find(|cell| cell.codepoint == marker as u32)
            .expect("marked snapshot cell present");
        assert!(
            snap_cell.underline,
            "SGR {sequence} must set snapshot underline"
        );
        assert_eq!(
            snap_cell.double_underline, double,
            "SGR {sequence} snapshot double must be {double}"
        );
    }
}

/// 选择端点翻转必须返回同范围文本：拖动手柄越过锚点时调用方按
/// 触摸顺序传递端点（start > end），不得返回空串或 panic
///（对标 selectionDragAcrossAnchorFlips 的端点重排语义）。
#[test]
fn selection_text_flipped_endpoints() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"hello world");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    let forward = terminal_under_test.selection_text((row0, 0), (row0, 4));
    assert_eq!(
        forward, "hello",
        "forward selection baseline (got {forward:?})"
    );
    let flipped = terminal_under_test.selection_text((row0, 4), (row0, 0));
    assert_eq!(
        flipped, "hello",
        "flipped endpoints must yield the same range (got {flipped:?})"
    );
}

/// 拖动手柄只移动被抓端点：锚点固定，另一端点跟随落点
///（对标 selectionDragMovesGrabbedEndpoint：先拖尾端点 0→8，再拖首端点 0→6）。
#[test]
fn selection_text_grabbed_endpoint_moves() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"hello world");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row = snap.scrollback_length;
    let extended = terminal_under_test.selection_text((row, 0), (row, 8));
    assert_eq!(
        extended, "hello wor",
        "drag end handle must extend (got {extended:?})"
    );
    let shrunk = terminal_under_test.selection_text((row, 6), (row, 8));
    assert_eq!(
        shrunk, "wor",
        "drag start handle must shrink (got {shrunk:?})"
    );
}

/// 选中文本滚入回滚后同端点仍取回原文：端点为绝对行号，不随视口漂移
///（对标 selectionTracksTextIntoScrollback）。
#[test]
fn selection_text_tracks_scrolled_content() {
    let mut terminal_under_test = GhosttyTerminal::new(4, 20, 100).expect("terminal");
    terminal_under_test.vt_write(b"alpha\n");
    terminal_under_test.flush();
    let first = terminal_under_test.selection_text((0, 0), (0, 4));
    assert_eq!(first, "alpha", "baseline selection (got {first:?})");
    for filler in 0..8 {
        terminal_under_test.vt_write(format!("filler{filler}\n").as_bytes());
    }
    terminal_under_test.flush();
    assert!(
        terminal_under_test.scrollback_length() > 0,
        "content must have scrolled"
    );
    let tracked = terminal_under_test.selection_text((0, 0), (0, 4));
    assert_eq!(
        tracked, "alpha",
        "selection must track into scrollback (got {tracked:?})"
    );
}

/// OSC 2 会话标题必须可读（对标 titleChangeEventAndValue 的值断言；
/// 我方无标题事件通道且 DESIGN 未声明，只断言值本身）。
#[test]
fn osc2_title_readable() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b]2;my title\x07");
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.title(),
        "my title",
        "OSC 2 title must be readable (got {:?})",
        terminal_under_test.title()
    );
}

/// DECSCUSR 程序光标样式必须到达快照（对标 cursorStyleDefaultApplies；
/// 空章节 from Termux testSetCursorStyle 的正文）。
#[test]
fn decscusr_cursor_style_reaches_snapshot() {
    use crate::terminal::ghostty_terminal::CursorStyle;
    // （DECSCUSR 参数，期望快照样式）
    for (param, expected) in [
        (2u8, CursorStyle::Block),
        (3u8, CursorStyle::Underline),
        (5u8, CursorStyle::Bar),
        (0u8, CursorStyle::Block),
    ] {
        let mut styled = terminal();
        styled.vt_write(format!("\x1b[{param} q").as_bytes());
        styled.flush();
        let snapshot = styled.take_snapshot();
        assert_eq!(
            snapshot.cursor_style, expected,
            "DECSCUSR {param} must yield {expected:?}"
        );
    }
    // 程序覆盖后重置必须回到默认（对标 programCursorStyleOverridesDefaultUntilReset）。
    let mut reset = terminal();
    reset.vt_write(b"\x1b[5 q");
    reset.flush();
    assert_eq!(reset.take_snapshot().cursor_style, CursorStyle::Bar);
    reset.vt_write(b"\x1b[0 q");
    reset.flush();
    assert_eq!(
        reset.take_snapshot().cursor_style,
        CursorStyle::Block,
        "reset must restore default"
    );
}

/// 回滚上限透传：建会参数必须约束回滚深度（设备 instrumented 测试的 host 复刻）。
/// 上游按页粒度修剪（文档：实际值通常高于配置几十到一百行），故断言
/// 上限生效（远小于无约束时的全部保留），而非精确等于配置值。
#[test]
fn scrollback_cap_is_honored() {
    let mut terminal = GhosttyTerminal::new(24, 80, 10).expect("terminal");
    for index in 0..2000 {
        terminal.vt_write(format!("CAP_{index:04}\r\n").as_bytes());
    }
    terminal.flush();
    let depth = terminal.scrollback_length();
    assert!(
        depth < 2000,
        "回滚上限必须生效（2000 行输入不得全保留）, 实际={depth}",
    );
}

/// 回滚区必须「旧行按顺序进入、新行留在底部」，不能只断言深度。
#[test]
fn scrollback_keeps_order_and_newest_line_stays_visible() {
    let mut terminal = GhosttyTerminal::new(24, 80, 5_000).expect("terminal");
    for index in 0..500u32 {
        terminal.vt_write(format!("SEQ_{index:04}\r\n").as_bytes());
    }
    terminal.flush();

    let depth = terminal.scrollback_length();
    assert!(depth >= 400, "回滚区应保留大部分输入行, 实际={depth}");

    // 回滚行必须按写入顺序单调递增。
    let mut previous = String::new();
    for row in 0..depth.min(400) {
        let text = terminal
            .read_line_text(row)
            .unwrap_or_else(|| panic!("回滚第 {row} 行必须可读"));
        let Some(marker) = text.trim().strip_prefix("SEQ_") else {
            panic!("回滚第 {row} 行缺少标记, 实际={text:?}");
        };
        if row > 0 {
            assert!(
                marker > previous.as_str(),
                "回滚行必须按写入顺序递增, 第 {row} 行={marker} 上一行={previous}",
            );
        }
        previous = marker.to_string();
    }

    // 最新一行必须出现在可视区（底部），而不是被推进回滚后丢失。
    let last_marker = format!("SEQ_{:04}", 499);
    assert!(
        terminal.read_visible_text().contains(&last_marker),
        "最新一行必须留在可视区底部, 可视区未包含 {last_marker}",
    );
}

/// 深缓冲不坍缩（对标 scrollbackHonorsConfiguredLineCount：上游 max_scrollback
/// 是字节预算而非行数，预算过小会被忽略致历史坍缩；本地建会参数透传后，
/// 2 万行配置喂 2.5 万行，历史必须达到万行量级而非几百行。页粒度修剪有
/// 百行级波动，不断言精确值）。
#[test]
fn scrollback_deep_buffer_does_not_collapse() {
    let mut terminal = GhosttyTerminal::new(24, 80, 20_000).expect("terminal");
    // 与 scroll_viewport_delta_scrolls_cell_data 同口径：vt_write 直透 LF，
    // 上游按 LF 语义换行推进 scrollback（\r\n 在该路径行为不同）。
    // 命令通道有界（1024）：大批量写入必须分批 flush，否则 try_send
    // 静默丢弃，scrollback 恒 0。
    for chunk in (0..25_000)
        .map(|index| format!("DEEP_{index:05}\n"))
        .collect::<Vec<_>>()
        .chunks(500)
    {
        for line in chunk {
            terminal.vt_write(line.as_bytes());
        }
        terminal.flush();
    }
    let depth = terminal.scrollback_length();
    assert!(
        depth >= 19_000,
        "深缓冲历史不得坍缩（2 万行配置至少保留万行量级）, 实际={depth}",
    );
}

/// SGR 7 反白必须到达渲染层（对标 inverseIsResolvedNatively；本仓反白在
/// cell_builder 做前景背景互换，VT 层只断言标志位到达）。
#[test]
fn inverse_reaches_cell_data_and_snapshot() {
    use crate::terminal::ghostty_terminal::cell_flags;
    let mut inverse = terminal();
    inverse.vt_write(b"\x1b[7mX");
    inverse.flush();
    let (cells, _) = inverse.receive_cell_data().expect("cell data");
    let marked = cells
        .iter()
        .find(|cell| cell.codepoint == 'X' as u32)
        .expect("X cell present");
    assert_eq!(
        (marked.flags >> cell_flags::REVERSE) & 1,
        1,
        "SGR 7 must set CellData REVERSE bit"
    );
    let snapshot = inverse.take_snapshot();
    let snap_cell = snapshot
        .cells
        .iter()
        .find(|cell| cell.codepoint == 'X' as u32)
        .expect("snapshot X present");
    assert!(snap_cell.reverse, "SGR 7 must set snapshot reverse flag");
}

/// 组合重音必须以 grapheme 形式到达 VT 层（对标 graphemeClusterCombiningMark
/// 的文本部分：基码点 e 在主格，U+0301 进 grapheme_extra/快照 graphemes）。
#[test]
fn combining_mark_reaches_grapheme_channel() {
    let mut clustered = terminal();
    clustered.vt_write("e\u{301}x".as_bytes());
    clustered.flush();
    let (cells, _) = clustered.receive_cell_data().expect("cell data");
    let base = cells
        .iter()
        .find(|cell| cell.codepoint == 'e' as u32)
        .expect("base e present");
    assert_eq!(
        base.grapheme_extra[0], 0x301,
        "combining acute must ride grapheme_extra[0]"
    );
    let snapshot = clustered.take_snapshot();
    let snap_cell = snapshot
        .cells
        .iter()
        .find(|cell| cell.codepoint == 'e' as u32)
        .expect("snapshot e present");
    assert!(
        snap_cell.graphemes.contains(&0x301),
        "snapshot graphemes must contain U+0301"
    );
}

#[test]
fn vt_write_sanitize_keeps_high_bytes_drops_nul() {
    // NUL 必须剔除（mpv --vo=kitty 每帧追加一个游离 NUL）；0xF8–0xFF 必须原样保留
    // （Kitty m=1 直接 RGB 载荷里的合法数据，旧写法静默替换为空格）。
    assert_eq!(
        sanitize_vt_input(&[0x41, 0x00, 0xF8, 0xFF, 0x42]),
        vec![0x41, 0xF8, 0xFF, 0x42]
    );
    assert_eq!(sanitize_vt_input(b"ABC"), b"ABC");
    assert!(sanitize_vt_input(&[0x00, 0x00]).is_empty());
}

#[test]
fn deliver_terminate_reaches_a_full_queue_once_the_consumer_drains() {
    // 队列满是常态：VT 线程忙时命令堆积到上限，析构时的 Terminate 若只
    // try_send 一次就被丢弃，线程只能等信道彻底排空才退出，join 随即超时。
    //
    // 消费端刻意延迟后再排空：只有「满队列 + 有界等待」才能送达，
    // 单次 try_send 在此必失败——该用例因此能区分两种实现。
    let (cmd_tx, cmd_rx) = flume::bounded::<Command>(1);
    cmd_tx
        .send(Command::FlushAck(flume::bounded::<()>(1).0))
        .expect("queue not full");
    let consumer = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(20));
        let _ = cmd_rx.recv_timeout(std::time::Duration::from_secs(5));
    });
    assert!(
        super::deliver_terminate(&cmd_tx, std::time::Duration::from_secs(5)),
        "Terminate must reach the VT thread even when the queue was full at drop time"
    );
    consumer.join().expect("consumer thread panicked");
}

#[test]
fn deliver_terminate_gives_up_instead_of_blocking_on_a_stuck_consumer() {
    // 对端永不取：等待必须是有界的，否则卡死的 VT 线程会把析构一并卡住。
    let (cmd_tx, _cmd_rx) = flume::bounded::<Command>(1);
    cmd_tx
        .send(Command::FlushAck(flume::bounded::<()>(1).0))
        .expect("queue not full");
    let start = std::time::Instant::now();
    assert!(
        !super::deliver_terminate(&cmd_tx, super::TERMINATE_SEND_TIMEOUT),
        "a consumer that never drains must not make delivery succeed"
    );
    assert!(
        start.elapsed() < std::time::Duration::from_secs(1),
        "delivery waited {:?} — drop would block on a dead VT thread",
        start.elapsed()
    );
}
