use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ghostty_terminal::GhosttyTerminal;
use crate::ghostty_terminal::{CellSnapshot, DumpedGrid};

/// Version of the snapshot format. Bump when making breaking changes.
const SNAPSHOT_VERSION: u32 = 1;

/// Human-readable cell representation for JSON snapshots.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CellJson {
    /// The cell content (character or empty string).
    pub content: String,
    /// Foreground color as hex "RRGGBB" or empty for default.
    #[serde(default)]
    pub fg: String,
    /// Background color as hex "RRGGBB" or empty for default.
    #[serde(default)]
    pub bg: String,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub underline: bool,
    #[serde(default)]
    pub reverse: bool,
}

/// Serializable snapshot of terminal state for regression testing.
/// Stored as JSON files alongside `.seq` input files.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TestSnapshot {
    pub version: u32,
    pub rows: u32,
    pub cols: u32,
    pub cursor_row: u32,
    pub cursor_col: u32,
    pub cursor_visible: bool,
    /// Number of scrollback rows.
    pub scrollback_rows: u32,
    /// Visible grid cells in row-major order.
    pub cells: Vec<CellJson>,
    /// Scrollback cells, each inner Vec is one row.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scrollback: Vec<Vec<CellJson>>,
    /// Optional theme name that this snapshot was generated with.
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
        fg: if cell.foreground[3] == 0.0 {
            String::new()
        } else {
            format!(
                "{:02X}{:02X}{:02X}",
                (cell.foreground[0] * 255.0).round() as u8,
                (cell.foreground[1] * 255.0).round() as u8,
                (cell.foreground[2] * 255.0).round() as u8
            )
        },
        bg: if cell.background[3] == 0.0 {
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

/// Capture a `TestSnapshot` from the current terminal state.
pub fn capture_snapshot(term: &GhosttyTerminal) -> TestSnapshot {
    let dumped = term.dump_grid();
    let cursor_x = term.cursor_x();
    let cursor_y = term.cursor_y();
    let cursor_visible = term.cursor_visible();
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

/// Result of comparing two snapshots.
#[derive(Debug, Default)]
pub struct DiffResult {
    /// Map: "R:C" -> description of mismatch at that cell.
    pub cell_diffs: HashMap<(u32, u32), String>,
    /// Cursor position mismatch.
    pub cursor_diff: Option<String>,
    /// Scrollback length mismatch.
    pub scrollback_diff: Option<String>,
    /// Dimension mismatch.
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

/// Compare two snapshots and return the differences.
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
        if exp.fg != act.fg {
            diffs.push(format!("fg {} got {}", exp.fg, act.fg));
        }
        if exp.bg != act.bg {
            diffs.push(format!("bg {} got {}", exp.bg, act.bg));
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

/// Load expected snapshot from a `.json` file.
pub fn load_expected(path: &Path) -> TestSnapshot {
    let data = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read expected snapshot {path:?}: {e}"));
    serde_json::from_str(&data).unwrap_or_else(|e| panic!("invalid JSON in {path:?}: {e}"))
}

/// Save snapshot to a `.json` file.
pub fn save_snapshot(path: &Path, snap: &TestSnapshot) {
    let data = serde_json::to_string_pretty(snap)
        .unwrap_or_else(|e| panic!("failed to serialize snapshot: {e}"));
    fs::write(path, &data).unwrap_or_else(|e| panic!("failed to write {path:?}: {e}"));
}

/// Run a ref test from a `.seq` file and compare against the expected `.json`.
/// Returns `true` if the test passed or the expected file was updated.
pub fn run_ref_test(
    seq_path: &Path,
    json_path: &Path,
    rows: u32,
    cols: u32,
    scrollback: u32,
) -> bool {
    let seq_bytes =
        fs::read(seq_path).unwrap_or_else(|e| panic!("failed to read seq {seq_path:?}: {e}"));

    let mut term = GhosttyTerminal::new(rows, cols, scrollback)
        .unwrap_or_else(|e| panic!("failed to create terminal ({rows}x{cols}): {e}"));

    // Use pty_write so that \n in the seq test files is converted to \r\n,
    // matching the expected behavior for text output. Raw VT sequences
    // in the test files do not contain bare \n data bytes.
    term.pty_write(&seq_bytes);
    term.flush();

    let actual = capture_snapshot(&term);

    if env::var("UPDATE_EXPECT").as_deref() == Ok("1") {
        let parent = json_path.parent().unwrap();
        fs::create_dir_all(parent).ok();
        save_snapshot(json_path, &actual);
        eprintln!("UPDATED: {}", json_path.display());
        return true;
    }

    assert!(
        json_path.exists(),
        "expected snapshot {} not found. Run with UPDATE_EXPECT=1 to generate",
        json_path.display()
    );

    let expected = load_expected(json_path);
    let result = diff(&expected, &actual);

    if !result.is_empty() {
        let mut msg = format!("REGRESSION: {}\n", seq_path.display());
        if let Some(d) = &result.dimension_diff {
            msg.push_str(&format!("  dimension: {d}\n"));
        }
        if let Some(d) = &result.cursor_diff {
            msg.push_str(&format!("  cursor: {d}\n"));
        }
        if let Some(d) = &result.scrollback_diff {
            msg.push_str(&format!("  scrollback: {d}\n"));
        }
        let mut sorted: Vec<_> = result.cell_diffs.keys().copied().collect();
        sorted.sort_unstable();
        let max_reported = 20;
        for (idx, (row, col)) in sorted.iter().enumerate() {
            if idx >= max_reported {
                msg.push_str(&format!(
                    "  ... and {} more cell diffs\n",
                    sorted.len() - max_reported
                ));
                break;
            }
            let detail = &result.cell_diffs[&(*row, *col)];
            msg.push_str(&format!("  ({row},{col}): {detail}\n"));
        }
        panic!("{msg}");
    }

    true
}

/// Run all ref tests in a directory. Tests are `.seq` files with matching `.json`.
/// Panics on the first failure.
pub fn run_ref_test_dir(dir: &str, rows: u32, cols: u32, scrollback: u32) {
    let test_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("ref")
        .join(dir);

    assert!(
        test_dir.exists(),
        "test directory {test_dir:?} does not exist"
    );

    let mut entries: Vec<_> = fs::read_dir(&test_dir)
        .unwrap_or_else(|e| panic!("failed to read {test_dir:?}: {e}"))
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("seq") {
            continue;
        }
        let json_path = path.with_extension("json");
        eprintln!(
            "  ref {dir}/{}",
            path.file_stem().unwrap().to_string_lossy()
        );
        run_ref_test(&path, &json_path, rows, cols, scrollback);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ghostty_terminal::GhosttyTerminal;

    fn make_term(rows: u32, cols: u32) -> GhosttyTerminal {
        GhosttyTerminal::new(rows, cols, 1000).expect("term")
    }

    #[test]
    fn capture_empty_snapshot() {
        let term = make_term(24, 80);
        let snap = capture_snapshot(&term);
        assert_eq!(snap.rows, 24);
        assert_eq!(snap.cols, 80);
        assert_eq!(snap.cells.len(), 24 * 80);
        assert!(snap.cursor_visible);
        assert_eq!(snap.scrollback_rows, 0);
    }

    #[test]
    fn capture_with_content() {
        let mut term = make_term(3, 10);
        term.vt_write(b"Hi");
        term.flush();
        let snap = capture_snapshot(&term);
        assert_eq!(snap.cells[0].content, "H");
        assert_eq!(snap.cells[1].content, "i");
        assert!(snap.cells[2].content.is_empty());
        assert_eq!(snap.cursor_col, 2);
        assert_eq!(snap.cursor_row, 0);
    }

    #[test]
    fn diff_identical_is_empty() {
        let term = make_term(3, 5);
        let a = capture_snapshot(&term);
        let b = capture_snapshot(&term);
        let result = diff(&a, &b);
        assert!(result.is_empty());
    }

    #[test]
    fn diff_detects_content_change() {
        let term = make_term(3, 5);
        let snap1 = capture_snapshot(&term);

        let mut snap2 = snap1.clone();
        snap2.cells[0].content = "X".to_string();

        let result = diff(&snap1, &snap2);
        assert!(!result.is_empty());
        assert!(result.cell_diffs.contains_key(&(0, 0)));
    }

    #[test]
    fn diff_detects_cursor_change() {
        let term = make_term(3, 5);
        let snap1 = capture_snapshot(&term);

        let mut snap2 = snap1.clone();
        snap2.cursor_col = 10;

        let result = diff(&snap1, &snap2);
        assert!(result.cursor_diff.is_some());
    }

    #[test]
    fn diff_detects_dimension_mismatch() {
        let mut term = make_term(3, 5);
        term.vt_write(b"test");
        term.flush();
        let snap1 = capture_snapshot(&term);

        let term2 = make_term(4, 5);
        let snap2 = capture_snapshot(&term2);

        let result = diff(&snap1, &snap2);
        assert!(result.dimension_diff.is_some());
    }

    #[test]
    fn serde_round_trip() {
        let term = make_term(3, 10);
        let snap = capture_snapshot(&term);
        let json = serde_json::to_string_pretty(&snap).unwrap();
        let restored: TestSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snap, restored);
    }

    #[test]
    fn serde_with_content_round_trip() {
        let mut term = make_term(3, 10);
        term.vt_write(b"Hello\nWorld");
        term.flush();
        let snap = capture_snapshot(&term);
        let json = serde_json::to_string_pretty(&snap).unwrap();
        let restored: TestSnapshot = serde_json::from_str(&json).unwrap();
        let result = diff(&snap, &restored);
        assert!(result.is_empty(), "{result:?}");
    }

    #[test]
    fn scrollback_captured() {
        let mut term = GhosttyTerminal::new(3, 10, 100).expect("term");
        for i in 0..10u8 {
            term.vt_write(format!("line {i}\n").as_bytes());
        }
        term.flush();
        let snap = capture_snapshot(&term);
        assert!(snap.scrollback_rows > 0);
        assert!(!snap.scrollback.is_empty());
    }
}
