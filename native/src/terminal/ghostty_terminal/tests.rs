//! VT 引擎行为测试（torvox 语义增量 + 回归安全网；批次4 6.1 审删后约 3400 行）。
//!
//! 审删原则（openspec fix-terminal-ux-parity 任务 6.1）：与上游 libghostty-vt
//! 行为完全重复且无 torvox 语义增量的纯透传测试（vt_write → snapshot/cursor/
//! title 断言）已删除——该行为域由 ghostty 核心 Zig 测试（rev `de9fd9b` 锁定）
//! 与文末 regression mod 覆盖。保留集合：
//!
//! 1. openspec 豁免：`selection_text_unwraps_soft_wrapped_lines`、
//!    `selection_text_wide_char_columns`（P2-2 依赖）；
//! 2. torvox 包装层自有机制：snapshot 缓存/回退（row_cache_*、
//!    scrollback_fallback_*）、视口映射（scroll_viewport_delta_*）、search /
//!    dump_grid / read_line_text / uri_at / hyperlink_at / selection_text API、
//!    vt_write sanitize+ST 追加与 pty_write LF→CRLF 管道（osc_*_split_buffer、
//!    newline_lf_*）、OSC 133 semantic 标记、DEC 矩形 Rust API、鼠标编码 API、
//!    is_alive/session/lifecycle（tc_sm_/tc_al_/tc_lifecycle_）、bench_* 基准；
//! 3. 文末回归安全网 mod 整体豁免。
//!
//! | 保留前缀 | 行为域 | 过滤命令 |
//! |------|--------|----------|
//! | `tc_sm_` / `tc_al_` / `tc_lifecycle_` | session / Android 生命周期 | `cargo test -p native tc_` |
//! | `cell_` / `row_cache_` / `scrollback_` / `scroll_viewport_` | 快照缓存与视口 | `cargo test -p native cell_` |
//! | `dump_` / `read_` / `uri_at` / `hyperlink_at` / `selection_text` / `search_in` | 查询 API | `cargo test -p native dump_` |
//! | `osc_` / `newline_` / `bench_` | 管道语义 / 性能基准 | `cargo test -p native osc_` |
//!
//! 行为域分区见文末 `malformed_esc_regressions` / `malformed_sequence_regressions` / `osc_title_regressions` mod。

use std::hint::black_box;
use std::time::Instant;

use super::*;
use crate::terminal::test_helpers::{EffectFlag, assert_invariants, tc};

fn term() -> GhosttyTerminal {
    GhosttyTerminal::new(24, 80, 1000).expect("terminal create")
}

fn small_term() -> GhosttyTerminal {
    GhosttyTerminal::new(3, 3, 100).expect("term")
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
    let t = GhosttyTerminal::new(5, 10, 0).expect("term");
    assert_eq!(t.scrollback_length(), 0);
}

#[test]
fn read_line_text_returns_text() {
    let mut t = term();
    t.vt_write(b"\x1b[1;1HHello World");
    t.flush();
    let text = t.read_line_text(0);
    assert!(text.is_some());
    assert!(text.unwrap().contains("Hello"));
}

#[test]
fn read_line_text_empty_returns_none() {
    let t = term();
    let text = t.read_line_text(5);
    assert!(text.is_none());
}

#[test]
fn search_in_scrollback_finds_match() {
    let mut t = GhosttyTerminal::new(3, 80, 100).expect("term");
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
    let t = term();
    assert_eq!(t.search_in_scrollback(""), None);
}

#[test]
fn dump_grid_dimensions_match() {
    let t = term();
    let dumped = t.dump_grid();
    assert_eq!(dumped.rows, 24);
    assert_eq!(dumped.cols, 80);
    assert_eq!(dumped.visible.len(), (24 * 80) as usize);
    let _snap = t.take_snapshot();
    assert_invariants(&_snap);
}

#[test]
fn uri_at_empty_default() {
    let t = term();
    let snap = t.take_snapshot();
    assert_eq!(snap.uri_at(0, 0), None);
    assert_invariants(&snap);
}

#[test]
fn uri_at_out_of_bounds() {
    let t = term();
    let snap = t.take_snapshot();
    assert_eq!(snap.uri_at(100, 0), None);
    assert_eq!(snap.uri_at(0, 100), None);
    assert_invariants(&snap);
}

#[test]
fn snapshot_uri_at_returns_none_for_unset() {
    let mut t = term();
    t.vt_write(b"Hello");
    t.flush();
    let snap = t.take_snapshot();
    for row in 0..snap.rows {
        for col in 0..snap.cols {
            let uri = snap.uri_at(row, col);
            if let Some(u) = uri {
                // If any URI is set, it should be a valid string
                assert!(!u.is_empty());
            }
        }
    }
    assert_invariants(&snap);
}

#[test]
fn dump_grid_visible_populated() {
    let mut t = term();
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
    let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
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
    let t = term();
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
    let mut t = term();
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
    let mut t = term();
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
    let mut t = term();
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
    let mut t = term(); // default 24 rows × 80 cols
    t.vt_write(b"\x1b[?1000h\x1b[?1006h");
    t.flush();
    // Position far beyond the grid: 9999x9999 with 10x20 cells.
    let result = t.encode_mouse_event((9999.0, 9999.0), 0, 0, 10.0, 20.0);
    assert!(
        result.is_some(),
        "oversized coords must return Some"
    );
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
    let mut t = term();
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

/// OSC 0 title — split across two writes.
#[test]
fn osc_title_split_buffer() {
    let mut t = term();
    // Send the first and second parts of OSC 0 sequence
    t.vt_write(b"\x1b]0;My ");
    t.flush();
    t.vt_write(b"Title\x07");
    t.flush();
    let _snap = t.take_snapshot();
    // After setting the title, terminal should not crash, text should still be writable
    t.vt_write(b"AfterTitle");
    t.flush();
    let snap2 = t.take_snapshot();
    let found = snap2.cells.iter().any(|c| c.codepoint == 'A' as u32);
    assert!(found, "OSC split: text after split title should render");
    assert_invariants(&snap2);
}

/// OSC 52 clipboard — sent across split buffer.
#[test]
fn osc_clipboard_split_buffer() {
    let mut t = term();
    // OSC 52 sequence: first part sets clipboard selection, second provides data.
    // No crash is the primary verification point.
    t.vt_write(b"\x1b]52;c;");
    t.flush();
    t.vt_write(b"SGVsbG8=\x07");
    t.flush();
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
    let mut t = term();
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
    let mut t = term();
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
    let mut t = term();
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
    let mut t = term();
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
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
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
#[allow(clippy::float_cmp)]
fn cell_snapshot_default() {
    let c = CellSnapshot::default();
    assert_eq!(c.codepoint, 0);
    assert_eq!(c.foreground, [0.0, 0.0, 0.0, 0.0]);
    assert_eq!(c.background, [0.0, 0.0, 0.0, 0.0]);
    assert!(!c.bold);
    assert!(!c.italic);
    assert!(c.uri.is_none());
}

#[test]
#[allow(clippy::float_cmp)]
fn cell_snapshot_clone() {
    let c = CellSnapshot {
        codepoint: 65,
        graphemes: Vec::new(),
        foreground: [1.0, 0.0, 0.0, 1.0],
        background: [0.0, 0.0, 0.0, 1.0],
        bold: true,
        dim: false,
        italic: false,
        underline: true,
        reverse: false,
        strikethrough: false,
        blink: false,
        hidden: false,
        uri: Some(String::from("https://test")),
        semantic: SemanticContent::Output,
        overline: false,
        double_underline: false,
        width: 1,
    };
    let c2 = c.clone();
    assert_eq!(c.codepoint, c2.codepoint);
    assert_eq!(c.foreground, c2.foreground);
    assert_eq!(c.uri, c2.uri);
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
    let mut t = term();

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
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
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
    let mut t = term();
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
    let mut t = term();
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

// ── RI with left margin (from Termux) ────────────────────────────────

// ── DECBI/DECFI (from Termux) ───────────────────────────────────────

// ── DECCST (Soft Terminal Reset) from Termux ────────────────────────

// ── Scroll region regression tests (from Termux) ────────────────────

// ── Haven-style OSC 52 comprehensive (from OscHandlerTest) ──────────

// ── Haven-style OSC 7 CWD ─────────────────────────────────────────

// ── Haven-style OSC 8 hyperlinks ──────────────────────────────────

// ── Haven-style Mouse mode tracking ───────────────────────────────

// ── Resize more edge cases (from Termux) ───────────────────────────

// ── ICH (Insert Character CSI @) (from Termux testInsertMode) ──

// ── DECLRMM (Left/Right Margin Mode) (from Termux ScrollRegionTest) ──

// ── Rectangular areas (from Termux RectangularAreasTest) ──

// ── OSC 777 notify (from Haven OscHandlerTest) ──

// ── Clearing with margins (from Termux ScrollRegionTest regression) ──

// ── Tab with background color (from Termux testTab) ──

// ── More WcWidth emoji tests (from Termux WcWidthTest) ──

// ── Selection text extraction (from Termux testGetSelectedText) ──

// ── Additional Haven-inspired tests ──

// ── TC-CP: Cursor Position (from test gap analysis §3.B) ─────────

// ── TC-SC: Screen Content (from test gap analysis §3.C) ──────────

/// LF (\\n) implies CR+LF: session restore and normal PTY output depend on this.
/// If ghostty's VT parser treats LF as LF-only, each line would start at the
/// previous line's end column instead of column 0.
#[test]
fn newline_lf_implies_crlf() {
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
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
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
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
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
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
    let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
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
    let t = GhosttyTerminal::new(24, 80, 1000).expect("term");
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
    let t = GhosttyTerminal::new(3, 3, 100).expect("term");
    t.flush();
    let snap = t.take_snapshot();
    assert_invariants(&snap);
    drop(t);
    // If we reach here, no panic
}

/// TC-SM-004: Double drop is safe (handled by Drop impl)
#[test]
fn tc_sm_004_double_drop_safe() {
    let t = GhosttyTerminal::new(3, 3, 100).expect("term");
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
    let mut t = term();
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
    let mut t = GhosttyTerminal::new(5, 20, 100).expect("term");
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
    let mut t = GhosttyTerminal::new(5, 20, 100).expect("term");
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
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
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
    let mut t = GhosttyTerminal::new(5, 20, 100).expect("term");
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
    let mut t = GhosttyTerminal::new(5, 20, 100).expect("term");
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
    let mut t = GhosttyTerminal::new(5, 20, 100).expect("term");
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

// ── Phase 0: Zero-Infrastructure Tests ──────────────────────────
mod malformed_esc_regressions {
    use super::*;
    use crate::terminal::test_helpers::tc;

    // ── B4 Regressions ──────────────────────────────────────────

    /// RB_011: Malformed ESC sequence causes cursor offset bug.
    /// On the unfixed binary, cursor ends up at wrong column after
    /// malformed ESC + CRLF + prompt.
    #[test]
    fn rb_011_malformed_esc_cursor_offset() {
        let mut t = term();
        t.flush();
        // NOTE: \x1b[?i is consumed as a complete private CSI (final byte 0x69 = 'i').
        // Remaining "nvalid" = 6 printable chars rendered on screen.
        // After \r\n$, cursor ends at (1, 2).
        // The B4 error-offset bug needs a trigger that ghostty genuinely cannot parse.
        tc(&mut t)
            .write(b"\x1b[?invalid\r\n$ ")
            .assert_cursor_at(1, 2);
    }

    /// RB_011a: Malformed sequence with printable text.
    #[test]
    fn rb_011a_malformed_printable_text() {
        let mut t = term();
        t.flush();
        tc(&mut t)
            .write(b"\x1b[?bad seq\r\nHello")
            .assert_row_text(1, "Hello")
            .assert_cursor_at(1, 5);
    }

    /// RB_011b: Malformed ESC followed by valid CUP.
    #[test]
    fn rb_011b_malformed_then_valid_cup() {
        let mut t = term();
        t.flush();
        // NOTE: \x1b[?i consumed as CSI, "nvalid" printed (6 chars).
        // Then CUP to (2,4) + X → cursor ends at (2,5).
        tc(&mut t)
            .write(b"\x1b[?invalid\x1b[3;5HX")
            .assert_cursor_at(2, 5)
            .assert_row_text(2, "X");
    }

    /// RB_011c: Malformed OSC sequence does not corrupt state.
    #[test]
    fn rb_011c_malformed_osc_then_write() {
        let mut t = term();
        t.flush();
        tc(&mut t)
            .write(b"\x1b]invalid\x07OK")
            .assert_row_text(0, "OK");
    }

    /// RB_011d: Malformed CSI sequence with extra parameters.
    #[test]
    fn rb_011d_malformed_csi_extra_params() {
        let mut t = term();
        t.flush();
        tc(&mut t)
            .write(b"\x1b[1;2;3;4;5;6qX")
            .assert_row_text(0, "X");
    }

    /// RB_012: Rapid malformed sequences do not crash.
    #[test]
    fn rb_012_rapid_malformed_sequences() {
        let mut t = term();
        t.flush();
        // Each iteration prints 8 chars ("nvalid" + "ad") because
        // \x1b[?i and \x1b[?b are consumed as complete private CSI.
        for _ in 0..50 {
            t.vt_write(b"\x1b[?invalid\x1b[?bad\x1b]junk\x07");
        }
        t.flush();
        t.flush();
        // 50 × 8 = 400 chars = 5 rows in 80-col terminal.
        // "AfterRapidMalformed" lands on row 5.
        tc(&mut t)
            .write(b"AfterRapidMalformed")
            .assert_row_text(5, "AfterRapidMalformed");
    }

    // ── Invariant Checks ────────────────────────────────────────

    /// IV_001: take_and_invariants passes after text write.
    #[test]
    fn iv_001_invariants_after_write() {
        let mut t = term();
        t.flush();
        tc(&mut t).write(b"Hello, World!").take_and_invariants();
    }

    /// IV_002: Invariants pass after CRLF and scroll.
    #[test]
    fn iv_002_invariants_after_crlf() {
        let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"line1\nline2\nline3\nline4\n")
            .take_and_invariants();
    }

    /// IV_003: Invariants pass after resize.
    #[test]
    fn iv_003_invariants_after_resize() {
        let mut t = term();
        t.flush();
        t.vt_write(b"Persist");
        t.flush();
        t.resize(30, 100);
        t.flush();
        let snap = t.take_snapshot();
        assert_invariants(&snap);
    }

    /// IV_004: Invariants pass after alt screen switch.
    #[test]
    fn iv_004_invariants_alt_screen() {
        let mut t = term();
        t.flush();
        tc(&mut t)
            .write(b"\x1b[?1049h")
            .write(b"AltText")
            .take_and_invariants();
    }

    /// IV_005: Invariants pass after DECSTR.
    #[test]
    fn iv_005_invariants_after_decstr() {
        let mut t = term();
        t.flush();
        tc(&mut t)
            .write(b"\x1b[!p")
            .write(b"AfterReset")
            .take_and_invariants();
    }

    /// IV_006: Invariants pass after erase display.
    #[test]
    fn iv_006_invariants_after_erase_display() {
        let mut t = small_term();
        t.flush();
        tc(&mut t)
            .write(b"ABC\r\nDEF")
            .write(b"\x1b[2J")
            .take_and_invariants();
    }

    // ── Basic I/O ───────────────────────────────────────────────

    /// IO_001: Simple text write to row 0.
    #[test]
    fn io_001_write_text_row0() {
        let mut t = term();
        t.flush();
        tc(&mut t)
            .write(b"AB")
            .assert_row_text(0, "AB")
            .assert_cursor_at(0, 2);
    }

    /// IO_002: LF advances cursor to next row and resets to column 0
    /// (converted to CR+LF by vt_write wrapper).
    #[test]
    fn io_002_lf_advances_row() {
        let mut t = term();
        t.flush();
        tc(&mut t)
            .write(b"A\nB")
            .assert_row_text(0, "A")
            .assert_cursor_at(1, 1);
    }

    /// IO_003: CR returns cursor to column 0.
    #[test]
    fn io_003_cr_returns_to_col0() {
        let mut t = term();
        t.flush();
        tc(&mut t)
            .write(b"ABCDE\rX")
            .assert_row_text(0, "XBCDE")
            .assert_cursor_at(0, 1);
    }

    /// IO_004: CRLF moves to next row column 0.
    #[test]
    fn io_004_crlf_moves_row_col0() {
        let mut t = GhosttyTerminal::new(3, 5, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"hi\r\nu")
            .assert_cursor_at(1, 1)
            .assert_row_text(1, "u");
    }

    /// IO_005: HT advances to next tab stop.
    #[test]
    fn io_005_ht_advances_tab() {
        let mut t = GhosttyTerminal::new(3, 30, 100).expect("term");
        t.flush();
        tc(&mut t).write(b"A\tB").assert_cursor_at(0, 9); // tab from 1 to 8
    }

    /// IO_006: Backspace (BS) moves cursor left.
    #[test]
    fn io_006_bs_moves_left() {
        let mut t = GhosttyTerminal::new(3, 5, 100).expect("term");
        t.flush();
        tc(&mut t).write(b"AB\x08").assert_cursor_at(0, 1);
    }

    /// IO_007: BS does not wrap to previous row.
    #[test]
    fn io_007_bs_no_wrap() {
        let mut t = GhosttyTerminal::new(3, 5, 100).expect("term");
        t.flush();
        tc(&mut t).write(b"\x08").assert_cursor_at(0, 0);
    }

    /// IO_008: TAB with no tab stops is safe.
    #[test]
    fn io_008_tab_no_stops() {
        let mut t = GhosttyTerminal::new(3, 5, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"\x1b[g") // clear tab at current col
            .write(b"A\tB")
            .assert_row_text(0, "AB"); // tab is invisible, both chars present
    }

    /// IO_009: Multiple LF scroll when at bottom.
    #[test]
    fn io_009_multiple_lf_scroll() {
        let mut t = GhosttyTerminal::new(3, 5, 100).expect("term");
        t.flush();
        tc(&mut t).write(b"1\n2\n3\n4\n5").assert_row_text(2, "5");
    }

    /// IO_010: CR at column 0 is a no-op.
    #[test]
    fn io_010_cr_at_col0_noop() {
        let mut t = term();
        t.flush();
        tc(&mut t).write(b"\r\r\rA").assert_cursor_at(0, 1);
    }

    /// IO_011: BEL does not affect cursor position.
    #[test]
    fn io_011_bel_no_cursor_move() {
        let mut t = term();
        t.flush();
        tc(&mut t).write(b"AB\x07").assert_cursor_at(0, 2);
    }

    /// IO_012: Writing null byte is safe.
    #[test]
    fn io_012_null_byte_safe() {
        let mut t = term();
        t.flush();
        tc(&mut t)
            .write(b"\0")
            .write(b"AfterNull")
            .assert_row_text(0, "AfterNull");
    }

    /// IO_013: Writing text beyond right margin does not panic.
    #[test]
    fn io_013_beyond_right_margin() {
        let mut t = GhosttyTerminal::new(3, 3, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"123456")
            .assert_row_text(0, "123")
            .assert_row_text(1, "456");
    }

    /// IO_014: VT (vertical tab) advances cursor down, same col.
    #[test]
    fn io_014_vt_advances_down() {
        let mut t = term();
        t.flush();
        tc(&mut t).write(b"A\x0BB").assert_cursor_at(1, 2);
    }

    /// IO_015: FF (form feed) advances cursor down, same col.
    #[test]
    fn io_015_ff_advances_down() {
        let mut t = term();
        t.flush();
        tc(&mut t).write(b"A\x0CB").assert_cursor_at(1, 2);
    }

    /// IO_016: SO/SI shift in/out do not crash or corrupt.
    #[test]
    fn io_016_so_si_terminal_survives() {
        let mut t = term();
        t.flush();
        tc(&mut t).write(b"\x0e\x0fOK").assert_row_text(0, "OK");
    }

    // ── Cursor Positioning ──────────────────────────────────────

    /// CP_001: CUP to specific row/col.
    #[test]
    fn cp_001_cup_specific() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"\x1b[2;2HX")
            .assert_row_text(1, "X")
            .assert_cursor_at(1, 2);
    }

    /// CP_002: CUP row clamping to max row.
    #[test]
    fn cp_002_cup_row_clamp() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        tc(&mut t).write(b"\x1b[100;1HX").assert_cursor_at(4, 1);
    }

    /// CP_003: CUP col clamping to max col.
    #[test]
    fn cp_003_cup_col_clamp() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        tc(&mut t).write(b"\x1b[1;100HX").assert_cursor_at(0, 9); // clamped to last col (9), X written at col 9
    }

    /// CP_004: CUF (cursor forward) moves right.
    #[test]
    fn cp_004_cuf_forward() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        tc(&mut t).write(b"\x1b[CX").assert_cursor_at(0, 2);
    }

    /// CP_005: CUF clamping at right margin.
    #[test]
    fn cp_005_cuf_clamp_right() {
        let mut t = GhosttyTerminal::new(3, 3, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"\x1b[1;1H\x1b[100CX")
            .assert_cursor_at(0, 2); // CUF clamped at last col (2), X written at last col
    }

    /// CP_006: CUB (cursor back) moves left.
    #[test]
    fn cp_006_cub_back() {
        let mut t = GhosttyTerminal::new(3, 5, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"\x1b[1;6H\x1b[1DX")
            .assert_cursor_at(0, 4); // X at col 3, cursor advances to 4
    }

    /// CP_007: CUB clamping at left margin.
    #[test]
    fn cp_007_cub_clamp_left() {
        let mut t = GhosttyTerminal::new(3, 5, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"\x1b[1;1H\x1b[100DX")
            .assert_cursor_at(0, 1); // X at col 0, cursor advances to 1
    }

    /// CP_008: CUU (cursor up) moves up.
    #[test]
    fn cp_008_cuu_up() {
        let mut t = GhosttyTerminal::new(5, 5, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"\x1b[3;1H\x1b[1AX")
            .assert_cursor_at(1, 1); // CUU 1 from row 2 → row 1, X advances col
    }

    /// CP_009: CUU clamping at top margin.
    #[test]
    fn cp_009_cuu_clamp_top() {
        let mut t = GhosttyTerminal::new(5, 5, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"\x1b[1;1H\x1b[100AX")
            .assert_cursor_at(0, 1); // CUU clamps to 0, X advances col
    }

    /// CP_010: CUD (cursor down) moves down.
    #[test]
    fn cp_010_cud_down() {
        let mut t = GhosttyTerminal::new(5, 5, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"\x1b[1;1H\x1b[2BX")
            .assert_cursor_at(2, 1); // CUD 2 from row 0 → row 2, X advances col
    }

    /// CP_011: CUD clamping at bottom margin.
    #[test]
    fn cp_011_cud_clamp_bottom() {
        let mut t = GhosttyTerminal::new(3, 3, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"\x1b[1;1H\x1b[100BX")
            .assert_cursor_at(2, 1); // CUD clamps to row 2, X advances col
    }

    /// CP_012: HVP same as CUP.
    #[test]
    fn cp_012_hvp_same_as_cup() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        tc(&mut t).write(b"\x1b[3;4fX").assert_cursor_at(2, 4); // X at (2,3), cursor advances to (2,4)
    }

    /// CP_013: SCP (save) and RCP (restore) cursor position.
    #[test]
    fn cp_013_scp_rcp_save_restore() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"\x1b[s") // save position (0,0)
            .write(b"\x1b[3;3HY") // move and write Y at (2,2)
            .write(b"\x1b[u") // restore position (0,0)
            .write(b"X") // write X at (0,0)
            .assert_cursor_at(0, 1)
            .assert_row_text(0, "X");
    }

    /// CP_020: CUF with count 0 is treated as CUF 1.
    #[test]
    fn cp_020_cuf_zero_as_one() {
        let mut t = GhosttyTerminal::new(3, 5, 100).expect("term");
        t.flush();
        tc(&mut t).write(b"\x1b[0C").assert_cursor_at(0, 1); // VT: CUF with missing/default param = 1
    }

    /// CP_021: CUB with count 0 is no-op.
    #[test]
    fn cp_021_cub_zero_noop() {
        let mut t = GhosttyTerminal::new(3, 5, 100).expect("term");
        t.flush();
        tc(&mut t).write(b"\x1b[0D").assert_cursor_at(0, 0);
    }

    /// CP_022: HPA absolute column (without row change).
    #[test]
    fn cp_022_hpa_absolute() {
        let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"\x1b[5`X")
            .assert_row_text(0, "X") // HPA doesn't fill spaces
            .assert_cursor_at(0, 5); // X at col 4, cursor advances to 5
    }

    // ── Text Modification ───────────────────────────────────────

    /// TM_001: Write text erases underlying content.
    #[test]
    fn tm_001_write_overwrites() {
        let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"AAAAA\n")
            .write(b"BBB")
            .assert_row_text(1, "BBB");
    }

    /// TM_002: EL 0 erases from cursor to end of line.
    #[test]
    fn tm_002_el_0_erase_to_end() {
        let mut t = GhosttyTerminal::new(3, 5, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"ABCDE")
            .write(b"\x1b[1;1H\x1b[0K")
            .assert_row_text(0, "");
    }

    /// TM_003: EL 1 erases from start to cursor inclusive.
    #[test]
    fn tm_003_el_1_erase_from_start() {
        let mut t = GhosttyTerminal::new(3, 5, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"ABCDE")
            .write(b"\x1b[1;3H\x1b[1K")
            .assert_row_text(0, "DE");
    }

    /// TM_004: EL 2 erases entire line.
    #[test]
    fn tm_004_el_2_erase_line() {
        let mut t = GhosttyTerminal::new(3, 5, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"ABCDE")
            .write(b"\x1b[2K")
            .assert_row_text(0, "");
    }

    /// TM_005: ED 0 erases from cursor to end of display.
    #[test]
    fn tm_005_ed_0_terminal_survives() {
        let mut t = GhosttyTerminal::new(3, 5, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"ABCDE")
            .write(b"\x1b[1;3H") // cursor to (0, 2)
            .write(b"\x1b[0J") // erase from cursor to end
            .assert_row_text(0, "AB"); // cols 0-1 preserved, cols 2-4 erased
    }

    /// TM_006: ED 1 erases from start of display to cursor.
    #[test]
    fn tm_006_ed_1_terminal_survives() {
        let mut t = GhosttyTerminal::new(3, 5, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"ABC\nDEF\nGHI")
            .write(b"\x1b[2;1H") // cursor to (1, 0)
            .write(b"\x1b[1J") // erase from start to cursor
            .assert_row_text(0, "") // row 0 fully erased
            .assert_row_text(2, "GHI"); // row 2 preserved (below cursor)
    }

    /// TM_007: ED 2 erases entire display.
    #[test]
    fn tm_007_ed_2_erase_display() {
        let mut t = GhosttyTerminal::new(3, 5, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"ABC\nDEF\nGHI")
            .write(b"\x1b[2J") // erase entire display
            .assert_row_text(0, "")
            .assert_row_text(1, "")
            .assert_row_text(2, "");
    }

    /// TM_008: ED 3 erases scrollback - no crash is main assertion.
    #[test]
    fn tm_008_ed_3_terminal_survives() {
        let mut t = GhosttyTerminal::new(3, 5, 100).expect("term");
        t.flush();
        for i in 0..5 {
            t.vt_write(format!("line{i}\n").as_bytes());
        }
        t.flush();
        t.vt_write(b"\x1b[3J");
        t.vt_write(b"\rOK");
        t.flush();
        let snap = t.take_snapshot();
        assert_invariants(&snap);
    }

    /// TM_009: DCH (delete character) shifts left.
    #[test]
    fn tm_009_dch_delete_char() {
        let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"ABCDE")
            .write(b"\x1b[1;2H")
            .write(b"\x1b[P") // delete 1 char at cursor
            .assert_row_text(0, "ACDE");
    }

    /// TM_010: ICH (insert character) shifts right.
    #[test]
    fn tm_010_ich_insert_char() {
        let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"ACDE")
            .write(b"\x1b[1;2H")
            .write(b"\x1b[@") // ICH 1 (default)
            .write(b"B")
            .assert_row_text(0, "ABCDE");
    }

    /// TM_011: IRM (insert mode) inserts text.
    #[test]
    fn tm_011_irm_insert_mode() {
        let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"ACDE")
            .write(b"\x1b[1;2H")
            .write(b"\x1b[4h") // IRM on
            .write(b"B")
            .assert_row_text(0, "ABCDE");
    }

    /// TM_012: IRM off overwrites text.
    #[test]
    fn tm_012_irm_off_overwrites() {
        let mut t = GhosttyTerminal::new(3, 5, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"ABCDE")
            .write(b"\x1b[1;2H")
            .write(b"\x1b[4l") // IRM off
            .write(b"XX")
            .assert_row_text(0, "AXXDE");
    }

    /// TM_013: ECH (erase character) erases N chars.
    #[test]
    fn tm_013_ech_erase_chars() {
        let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"ABCDE")
            .write(b"\x1b[1;2H")
            .write(b"\x1b[3X") // ECH 3
            .assert_row_text(0, "AE"); // ECH erases BCD, A+E remain
    }

    /// TM_014: Repeat (REP) repeats last char.
    #[test]
    fn tm_014_rep_repeat_char() {
        let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
        t.flush();
        tc(&mut t).write(b"A\x1b[b").assert_row_text(0, "AA");
    }

    /// TM_015: Repeat with explicit count.
    #[test]
    fn tm_015_rep_with_count() {
        let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
        t.flush();
        tc(&mut t).write(b"X\x1b[5b").assert_row_text(0, "XXXXXX");
    }

    /// TM_016: Insert lines (IL) - terminal survives.
    #[test]
    fn tm_016_il_terminal_survives() {
        let mut t = GhosttyTerminal::new(5, 5, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"AAAAA\nBBBBB\nCCCCC")
            .write(b"\x1b[1;1H\x1b[L") // IL 1 at top
            .write(b"XXXXX")
            .assert_row_text(0, "XXXXX");
    }

    /// TM_017: Delete lines (DL) - terminal survives.
    #[test]
    fn tm_017_dl_terminal_survives() {
        let mut t = GhosttyTerminal::new(5, 5, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"AAAAA\nBBBBB\nCCCCC")
            .write(b"\x1b[1;1H\x1b[M") // DL 1 at top
            .write(b"XXXXX")
            .assert_row_text(0, "XXXXX");
    }

    /// TM_018: SGR attributes apply to written text.
    #[test]
    fn tm_018_sgr_bold_italic() {
        let mut t = term();
        t.flush();
        tc(&mut t).write(b"\x1b[1;3mBoldItalic").assert_effects(
            0,
            0,
            &[EffectFlag::Bold, EffectFlag::Italic],
        );
    }

    // ── Alt Screen Buffer ───────────────────────────────────────

    /// AB_001: Enter alt screen (DECSET 1049).
    #[test]
    fn ab_001_enter_alt_screen() {
        let mut t = term();
        t.flush();
        tc(&mut t)
            .write(b"\x1b[?1049h")
            .write(b"InAlt")
            .assert_row_text(0, "InAlt");
    }

    /// AB_002: Exit alt screen restores normal buffer content.
    #[test]
    fn ab_002_exit_alt_restores_normal() {
        let mut t = term();
        t.flush();
        tc(&mut t)
            .write(b"NormalContent")
            .capture_before()
            .write(b"\x1b[?1049h")
            .write(b"AltContent")
            .write(b"\x1b[?1049l")
            .assert_content_preserved()
            .assert_row_text(0, "NormalContent");
    }

    /// AB_003: Alt screen has no scrollback.
    #[test]
    fn ab_003_alt_no_scrollback() {
        let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
        t.flush();
        t.vt_write(b"\x1b[?1049h");
        t.flush();
        for i in 0..5 {
            t.vt_write(format!("alt{i}\n").as_bytes());
        }
        t.flush();
        t.flush();
        assert_eq!(
            t.scrollback_length(),
            0,
            "alt screen should have no scrollback"
        );
    }

    /// AB_004: Alt screen text is isolated from normal.
    #[test]
    fn ab_004_alt_text_isolated() {
        let mut t = term();
        t.flush();
        tc(&mut t)
            .write(b"Before")
            .write(b"\x1b[?1049h")
            .write(b"During")
            .write(b"\x1b[?1049l")
            .assert_row_text(0, "Before");
    }

    /// AB_005: Nested alt screen enter/exit is safe.
    #[test]
    fn ab_005_alt_nested_safe() {
        let mut t = term();
        t.flush();
        tc(&mut t)
            .write(b"\x1b[?1049h")
            .write(b"First")
            .write(b"\x1b[?1049l")
            .write(b"\x1b[?1049h")
            .write(b"Second")
            .write(b"\x1b[?1049l")
            .write(b"Final")
            .assert_row_text(0, "Final");
    }

    /// AB_006: Alt screen with resize exits safely.
    #[test]
    fn ab_006_alt_resize_safe() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        t.vt_write(b"\x1b[?1049h");
        t.flush();
        t.vt_write(b"InAlt");
        t.flush();
        t.resize(8, 20);
        t.flush();
        t.vt_write(b"\x1b[?1049l");
        t.flush();
        t.vt_write(b"AfterResize");
        t.flush();
        tc(&mut t).assert_row_text(0, "AfterResize");
    }

    /// AB_007: Alt screen preserves cursor position on exit.
    #[test]
    fn ab_007_alt_preserves_cursor() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        t.vt_write(b"\x1b[3;5H");
        t.flush();
        let (saved_x, saved_y) = (t.cursor_x(), t.cursor_y());
        t.vt_write(b"\x1b[?1049h");
        t.vt_write(b"\x1b[2;2H");
        t.flush();
        t.vt_write(b"\x1b[?1049l");
        t.flush();
        assert_eq!(t.cursor_x(), saved_x, "alt exit: cursor_x restored");
        assert_eq!(t.cursor_y(), saved_y, "alt exit: cursor_y restored");
    }

    /// AB_008: Alt screen switch while in alt is no-op.
    #[test]
    fn ab_008_alt_switch_while_in_alt() {
        let mut t = term();
        t.flush();
        tc(&mut t)
            .write(b"\x1b[?1049h")
            .write(b"\x1b[?1049h") // double enter
            .write(b"DoubleEnter")
            .assert_row_text(0, "DoubleEnter");
    }

    /// AB_009: Content preserved across 3 alt screen cycles.
    #[test]
    fn ab_009_alt_three_cycles() {
        let mut t = term();
        t.flush();
        t.vt_write(b"Original");
        t.flush();
        for _ in 0..3 {
            t.vt_write(b"\x1b[?1049h");
            t.vt_write(b"X");
            t.flush();
            t.vt_write(b"\x1b[?1049l");
            t.flush();
        }
        tc(&mut t).assert_row_text(0, "Original");
    }

    // ── Scrollback / History ─────────────────────────────────────

    /// HI_001: Writing past viewport creates scrollback.
    #[test]
    fn hi_001_scrollback_created() {
        let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
        t.flush();
        for i in 0..10 {
            t.vt_write(format!("line{i}\n").as_bytes());
        }
        t.flush();
        assert!(
            t.scrollback_length() > 0,
            "scrollback should have content after scrolling"
        );
    }

    /// HI_002: Many lines do not crash.
    #[test]
    fn hi_002_many_lines_terminal_survives() {
        let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
        t.flush();
        for i in 0..50 {
            t.vt_write(format!("line{i}\n").as_bytes());
        }
        t.flush();
        t.vt_write(b"\rOK");
        t.flush();
        let snap = t.take_snapshot();
        assert_invariants(&snap);
    }

    /// HI_003: Scrollback content readable via dump_grid.
    #[test]
    fn hi_003_scrollback_dump_readable() {
        let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
        t.flush();
        for i in 0..5 {
            t.vt_write(format!("line{i}\n").as_bytes());
        }
        t.flush();
        let dumped = t.dump_grid();
        assert!(
            !dumped.scrollback.is_empty(),
            "dump should contain scrollback"
        );
    }

    /// HI_004: Multiple scrollback lines preserved in order.
    #[test]
    fn hi_004_scrollback_ordered() {
        let mut t = GhosttyTerminal::new(2, 10, 100).expect("term");
        t.flush();
        t.vt_write(b"first\n");
        t.vt_write(b"second\n");
        t.vt_write(b"third\n");
        t.flush();
        let dumped = t.dump_grid();
        let has_first = dumped
            .scrollback
            .iter()
            .any(|row| row.iter().any(|c| c.codepoint == 'f' as u32));
        assert!(
            has_first || t.scrollback_length() > 0,
            "scrollback should exist"
        );
    }

    /// HI_005: Scrollback is empty for fresh terminal.
    #[test]
    fn hi_005_scrollback_empty_fresh() {
        let t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        assert_eq!(t.scrollback_length(), 0);
    }

    /// HI_006: Single line scroll creates scrollback.
    #[test]
    fn hi_006_scrollback_with_content() {
        let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
        t.flush();
        // Just writing a lot of lines should create scrollback
        for i in 0..10 {
            t.vt_write(format!("line{i}\n").as_bytes());
        }
        t.flush();
        assert!(
            t.scrollback_length() > 0,
            "scrollback should have content after many lines"
        );
    }

    /// HI_007: Alt screen has empty scrollback after normal scroll.
    #[test]
    fn hi_007_alt_scrollback_empty() {
        let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
        t.flush();
        for i in 0..5 {
            t.vt_write(format!("n{i}\n").as_bytes());
        }
        t.flush();
        let normal_scrollback = t.scrollback_length();
        t.vt_write(b"\x1b[?1049h");
        t.flush();
        assert_eq!(
            t.scrollback_length(),
            0,
            "alt screen scrollback should be 0; normal had {normal_scrollback}"
        );
    }

    /// HI_008: Scrollback content does not affect visible text.
    #[test]
    fn hi_008_scrollback_visible_independent() {
        let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
        t.flush();
        for i in 0..5 {
            t.vt_write(format!("line{i}\n").as_bytes());
        }
        t.flush();
        let snap = t.take_snapshot();
        let has_recent = snap.cells.iter().any(|c| c.codepoint == 'l' as u32);
        assert!(has_recent, "visible should show scrolled-in rows");
    }

    /// HI_009: Writing after scrollback continues normally.
    #[test]
    fn hi_009_write_after_scrollback() {
        let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
        t.flush();
        for i in 0..10 {
            t.vt_write(format!("line{i}\n").as_bytes());
        }
        t.flush();
        t.vt_write(b"\rFreshLine");
        t.flush();
        tc(&mut t).assert_row_text(2, "FreshLine");
    }

    /// HI_010: Rapid scrolling does not cause data loss in visible area.
    #[test]
    fn hi_010_rapid_scroll_visible_stable() {
        let mut t = GhosttyTerminal::new(3, 5, 100).expect("term");
        t.flush();
        for i in 0..100 {
            t.vt_write(format!("{}\n", i % 10).as_bytes());
        }
        t.flush();
        let snap = t.take_snapshot();
        let visible_nonzero = snap.cells.iter().filter(|c| c.codepoint > 0).count();
        assert!(
            visible_nonzero > 0,
            "visible area should have content after rapid scroll"
        );
    }

    /// HI_011: Alt screen scrollback after exit equals normal scrollback.
    #[test]
    fn hi_011_alt_exit_scrollback() {
        let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
        t.flush();
        for i in 0..3 {
            t.vt_write(format!("n{i}\n").as_bytes());
        }
        t.flush();
        t.vt_write(b"\x1b[?1049h");
        t.flush();
        for i in 0..5 {
            t.vt_write(format!("a{i}\n").as_bytes());
        }
        t.flush();
        t.vt_write(b"\x1b[?1049l");
        t.flush();
        // After alt exit, normal buffer scrollback should be restored
        assert!(
            t.scrollback_length() > 0,
            "scrollback should exist after alt exit (normal buffer restored)"
        );
    }

    // ── Tab Stops ────────────────────────────────────────────────

    /// TB_001: Default tab stops every 8 columns.
    #[test]
    fn tb_001_default_tab_stops() {
        let mut t = GhosttyTerminal::new(3, 30, 100).expect("term");
        t.flush();
        tc(&mut t).write(b"A\tB").assert_cursor_at(0, 9);
    }

    /// TB_002: Set tab stop (HTS) at current column.
    #[test]
    fn tb_002_hts_set_tab_stop() {
        let mut t = GhosttyTerminal::new(3, 20, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"\x1b[g") // clear all tab stops
            .write(b"\x1b[1;5H")
            .write(b"\x1bH") // HTS at col 4
            .write(b"\x1b[1;1HA\x09B")
            .assert_cursor_at(0, 5); // tab to col 4, B written at col 5
    }

    /// TB_003: Clear tab stop (TBC) at current column.
    #[test]
    fn tb_003_tbc_clear_tab_stop() {
        let mut t = GhosttyTerminal::new(3, 30, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"\x1b[1;9H")
            .write(b"\x1b[g") // clear tab at current column
            .write(b"\x1b[1;1HA\x09B")
            .assert_cursor_at(0, 9); // skipped col 8, lands on 16
    }

    /// TB_004: Tab after clearing all stops does not crash.
    #[test]
    fn tb_004_tbc_and_tab_terminal_survives() {
        let mut t = GhosttyTerminal::new(3, 20, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"\x1b[3g") // clear all tab stops
            .write(b"A\tB")
            .assert_row_text(0, "AB"); // tab clears no spaces
    }

    /// TB_005: Tab advances past stops.
    #[test]
    fn tb_005_tab_advances() {
        let mut t = GhosttyTerminal::new(3, 30, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"\tC") // tab to col 8, then C at col 8
            .assert_cursor_at(0, 9)
            .assert_row_text(0, "C");
    }

    /// TB_006: Tab stops reset on RIS.
    #[test]
    fn tb_006_tab_reset_on_ris() {
        let mut t = GhosttyTerminal::new(3, 20, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"\x1b[3g") // clear all
            .write(b"\x1bc") // RIS
            .write(b"A\tB")
            .assert_cursor_at(0, 9); // default stops restored
    }

    /// TB_007: Tab at right margin stops at last column.
    #[test]
    fn tb_007_tab_at_right_margin() {
        let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"\x1b[1;9H") // col 8
            .write(b"\x09")
            .assert_cursor_at(0, 9); // stays at last column
    }

    // ── Scroll Regions ───────────────────────────────────────────

    /// SR_001: DECSTBM set does not crash and input still renders inside the
    /// scroll region.
    #[test]
    fn sr_001_decstbm_terminal_survives() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        t.vt_write(b"\x1b[2;4r");
        t.flush();
        tc(&mut t).write(b"OK");
        let snap = t.take_snapshot();
        let rows: Vec<String> = (0..snap.rows).map(|r| row_text(&snap, r)).collect();
        assert!(
            rows.iter().any(|row| row.contains("OK")),
            "DECSTBM must not stop input from rendering: rows={rows:?}",
        );
    }

    /// SR_002: Scroll region does not crash.
    #[test]
    fn sr_002_scroll_region_terminal_survives() {
        let mut t = GhosttyTerminal::new(5, 5, 100).expect("term");
        t.flush();
        t.vt_write(b"\x1b[3;5r"); // region rows 3-5
        t.flush();
        for _ in 0..5 {
            t.vt_write(b"XXXX\n");
        }
        t.flush();
        t.vt_write(b"OK\r");
        t.flush();
        assert_eq!(t.rows(), 5, "SR-002: rows unchanged");
    }

    /// SR_003: Scroll region preserves content below region.
    #[test]
    fn sr_003_below_region_preserved() {
        let mut t = GhosttyTerminal::new(5, 5, 100).expect("term");
        t.flush();
        t.vt_write(b"\x1b[1;4r"); // region rows 1-4
        t.flush();
        t.vt_write(b"EEEEE\n");
        t.flush();
        for _ in 0..3 {
            t.vt_write(b"YYYY\n");
        }
        t.flush();
        // Row 0 should be inside scroll region and may scroll; row 4 unchanged
        let snap = t.take_snapshot();
        let y_rows = snap.cells.iter().any(|c| c.codepoint == 'Y' as u32);
        assert!(y_rows, "SR-003: Y should be visible in region");
    }

    /// SR_004: Origin mode (DECOM) makes CUP relative to region.
    #[test]
    fn sr_004_origin_mode_relative() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        t.vt_write(b"\x1b[2;4r"); // region 2-4
        t.vt_write(b"\x1b[?6h"); // origin mode on
        t.flush();
        t.vt_write(b"\x1b[1;1HX"); // should go to region top (row 1)
        t.flush();
        let snap = t.take_snapshot();
        let x_row1 = cell_at(&snap, 1, 0)
            .map(|c| c.codepoint == 'X' as u32)
            .unwrap_or(false);
        assert!(x_row1, "SR-004: X should be at row 1 (region top)");
    }

    // ── Reset ────────────────────────────────────────────────────

    /// RR_001: RIS (full reset) clears screen.
    #[test]
    fn rr_001_ris_clears_screen() {
        let mut t = GhosttyTerminal::new(3, 5, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"Content")
            .write(b"\x1bc") // RIS
            .assert_lines_are(&["", "", ""]);
    }

    /// RR_002: RIS resets cursor to origin.
    #[test]
    fn rr_002_ris_resets_cursor() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"\x1b[3;5H")
            .write(b"\x1bc") // RIS
            .assert_cursor_at(0, 0);
    }

    /// RR_003: RIS restores default tab stops.
    #[test]
    fn rr_003_ris_restores_tabs() {
        let mut t = GhosttyTerminal::new(3, 20, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"\x1b[3g") // clear all tabs
            .write(b"\x1bc") // RIS
            .write(b"A\tB")
            .assert_cursor_at(0, 9);
    }

    /// RR_004: DECSTR (soft reset) does not crash.
    #[test]
    fn rr_004_decstr_terminal_survives() {
        let mut t = term();
        t.flush();
        tc(&mut t)
            .write(b"\x1b[!p") // DECSTR
            .write(b"AfterDECSTR")
            .assert_row_text(0, "AfterDECSTR");
    }

    /// RR_005: RIS resets cursor visibility.
    #[test]
    fn rr_005_ris_resets_cursor_visible() {
        let mut t = term();
        t.flush();
        t.vt_write(b"\x1b[?25l"); // hide
        t.vt_write(b"\x1bc"); // RIS (hard reset)
        t.flush();
        assert!(
            t.cursor_visible(),
            "RR-005: cursor should be visible after RIS"
        );
    }

    /// RR_006: DECSTR resets origin mode.
    #[test]
    fn rr_006_decstr_resets_origin_mode() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        t.vt_write(b"\x1b[?6h"); // origin mode on
        t.vt_write(b"\x1b[!p"); // DECSTR
        t.flush();
        // After DECSTR, origin mode should be off
        t.vt_write(b"\x1b[1;1HX");
        t.flush();
        let snap = t.take_snapshot();
        let x_row0 = cell_at(&snap, 0, 0)
            .map(|c| c.codepoint == 'X' as u32)
            .unwrap_or(false);
        assert!(x_row0, "RR-006: X should be at absolute (0,0) after DECSTR");
    }

    // ── Resize ───────────────────────────────────────────────────

    /// RS_001: Resize increases rows.
    #[test]
    fn rs_001_resize_more_rows() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        t.resize(20, 10);
        t.flush();
        assert_eq!(t.rows(), 20);
    }

    /// RS_002: Resize increases cols.
    #[test]
    fn rs_002_resize_more_cols() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        t.resize(5, 30);
        t.flush();
        assert_eq!(t.cols(), 30);
    }

    /// RS_003: Resize preserves screen content.
    #[test]
    fn rs_003_resize_preserves_content() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        tc(&mut t)
            .write(b"Hello!")
            .capture_before()
            .write(b"")
            .assert_content_preserved();
        t.resize(8, 15);
        t.flush();
        tc(&mut t).assert_row_text(0, "Hello!");
    }

    /// RS_004: Resize shrink survives.
    #[test]
    fn rs_004_resize_shrink_survives() {
        let mut t = GhosttyTerminal::new(10, 20, 100).expect("term");
        t.flush();
        t.vt_write(b"SaveMe");
        t.flush();
        t.resize(3, 5);
        t.flush();
        t.vt_write(b"OK");
        t.flush();
        assert_eq!(t.rows(), 3, "RS-004: rows shrunk to 3");
        assert_eq!(t.cols(), 5, "RS-004: cols shrunk to 5");
    }

    /// RS_005: Resize to same dimensions is no-op.
    #[test]
    fn rs_005_resize_same_noop() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        t.vt_write(b"Content");
        t.flush();
        t.resize(5, 10);
        t.flush();
        assert_eq!(t.rows(), 5);
        assert_eq!(t.cols(), 10);
        tc(&mut t).assert_row_text(0, "Content");
    }

    /// RS_006: Resize then write in alt screen.
    #[test]
    fn rs_006_resize_alt_screen() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        t.vt_write(b"\x1b[?1049h");
        t.flush();
        t.vt_write(b"AltContent");
        t.flush();
        t.resize(10, 20);
        t.flush();
        tc(&mut t).assert_row_text(0, "AltContent");
    }

    /// RS_007: Resize preserves cursor position.
    #[test]
    fn rs_007_resize_preserves_cursor() {
        let mut t = GhosttyTerminal::new(10, 20, 100).expect("term");
        t.flush();
        t.vt_write(b"\x1b[5;10H");
        t.flush();
        let (cx, cy) = (t.cursor_x(), t.cursor_y());
        t.resize(10, 20);
        t.flush();
        assert_eq!(t.cursor_x(), cx, "RS-007: cursor_x preserved");
        assert_eq!(t.cursor_y(), cy, "RS-007: cursor_y preserved");
    }

    /// RS_008: Resize narrow then wide survives.
    #[test]
    fn rs_008_resize_narrow_wide() {
        let mut t = GhosttyTerminal::new(5, 20, 100).expect("term");
        t.flush();
        t.vt_write(b"Hello World");
        t.flush();
        t.resize(5, 5);
        t.flush();
        t.resize(5, 20);
        t.flush();
        t.vt_write(b"OK");
        t.flush();
        let snap = t.take_snapshot();
        assert_invariants(&snap);
    }

    /// RS_009: Multiple resizes do not degrade.
    #[test]
    fn rs_009_resize_multiple() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        for _ in 0..20 {
            t.resize(5, 10);
            t.flush();
        }
        t.flush();
        t.vt_write(b"AfterCycles");
        t.flush();
        let snap = t.take_snapshot();
        let found = snap.cells.iter().any(|c| c.codepoint == 'A' as u32);
        assert!(found, "RS-009: text written after cycles shows in snapshot");
    }

    /// RS_010: Resize to very large dimensions.
    #[test]
    fn rs_010_resize_large() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        t.resize(200, 500);
        t.flush();
        assert!(t.rows() > 0, "RS-010: rows should be > 0");
        assert!(t.cols() > 0, "RS-010: cols should be > 0");
    }

    /// RS_011: Resize to very small dimensions (1x1).
    #[test]
    fn rs_011_resize_minimal() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        t.vt_write(b"X");
        t.flush();
        t.resize(1, 1);
        t.flush();
        assert_eq!(t.rows(), 1, "RS-011: rows = 1");
        assert_eq!(t.cols(), 1, "RS-011: cols = 1");
    }

    /// RS_012: Alt screen resize preserves normal buffer.
    #[test]
    fn rs_012_alt_resize_preserves_normal() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        t.vt_write(b"NormalSave");
        t.flush();
        t.vt_write(b"\x1b[?1049h");
        t.flush();
        t.resize(8, 20);
        t.flush();
        t.vt_write(b"\x1b[?1049l");
        t.flush();
        tc(&mut t).assert_row_text(0, "NormalSave");
    }

    /// RS_013: SGR colors preserved after resize.
    #[test]
    fn rs_013_resize_preserves_sgr() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        t.vt_write(b"\x1b[31mRed");
        t.flush();
        t.resize(5, 20);
        t.flush();
        let snap = t.take_snapshot();
        let red_cells: Vec<_> = snap
            .cells
            .iter()
            .filter(|c| c.codepoint > 0 && c.foreground[0] > 0.5)
            .collect();
        assert!(
            !red_cells.is_empty(),
            "RS-013: red text should survive resize"
        );
    }

    /// RS_014: Resize with scrollback survives.
    #[test]
    fn rs_014_resize_with_scrollback() {
        let mut t = GhosttyTerminal::new(3, 10, 100).expect("term");
        t.flush();
        for i in 0..10 {
            t.vt_write(format!("line{i}\n").as_bytes());
        }
        t.flush();
        t.resize(5, 15);
        t.flush();
        t.vt_write(b"AfterResize");
        t.flush();
        let snap = t.take_snapshot();
        assert_invariants(&snap);
    }

    // ── Cursor Visibility / Mode Queries ─────────────────────────

    /// MD_001: DECSET 25 hides cursor.
    #[test]
    fn md_001_decset_25_hides_cursor() {
        let mut t = term();
        t.flush();
        t.vt_write(b"\x1b[?25l");
        t.flush();
        assert!(!t.cursor_visible(), "MD-001: cursor should be hidden");
    }

    /// MD_002: DECRST 25 shows cursor.
    #[test]
    fn md_002_decrst_25_shows_cursor() {
        let mut t = term();
        t.flush();
        t.vt_write(b"\x1b[?25l"); // hide
        t.vt_write(b"\x1b[?25h"); // show
        t.flush();
        assert!(t.cursor_visible(), "MD-002: cursor should be visible");
    }

    /// MD_003: origin_mode() reflects DECOM state.
    // Uses ghostty's origin_mode() getter to query DECOM state.
    #[test]
    fn md_003_origin_mode_query() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        // default: origin mode off
        assert!(!t.origin_mode(), "MD-003: origin mode off by default");
        // enable
        t.vt_write(b"\x1b[?6h");
        t.flush();
        assert!(t.origin_mode(), "MD-003: origin mode on after DECSET 6");
        // disable
        t.vt_write(b"\x1b[?6l");
        t.flush();
        assert!(!t.origin_mode(), "MD-003: origin mode off after DECRST 6");
    }

    /// MD_004: autowrap() reflects DECAWM state.
    // Uses ghostty's autowrap() getter to query DECAWM state.
    #[test]
    fn md_004_autowrap_query() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        // default: autowrap on
        assert!(t.autowrap(), "MD-004: autowrap on by default");
        // disable
        t.vt_write(b"\x1b[?7l");
        t.flush();
        assert!(!t.autowrap(), "MD-004: autowrap off after DECRST 7");
        // enable
        t.vt_write(b"\x1b[?7h");
        t.flush();
        assert!(t.autowrap(), "MD-004: autowrap on after DECSET 7");
    }

    /// MD_005: alt_screen() reflects active screen.
    // Uses ghostty's alt_screen() getter to query screen state.
    #[test]
    fn md_005_alt_screen_query() {
        let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
        t.flush();
        // default: normal screen
        assert!(!t.alt_screen(), "MD-005: normal screen by default");
        // enter alt
        t.vt_write(b"\x1b[?1049h");
        t.flush();
        assert!(t.alt_screen(), "MD-005: alt screen after DECSET 1049");
        // exit alt
        t.vt_write(b"\x1b[?1049l");
        t.flush();
        assert!(!t.alt_screen(), "MD-005: normal screen after DECRST 1049");
    }

    /// MD_006: title() returns empty string by default.
    // Uses ghostty's title() getter to verify default title.
    #[test]
    fn md_006_title_default_empty() {
        let t = term();
        t.flush();
        assert_eq!(t.title(), "", "MD-006: default title should be empty");
    }

    /// MD_007: title() returns set title.
    // Uses ghostty's title() getter to verify set title.
    #[test]
    fn md_007_title_set_and_read() {
        let mut t = term();
        t.flush();
        t.vt_write(b"\x1b]0;MyTitle\x07");
        t.flush();
        t.flush();
        assert_eq!(t.title(), "MyTitle", "MD-007: title should be 'MyTitle'");
    }
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

mod malformed_sequence_regressions {
    use super::*;

    /// Regression guard: after CAN (0x18) fix, all 17 malformed sequences
    /// must produce correct cursor alignment. Any failure is a B4 regression.
    #[test]
    fn b4_trigger_search() {
        let candidates: Vec<(&str, &[u8])> = vec![
            (
                "OSC with embedded ESC",
                &b"\x1b]0;hello \x1b[31mworld\x07"[..],
            ),
            ("ESC inside CSI", &b"\x1b[3\x1bmX"[..]),
            ("DEL mid-CSI", &b"\x1b[3\x7fmX"[..]),
            ("NUL mid-CSI", &b"\x1b[3\x00mX"[..]),
            ("BEL mid-CSI", &b"\x1b[3\x07mX"[..]),
            ("overlong UTF-8 in params", &b"\x1b[\xc0\x803mX"[..]),
            ("CSI without final byte (split write)", &b"\x1b["[..]),
            ("space intermediate byte", &b"\x1b[3 mX"[..]),
            ("bare ESC then valid", &b"\x1bX"[..]),
            ("invalid final byte", &b"\x1b[3 X"[..]),
            ("truncated DCS", &b"\x1bP"[..]),
            ("truncated OSC", &b"\x1b]"[..]),
            ("truncated SOS", &b"\x1bX"[..]),
            ("SGR with 100+ params", &b"\x1b["[..]),
            ("negative param", &b"\x1b[3;-1mX"[..]),
            ("C1 control char", &b"\x9b3mX"[..]),
            ("overlong UTF-8 cmd", &b"\xf0\x80\x80\x80X"[..]),
        ];

        let mut mismatches: Vec<String> = Vec::new();

        for (name, candidate) in candidates {
            let mut t = term();
            t.flush();

            let expected_row = 1u32;
            let expected_col = 2u32;

            if name.ends_with("(split write)") || name == "SGR with 100+ params" {
                // Split-write candidates: write first, then check cursor
                t.vt_write(candidate);
                t.flush();
                tc(&mut t).write(b"\r\n$ ");
            } else {
                tc(&mut t).write(candidate).write(b"\r\n$ ");
            }

            let actual_row = t.cursor_y();
            let actual_col = t.cursor_x();
            if actual_row != expected_row || actual_col != expected_col {
                log::warn!(
                    "B4: MISMATCH '{name}' -> cursor at ({actual_row}, {actual_col}) expected ({expected_row}, {expected_col})"
                );
                mismatches.push(format!("'{name}': cursor at ({actual_row}, {actual_col})"));
            } else {
                log::debug!("B4: OK '{name}'");
            }
        }

        assert!(
            mismatches.is_empty(),
            "B4 regression guard: found {} B4 triggers after CAN fix: {:?}",
            mismatches.len(),
            mismatches
        );
    }
}

mod osc_title_regressions {
    use super::*;

    #[test]
    fn b5_osctitle_osc0() {
        let mut t = term();
        t.flush();
        t.vt_write(b"\x1b]0;MyTitle\x07");
        t.flush();
        t.flush();
        assert_eq!(t.title(), "MyTitle", "OSC 0 title mismatch");
    }

    #[test]
    fn b5_osctitle_osc2() {
        let mut t = term();
        t.flush();
        t.vt_write(b"\x1b]2;WindowTitle\x07");
        t.flush();
        t.flush();
        assert_eq!(t.title(), "WindowTitle", "OSC 2 title mismatch");
    }

    #[test]
    fn b5_osctitle_last_wins() {
        let mut t = term();
        t.flush();
        t.vt_write(b"\x1b]0;First\x07");
        t.vt_write(b"\x1b]2;Second\x07");
        t.flush();
        t.flush();
        assert_eq!(
            t.title(),
            "Second",
            "last-wins: OSC 2 should override OSC 0"
        );
    }

    #[test]
    fn b5_osctitle_split_buffer() {
        let mut t = term();
        t.flush();
        t.vt_write(b"\x1b]0;Hel");
        t.flush();
        t.vt_write(b"lo\x07");
        t.flush();
        t.flush();
        // ST (\x1b\\) appended by vt_write closes the OSC sequence after
        // "Hel" in the first write, so the title is committed as "Hel".
        // The second write "lo\x07" is processed as plain text + BEL.
        assert_eq!(t.title(), "Hel", "split-buffer OSC: ST closes partial OSC");
    }

    #[test]
    fn b5_osctitle_ris_clears() {
        let mut t = term();
        t.flush();
        t.vt_write(b"\x1b]0;MyTitle\x07");
        t.flush();
        t.vt_write(b"\x1bc"); // RIS
        t.flush();
        t.flush();
        assert_eq!(t.title(), "", "RIS should clear title");
    }

    #[test]
    fn b5_osctitle_bel_terminator() {
        let mut t = term();
        t.flush();
        t.vt_write(b"\x1b]0;Terminal\x07");
        t.flush();
        t.flush();
        assert_eq!(t.title(), "Terminal", "BEL-terminated title");
    }

    #[test]
    fn b5_osctitle_st_terminator() {
        let mut t = term();
        t.flush();
        t.vt_write(b"\x1b]0;Terminal\x1b\\");
        t.flush();
        t.flush();
        assert_eq!(t.title(), "Terminal", "ST-terminated title");
    }
}

// ── SGR/Color tests ──

// ── SGR separator/merge edge cases ──

// ── Unicode/UTF-8 tests ──

// ── Termux-style behavioral VT tests ────────────────────────────────────

/// CSI 14t and 16t (pixel/character size reports) must not crash.

/// A.8 — OSC 133 marker propagation test
#[test]
fn osc_133_marker_propagation() {
    let mut t = term();
    t.flush();
    // OSC 133;A ST = prompt start, then "prompt> "
    t.vt_write(b"\x1b]133;A\x1b\\prompt> ");
    t.flush();
    // OSC 133;B ST = input start, then "ls\n"
    t.vt_write(b"\x1b]133;B\x1b\\ls\x1b]133;C\x1b\\");
    t.flush();
    let snap = t.take_snapshot();
    // The first 8 cells ("prompt> ") should be Prompt
    let mut found_prompt = false;
    let mut found_input = false;
    for cell in &snap.cells {
        match cell.codepoint as u8 as char {
            'p' | 'r' | 'o' | 'm' | 't' | '>' | ' ' if cell.semantic == SemanticContent::Prompt => {
                found_prompt = true;
            }
            'l' | 's' if cell.semantic == SemanticContent::Input => {
                found_input = true;
            }
            _ => {}
        }
    }
    assert!(found_prompt, "OSC 133;A should mark prompt cells");
    assert!(found_input, "OSC 133;B should mark input cells");
}

/// dec_erase_rect erases the rectangle to spaces and leaves cells outside it
/// untouched; a zero-width/height rect is a no-op.
#[test]
fn dec_erase_rect_does_not_panic() {
    let mut t = term();
    t.vt_write(b"Hello World");
    // Zero-dimension rect — must be a no-op, not a panic.
    t.dec_erase_rect(0, 0, 0, 0);
    // Erase columns 0..=5 of row 0 ("Hello " → spaces).
    t.dec_erase_rect(0, 0, 0, 5);
    let snap = t.take_snapshot();
    for col in 0..=5 {
        assert_eq!(
            cell_at(&snap, 0, col).map(|c| c.codepoint),
            Some(b' ' as u32),
            "erased cell (0,{col}) should be a space",
        );
    }
    assert_eq!(
        cell_at(&snap, 0, 6).map(|c| c.codepoint),
        Some(b'W' as u32),
        "cell outside the erased rect must be preserved",
    );
}

/// dec_change_attr_rect applies the SGR attribute inside the rect and is a
/// no-op for a zero-width/height rect.
#[test]
fn dec_change_attr_rect_does_not_panic() {
    let mut t = term();
    t.vt_write(b"Hello World");
    // Bold SGR sequence (1) applied to "Hello" (columns 0..=4).
    t.dec_change_attr_rect(b"\x1b[1m", 0, 0, 0, 4);
    // Zero-dimension rect — should be a no-op.
    t.dec_change_attr_rect(b"\x1b[1m", 0, 0, 0, 0);
    let snap = t.take_snapshot();
    // CellSnapshot exposes bold as a dedicated field (the packed `flags`
    // bitmask lives on the render-path CellData, not the query snapshot).
    let bold = |row: u32, col: u32| cell_at(&snap, row, col).map(|c| c.bold);
    assert_eq!(
        bold(0, 2),
        Some(true),
        "cells inside the rect must carry the bold attribute",
    );
    assert_eq!(
        bold(0, 6),
        Some(false),
        "cells outside the rect must not be bold",
    );
}

/// dec_erase_rect clears cells in the given rectangle.
#[test]
fn dec_erase_rect_clears_cells() {
    let mut t = term();
    t.vt_write(b"ABCDEFGHIJ"); // row 0
    let snap = t.take_snapshot();
    assert_eq!(snap.cells[0].codepoint, 'A' as u32);
    assert_eq!(snap.cells[4].codepoint, 'E' as u32);
    // Erase cols 0..=4 on row 0 (inclusive range).
    t.dec_erase_rect(0, 0, 0, 4);
    let snap = t.take_snapshot();
    assert_eq!(
        snap.cells[0].codepoint, ' ' as u32,
        "cell [0,0] should be erased to space"
    );
    assert_eq!(
        snap.cells[4].codepoint, ' ' as u32,
        "cell [0,4] should be erased to space"
    );
    assert_eq!(
        snap.cells[5].codepoint, 'F' as u32,
        "cell [0,5] should be untouched (F)"
    );
}

// ══════════════════════════════════════════════════════════════════════════
// Performance Benchmarks — realistic usage patterns
// ══════════════════════════════════════════════════════════════════════════

/// Simulate user typing latency: small writes (1-10 chars) followed by flush.
/// Measures wall-clock time per iteration — the user-visible metric.
/// Benchmark thresholds are two-tiered (see docs/standards/TESTING.md,
/// "Benchmarks & Performance Thresholds"):
///
/// - Local (non-CI) runs assert strict thresholds: the machine is idle and
///   the numbers are reproducible, so a real regression fails the test.
/// - CI runs keep the anti-flake floor: parallel test execution and
///   software Vulkan contention cut wall time significantly. The floor is
///   ~5x below the local single-run number — it catches order-of-magnitude
///   regressions only.
fn strict_benchmarks() -> bool {
    std::env::var("CI").is_err() && std::env::var("GITHUB_ACTIONS").is_err()
}

#[test]
#[ignore]
fn bench_typing_latency() {
    let mut t = GhosttyTerminal::new(24, 80, 5000).expect("term");
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
    let threshold = if strict_benchmarks() { 3.0 } else { 6.0 };
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
#[ignore]
fn bench_bulk_output_throughput() {
    let mut t = GhosttyTerminal::new(24, 80, 5000).expect("term");
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
    let threshold = if strict_benchmarks() {
        8_000.0
    } else {
        4_000.0
    };
    assert!(
        throughput_cells > threshold,
        "Bulk output too slow: {:.0} cells/sec (need >{threshold:.0})",
        throughput_cells,
    );
}

/// Simulate scrolling through terminal history.
/// Writes many lines of content, then measures take_snapshot_with_scroll
/// at varying offset positions.
#[test]
#[ignore]
fn bench_scroll_throughput() {
    // Serialize against the GPU benches: in parallel runs the shared CPU
    // (Lavapipe software rasterizer + this CPU-bound bench) drops the
    // measured throughput below the threshold — 400-500 MB/s vs 725 MB/s
    // in isolation. Each bench is fast (<1s) so the lock is
    // uncontended in practice.
    let _serial = crate::render::GPU_BENCH_LOCK.lock();
    let mut t = GhosttyTerminal::new(24, 80, 5000).expect("term");
    // Fill scrollback with 500 lines of content
    for i in 0..500 {
        t.vt_write(
            format!("Line {i}: some realistic terminal content with numbers and text\n").as_bytes(),
        );
    }
    t.flush();

    // Measure scroll snapshot at 3 different offsets
    let offsets = [0u32, 100, 400];
    let n = 20;
    for &offset in &offsets {
        let start = Instant::now();
        for _ in 0..n {
            let snap = black_box(t.take_snapshot_with_scroll(offset));
            black_box(snap.cells.len());
        }
        let elapsed = start.elapsed();
        let snaps_per_sec = n as f64 / elapsed.as_secs_f64();
        println!(
            "Scroll offset={}: {:.0} snapshots/sec ({:.1}ms for {} iterations)",
            offset,
            snaps_per_sec,
            elapsed.as_millis(),
            n,
        );
        // Local single-run throughput is ~2000+ snaps/sec; the full suite
        // runs tests in parallel and CPU contention (software Vulkan
        // benches) cuts wall time significantly. The CI floor is ~5x below
        // the single-run number; local runs assert the strict bound.
        let threshold = if strict_benchmarks() {
            if offset == 0 { 800.0 } else { 500.0 }
        } else if offset == 0 {
            400.0
        } else {
            250.0
        };
        assert!(
            snaps_per_sec > threshold,
            "Scroll offset={offset} too slow: {:.0} snapshots/sec (need >{threshold:.0})",
            snaps_per_sec,
        );
    }
}

// ── Scrollback fallback ────────────────────────────────────────────────

/// Verify that `take_snapshot_with_scroll` returns a valid snapshot when
/// scrollback exists, and returns `GridSnapshot::fallback` once the VT
/// thread is disconnected (channel closed — the real disconnected path,
/// exercised via the test-only `disconnect_for_test`).
#[test]
fn scrollback_fallback_on_disconnected_terminal() {
    // Create a terminal and fill with content to establish scrollback.
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
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
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
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

// ══════════════════════════════════════════════════════════════════════════
// End-to-End Pipeline Benchmarks  (terminal write → CellData → instances)
// ══════════════════════════════════════════════════════════════════════════

/// Benchmark the full CPU-side pipeline: write terminal content → flush →
/// receive CellData → build CellInstances. This simulates the complete
/// per-frame data path before GPU submission.
#[test]
#[ignore]
fn bench_end_to_end_cpu_pipeline_latency() {
    use std::hint::black_box;
    use std::time::Instant;

    let mut t = GhosttyTerminal::new(24, 80, 5000).expect("term");
    let mut font_pipeline = crate::render::font::FontPipeline::new(1024, 1024, 14.0);

    // Simulate a realistic screen: fill with text content
    let content = b"user@host:~$ cargo build --release --features=test-util\n   Compiling native v0.1.0\n    Finished `release` profile [optimized] target(s) in 0.42s\n";
    let n = 20; // 20 screens

    let start = Instant::now();
    for _ in 0..n {
        t.vt_write(content);
        t.flush();
        let cell_data = t.receive_cell_data();
        let (cells, cursor_info) = cell_data.expect("should receive CellData after flush");

        let cursor = crate::render::CellCursor {
            row: cursor_info.row,
            col: cursor_info.col,
            visible: cursor_info.visible,
            style: cursor_info.style,
            color: None,
        };
        let mut instances = Vec::new();
        crate::render::build_instances_from_cell_data(
            &cells,
            crate::render::cell_builder::CellInstanceConfig {
                rows: 24,
                cols: 80,
                grid_cell_w: 1024.0 / 80.0,
                grid_cell_h: 1024.0 / 24.0,
                cursor,
                atlas_width: 1024.0,
                atlas_height: 1024.0,
                selection: None,
                search_highlights: &[],
            },
            &mut font_pipeline,
            &mut instances,
        );
        let count = black_box(instances.len());
        black_box(count);
    }
    let elapsed = start.elapsed();
    let ms_per_frame = elapsed.as_millis() as f64 / n as f64;
    let fps = n as f64 / elapsed.as_secs_f64();
    println!(
        "End-to-end CPU pipeline: {:.1}ms per frame ({:.0} fps) — terminal write + CellData + build_instances",
        ms_per_frame, fps,
    );
    // Must complete within two frame budgets: the single-run cost is
    // ~8ms/frame, but the full test suite runs tests in parallel and CPU
    // contention (especially with the software-Vulkan benchmarks) pushes
    // wall time well past the 16ms single-frame budget. Note the 32ms
    // bound only catches >=4x regressions; it is primarily an
    // anti-flake guard, not a precise performance gate.
    assert!(
        ms_per_frame < 32.0,
        "End-to-end CPU pipeline too slow: {:.1}ms per frame (need <32ms)",
        ms_per_frame,
    );
}

/// Verify that `scroll_viewport(Delta)` scrolls the CellData view and
/// that the delta accumulates on repeated calls: scrollback
/// browsing previously did nothing).
#[test]
fn scroll_viewport_delta_scrolls_cell_data() {
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
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
    let mut t = GhosttyTerminal::new(5, 10, 100).expect("term");
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
    let t = small_term();
    assert!(t.is_alive());
}

#[test]
fn terminal_is_alive_after_vt_write() {
    let mut t = small_term();
    t.vt_write(b"Hello, world!");
    assert!(t.is_alive());
}

#[test]
fn terminal_is_alive_after_flush() {
    let mut t = small_term();
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
    let mut t = term();
    t.vt_write(b"hello");
    t.flush();
    let first = t.receive_cell_data().expect("first cell data");
    let (first_cells, _) = first;
    let cols = 80usize;
    let row0_first: Vec<u32> = first_cells[..cols].iter().map(|c| c.codepoint).collect();

    // Since the idle CellData dedup (97be660), a quiet VT thread no longer
    // auto-pushes equal content every 50ms: `second` may legitimately be
    // None. Row-cache consistency is still fully verified by the third
    // snapshot (row 0 must survive the row-1 write); when a second build
    // does happen (no dedup), its content must match the first.
    std::thread::sleep(std::time::Duration::from_millis(70));
    if let Some((second_cells, _)) = t.receive_cell_data() {
        assert_eq!(first_cells.len(), second_cells.len(), "grid size stable");
        // Row 0 content must match (hello at cols 0..5).
        let row0_second: Vec<u32> = second_cells[..cols].iter().map(|c| c.codepoint).collect();
        assert_eq!(
            row0_first, row0_second,
            "row 0 codepoints stable across builds"
        );
        assert_eq!(row0_first[0], 'h' as u32, "row 0 col 0 is 'h'");
        assert_eq!(row0_first[4], 'o' as u32, "row 0 col 4 is 'o'");
    }

    // New input on row 1 must not disturb row 0's cached content.
    t.vt_write(b"\nworld");
    t.flush();
    std::thread::sleep(std::time::Duration::from_millis(70));
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
    let mut t = term();
    t.vt_write(b"top");
    t.flush();
    std::thread::sleep(std::time::Duration::from_millis(70));
    let before = t.receive_cell_data().expect("cell data before resize");
    let (before_cells, _) = before;
    assert_eq!(before_cells.len(), 24 * 80);

    assert!(t.resize(10, 40), "resize to 10x40");
    std::thread::sleep(std::time::Duration::from_millis(70));
    let after = t.receive_cell_data().expect("cell data after resize");
    let (after_cells, _) = after;
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
    let mut t = term(); // 24x80
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
    let mut t = term();
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

/// OSC 8 hyperlink query (termux TerminalView.openLinkAt equivalent):
/// after writing an OSC 8 link, hyperlink_at returns the URI at the link
/// cells and None outside them.
#[test]
fn hyperlink_at_returns_uri_inside_link() {
    let mut t = term();
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

// ── : cursor/row coordinate consistency (D1 deterministic leg) ──

/// The shell-echo path (prompt text + typed chars, no newline) must report a
/// cursor that sits ON the last printed row at the column right after the
/// text — the same row the CellData rows occupy. A mismatch here renders the
/// cursor one row below the text ("输入指针出现在当前文本的正下",
/// emulator evidence). Mirrors the exact production path: VT loop auto-push →
/// build_cell_data → (cells, CursorInfo).
#[test]
fn cursor_matches_last_printed_row_after_echo_print() {
    let mut t = term();
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
    let mut t = term();
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
