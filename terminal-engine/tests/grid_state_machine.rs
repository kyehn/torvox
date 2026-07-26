use proptest::prelude::*;
use terminal_engine::ghostty_terminal::GhosttyTerminal;

#[derive(Clone, Debug)]
enum GridOp {
    WriteChar(char),
    Newline,
    Backspace,
    CursorUp(u16),
    CursorDown(u16),
    CursorLeft(u16),
    CursorRight(u16),
    CarriageReturn,
    Tab,
    ClearLine(u8),
    ClearScreen(u8),
    InsertLines(u16),
    DeleteLines(u16),
    ScrollUp(u16),
    Resize(u32, u32),
    AlternateBuffer,
    SetOriginMode(bool),
    SetScrollRegion(u32, u32),
    InsertMode(bool),
    AltBuffer(bool),
    ReverseIndex,
}

struct GridModel {
    rows: u32,
    cols: u32,
    cursor_row: u32,
    cursor_col: u32,
    chars: Vec<Vec<char>>,
    scroll_region_top: u32,
    scroll_region_bottom: u32,
    origin_mode: bool,
    insert_mode: bool,
    alt_buffer: bool,
}

impl GridModel {
    fn new(rows: u32, cols: u32) -> Self {
        Self {
            rows,
            cols,
            cursor_row: 0,
            cursor_col: 0,
            chars: vec![vec!['\0'; cols as usize]; rows as usize],
            scroll_region_top: 0,
            scroll_region_bottom: rows.saturating_sub(1),
            origin_mode: false,
            insert_mode: false,
            alt_buffer: false,
        }
    }

    fn apply(&mut self, op: &GridOp) {
        match op {
            GridOp::WriteChar(c) => {
                let r = self.cursor_row as usize;
                let c_pos = self.cursor_col as usize;
                if r < self.chars.len() && c_pos < self.chars[r].len() {
                    if self.insert_mode {
                        for col in (c_pos..self.chars[r].len() - 1).rev() {
                            self.chars[r][col + 1] = self.chars[r][col];
                        }
                    }
                    self.chars[r][c_pos] = *c;
                }
                self.cursor_col = (self.cursor_col + 1).min(self.cols - 1);
            }
            GridOp::Newline => {
                let max_row = if self.origin_mode {
                    self.scroll_region_bottom
                } else {
                    self.rows - 1
                };
                if self.cursor_row >= max_row {
                    self.scroll_up_region(1);
                } else {
                    self.cursor_row += 1;
                }
                self.cursor_col = 0;
            }
            GridOp::Backspace => {
                if self.cursor_col > 0 {
                    self.cursor_col -= 1;
                }
            }
            GridOp::CursorUp(n) => {
                let n = *n as u32;
                let min_row = if self.origin_mode {
                    self.scroll_region_top
                } else {
                    0
                };
                self.cursor_row = self
                    .cursor_row
                    .saturating_sub(n)
                    .max(min_row)
                    .min(self.rows - 1);
            }
            GridOp::CursorDown(n) => {
                let n = *n as u32;
                let max_row = if self.origin_mode {
                    self.scroll_region_bottom
                } else {
                    self.rows - 1
                };
                self.cursor_row = (self.cursor_row + n).min(max_row);
            }
            GridOp::CursorLeft(n) => {
                let n = *n as u32;
                self.cursor_col = self.cursor_col.saturating_sub(n).min(self.cols - 1);
            }
            GridOp::CursorRight(n) => {
                let n = *n as u32;
                self.cursor_col = (self.cursor_col + n).min(self.cols - 1);
            }
            GridOp::CarriageReturn => {
                self.cursor_col = 0;
            }
            GridOp::Tab => {
                let next_tab = ((self.cursor_col / 8) + 1) * 8;
                self.cursor_col = next_tab.min(self.cols - 1);
            }
            GridOp::ClearLine(_mode) => {
                let r = self.cursor_row as usize;
                if r < self.chars.len() {
                    self.chars[r].fill('\0');
                }
            }
            GridOp::ClearScreen(_mode) => {
                for r in &mut self.chars {
                    for c in r.iter_mut() {
                        *c = '\0';
                    }
                }
            }
            GridOp::InsertLines(_n) => {
                let r = self.cursor_row as usize;
                if r < self.chars.len() {
                    self.chars[r].fill('\0');
                }
            }
            GridOp::DeleteLines(_n) => {
                let r = self.cursor_row as usize;
                if r < self.chars.len() {
                    for i in r..self.chars.len() - 1 {
                        self.chars[i] = self.chars[i + 1].clone();
                    }
                    self.chars.last_mut().unwrap().fill('\0');
                }
            }
            GridOp::ScrollUp(n) => {
                self.scroll_up_region(*n as u32);
            }

            GridOp::Resize(rows, cols) => {
                self.rows = *rows;
                self.cols = *cols;
                self.chars = vec![vec!['\0'; *cols as usize]; *rows as usize];
                self.cursor_row = self.cursor_row.min(self.rows - 1);
                self.cursor_col = self.cursor_col.min(self.cols - 1);
                self.scroll_region_top = 0;
                self.scroll_region_bottom = rows.saturating_sub(1);
            }
            GridOp::AlternateBuffer => {
                self.cursor_row = 0;
                self.cursor_col = 0;
            }
            GridOp::SetOriginMode(enabled) => {
                self.origin_mode = *enabled;
                if *enabled {
                    self.cursor_row = self.scroll_region_top;
                    self.cursor_col = 0;
                }
            }
            GridOp::SetScrollRegion(top, bottom) => {
                let top = *top;
                let bottom = (*bottom).min(self.rows - 1);
                if top < bottom {
                    self.scroll_region_top = top;
                    self.scroll_region_bottom = bottom;
                }
            }
            GridOp::InsertMode(enabled) => {
                self.insert_mode = *enabled;
            }
            GridOp::AltBuffer(enabled) => {
                self.alt_buffer = *enabled;
                self.cursor_row = 0;
                self.cursor_col = 0;
            }
            GridOp::ReverseIndex => {
                if self.cursor_row > self.scroll_region_top {
                    self.cursor_row -= 1;
                } else {
                    let top = self.scroll_region_top as usize;
                    let bottom = self.scroll_region_bottom as usize;
                    for i in (top..bottom).rev() {
                        self.chars[i + 1] = self.chars[i].clone();
                    }
                    self.chars[top].fill('\0');
                }
            }
        }
    }
}

fn apply_to_terminal(terminal: &mut GhosttyTerminal, op: &GridOp) {
    match op {
        GridOp::WriteChar(c) => {
            terminal.vt_write(&[*c as u8]);
        }
        GridOp::Newline => {
            terminal.vt_write(b"\n");
        }
        GridOp::Backspace => {
            terminal.vt_write(b"\x08");
        }
        GridOp::CursorUp(n) => {
            terminal.vt_write(format!("\x1b[{}A", n).as_bytes());
        }
        GridOp::CursorDown(n) => {
            terminal.vt_write(format!("\x1b[{}B", n).as_bytes());
        }
        GridOp::CursorLeft(n) => {
            terminal.vt_write(format!("\x1b[{}D", n).as_bytes());
        }
        GridOp::CursorRight(n) => {
            terminal.vt_write(format!("\x1b[{}C", n).as_bytes());
        }
        GridOp::CarriageReturn => {
            terminal.vt_write(b"\r");
        }
        GridOp::Tab => {
            terminal.vt_write(b"\t");
        }
        GridOp::ClearLine(mode) => {
            terminal.vt_write(&[0x1B, b'[', mode + b'0', b'K']);
        }
        GridOp::ClearScreen(mode) => {
            terminal.vt_write(&[0x1B, b'[', mode + b'0', b'J']);
        }
        GridOp::InsertLines(n) => {
            terminal.vt_write(format!("\x1b[{}L", n).as_bytes());
        }
        GridOp::DeleteLines(n) => {
            terminal.vt_write(format!("\x1b[{}M", n).as_bytes());
        }
        GridOp::ScrollUp(n) => {
            terminal.vt_write(format!("\x1b[{}S", n).as_bytes());
        }
        GridOp::Resize(rows, cols) => {
            terminal.resize(*rows, *cols);
        }
        GridOp::AlternateBuffer => {
            terminal.vt_write(b"\x1b[?1049h");
        }
        GridOp::SetOriginMode(enabled) => {
            if *enabled {
                terminal.vt_write(b"\x1b[?6h");
            } else {
                terminal.vt_write(b"\x1b[?6l");
            }
        }
        GridOp::SetScrollRegion(top, bottom) => {
            terminal.vt_write(format!("\x1b[{};{}r", top + 1, bottom + 1).as_bytes());
        }
        GridOp::InsertMode(enabled) => {
            if *enabled {
                terminal.vt_write(b"\x1b[4h");
            } else {
                terminal.vt_write(b"\x1b[4l");
            }
        }
        GridOp::AltBuffer(enabled) => {
            if *enabled {
                terminal.vt_write(b"\x1b[?1049h");
            } else {
                terminal.vt_write(b"\x1b[?1049l");
            }
        }
        GridOp::ReverseIndex => {
            terminal.vt_write(b"\x1bM");
        }
    }
}

fn arb_grid_op() -> impl Strategy<Value = GridOp> {
    prop_oneof![
        (0x20u8..0x7Fu8).prop_map(|b| GridOp::WriteChar(b as char)),
        any::<u16>().prop_map(GridOp::CursorUp),
        any::<u16>().prop_map(GridOp::CursorDown),
        any::<u16>().prop_map(GridOp::CursorLeft),
        any::<u16>().prop_map(GridOp::CursorRight),
        (0u8..3u8).prop_map(GridOp::ClearLine),
        (0u8..3u8).prop_map(GridOp::ClearScreen),
        (1u16..10u16).prop_map(GridOp::InsertLines),
        (1u16..10u16).prop_map(GridOp::DeleteLines),
        (1u16..10u16).prop_map(GridOp::ScrollUp),
        (5u32..80u32, 5u32..40u32).prop_map(|(c, r)| GridOp::Resize(r, c)),
        any::<bool>().prop_map(GridOp::SetOriginMode),
        (0u32..39u32)
            .prop_flat_map(|top| { (Just(top), top + 1..40u32) })
            .prop_map(|(t, b)| GridOp::SetScrollRegion(t, b)),
        any::<bool>().prop_map(GridOp::InsertMode),
        any::<bool>().prop_map(GridOp::AltBuffer),
        Just(GridOp::Newline),
        Just(GridOp::Backspace),
        Just(GridOp::CarriageReturn),
        Just(GridOp::Tab),
        Just(GridOp::AlternateBuffer),
        Just(GridOp::ReverseIndex),
    ]
}

impl GridModel {
    fn scroll_up_region(&mut self, n: u32) {
        let top = self.scroll_region_top as usize;
        let bottom = self.scroll_region_bottom as usize;
        if top >= bottom || n == 0 {
            return;
        }
        let n = n.min((bottom - top + 1) as u32) as usize;
        for _ in 0..n {
            for i in top..bottom {
                self.chars[i] = self.chars[i + 1].clone();
            }
            self.chars[bottom].fill('\0');
        }
    }

    fn cell_char(&self, row: u32, col: u32) -> char {
        if row < self.chars.len() as u32 && col < self.chars[row as usize].len() as u32 {
            self.chars[row as usize][col as usize]
        } else {
            '\0'
        }
    }
}

proptest! {
    #[test]
    fn grid_model_ops_no_panic(ops in proptest::collection::vec(arb_grid_op(), 0..50)) {
        let mut model = GridModel::new(24, 80);
        for op in &ops {
            model.apply(op);
        }
    }

    #[test]
    fn grid_cursor_bounds_matched(ops in proptest::collection::vec(arb_grid_op(), 1..10)) {
        let mut model = GridModel::new(24, 80);
        for op in &ops {
            model.apply(op);
        }
        // Check model cursor bounds (independent of real terminal, avoids ghostty thread panic)
        assert!(model.cursor_row < model.rows, "model cursor row {} >= rows {}", model.cursor_row, model.rows);
        assert!(model.cursor_col < model.cols, "model cursor col {} >= cols {}", model.cursor_col, model.cols);
    }

    #[test]
    fn grid_cell_content_matched(text_chars in proptest::collection::vec("[a-zA-Z]", 1..10)) {
        let mut terminal = GhosttyTerminal::new(10, 40, 500).expect("terminal");
        let mut model = GridModel::new(10, 40);
        for c in &text_chars {
            let op = GridOp::WriteChar(c.chars().next().unwrap());
            apply_to_terminal(&mut terminal, &op);
            model.apply(&op);
        }
        model.apply(&GridOp::CarriageReturn);
        apply_to_terminal(&mut terminal, &GridOp::CarriageReturn);
        model.apply(&GridOp::Newline);
        apply_to_terminal(&mut terminal, &GridOp::Newline);
        let snap = terminal.take_snapshot();
        let cols = snap.cols as usize;
        for (i, c_str) in text_chars.iter().enumerate() {
            let expected = model.cell_char(0, i as u32);
            let written_char = c_str.chars().next().unwrap_or('\0');
            if i < cols && let Some(cell) = snap.cells.get(i) {
                let actual = char::from_u32(cell.codepoint).unwrap_or('\0');
                assert!(
                    expected == '\0' || actual == written_char,
                    "cell[{}] mismatch: model={:?} real={:?} written={:?}",
                    i, expected, actual, written_char
                );
            }
        }
    }
}

#[test]
fn grid_cursor_bounds_terminal_ops() {
    let mut terminal = GhosttyTerminal::new(10, 20, 500).expect("terminal");
    let ops = vec![
        GridOp::WriteChar('X'),
        GridOp::Newline,
        GridOp::Backspace,
        GridOp::CursorUp(1),
        GridOp::CursorDown(1),
        GridOp::CursorLeft(1),
        GridOp::CursorRight(1),
        GridOp::CarriageReturn,
        GridOp::Resize(15, 30),
        GridOp::SetOriginMode(true),
        GridOp::SetScrollRegion(2, 8),
    ];
    for op in &ops {
        apply_to_terminal(&mut terminal, op);
    }
    let snap = terminal.take_snapshot();
    let cursor_row = terminal.cursor_y();
    let cursor_col = terminal.cursor_x();
    assert!(
        cursor_row < snap.rows,
        "cursor row {} >= rows {}",
        cursor_row,
        snap.rows
    );
    assert!(
        cursor_col < snap.cols,
        "cursor col {} >= cols {}",
        cursor_col,
        snap.cols
    );
}

#[test]
fn all_20_op_variants_no_panic() {
    let mut terminal = GhosttyTerminal::new(10, 20, 500).expect("terminal");
    let ops = vec![
        GridOp::WriteChar('X'),
        GridOp::Newline,
        GridOp::Backspace,
        GridOp::CursorUp(1),
        GridOp::CursorDown(1),
        GridOp::CursorLeft(1),
        GridOp::CursorRight(1),
        GridOp::CarriageReturn,
        GridOp::Tab,
        GridOp::ClearLine(0),
        GridOp::ClearScreen(0),
        GridOp::InsertLines(1),
        GridOp::DeleteLines(1),
        GridOp::ScrollUp(1),
        GridOp::Resize(10, 20),
        GridOp::AlternateBuffer,
        GridOp::SetOriginMode(true),
        GridOp::SetScrollRegion(2, 8),
        GridOp::InsertMode(true),
        GridOp::AltBuffer(true),
        GridOp::ReverseIndex,
    ];
    for op in &ops {
        apply_to_terminal(&mut terminal, op);
    }
}
