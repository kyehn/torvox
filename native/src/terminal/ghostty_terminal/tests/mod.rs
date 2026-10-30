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
//! 删除集合（上游 libghostty-vt 的职责，由上游自己的测试覆盖；依赖内的测试
//! 不随 `cargo test` 运行，故本仓任何包装改动都必须在上面四类里留下断言）：
//! 光标移动与钳制、擦除/插入/删除、制表位、滚动区域、复位、尺寸重排、
//! DECSET 模式矩阵、标题读写语义、纯 VT 一致性用例与重排回路
//! （对应的集成测试已按 TESTING「只测试本项目功能」删除）。

use std::hint::black_box;

use std::time::Instant;

use super::*;

use crate::terminal::ghostty_terminal::public_api::sanitize_vt_input;

use crate::terminal::test_helpers::{assert_invariants, row_text, search_viewport_matches};

use libghostty_vt::key::Mods;

fn terminal() -> GhosttyTerminal {
    GhosttyTerminal::new(24, 80, 1000).expect("terminal create")
}

/// 查询就绪轮询的上界与间隔。
const READY_POLL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

const READY_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(10);

fn small_terminal() -> GhosttyTerminal {
    GhosttyTerminal::new(3, 3, 100).expect("terminal")
}

/// 默认主题前景（浮点）：断言锚定当前默认调色板而不手写色值，
/// 换默认主题时用例不随之漂移失效。
fn default_foreground() -> [f32; 4] {
    GhosttyTerminal::color_to_float(crate::terminal::theme::default_dark().palette.foreground)
}

/// 默认主题背景（浮点）：同上，不手写色值。
fn default_background() -> [f32; 4] {
    GhosttyTerminal::color_to_float(crate::terminal::theme::default_dark().palette.background)
}

/// 默认主题第 `index` 号 ANSI 槽位色（浮点）：同上，不手写色值。
fn default_ansi(index: usize) -> [f32; 4] {
    GhosttyTerminal::color_to_float(crate::terminal::theme::default_dark().palette.ansi[index])
}

/// 单元前景与主题色（浮点）的比较：字节转浮点有量化误差。
fn assert_color(actual: [f32; 4], expected: [f32; 4], context: &str) {
    for channel in 0..3 {
        assert!(
            (actual[channel] - expected[channel]).abs() < 0.02,
            "{context}：通道{channel} 须为 {}，实际 {}",
            expected[channel],
            actual[channel]
        );
    }
}

/// 写入 20 行文本并排空首帧：视口滚动用例的共同前置。
fn terminal_with_twenty_lines() -> GhosttyTerminal {
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    for line_number in 0..20 {
        terminal_under_test.vt_write(format!("line {line_number}\n").as_bytes());
    }
    terminal_under_test.flush();
    assert!(
        terminal_under_test.receive_cell_data().is_some(),
        "首帧必须到达"
    );
    terminal_under_test
}

/// 等待一帧推送：内容不变的滚动与重推用例必须在超时前收到帧。
fn await_pushed_frame(terminal_under_test: &mut GhosttyTerminal, context: &str) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut pushed = false;
    while !pushed {
        pushed = terminal_under_test.receive_cell_data().is_some();
        assert!(
            pushed || std::time::Instant::now() < deadline,
            "{context}：2s 内未收到 CellData 帧"
        );
        if !pushed {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
}

/// 视口某格当前的前景色：选区反白与清除恢复断言的共同取格方式。
fn viewport_cell_foreground(terminal_under_test: &mut GhosttyTerminal, col: u32) -> [f32; 4] {
    terminal_under_test.flush();
    let (cells, _) = terminal_under_test.receive_cell_data().expect("cell data");
    cells
        .iter()
        .find(|cell| cell.row == 0 && cell.col == col)
        .expect("cell present")
        .foreground
}

/// 按码点取渲染单元：样式位断言的共同取格方式。
fn marked_cell(cells: &[CellData], marker: u8) -> &CellData {
    cells
        .iter()
        .find(|cell| cell.codepoint == marker as u32)
        .expect("marked cell present")
}

/// SGR31 前景须为 #FF5555：默认配置与默认主题配置共用同一判据。
fn assert_sgr31_red_foreground(cells: &[CellData], context: &str) {
    let red_cell = cells
        .iter()
        .find(|cell| cell.codepoint == 'R' as u32)
        .expect("R cell present");
    assert!(
        (red_cell.foreground[0] - 1.0).abs() < 0.02
            && (red_cell.foreground[1] - 0x55 as f32 / 255.0).abs() < 0.02
            && (red_cell.foreground[2] - 0x55 as f32 / 255.0).abs() < 0.02,
        "{context} 前景须为 #FF5555，实际 {:?}",
        red_cell.foreground
    );
}

#[test]
// 与字面量默认值精确比较，不涉及浮点运算。
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

/// 粘贴编码辅助：经上游 Native，两模式语义与参考一致。
fn encode_paste_text(text: &str, bracketed: bool) -> Vec<u8> {
    let mut data = text.as_bytes().to_vec();
    let mut output = vec![0u8; data.len() + 16];
    let written =
        libghostty_vt::paste::encode(&mut data, bracketed, &mut output).expect("paste encode");
    output.truncate(written);
    output
}

/// 取下一帧 CellData；VT 线程按批推送，轮询到有帧为止（不依赖固定时长）。
fn next_cell_frame(terminal_under_test: &GhosttyTerminal) -> Vec<CellData> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if let Some((cells, _)) = terminal_under_test.receive_cell_data() {
            return cells;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "5s 内未收到 CellData 帧"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

/// 按网格行取行文本：断言对象是渲染热路径消费的那份数据。
fn row_text_from_cells(cells: &[CellData], row: u32) -> String {
    let mut by_col: Vec<(u32, char)> = cells
        .iter()
        .filter(|cell| cell.row == row && cell.codepoint != 0)
        .map(|cell| (cell.col, char::from_u32(cell.codepoint).unwrap_or('\0')))
        .collect();
    by_col.sort_by_key(|(col, _)| *col);
    by_col.into_iter().map(|(_, ch)| ch).collect()
}

mod bell;
mod grid_text;
mod lifecycle;
mod mouse_osc;
mod osc_state;
mod render_cache;
mod selection_search;
