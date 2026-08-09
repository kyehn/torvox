use crate::terminal::ghostty_terminal::types::{CellData, CursorInfo, CursorStyle};

/// CPU-side terminal cell for screenshot generation (no GPU needed).
#[derive(Debug, Clone)]
pub struct CpuCell {
    pub codepoint: char,
    pub fg: [u8; 4],
    pub bg: [u8; 4],
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub reverse_video: bool,
    pub dim: bool,
}

impl Default for CpuCell {
    fn default() -> Self {
        Self {
            codepoint: ' ',
            fg: [255, 255, 255, 255],
            bg: [0, 0, 0, 0],
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            reverse_video: false,
            dim: false,
        }
    }
}

/// CPU-side cursor representation.
#[derive(Debug, Clone)]
pub struct CpuCursor {
    pub row: u32,
    pub col: u32,
    pub visible: bool,
    pub style: CursorStyle,
    pub color: Option<[u8; 4]>,
}

/// CPU-side terminal frame for CI testing — no GPU needed.
///
/// Built from `CellData` slices (shares computation with GPU path
/// but skips vertex/instance encoding). Can generate PNG screenshots
/// using CPU rasterization.
#[derive(Debug, Clone)]
pub struct CpuFrame {
    pub width: u32,
    pub height: u32,
    pub cells: Vec<CpuCell>,
    pub cursor: Option<CpuCursor>,
}

impl CpuFrame {
    /// Build from CellData slice + cursor info.
    pub fn from_cell_data(cells: &[CellData], rows: u32, cols: u32, cursor: &CursorInfo) -> Self {
        let mut frame_cells = Vec::with_capacity((rows * cols) as usize);

        for cd in cells {
            let codepoint = char::from_u32(cd.codepoint).unwrap_or('\0');

            // Check flags (bit positions from CellData.flags)
            const REVERSE_BIT: u32 = 2;
            const BOLD_BIT: u32 = 0;
            const ITALIC_BIT: u32 = 1;
            const UNDERLINE_BIT: u32 = 3;
            const STRIKETHROUGH_BIT: u32 = 4;
            const DIM_BIT: u32 = 5;

            let fg = f32_colors_to_u8(&cd.fg_color);
            let bg = f32_colors_to_u8(&cd.bg_color);

            frame_cells.push(CpuCell {
                codepoint,
                fg,
                bg,
                bold: cd.flags & (1 << BOLD_BIT) != 0,
                italic: cd.flags & (1 << ITALIC_BIT) != 0,
                underline: cd.flags & (1 << UNDERLINE_BIT) != 0,
                strikethrough: cd.flags & (1 << STRIKETHROUGH_BIT) != 0,
                reverse_video: cd.flags & (1 << REVERSE_BIT) != 0,
                dim: cd.flags & (1 << DIM_BIT) != 0,
            });
        }

        let cpu_cursor = CpuCursor {
            row: cursor.row,
            col: cursor.col,
            visible: cursor.visible,
            style: cursor.style,
            color: None,
        };

        Self {
            width: cols,
            height: rows,
            cells: frame_cells,
            cursor: if cursor.visible {
                Some(cpu_cursor)
            } else {
                None
            },
        }
    }

    /// Get cell at (row, col) or None if out of bounds.
    pub fn cell_at(&self, row: u32, col: u32) -> Option<&CpuCell> {
        if row >= self.height || col >= self.width {
            return None;
        }
        let idx = (row * self.width + col) as usize;
        self.cells.get(idx)
    }

    /// Get the display text for a specific row (non-space characters).
    pub fn row_text(&self, row: u32) -> String {
        if row >= self.height {
            return String::new();
        }
        let start = (row * self.width) as usize;
        let end = start + self.width as usize;
        let row_cells = &self.cells[start..end];

        let text: String = row_cells
            .iter()
            .map(|c| {
                if c.codepoint == '\0' || c.codepoint == ' ' {
                    ' '
                } else {
                    c.codepoint
                }
            })
            .collect();

        // Trim trailing whitespace
        text.trim_end().to_string()
    }

    /// Get all visible text as lines.
    pub fn text_lines(&self) -> Vec<String> {
        (0..self.height).map(|r| self.row_text(r)).collect()
    }

    /// Search for text content in the frame.
    pub fn find_text(&self, needle: &str) -> Vec<TextHit> {
        let mut hits = Vec::new();
        for row in 0..self.height {
            let line = self.row_text(row);
            let mut start = 0;
            while let Some(pos) = line[start..].find(needle) {
                let absolute = start + pos;
                hits.push(TextHit {
                    row,
                    col: absolute as u32,
                    text: needle.to_string(),
                });
                start = absolute + 1;
            }
        }
        hits
    }

    /// Extract text items with position information for LiveTest assertions.
    pub fn extract_text_items(&self) -> Vec<TextItem> {
        let mut items = Vec::new();
        for row in 0..self.height {
            let line = self.row_text(row);
            if !line.is_empty() {
                items.push(TextItem {
                    text: line,
                    row,
                    col: 0,
                    width: self.width,
                    height: 1,
                });
            }
        }
        items
    }
}

/// A text search hit in the frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextHit {
    pub row: u32,
    pub col: u32,
    pub text: String,
}

/// A text item with position for LiveTest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextItem {
    pub text: String,
    pub row: u32,
    pub col: u32,
    pub width: u32,
    pub height: u32,
}

/// Convert f32 RGBA colors (0.0–1.0) to u8 (0–255).
fn f32_colors_to_u8(colors: &[f32; 4]) -> [u8; 4] {
    [
        (colors[0].clamp(0.0, 1.0) * 255.0) as u8,
        (colors[1].clamp(0.0, 1.0) * 255.0) as u8,
        (colors[2].clamp(0.0, 1.0) * 255.0) as u8,
        (colors[3].clamp(0.0, 1.0) * 255.0) as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::ghostty_terminal::types::{CellData, CursorInfo, CursorStyle};

    fn make_cell(codepoint: u32, row: u32, col: u32) -> CellData {
        CellData {
            codepoint,
            width: 1,
            grapheme_extra: [0; 7],
            fg_color: [1.0, 1.0, 1.0, 1.0],
            bg_color: [0.0, 0.0, 0.0, 1.0],
            flags: 0,
            row,
            col,
        }
    }

    fn make_cursor(row: u32, col: u32) -> CursorInfo {
        CursorInfo {
            row,
            col,
            visible: true,
            style: CursorStyle::Block,
        }
    }

    #[test]
    fn frame_basic_construction() {
        // 3x2 grid: "Hi\n  "
        let cells = vec![
            make_cell('H' as u32, 0, 0),
            make_cell('i' as u32, 0, 1),
            make_cell(' ' as u32, 0, 2),
            make_cell(' ' as u32, 1, 0),
            make_cell(' ' as u32, 1, 1),
            make_cell(' ' as u32, 1, 2),
        ];
        let cursor = make_cursor(0, 0);
        let frame = CpuFrame::from_cell_data(&cells, 2, 3, &cursor);

        assert_eq!(frame.width, 3);
        assert_eq!(frame.height, 2);
        assert_eq!(frame.cells.len(), 6);
        assert_eq!(frame.row_text(0), "Hi");
        assert_eq!(frame.row_text(1), "");
    }

    #[test]
    fn cell_at_bounds() {
        let cells = vec![make_cell('A' as u32, 0, 0)];
        let cursor = make_cursor(0, 0);
        let frame = CpuFrame::from_cell_data(&cells, 1, 1, &cursor);

        assert!(frame.cell_at(0, 0).is_some());
        assert!(frame.cell_at(0, 1).is_none());
        assert!(frame.cell_at(1, 0).is_none());
    }

    #[test]
    fn find_text_finds_matches() {
        let cells: Vec<CellData> = "Hello "
            .chars()
            .enumerate()
            .map(|(i, c)| make_cell(c as u32, 0, i as u32))
            .collect();
        let cursor = make_cursor(0, 0);
        let frame = CpuFrame::from_cell_data(&cells, 1, 6, &cursor);

        let hits = frame.find_text("ell");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].col, 1);
    }

    #[test]
    fn find_text_multiple_matches() {
        let cells: Vec<CellData> = "ababa"
            .chars()
            .enumerate()
            .map(|(i, c)| make_cell(c as u32, 0, i as u32))
            .collect();
        let cursor = make_cursor(0, 0);
        let frame = CpuFrame::from_cell_data(&cells, 1, 5, &cursor);

        let hits = frame.find_text("aba");
        assert_eq!(hits.len(), 2); // "aba" at 0 and "aba" at 2
    }

    #[test]
    fn text_lines_collects_all_rows() {
        let cells: Vec<CellData> = "ABC"
            .chars()
            .chain(std::iter::repeat(' ').take(3))
            .chain("DEF".chars())
            .chain(std::iter::repeat(' ').take(3))
            .enumerate()
            .map(|(i, c)| make_cell(c as u32, (i / 3) as u32, (i % 3) as u32))
            .collect();
        let cursor = make_cursor(0, 0);
        let frame = CpuFrame::from_cell_data(&cells, 2, 3, &cursor);

        let lines = frame.text_lines();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "ABC");
        assert_eq!(lines[1], "DEF");
    }

    #[test]
    fn extract_text_items() {
        let cells: Vec<CellData> = "Hi"
            .chars()
            .chain(std::iter::repeat(' ').take(2))
            .chain("By".chars())
            .chain(std::iter::repeat(' ').take(2))
            .enumerate()
            .map(|(i, c)| make_cell(c as u32, (i / 2) as u32, (i % 2) as u32))
            .collect();
        let cursor = make_cursor(0, 0);
        let frame = CpuFrame::from_cell_data(&cells, 2, 2, &cursor);

        let items = frame.extract_text_items();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].text, "Hi");
        assert_eq!(items[1].text, "By");
    }

    #[test]
    fn reverse_video_swaps_fg_bg() {
        let mut cell = make_cell('X' as u32, 0, 0);
        cell.flags = 1 << 2; // REVERSE_BIT
        let cells = vec![cell];
        let cursor = make_cursor(0, 0);
        let frame = CpuFrame::from_cell_data(&cells, 1, 1, &cursor);

        let cpu_cell = frame.cell_at(0, 0).unwrap();
        assert!(cpu_cell.reverse_video);
        // fg should be white (1,1,1,1), bg should be black (0,0,0,1)
        assert_eq!(cpu_cell.fg, [255, 255, 255, 255]);
        assert_eq!(cpu_cell.bg, [0, 0, 0, 255]);
    }

    #[test]
    fn cursor_visibility() {
        let cells = vec![make_cell(' ' as u32, 0, 0)];

        let mut cursor = make_cursor(0, 0);
        cursor.visible = true;
        let frame = CpuFrame::from_cell_data(&cells, 1, 1, &cursor);
        assert!(frame.cursor.is_some());

        let mut cursor2 = make_cursor(0, 0);
        cursor2.visible = false;
        let frame2 = CpuFrame::from_cell_data(&cells, 1, 1, &cursor2);
        assert!(frame2.cursor.is_none());
    }

    #[test]
    fn f32_to_u8_color_conversion() {
        assert_eq!(
            f32_colors_to_u8(&[1.0, 0.5, 0.0, 1.0]),
            [255, 128, 0, 255]
        );
        assert_eq!(f32_colors_to_u8(&[0.0, 0.0, 0.0, 0.0]), [0, 0, 0, 0]);
        assert_eq!(
            f32_colors_to_u8(&[0.8, 0.8, 0.8, 1.0]),
            [204, 204, 204, 255]
        );
    }

    #[test]
    fn flags_bold_italic_underline() {
        let mut cell = make_cell('T' as u32, 0, 0);
        cell.flags = (1 << 0) | (1 << 1) | (1 << 3); // bold | italic | underline
        let cells = vec![cell];
        let cursor = make_cursor(0, 0);
        let frame = CpuFrame::from_cell_data(&cells, 1, 1, &cursor);

        let cpu_cell = frame.cell_at(0, 0).unwrap();
        assert!(cpu_cell.bold);
        assert!(cpu_cell.italic);
        assert!(cpu_cell.underline);
        assert!(!cpu_cell.strikethrough);
    }
}
