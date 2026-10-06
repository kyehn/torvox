use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::terminal::ghostty_terminal::GhosttyTerminal;
use crate::terminal::ghostty_terminal::{CellSnapshot, DumpedGrid};

/// 快照格式版本，出现破坏性改动时递增。
const SNAPSHOT_VERSION: u32 = 1;

/// JSON 快照中单元的可读表示。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CellJson {
    /// 单元内容（字符或空串）。
    pub content: String,
    /// 前景色，十六进制 "RRGGBB"，空串表示默认。
    #[serde(default)]
    pub foreground: String,
    /// 背景色，十六进制 "RRGGBB"，空串表示默认。
    #[serde(default)]
    pub background: String,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub underline: bool,
    #[serde(default)]
    pub reverse: bool,
}

/// 供回归测试使用的终端状态快照。
/// 与 `.seq` 输入文件并排存为 JSON 文件。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TestSnapshot {
    pub version: u32,
    pub rows: u32,
    pub cols: u32,
    pub cursor_row: u32,
    pub cursor_col: u32,
    pub cursor_visible: bool,
    /// 回滚行数。
    pub scrollback_rows: u32,
    /// 可见网格单元，按行优先排列。
    pub cells: Vec<CellJson>,
    /// 回滚区单元，内层 Vec 为一行。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scrollback: Vec<Vec<CellJson>>,
    /// 生成该快照时所用的主题名（可选）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_name: Option<String>,
}

fn cell_to_json(cell: &CellSnapshot) -> CellJson {
    CellJson {
        content: if cell.codepoint == 0 {
            String::new()
        } else {
            char::from_u32(cell.codepoint)
                .map(|c| c.to_string())
                .unwrap_or_default()
        },
        foreground: if cell.foreground[3] == 0.0 {
            String::new()
        } else {
            format!(
                "{:02X}{:02X}{:02X}",
                (cell.foreground[0] * 255.0).round() as u8,
                (cell.foreground[1] * 255.0).round() as u8,
                (cell.foreground[2] * 255.0).round() as u8
            )
        },
        background: if cell.background[3] == 0.0 {
            String::new()
        } else {
            format!(
                "{:02X}{:02X}{:02X}",
                (cell.background[0] * 255.0).round() as u8,
                (cell.background[1] * 255.0).round() as u8,
                (cell.background[2] * 255.0).round() as u8
            )
        },
        bold: cell.bold,
        italic: cell.italic,
        underline: cell.underline,
        reverse: cell.reverse,
    }
}

/// 从当前终端状态采集 `TestSnapshot`。
pub fn capture_snapshot(terminal: &GhosttyTerminal) -> TestSnapshot {
    let dumped = terminal.dump_grid();
    let cursor_x = terminal.cursor_x();
    let cursor_y = terminal.cursor_y();
    let cursor_visible = terminal.cursor_visible();
    from_dumped_grid(&dumped, cursor_x, cursor_y, cursor_visible)
}

fn from_dumped_grid(
    dumped: &DumpedGrid,
    cursor_x: u32,
    cursor_y: u32,
    cursor_visible: bool,
) -> TestSnapshot {
    let cells: Vec<CellJson> = dumped.visible.iter().map(cell_to_json).collect();
    let scrollback: Vec<Vec<CellJson>> = dumped
        .scrollback
        .iter()
        .map(|row| row.iter().map(cell_to_json).collect())
        .collect();
    TestSnapshot {
        version: SNAPSHOT_VERSION,
        rows: dumped.rows,
        cols: dumped.cols,
        cursor_row: cursor_y,
        cursor_col: cursor_x,
        cursor_visible,
        scrollback_rows: dumped.scrollback.len() as u32,
        cells,
        scrollback,
        theme_name: None,
    }
}

/// 两个快照的比较结果。
#[derive(Debug, Default)]
pub struct DiffResult {
    /// 映射："R:C" -> 该单元处的差异描述。
    pub cell_diffs: HashMap<(u32, u32), String>,
    /// 光标位置差异。
    pub cursor_diff: Option<String>,
    /// 回滚区长度差异。
    pub scrollback_diff: Option<String>,
    /// 尺寸差异。
    pub dimension_diff: Option<String>,
}

impl DiffResult {
    pub fn is_empty(&self) -> bool {
        self.cell_diffs.is_empty()
            && self.cursor_diff.is_none()
            && self.scrollback_diff.is_none()
            && self.dimension_diff.is_none()
    }
}

/// 比较两个快照并返回差异。
pub fn diff(expected: &TestSnapshot, actual: &TestSnapshot) -> DiffResult {
    let mut result = DiffResult::default();

    if expected.rows != actual.rows || expected.cols != actual.cols {
        result.dimension_diff = Some(format!(
            "size {}x{} (expected) vs {}x{} (actual)",
            expected.cols, expected.rows, actual.cols, actual.rows
        ));
        return result;
    }

    for (i, (exp, act)) in expected.cells.iter().zip(actual.cells.iter()).enumerate() {
        let row = i as u32 / expected.cols;
        let col = i as u32 % expected.cols;
        let mut diffs = Vec::new();

        if exp.content != act.content {
            diffs.push(format!("content {:?} got {:?}", exp.content, act.content));
        }
        if exp.foreground != act.foreground {
            diffs.push(format!(
                "foreground {} got {}",
                exp.foreground, act.foreground
            ));
        }
        if exp.background != act.background {
            diffs.push(format!(
                "background {} got {}",
                exp.background, act.background
            ));
        }
        if exp.bold != act.bold {
            diffs.push(format!("bold {} got {}", exp.bold, act.bold));
        }
        if exp.italic != act.italic {
            diffs.push(format!("italic {} got {}", exp.italic, act.italic));
        }
        if exp.underline != act.underline {
            diffs.push(format!("underline {} got {}", exp.underline, act.underline));
        }
        if exp.reverse != act.reverse {
            diffs.push(format!("reverse {} got {}", exp.reverse, act.reverse));
        }

        if !diffs.is_empty() {
            result.cell_diffs.insert((row, col), diffs.join(", "));
        }
    }

    if expected.cursor_row != actual.cursor_row
        || expected.cursor_col != actual.cursor_col
        || expected.cursor_visible != actual.cursor_visible
    {
        result.cursor_diff = Some(format!(
            "cursor ({},{}) vis={} expected, ({},{}) vis={} actual",
            expected.cursor_row,
            expected.cursor_col,
            expected.cursor_visible,
            actual.cursor_row,
            actual.cursor_col,
            actual.cursor_visible,
        ));
    }

    if expected.scrollback_rows != actual.scrollback_rows {
        result.scrollback_diff = Some(format!(
            "scrollback rows {} expected, {} actual",
            expected.scrollback_rows, actual.scrollback_rows
        ));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::ghostty_terminal::GhosttyTerminal;

    fn make_terminal(rows: u32, cols: u32) -> GhosttyTerminal {
        GhosttyTerminal::new(rows, cols, 1000).expect("terminal")
    }

    #[test]
    fn capture_empty_snapshot() {
        let terminal = make_terminal(24, 80);
        let snap = capture_snapshot(&terminal);
        assert_eq!(snap.rows, 24);
        assert_eq!(snap.cols, 80);
        assert_eq!(snap.cells.len(), 24 * 80);
        assert!(snap.cursor_visible);
        assert_eq!(snap.scrollback_rows, 0);
    }

    #[test]
    fn capture_with_content() {
        let mut terminal = make_terminal(3, 10);
        terminal.vt_write(b"Hi");
        terminal.flush();
        let snap = capture_snapshot(&terminal);
        assert_eq!(snap.cells[0].content, "H");
        assert_eq!(snap.cells[1].content, "i");
        assert!(snap.cells[2].content.is_empty());
        assert_eq!(snap.cursor_col, 2);
        assert_eq!(snap.cursor_row, 0);
    }

    #[test]
    fn diff_identical_is_empty() {
        let terminal = make_terminal(3, 5);
        let a = capture_snapshot(&terminal);
        let b = capture_snapshot(&terminal);
        let result = diff(&a, &b);
        assert!(result.is_empty());
    }

    #[test]
    fn diff_detects_content_change() {
        let terminal = make_terminal(3, 5);
        let snap1 = capture_snapshot(&terminal);

        let mut snap2 = snap1.clone();
        snap2.cells[0].content = "X".to_string();

        let result = diff(&snap1, &snap2);
        assert!(!result.is_empty());
        assert!(result.cell_diffs.contains_key(&(0, 0)));
    }

    #[test]
    fn diff_detects_cursor_change() {
        let terminal = make_terminal(3, 5);
        let snap1 = capture_snapshot(&terminal);

        let mut snap2 = snap1.clone();
        snap2.cursor_col = 10;

        let result = diff(&snap1, &snap2);
        assert!(result.cursor_diff.is_some());
    }

    #[test]
    fn diff_detects_dimension_mismatch() {
        let mut terminal = make_terminal(3, 5);
        terminal.vt_write(b"test");
        terminal.flush();
        let snap1 = capture_snapshot(&terminal);

        let second_terminal = make_terminal(4, 5);
        let snap2 = capture_snapshot(&second_terminal);

        let result = diff(&snap1, &snap2);
        assert!(result.dimension_diff.is_some());
    }

    #[test]
    fn serde_round_trip() {
        let terminal = make_terminal(3, 10);
        let snap = capture_snapshot(&terminal);
        let json = serde_json::to_string_pretty(&snap).unwrap();
        let restored: TestSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snap, restored);
    }

    #[test]
    fn serde_with_content_round_trip() {
        let mut terminal = make_terminal(3, 10);
        terminal.vt_write(b"Hello\nWorld");
        terminal.flush();
        let snap = capture_snapshot(&terminal);
        let json = serde_json::to_string_pretty(&snap).unwrap();
        let restored: TestSnapshot = serde_json::from_str(&json).unwrap();
        let result = diff(&snap, &restored);
        assert!(result.is_empty(), "{result:?}");
    }

    #[test]
    fn scrollback_captured() {
        let mut terminal = GhosttyTerminal::new(3, 10, 100).expect("terminal");
        for i in 0..10u8 {
            terminal.vt_write(format!("line {i}\n").as_bytes());
        }
        terminal.flush();
        let snap = capture_snapshot(&terminal);
        assert!(snap.scrollback_rows > 0);
        assert!(!snap.scrollback.is_empty());
    }
}
