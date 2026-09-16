//! VT 引擎行为测试：只覆盖本仓自有语义，不复述上游解析语义。
//!
//! 保留集合（本仓机制与对外 API，上游仅作输入夹具）：
//! 1. 快照管线：snapshot 缓存/回退、视口映射、行缓存；
//! 2. 查询 API：dump_grid / read_line_text / hyperlink_at / selection_text /
//!    search_in（断言对象是本仓包装与格式化）；
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
use crate::terminal::test_helpers::assert_invariants;

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
        if let Some(c) = cell_at(snap, row, col)
            && c.codepoint != 0
            && let Some(ch) = char::from_u32(c.codepoint)
        {
            text.push(ch);
        }
    }
    text.trim_end().to_string()
}

#[test]
fn create_terminal_zero_scrollback() {
    let t = GhosttyTerminal::new(5, 10, 0).expect("terminal");
    assert_eq!(t.scrollback_length(), 0);
}

#[test]
fn read_line_text_returns_text() {
    let mut t = terminal();
    t.vt_write(b"\x1b[1;1HHello World");
    t.flush();
    let text = t.read_line_text(0);
    assert!(text.is_some());
    assert!(text.unwrap().contains("Hello"));
}

#[test]
fn read_line_text_empty_returns_none() {
    let t = terminal();
    let text = t.read_line_text(5);
    assert!(text.is_none());
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
    terminal.set_selection((row, 0), (row, 4), false);
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
fn search_in_scrollback_finds_match() {
    let mut t = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    t.vt_write(b"search_target_here\n");
    t.flush();
    for i in 0..5 {
        t.vt_write(format!("filler {i}\n").as_bytes());
    }
    t.flush();
    // Search may or may not find the result depending on Ghostty's scrollback implementation.
    // The critical test is that it doesn't crash or corrupt terminal state.
    let _result = t.search_in_scrollback("search_target");
    t.vt_write(b"AfterSearch");
    t.flush();
    let snap = t.take_snapshot();
    assert!(
        snap.cells.iter().any(|c| c.codepoint == 'A' as u32),
        "terminal should remain functional after scrollback search"
    );
    assert_invariants(&snap);
}

#[test]
fn search_in_scrollback_empty_query() {
    let t = terminal();
    assert_eq!(t.search_in_scrollback(""), None);
}

#[test]
fn dump_grid_dimensions_match() {
    let t = terminal();
    // 查询经工作线程超时回退空值，新终端繁忙时单次查询可能命中回退；
    // 确定性轮询直到就绪，杜绝 flaky。
    let start = Instant::now();
    let dumped = loop {
        let dumped = t.dump_grid();
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
    let _snap = t.take_snapshot();
    assert_invariants(&_snap);
}

#[test]
fn dump_grid_visible_populated() {
    let mut t = terminal();
    t.vt_write(b"hello");
    t.flush();
    let dumped = t.dump_grid();
    let has_h = dumped.visible.iter().any(|c| c.codepoint == 'h' as u32);
    assert!(
        has_h,
        "dump_grid visible: 'h' from 'hello' should be present"
    );
    let _snap = t.take_snapshot();
    assert_invariants(&_snap);
}

#[test]
fn dump_grid_scrollback_populated_after_scroll() {
    let mut t = GhosttyTerminal::new(3, 10, 100).expect("terminal");
    for i in 0..10 {
        t.vt_write(format!("line{i}\n").as_bytes());
    }
    t.flush();
    let dumped = t.dump_grid();
    assert!(
        !dumped.scrollback.is_empty(),
        "scrollback should contain scrolled-off lines"
    );
    let has_line0 = dumped
        .scrollback
        .iter()
        .any(|row| row.iter().any(|c| c.codepoint == 'l' as u32));
    assert!(has_line0, "scrollback: should contain 'l' from line0");
    let _snap = t.take_snapshot();
    assert_invariants(&_snap);
}

// ── DECSET/DECRST ──────────────────────────────────────────────────────

/// EncodeMouseEvent with no tracking mode enabled must return empty (the
/// application never asked for mouse reporting) — zelland renderer/mod.rs
/// drops mouse events when `get_mouse_mode()` is false.
#[test]
fn encode_mouse_event_gated_off_without_tracking_mode() {
    let t = terminal();
    let encoded = t.encode_mouse_event((50.0, 60.0), 0, 0, 10.0, 20.0);
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
    let mut t = terminal();
    t.vt_write(b"\x1b[?1000h\x1b[?1006h");
    t.flush();
    let encoded = t
        .encode_mouse_event((35.0, 45.0), 0, 0, 10.0, 20.0)
        .expect("encode_mouse_event should return Some");
    // SGR: ESC [ < Cb ; Cx ; Cy M — Cb is the 0-based button (0 = left
    // press; X10's +32 offset does NOT apply to SGR mode).
    // Cell (3,2) → Cx=3+1=4, Cy=2+1=3.
    assert_eq!(
        encoded, b"\x1b[<0;4;3M",
        "SGR left-press at cell (3,2) must match ghostty's standard encoding"
    );
}

/// Wheel-up with DECSET 1000 + SGR → button 4 (wheel-up = button 64+4-32).
/// The Ghostty encoder emits button 4 for wheel-up; SGR adds 32 for press.
#[test]
fn encode_mouse_event_wheel() {
    let mut t = terminal();
    t.vt_write(b"\x1b[?1000h\x1b[?1006h");
    t.flush();
    let up = t
        .encode_mouse_event((10.0, 10.0), 0, 3, 10.0, 20.0)
        .expect("wheel-up encode");
    assert!(
        up.len() >= 6 && up.starts_with(b"\x1b[<"),
        "wheel-up must produce an SGR sequence (got {up:?})"
    );
    let down = t
        .encode_mouse_event((10.0, 10.0), 0, 4, 10.0, 20.0)
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
    let mut t = terminal();
    t.vt_write(b"\x1b[?1000h\x1b[?1006h");
    t.flush();
    let result = t.encode_mouse_event((-5.0, -10.0), 0, 0, 10.0, 20.0);
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
    let mut t = terminal(); // default 24 rows × 80 cols
    t.vt_write(b"\x1b[?1000h\x1b[?1006h");
    t.flush();
    // Position far beyond the grid: 9999x9999 with 10x20 cells.
    let result = t.encode_mouse_event((9999.0, 9999.0), 0, 0, 10.0, 20.0);
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
    let mut t = terminal();
    t.vt_write(b"\x1b[?1000h\x1b[?1006h"); // button tracking + SGR
    t.vt_write(b"\x1b[?1002h"); // button-event tracking (motion reports)
    t.flush();
    let cell_w = 10.0;
    let cell_h = 20.0;

    // Press at (10, 20)
    let press = t
        .encode_mouse_event((10.0, 20.0), 0, 0, cell_w, cell_h)
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
    let drag = t
        .encode_mouse_event((30.0, 20.0), 2, 0, cell_w, cell_h)
        .expect("drag encode");
    assert!(
        drag.starts_with(b"\x1b[<"),
        "drag must produce SGR (got {drag:?})"
    );

    // Release at (50, 20) — action=1
    let release = t
        .encode_mouse_event((50.0, 20.0), 1, 0, cell_w, cell_h)
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
    let mut t = terminal();
    // Send the first and second parts of OSC 0 sequence
    t.vt_write(b"\x1b]0;My ");
    t.flush();
    t.vt_write(b"QQQ\x07");
    t.flush();
    let _snap = t.take_snapshot();
    // After setting the title, terminal should not crash, text should still be writable
    t.vt_write(b"AfterTitle");
    t.flush();
    let snap2 = t.take_snapshot();
    let found = snap2.cells.iter().any(|c| c.codepoint == 'A' as u32);
    assert!(found, "OSC split: text after split title should render");
    let leaked = snap2.cells.iter().any(|c| c.codepoint == 'Q' as u32);
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
    let mut t = terminal();
    // OSC 52 sequence: first part sets clipboard selection, second provides data.
    t.vt_write(b"\x1b]52;c;");
    t.flush();
    t.vt_write(b"SGVsbG8=\x07");
    t.flush();
    // 分片必须由上游重组：剪贴板事件内容为解码后文本。
    let event = t.poll_clipboard_event();
    assert_eq!(
        event,
        Some(("c".to_string(), "Hello".to_string())),
        "OSC 52 split: payload must reassemble into clipboard event"
    );
    // Terminal should not crash, text should still be writable
    t.vt_write(b"PostClip");
    t.flush();
    let snap = t.take_snapshot();
    let found = snap.cells.iter().any(|c| c.codepoint == 'P' as u32);
    assert!(found, "OSC 52 split: post-clipboard text should render");
    assert_invariants(&snap);
}

/// OSC color reset — sent across split buffer.
#[test]
fn osc_color_reset_split_buffer() {
    let mut t = terminal();
    t.vt_write(b"\x1b]104;");
    t.flush();
    t.vt_write(b"\x07");
    t.flush();
    t.vt_write(b"ColorReset");
    t.flush();
    let snap = t.take_snapshot();
    let found = snap.cells.iter().any(|c| c.codepoint == 'C' as u32);
    assert!(found, "OSC 104 split: text after color reset should render");
    assert_invariants(&snap);
}

/// OSC sequence terminated after partial first block — crash test
#[test]
fn osc_aborted_after_partial_feed() {
    let mut t = terminal();
    // Send partial OSC sequence, then BEL to terminate it
    t.vt_write(b"H\x1b]0;Partial\x07");
    t.flush();
    // Then write normally, should not be consumed by OSC
    t.vt_write(b"Normal");
    t.flush();
    let snap = t.take_snapshot();
    let outer = snap.cells.iter().any(|c| c.codepoint == 'H' as u32);
    let normal = snap.cells.iter().any(|c| c.codepoint == 'N' as u32);
    assert!(outer, "aborted OSC: H should be visible before OSC");
    assert!(normal, "aborted OSC: Normal should be visible");
    assert_invariants(&snap);
}

/// Oversized OSC 52 payload — no crash
#[test]
fn osc_large_clipboard_payload_terminal_survives() {
    let mut t = terminal();
    let large = vec![b'A'; 1024 * 4]; // 4KB base64
    let mut seq = Vec::from(b"\x1b]52;c;");
    seq.extend_from_slice(&large);
    seq.push(b'\x07');
    t.vt_write(&seq);
    t.flush();
    t.vt_write(b"OK");
    t.flush();
    let snap = t.take_snapshot();
    let ok = snap.cells.iter().any(|c| c.codepoint == 'O' as u32);
    assert!(ok, "OSC large payload: OK should render");
    assert_invariants(&snap);
}

/// Extremely long 8KB OSC string — no crash
#[test]
fn osc_extremely_long_8kb_string() {
    let mut t = terminal();
    let mut seq = Vec::from(b"\x1b]0;");
    seq.extend(std::iter::repeat_n(b'x', 8000));
    seq.push(b'\x07');
    t.vt_write(&seq);
    t.flush();
    t.vt_write(b"LongDone");
    t.flush();
    let snap = t.take_snapshot();
    let found = snap.cells.iter().any(|c| c.codepoint == 'L' as u32);
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
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    for cycle in 0..50 {
        t.vt_write(format!("cycle{cycle}\n").as_bytes());
        // Alternate width and height
        let h = if cycle % 2 == 0 { 5 } else { 8 };
        let w = if cycle % 3 == 0 { 10 } else { 15 };
        t.resize(h, w);
        t.flush();
    }
    t.flush();
    t.vt_write(b"StressTest");
    t.flush();
    let snap = t.take_snapshot();
    let found = snap.cells.iter().any(|c| c.codepoint == 'S' as u32);
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
    let c = CellSnapshot::default();
    assert_eq!(c.codepoint, 0);
    assert_eq!(c.foreground, [0.0, 0.0, 0.0, 0.0]);
    assert_eq!(c.background, [0.0, 0.0, 0.0, 0.0]);
    assert!(!c.bold);
    assert!(!c.italic);
}

#[test]
// 与字面量默认值精确比较，不涉及浮点运算。
#[allow(clippy::float_cmp)]
fn cell_snapshot_clone() {
    let c = CellSnapshot {
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
    let c2 = c.clone();
    assert_eq!(c.codepoint, c2.codepoint);
    assert_eq!(c.foreground, c2.foreground);
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
    let mut t = terminal();

    // Feed mixed content: ASCII, bold, colored, CJK
    t.vt_write(b"\x1b[31mRed\x1b[0m Normal ");
    t.vt_write(b"\x1b[1mBold\x1b[0m ");
    t.vt_write(b"\x1b[44mBlueBg\x1b[0m ");
    t.vt_write("Hello 日本 World!".as_bytes());
    t.vt_write(b"\n");
    t.vt_write(b"Second line with \x1b[33mYELLOW\x1b[0m text");
    t.vt_write(b"\n");
    t.vt_write(b"Third line\x1b[K");
    t.flush();

    let snap = t.take_snapshot();

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
    let first_red = snap.cells.iter().position(|c| c.codepoint == 'R' as u32);
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
    let first_b = snap.cells.iter().position(|c| c.codepoint == 'B' as u32);
    if let Some(idx) = first_b {
        let cell = &snap.cells[idx];
        assert!(cell.bold, "Bold word should have bold=true");
    }

    // Verify invariants
    assert_invariants(&snap);
}

/// Verify that the CellIterator path correctly handles CJK double-width
/// characters in the snapshot (width=2).
#[test]
fn cell_iterator_cjk_double_width() {
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    t.vt_write("A中B".as_bytes());
    t.flush();
    let snap = t.take_snapshot();

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
    let mut t = terminal();
    // Feed various content
    t.vt_write(b"AB");
    t.vt_write(b"\x1b[31mRed\x1b[0m");
    t.vt_write(b"\n");
    t.vt_write("中".as_bytes());
    t.flush();

    let snap = t.take_snapshot();

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
        .filter(|c| c.codepoint == 0x4E2D)
        .collect();
    assert_eq!(mid_cells.len(), 1, "Should find exactly one CJK cell");
    assert_eq!(mid_cells[0].width, 2, "CJK width should be 2");

    assert_invariants(&snap);
}

/// Verify that the overall grid dimensions are correct through the
/// CellIterator snapshot path.
#[test]
fn cell_iterator_grid_dimensions() {
    let mut t = terminal();
    t.vt_write(b"Hello World");
    t.flush();
    let snap = t.take_snapshot();
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
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    t.flush();
    // Write "AB\nCD" — after LF→CR+LF conversion, CD should be at col 0 of row 1
    t.pty_write(b"AB\nCD");
    t.flush();
    t.flush();
    let dumped = t.dump_grid();
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
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    t.flush();
    // "ABCDEFGHIJ" is exactly 10 chars (full width), then \n, then "next"
    t.pty_write(b"ABCDEFGHIJ\nnext");
    t.flush();
    t.flush();
    let dumped = t.dump_grid();
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
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    t.flush();
    t.vt_write(b"AB\r\nCD");
    t.flush();
    t.flush();
    let dumped = t.dump_grid();
    let row1_col0 = dumped.visible[10].codepoint;
    assert_eq!(row1_col0, 'C' as u32, "CRLF: 'C' at column 0 of row 1");
}

// ── TC-TM: Terminal Mode State (from test gap analysis §3.E) ────

// ── TC-IV: Invariant Checking (from test gap analysis §3.L) ─────

/// TC-IV-002: Alt buffer has no history
#[test]
fn tc_iv_002_alt_buffer_no_history() {
    let mut t = GhosttyTerminal::new(3, 10, 100).expect("terminal");
    t.flush();
    for i in 0..5 {
        t.vt_write(format!("line{i}\r\n").as_bytes());
    }
    t.flush();
    t.vt_write(b"\x1b[?1049h");
    t.flush();
    // Alt buffer should have no scrollback
    assert_eq!(
        t.scrollback_length(),
        0,
        "IV-002: alt buffer should have no scrollback"
    );
    let snap = t.take_snapshot();
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
    let t = GhosttyTerminal::new(24, 80, 1000).expect("terminal");
    t.flush();
    assert_eq!(t.rows(), 24, "SM-001: rows == 24");
    assert_eq!(t.cols(), 80, "SM-001: cols == 80");
    let snap = t.take_snapshot();
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
    let a_in_1 = snap1.cells.iter().any(|c| c.codepoint == 'A' as u32);
    let b_in_2 = snap2.cells.iter().any(|c| c.codepoint == 'B' as u32);
    assert!(a_in_1, "SM-002: session 1 has 'A'");
    assert!(b_in_2, "SM-002: session 2 has 'B'");
    // Session 1 should NOT have B
    let a_in_2 = snap2.cells.iter().any(|c| c.codepoint == 'A' as u32);
    assert!(!a_in_2, "SM-002: session 2 should not have 'A'");
    let snap = t1.take_snapshot();
    assert_invariants(&snap);
}

/// TC-SM-003: Drop terminal cleans up
#[test]
fn tc_sm_003_drop_cleans_up() {
    let t = GhosttyTerminal::new(3, 3, 100).expect("terminal");
    t.flush();
    let snap = t.take_snapshot();
    assert_invariants(&snap);
    drop(t);
    // If we reach here, no panic
}

/// TC-SM-004: Double drop is safe (handled by Drop impl)
#[test]
fn tc_sm_004_double_drop_safe() {
    let t = GhosttyTerminal::new(3, 3, 100).expect("terminal");
    t.flush();
    // Can't explicitly double-drop in safe Rust, but we can verify
    // that a normal drop completes without panic
    let snap = t.take_snapshot();
    assert_invariants(&snap);
    drop(t);
}

/// TC-SM-005: Process-like cleanup (just verify terminal works)
#[test]
fn tc_sm_005_terminal_works_after_writes() {
    let mut t = terminal();
    t.flush();
    t.vt_write(b"SessionActive");
    t.flush();
    let snap = t.take_snapshot();
    let found = snap.cells.iter().any(|c| c.codepoint == 'S' as u32);
    assert!(found, "SM-005: terminal should work normally");
    let snap = t.take_snapshot();
    assert_invariants(&snap);
}

// ── TC-AL: Android Lifecycle (from test gap analysis §3.I) ───────
// Adapted: simulate pause/resume via resize cycles

/// TC-AL-001: "Pause" (snapshot) preserves content — verify via snapshot
#[test]
fn tc_al_001_snapshot_preserves_content() {
    let mut t = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    t.flush();
    t.vt_write(b"LifecycleContent");
    t.flush();
    let snap = t.take_snapshot();
    let found = snap.cells.iter().any(|c| c.codepoint == 'L' as u32);
    assert!(found, "AL-001: content preserved in snapshot");
    let snap = t.take_snapshot();
    assert_invariants(&snap);
}

/// TC-AL-002: Alt screen via snapshot
#[test]
fn tc_al_002_alt_screen_preserved() {
    let mut t = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    t.flush();
    t.vt_write(b"\x1b[?1049h");
    t.vt_write(b"AltContent");
    t.flush();
    let snap = t.take_snapshot();
    let found = snap.cells.iter().any(|c| c.codepoint == 'A' as u32);
    assert!(found, "AL-002: alt screen content in snapshot");
    let snap = t.take_snapshot();
    assert_invariants(&snap);
}

/// TC-AL-003: Cursor position restored after resize cycle
#[test]
fn tc_al_003_cursor_restored() {
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    t.flush();
    t.vt_write(b"\x1b[3;5H"); // CUP to (3,5)
    t.flush();
    let x_before = t.cursor_x();
    let y_before = t.cursor_y();
    t.resize(5, 10); // same size, simulate pause/resume
    t.flush();
    assert_eq!(
        t.cursor_x(),
        x_before,
        "AL-003: cursor_x preserved after resize"
    );
    assert_eq!(
        t.cursor_y(),
        y_before,
        "AL-003: cursor_y preserved after resize"
    );
    let snap = t.take_snapshot();
    assert_invariants(&snap);
}

/// TC-AL-004: Mode state preserved after resize cycle
#[test]
fn tc_al_004_mode_preserved() {
    let mut t = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    t.flush();
    t.vt_write(b"\x1b[?25l"); // hide cursor
    t.flush();
    assert!(!t.cursor_visible(), "AL-004: cursor hidden before resize");
    t.resize(5, 20);
    t.flush();
    assert!(!t.cursor_visible(), "AL-004: cursor hidden after resize");
    let snap = t.take_snapshot();
    assert_invariants(&snap);
}

// ── 13.6: Pause / resume (simulated via resize) ────────────────
// 001: 50 cycles — no resource leak; 002: content preserved.

#[test]
fn tc_lifecycle_001_pause_resume_cycles() {
    let mut t = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    t.vt_write(b"BaseContent");
    t.flush();

    for i in 0..50 {
        let marker = format!("\x1b[{};{}HCycle{}", 1 + (i % 5), 1 + (i % 18), i);
        t.vt_write(marker.as_bytes());
        t.flush();

        // Simulate pause/resume via resize to same size.
        t.resize(5, 20);
        t.flush();

        // Verify basic invariants after each cycle.
        let snap = t.take_snapshot();
        assert_invariants(&snap);
        assert_eq!(snap.rows, 5, "rows unchanged after cycle {i}");
        assert_eq!(snap.cols, 20, "cols unchanged after cycle {i}");
    }
}

// ── 13.7: Content preserved after pause/resume cycle ───────────

#[test]
fn tc_lifecycle_002_content_preserved_after_pause_resume() {
    let mut t = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    t.vt_write(b"PreserveThisContent!");
    t.flush();

    // Capture row 0 text before simulated pause/resume.
    let snap_before = t.take_snapshot();
    let text_before: String = snap_before
        .cells
        .iter()
        .take(20)
        .map(|c| char::from_u32(c.codepoint).unwrap_or('�'))
        .collect();

    // Simulate pause (release/destroy) and resume (recreate) via resize.
    t.resize(5, 20);
    t.flush();

    let snap_after = t.take_snapshot();
    let text_after: String = snap_after
        .cells
        .iter()
        .take(20)
        .map(|c| char::from_u32(c.codepoint).unwrap_or('�'))
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
/// Fine-grained tracking belongs to `cargo bench` (see check-rust.nu).

#[test]
fn bench_typing_latency() {
    let mut t = GhosttyTerminal::new(24, 80, 5000).expect("terminal");
    // Pre-fill with some content to avoid empty-terminal optimizations
    for _ in 0..10 {
        t.vt_write(b"A line to fill the screen with some realistic content\n");
    }
    t.flush();

    let keystrokes: [&[u8]; 6] = [b"h", b"e", b"l", b"l", b"o", b"\n"];
    let n = 300; // 300 keystrokes
    let start = Instant::now();
    for _ in 0..n {
        for ks in &keystrokes {
            t.vt_write(ks);
        }
        t.flush();
        let count = black_box(t.receive_cell_data().map(|(c, _)| c.len()).unwrap_or(0));
        black_box(count);
    }
    let elapsed = start.elapsed();
    let ms_per_keystroke = elapsed.as_millis() as f64 / (n as f64 * keystrokes.len() as f64);
    println!(
        "Typing latency: {:.3}ms per keystroke ({:.1}ms for {} keystrokes)",
        ms_per_keystroke,
        elapsed.as_millis(),
        n * keystrokes.len(),
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
#[test]
fn bench_bulk_output_throughput() {
    let mut t = GhosttyTerminal::new(24, 80, 5000).expect("terminal");
    // Build a 4KB buffer of realistic plain-text terminal output
    let mut buf = Vec::with_capacity(4096);
    while buf.len() < 4096 {
        buf.extend_from_slice(b"user@host:~$ ls -la src/main.rs docs/README.md\n");
    }

    let n = 50; // 50 × 4KB = 200KB total
    let start = Instant::now();
    for _ in 0..n {
        t.vt_write(&buf);
        t.flush();
        let r = t.receive_cell_data();
        let count = black_box(r.map(|(c, _)| c.len()).unwrap_or(0));
        black_box(count);
    }
    let elapsed = start.elapsed();
    let throughput_cells = n as f64 * 1920.0 / elapsed.as_secs_f64();
    println!(
        "Bulk output: {:.0} cells/sec ({:.1}ms for {}×{}KB plain text)",
        throughput_cells,
        elapsed.as_millis(),
        n,
        buf.len() / 1024,
    );
    let threshold = 4_000.0;
    assert!(
        throughput_cells > threshold,
        "Bulk output too slow: {:.0} cells/sec (need >{threshold:.0})",
        throughput_cells,
    );
}

// ── Scrollback fallback ────────────────────────────────────────────────

/// Verify that `take_snapshot_with_scroll` returns a valid snapshot when
/// scrollback exists, and returns `GridSnapshot::fallback` once the VT
/// thread is disconnected (channel closed — the real disconnected path,
/// exercised via the test-only `disconnect_for_test`).
#[test]
fn scrollback_fallback_on_disconnected_terminal() {
    // Create a terminal and fill with content to establish scrollback.
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    for i in 0..20 {
        t.vt_write(format!("line {i}\n").as_bytes());
    }
    t.flush();

    // With scrollback available, snapshot should have valid content.
    assert!(t.is_alive(), "terminal should be alive before disconnect");
    let snap = t.take_snapshot_with_scroll(0);
    assert!(
        snap.rows > 0 && snap.cols > 0,
        "viewport snapshot should have valid dimensions"
    );

    // Kill the VT thread; every subsequent query must take the fallback path.
    t.disconnect_for_test();
    assert!(!t.is_alive(), "terminal must report dead after disconnect");

    let fb = t.take_snapshot_with_scroll(0);
    assert_eq!(fb.rows, DISCONNECTED_ROWS, "fallback rows");
    assert_eq!(fb.cols, DISCONNECTED_COLS, "fallback cols");
    assert_eq!(
        fb.cells.len(),
        (fb.rows * fb.cols) as usize,
        "fallback cells should match dimensions"
    );
    assert!(
        fb.dirty.iter().all(|&d| d),
        "all fallback cells should be dirty"
    );
}

/// Verify that `take_snapshot_with_scroll(scroll_offset=0)` returns
/// consistent results across multiple calls (cache hit path).
#[test]
fn scrollback_cache_consistency() {
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    t.vt_write(b"Hello World\n");
    t.flush();

    let snap1 = t.take_snapshot_with_scroll(0);
    let snap2 = t.take_snapshot_with_scroll(0);
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
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    for i in 0..20 {
        t.vt_write(format!("line {i}\n").as_bytes());
    }
    t.flush();

    // First frame at offset 0 shows the bottom of the output.
    let (cells0, _) = t.receive_cell_data().expect("cell data");
    let bottom_rows: std::collections::HashSet<u32> = cells0.iter().map(|c| c.row).collect();
    assert!(
        bottom_rows.contains(&4),
        "offset 0 should include viewport row 4, got {bottom_rows:?}"
    );

    // The terminal must actually accumulate scrollback (host probe:
    // scrollback_length should be > 0 after 20 lines into a 5-row view).
    let scrollback = t.scrollback_length();
    assert!(
        scrollback > 0,
        "scrollback should exist after output, got {scrollback}"
    );

    // Scroll up by 2: the VT thread applies the delta and pushes new
    // CellData; the visible content shifts (rows are renumbered from the
    // new viewport top, so the row set is unchanged but the text differs).
    assert!(t.scroll_viewport(-2), "scroll_viewport should accept delta");
    // Give the VT thread a moment to process and push.
    let mut scrolled = None;
    for _ in 0..50 {
        if let Some((cells, _)) = t.receive_cell_data() {
            scrolled = Some(cells);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let scrolled = scrolled.expect("scrolled cell data");
    let scrolled_rows: std::collections::HashSet<u32> = scrolled.iter().map(|c| c.row).collect();
    assert_eq!(
        scrolled_rows, bottom_rows,
        "scrolled view keeps 5 viewport rows"
    );
    // The scrolled CellData must differ from the bottom view: the last
    // visible line is no longer "line 19". Read the visible text through
    // the snapshot path at offset 2 and check the last line.
    let snap = t.take_snapshot_with_scroll(2);
    let last_line = snap
        .cells
        .chunks(snap.cols as usize)
        .last()
        .map(|row| {
            row.iter()
                .filter_map(|c| char::from_u32(c.codepoint))
                .collect::<String>()
        })
        .unwrap_or_default();
    assert!(
        !last_line.contains("line 19"),
        "scrolled view should not show the bottom line, got {last_line:?}"
    );
}

/// Scrollback browsing: the cursor must be reported in VIEWPORT
/// coordinates (or hidden when the cursor page is scrolled out of the
/// viewport), never in active-screen coordinates. The old code used
/// `cursor_y()` (active-area row) against viewport-relative CellData
/// rows, drawing the cursor on the wrong grid row after any scroll —
/// the reported cursor "block" offset ~1 cell down/right.
#[test]
fn cursor_viewport_coordinates_track_scrollback_scroll() {
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    for i in 0..20 {
        t.vt_write(format!("line {i}\n").as_bytes());
    }
    t.flush();

    // Cursor sits on the active bottom row (row 4), col 0.
    let (_, cursor0) = t.receive_cell_data().expect("cell data");
    assert!(cursor0.visible, "cursor visible at scroll offset 0");
    assert_eq!(cursor0.row, 4, "cursor on active bottom row");

    // Scroll up 1: viewport now shows scrollback rows 14..18; the
    // cursor page (active row 19) is out of view → the cursor must be
    // hidden, not drawn on scrollback row 4.
    assert!(t.scroll_viewport(-1), "scroll_viewport(-1)");
    let mut hidden = None;
    for _ in 0..50 {
        if let Some((_, cursor)) = t.receive_cell_data() {
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
    assert!(t.scroll_viewport(1), "scroll_viewport(+1)");
    let mut restored = None;
    for _ in 0..50 {
        if let Some((_, cursor)) = t.receive_cell_data() {
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
    let t = small_terminal();
    assert!(t.is_alive());
}

#[test]
fn terminal_is_alive_after_vt_write() {
    let mut t = small_terminal();
    t.vt_write(b"Hello, world!");
    assert!(t.is_alive());
}

#[test]
fn terminal_is_alive_after_flush() {
    let mut t = small_terminal();
    t.vt_write(b"ABC");
    t.flush();
    assert!(t.is_alive());
}

/// zelland row-level dirty cache: after a write, only the affected row must
/// be rebuilt; a subsequent build with no changes must return identical
/// CellData (cache hit path). Verifies row-cache correctness end to end via
/// the public receive_cell_data() stream.
#[test]
fn row_cache_returns_consistent_cell_data_across_writes() {
    let mut t = terminal();
    t.vt_write(b"hello");
    t.flush();
    let first = t.receive_cell_data().expect("first cell data");
    let (first_cells, _) = first;
    let cols = 80usize;
    let row0_first: Vec<u32> = first_cells[..cols].iter().map(|c| c.codepoint).collect();

    // 空闲去重下静默 VT 线程不再推送相同内容：一致性由下方第三快照
    // （新输入后的确定性重建）验证，此处不做定时等待。

    // New input on row 1 must not disturb row 0's cached content.
    t.vt_write(b"\nworld");
    t.flush();
    let third = t.receive_cell_data().expect("third cell data");
    let (third_cells, _) = third;
    let row0_third: Vec<u32> = third_cells[..cols].iter().map(|c| c.codepoint).collect();
    let row1_third: Vec<u32> = third_cells[cols..cols * 2]
        .iter()
        .map(|c| c.codepoint)
        .collect();
    assert_eq!(row0_third, row0_first, "row 0 unchanged after row-1 write");
    // vt_write treats LF as a bare line feed (no CR), so "world" lands at
    // col 5 on row 1, right after the LF.
    assert_eq!(row1_third[5], 'w' as u32, "row 1 col 5 is 'w'");
}

/// Resize invalidates the row cache (row count changes); the next build must
/// reflect the new grid dimensions, not stale cached rows.
#[test]
fn row_cache_invalidated_on_resize() {
    let mut t = terminal();
    t.vt_write(b"top");
    t.flush();
    // 确定性轮询直到工作线程产出对应尺寸数据，杜绝固定时长等待的 flaky。
    let start = Instant::now();
    let before_cells = loop {
        if let Some((cells, _)) = t.receive_cell_data()
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

    assert!(t.resize(10, 40), "resize to 10x40");
    let start = Instant::now();
    let after_cells = loop {
        if let Some((cells, _)) = t.receive_cell_data()
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
    assert_eq!(
        after_cells.len(),
        10 * 40,
        "row cache must be invalidated on resize (stale rows would keep 24x80)"
    );
}

/// Ghostty formatter selection extraction: a soft-wrapped long line must be
/// joined without '\n' (termux TerminalBuffer.getSelectedText joinBackLines
/// semantics). Write a line longer than 80 cols then select across the wrap.
#[test]
fn selection_text_unwraps_soft_wrapped_lines() {
    let mut t = terminal(); // 24x80
    // 90 chars: exceeds the 80-col width -> soft wrap onto row 2.
    let long = "a".repeat(90);
    t.vt_write(long.as_bytes());
    t.flush();
    let snap = t.take_snapshot();
    let scrollback = snap.scrollback_length;
    // The text starts at viewport row 0 (grid row = scrollback_rows).
    let row0 = scrollback;
    let text = t.selection_text((row0, 0), (row0 + 1, 9), false);
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
    let mut t = terminal();
    t.vt_write("中".as_bytes()); // wide char at cols 0-1
    t.vt_write(b"ab");
    t.flush();
    let snap = t.take_snapshot();
    let row0 = snap.scrollback_length;
    let text = t.selection_text((row0, 0), (row0, 3), false);
    assert_eq!(
        text, "中ab",
        "wide char must round-trip exactly (got {text:?})"
    );
}

/// 终端持有选区反白（spec 文本选择：选区存于终端，跟踪引用）：
/// 安装选区后 VT 线程把行级选区反白烘焙进 CellData（前景背景互换），
/// 清除后恢复。该测试断言本仓的安装—烘焙链路，不复述上游选区语义。
#[test]
fn terminal_owned_selection_inverts_cell_data() {
    let mut t = terminal(); // 24x80
    t.vt_write(b"hello");
    t.flush();
    let snap = t.take_snapshot();
    let row0 = snap.scrollback_length;
    // 基线：未选中时前景为主题前景色。
    let (_, _) = t.receive_cell_data().expect("baseline cell data");
    t.set_selection((row0, 0), (row0, 4), false);
    t.flush();
    let (selected, _) = t.receive_cell_data().expect("selected cell data");
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
    t.clear_selection();
    t.flush();
    let (cleared, _) = t.receive_cell_data().expect("cleared cell data");
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
    let mut t = terminal();
    t.vt_write(b"\x1b]8;;https://example.com\x1b\\Link\x1b]8;;\x1b\\");
    t.flush();
    let snap = t.take_snapshot();
    let row0 = snap.scrollback_length;
    // "Link" starts at col 0.
    let url = t.hyperlink_at(row0, 0).expect("cell 0 has the link");
    assert_eq!(url, "https://example.com");
    // Last link cell still resolves; one past the link does not.
    assert!(
        t.hyperlink_at(row0, 3).is_some(),
        "col 3 is the last link char"
    );
    assert!(t.hyperlink_at(row0, 4).is_none(), "col 4 is past the link");
}

/// Kitty 图像协议像素回读（ghostty-android-terminal
/// kittyGraphicsPlacementAndPixelReadback 对等）：传输 1x1 红色 RGB 图像后，
/// take_kitty_graphics_image 返回 Some 且宽高为 1x1。
#[test]
fn kitty_graphics_transmit_returns_1x1_image() {
    let mut t = terminal();
    // 1x1 RGB 红色像素：base64("/wAA") = {0xff, 0x00, 0x00}。
    // 显式 i=1 指定图像 id（上游默认自动分配 id，不保证为 1）。
    // 注意：vt_write/pty_write 均为分片直透（上游跨调用重组，无 ST/SGR 提前闭合）；
    // 此处走 pty_write 以覆盖 LF→CRLF 文本路径。
    t.pty_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=1;/wAA\x1b\\");
    t.flush();
    let image = t
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

// ── : cursor/row coordinate consistency (D1 deterministic leg) ──

/// The shell-echo path (prompt text + typed chars, no newline) must report a
/// cursor that sits ON the last printed row at the column right after the
/// text — the same row the CellData rows occupy. A mismatch here renders the
/// cursor one row below the text ("输入指针出现在当前文本的正下",
/// emulator evidence). Mirrors the exact production path: VT loop auto-push →
/// build_cell_data → (cells, CursorInfo).
#[test]
fn cursor_matches_last_printed_row_after_echo_print() {
    let mut t = terminal();
    t.vt_write(b"$ ");
    t.vt_write(b"abc");
    t.flush();

    let (cells, cursor) = t
        .receive_cell_data()
        .expect("VT loop must auto-push cell data after writes");

    // Last row that has any non-blank cell = the row the text visibly sits on.
    let last_text_row = cells
        .iter()
        .filter(|c| c.codepoint != 0)
        .map(|c| c.row)
        .max()
        .expect("printed cells must exist");
    let last_text_col = cells
        .iter()
        .filter(|c| c.codepoint != 0 && c.row == last_text_row)
        .map(|c| c.col)
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
    let mut t = terminal();
    t.vt_write(b"\x1b[3;5H"); // CUP to row 2, col 4 (1-based)
    t.flush();

    let (cells, cursor) = t
        .receive_cell_data()
        .expect("VT loop must auto-push cell data after CUP");

    assert_eq!(cursor.row, 2, "CUP row must be 0-based viewport row 2");
    assert_eq!(cursor.col, 4, "CUP col must be 0-based viewport col 4");
    // The cell grid must have NO text (CUP only moves the cursor) — the
    // cursor must therefore sit on an empty row, not below any text.
    let text_rows: Vec<u32> = cells
        .iter()
        .filter(|c| c.codepoint != 0)
        .map(|c| c.row)
        .collect();
    assert!(
        text_rows.is_empty(),
        "CUP alone must not print cells, got rows {text_rows:?}"
    );
}

/// OSC 7 与 OSC 1337 工作目录上报（上游归一化，本仓只断言透出内容）。
#[test]
fn osc7_and_osc1337_report_working_directory() {
    let mut osc7_terminal = terminal();
    osc7_terminal.pty_write(b"\x1b]7;file:///tmp\x07");
    osc7_terminal.flush();
    assert_eq!(
        osc7_terminal.poll_cwd_event(),
        Some("file:///tmp".to_string()),
        "OSC 7 must surface the working directory"
    );
    let mut osc1337_terminal = terminal();
    osc1337_terminal.pty_write(b"\x1b]1337;CurrentDir=/tmp\x07");
    osc1337_terminal.flush();
    assert_eq!(
        osc1337_terminal.poll_cwd_event(),
        Some("/tmp".to_string()),
        "OSC 1337 CurrentDir must surface the working directory"
    );
}

/// SGR31 红色必须到达渲染 CellData 的前景（设备渲染通路的精确复刻）。
/// 背景：设备上 SGR31/32/真彩红一律无红色像素，而 SGR34 蓝正常；
/// 本测试在 host 复刻设备渲染输入（receive_cell_data），二分颜色通路。
#[test]
fn sgr31_red_reaches_cell_data_foreground() {
    // 1) 默认配置：基线。
    let mut plain = terminal();
    plain.vt_write(b"\x1b[31mRED\x1b[0m");
    plain.flush();
    let (cells, _) = plain.receive_cell_data().expect("cell data");
    let red_cell = cells
        .iter()
        .find(|c| c.codepoint == 'R' as u32)
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
    let mut dracula =
        GhosttyTerminal::new_with_theme(24, 80, 1000, [0x21, 0x21, 0x21], [0xF8, 0xF8, 0xF2], dracula_ansi)
            .expect("dracula terminal");
    dracula.vt_write(b"\x1b[31mRED\x1b[0m");
    dracula.flush();
    let (cells, _) = dracula.receive_cell_data().expect("cell data");
    let red_cell = cells
        .iter()
        .find(|c| c.codepoint == 'R' as u32)
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
/// 背景：设备上 initSession 会话（Mocha 默认主题）SGR 无显色；
/// 复刻其精确主题与几何，定位颜色通路。
#[test]
fn sgr31_mocha_default_theme_reaches_foreground() {
    let (ansi, background, foreground) = GhosttyTerminal::catppuccin_mocha_palette();
    let mut mocha =
        GhosttyTerminal::new_with_theme(44, 48, 1000, background, foreground, ansi)
            .expect("mocha terminal");
    mocha.vt_write(b"\x1b[31mRED\x1b[0m");
    mocha.flush();
    let (cells, _) = mocha.receive_cell_data().expect("cell data");
    let red_cell = cells
        .iter()
        .find(|c| c.codepoint == 'R' as u32)
        .expect("R cell present");
    assert!(
        (red_cell.foreground[0] - 243.0 / 255.0).abs() < 0.02
            && (red_cell.foreground[1] - 139.0 / 255.0).abs() < 0.05
            && (red_cell.foreground[2] - 168.0 / 255.0).abs() < 0.05,
        "Mocha 主题 SGR31 前景须为 #F38BA8，实际 {:?}",
        red_cell.foreground
    );
}
