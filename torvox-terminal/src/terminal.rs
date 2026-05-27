extern crate alloc;

use alloc::collections::BTreeSet;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use thiserror::Error;
use torvox_core::ansi::ansi_to_rgb;
use torvox_core::cell::{Attrs, Cell, Color};
use torvox_core::cursor::CursorState;
use torvox_core::grid::Grid;

const DEFAULT_TAB_STOP_INTERVAL: u32 = 8;
const MAX_TAB_STOPS: u32 = 512;

const C0_HT: u8 = 0x09;
const C0_BS: u8 = 0x08;
const C0_CR: u8 = 0x0D;
const C0_LF: u8 = 0x0A;
const C0_VT: u8 = 0x0B;
const C0_FF: u8 = 0x0C;
const C0_BEL: u8 = 0x07;
const C0_SO: u8 = 0x0E;
const C0_SI: u8 = 0x0F;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Charset {
    #[default]
    Ascii,
    SpecialGraphics,
}

#[derive(Debug, Error)]
pub enum TerminalStateError {
    #[error("terminal init failed: {0}")]
    Init(String),
}

#[derive(Debug, Clone)]
pub struct TerminalState {
    pub grid: Grid,
    pub cursor: CursorState,
    saved_cursor: Option<CursorState>,
    scroll_top: u32,
    scroll_bottom: u32,
    current_attrs: Attrs,
    fg_color: Color,
    bg_color: Color,
    tab_stops: Vec<bool>,
    modes: BTreeSet<u16>,
    charsets: [Charset; 4],
    active_charset: usize,
    origin_mode: bool,
    wrap_around: bool,
    insert_mode: bool,
    alt_grid: Option<Grid>,
    title: Option<String>,
}

impl TerminalState {
    pub fn new(rows: u32, cols: u32) -> Result<Self, TerminalStateError> {
        let mut tab_stops = vec![false; MAX_TAB_STOPS as usize];
        let mut i = DEFAULT_TAB_STOP_INTERVAL;
        while i < MAX_TAB_STOPS {
            tab_stops[i as usize] = true;
            i += DEFAULT_TAB_STOP_INTERVAL;
        }
        Ok(Self {
            grid: Grid::new(rows, cols),
            cursor: CursorState::default(),
            saved_cursor: None,
            scroll_top: 0,
            scroll_bottom: rows,
            current_attrs: Attrs::default(),
            fg_color: Color::default(),
            bg_color: Color::default(),
            tab_stops,
            modes: BTreeSet::new(),
            charsets: [Charset::default(); 4],
            active_charset: 0,
            origin_mode: false,
            wrap_around: true,
            insert_mode: false,
            alt_grid: None,
            title: None,
        })
    }

    pub fn rows(&self) -> u32 {
        self.grid.rows()
    }

    pub fn cols(&self) -> u32 {
        self.grid.cols()
    }

    pub fn resize(&mut self, rows: u32, cols: u32) {
        self.grid.resize(rows, cols);
        self.scroll_bottom = rows;
        self.clamp_cursor();
    }

    fn clamp_cursor(&mut self) {
        let max_row = self.grid.rows().saturating_sub(1);
        let max_col = self.grid.cols().saturating_sub(1);
        if self.cursor.row > max_row {
            self.cursor.row = max_row;
        }
        if self.cursor.col > max_col {
            self.cursor.col = max_col;
        }
    }

    fn effective_row(&self) -> u32 {
        if self.origin_mode {
            self.cursor.row + self.scroll_top
        } else {
            self.cursor.row
        }
    }

    fn advance_line(&mut self) {
        let row = self.effective_row();
        if row >= self.scroll_bottom - 1 {
            self.grid
                .scroll_up(self.scroll_top, self.scroll_bottom, self.grid.cols());
        } else {
            self.cursor.row += 1;
        }
    }

    fn next_tab_stop(&self, col: u32) -> u32 {
        let mut i = col + 1;
        while i < MAX_TAB_STOPS && i < self.grid.cols() {
            if self.tab_stops[i as usize] {
                return i;
            }
            i += 1;
        }
        self.grid.cols().saturating_sub(1)
    }

    fn prev_tab_stop(&self, col: u32) -> u32 {
        if col == 0 {
            return 0;
        }
        let mut i = col.saturating_sub(1);
        loop {
            if self.tab_stops[i as usize] {
                return i;
            }
            if i == 0 {
                break;
            }
            i -= 1;
        }
        0
    }

    fn save_cursor_position(&mut self) {
        self.saved_cursor = Some(self.cursor);
    }

    fn restore_cursor_position(&mut self) {
        if let Some(saved) = self.saved_cursor {
            self.cursor = saved;
            self.clamp_cursor();
        }
    }

    fn clear_screen(&mut self, mode: u32) {
        let cols = self.grid.cols();
        let rows = self.grid.rows();
        match mode {
            0 => {
                let row = self.effective_row();
                let col = self.cursor.col;
                self.grid.clear_cells(row, col, cols);
                for r in (row + 1)..rows {
                    self.grid.clear_cells(r, 0, cols);
                }
            }
            1 => {
                let row = self.effective_row();
                for r in 0..row {
                    self.grid.clear_cells(r, 0, cols);
                }
                self.grid.clear_cells(row, 0, self.cursor.col + 1);
            }
            2 | 3 => {
                for r in 0..rows {
                    self.grid.clear_cells(r, 0, cols);
                }
            }
            _ => {}
        }
    }

    fn clear_line(&mut self, mode: u32) {
        let cols = self.grid.cols();
        let row = self.effective_row();
        let col = self.cursor.col;
        match mode {
            0 => self.grid.clear_cells(row, col, cols),
            1 => self.grid.clear_cells(row, 0, col + 1),
            2 => self.grid.clear_cells(row, 0, cols),
            _ => {}
        }
    }

    fn erase_chars(&mut self, count: u32) {
        let row = self.effective_row();
        let col = self.cursor.col;
        let end = (col + count).min(self.grid.cols());
        self.grid.clear_cells(row, col, end);
    }

    fn insert_blank_chars(&mut self, count: u32) {
        let row = self.effective_row();
        let col = self.cursor.col;
        let cols = self.grid.cols();
        if count == 0 || col >= cols {
            return;
        }
        let Some(line) = self.grid.get_mut(row) else {
            return;
        };
        let actual = count.min(cols - col);
        // Shift cells right by `actual` positions, in-place from right to left
        let mut i = cols;
        while i > col + actual {
            i -= 1;
            if let (Some(src), Some(dst)) = (line.get(i - actual).copied(), line.get_mut(i)) {
                *dst = src;
            }
        }
        // Fill blank area
        for c in col..col + actual {
            if let Some(cell) = line.get_mut(c) {
                *cell = Cell::default();
            }
        }
        self.grid.mark_row_dirty(row);
    }

    fn delete_chars(&mut self, count: u32) {
        let row = self.effective_row();
        let col = self.cursor.col;
        let cols = self.grid.cols();
        if count == 0 || col >= cols {
            return;
        }
        let Some(line) = self.grid.get_mut(row) else {
            return;
        };
        let actual = count.min(cols - col);
        // Shift cells left by `actual` positions, in-place from left to right
        for c in col..cols - actual {
            if let (Some(src), Some(dst)) = (line.get(c + actual).copied(), line.get_mut(c)) {
                *dst = src;
            }
        }
        // Fill blank area at end
        for c in cols - actual..cols {
            if let Some(cell) = line.get_mut(c) {
                *cell = Cell::default();
            }
        }
        self.grid.mark_row_dirty(row);
    }

    fn insert_lines(&mut self, count: u32) {
        let row = self.effective_row();
        let cols = self.grid.cols();
        self.grid.insert_lines(row, count, self.scroll_bottom, cols);
    }

    fn delete_lines(&mut self, count: u32) {
        let row = self.effective_row();
        let cols = self.grid.cols();
        self.grid.delete_lines(row, count, self.scroll_bottom, cols);
    }

    fn scroll_up(&mut self, count: u32) {
        let cols = self.grid.cols();
        for _ in 0..count {
            self.grid
                .scroll_up(self.scroll_top, self.scroll_bottom, cols);
        }
    }

    fn scroll_down(&mut self, count: u32) {
        let cols = self.grid.cols();
        for _ in 0..count {
            self.grid
                .scroll_down(self.scroll_top, self.scroll_bottom, cols);
        }
    }

    fn set_sgr(&mut self, params: &[u16]) {
        if params.is_empty() {
            self.current_attrs = Attrs::default();
            self.fg_color = Color::default();
            self.bg_color = Color::default();
            return;
        }
        let mut i = 0;
        while i < params.len() {
            match params[i] {
                0 => {
                    self.current_attrs = Attrs::default();
                    self.fg_color = Color::default();
                    self.bg_color = Color::default();
                }
                1 => self.current_attrs.bold = true,
                2 => self.current_attrs.dim = true,
                3 => self.current_attrs.italic = true,
                4 => self.current_attrs.underline = true,
                5 => self.current_attrs.blink = true,
                7 => self.current_attrs.reverse = true,
                8 => self.current_attrs.hidden = true,
                9 => self.current_attrs.strikethrough = true,
                21 => self.current_attrs.double_underline = true,
                22 => {
                    self.current_attrs.bold = false;
                    self.current_attrs.dim = false;
                }
                23 => self.current_attrs.italic = false,
                24 => self.current_attrs.underline = false,
                25 => self.current_attrs.blink = false,
                27 => self.current_attrs.reverse = false,
                28 => self.current_attrs.hidden = false,
                29 => self.current_attrs.strikethrough = false,
                30..=37 => {
                    let idx = (params[i] - 30) as u8;
                    let rgb = ansi_to_rgb(idx);
                    self.fg_color = Color::new(rgb[0], rgb[1], rgb[2]);
                }
                38 => {
                    if let Some(color) = self.parse_sgr_color(params, &mut i) {
                        self.fg_color = color;
                    }
                }
                39 => self.fg_color = Color::default(),
                40..=47 => {
                    let idx = (params[i] - 40) as u8;
                    let rgb = ansi_to_rgb(idx);
                    self.bg_color = Color::new(rgb[0], rgb[1], rgb[2]);
                }
                48 => {
                    if let Some(color) = self.parse_sgr_color(params, &mut i) {
                        self.bg_color = color;
                    }
                }
                49 => self.bg_color = Color::default(),
                53 => self.current_attrs.overline = true,
                55 => self.current_attrs.overline = false,
                90..=97 => {
                    let idx = (params[i] - 90 + 8) as u8;
                    let rgb = ansi_to_rgb(idx);
                    self.fg_color = Color::new(rgb[0], rgb[1], rgb[2]);
                }
                100..=107 => {
                    let idx = (params[i] - 100 + 8) as u8;
                    let rgb = ansi_to_rgb(idx);
                    self.bg_color = Color::new(rgb[0], rgb[1], rgb[2]);
                }
                _ => {}
            }
            i += 1;
        }
    }

    fn parse_sgr_color(&self, params: &[u16], i: &mut usize) -> Option<Color> {
        let next = params.get(*i + 1).copied().unwrap_or(0);
        match next {
            5 => {
                *i += 2;
                let idx = params.get(*i).copied().unwrap_or(0) as u8;
                let rgb = ansi_to_rgb(idx);
                Some(Color::new(rgb[0], rgb[1], rgb[2]))
            }
            2 => {
                *i += 2;
                let r = params.get(*i).copied().unwrap_or(0) as u8;
                *i += 1;
                let g = params.get(*i).copied().unwrap_or(0) as u8;
                *i += 1;
                let b = params.get(*i).copied().unwrap_or(0) as u8;
                Some(Color::new(r, g, b))
            }
            _ => None,
        }
    }

    fn set_scrolling_region(&mut self, top: u32, bottom: u32) {
        let rows = self.grid.rows();
        let top = if top == 0 { 0 } else { (top - 1).min(rows - 1) };
        let bottom = if bottom == 0 { rows } else { bottom.min(rows) };
        if top < bottom {
            self.scroll_top = top;
            self.scroll_bottom = bottom;
            self.cursor.move_to(0, 0);
        }
    }

    fn dec_set_mode(&mut self, mode: u32) {
        match mode {
            1 => {}
            3 => {}
            5 => {}
            6 => self.origin_mode = true,
            7 => self.wrap_around = true,
            12 => {}
            25 => self.cursor.visible = true,
            1049 => {
                let cols = self.grid.cols();
                let rows = self.grid.rows();
                self.alt_grid = Some(core::mem::replace(&mut self.grid, Grid::new(rows, cols)));
                self.save_cursor_position();
                for r in 0..rows {
                    self.grid.clear_cells(r, 0, cols);
                }
                self.grid.mark_all_dirty();
                self.cursor.move_to(0, 0);
            }
            2004 => {}
            2026 => {}
            _ => {}
        }
        self.modes.insert(mode as u16);
    }

    fn dec_reset_mode(&mut self, mode: u32) {
        match mode {
            1 => {}
            3 => {}
            5 => {}
            6 => self.origin_mode = false,
            7 => self.wrap_around = false,
            12 => {}
            25 => self.cursor.visible = false,
            1049 => {
                if let Some(alt) = self.alt_grid.take() {
                    self.grid = alt;
                    self.grid.mark_all_dirty();
                    self.restore_cursor_position();
                }
            }
            2004 => {}
            2026 => {}
            _ => {}
        }
        self.modes.remove(&(mode as u16));
    }
}

impl vte::Perform for TerminalState {
    fn print(&mut self, c: char) {
        let row = self.effective_row();
        let cols = self.grid.cols();

        if self.insert_mode {
            self.insert_blank_chars(1);
        }

        if let Some(cell) = self.grid.cell_mut(row, self.cursor.col) {
            cell.char = c;
            cell.fg = self.fg_color;
            cell.bg = self.bg_color;
            cell.attrs = self.current_attrs;
        }

        if self.cursor.col + 1 >= cols {
            if self.wrap_around {
                self.cursor.col = 0;
                self.advance_line();
            }
        } else {
            self.cursor.col += 1;
        }
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            C0_HT => {
                self.cursor.col = self.next_tab_stop(self.cursor.col);
            }
            C0_BS => {
                self.cursor.col = self.cursor.col.saturating_sub(1);
            }
            C0_CR => {
                self.cursor.col = 0;
            }
            C0_LF | C0_VT | C0_FF => {
                self.advance_line();
            }
            C0_BEL => {}
            C0_SO => {
                self.active_charset = 1;
            }
            C0_SI => {
                self.active_charset = 0;
            }
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        ignore: bool,
        action: char,
    ) {
        if ignore {
            return;
        }

        let mut params_iter = params.iter();
        let mut next_param = || -> u32 {
            params_iter
                .next()
                .map(|p| p.first().copied().unwrap_or(0) as u32)
                .unwrap_or(0)
        };

        match (action, intermediates) {
            ('A', []) => {
                let n = next_param().max(1);
                self.cursor.row = self.cursor.row.saturating_sub(n);
                self.clamp_cursor();
            }
            ('B', []) | ('e', []) => {
                let n = next_param().max(1);
                self.cursor.row = (self.cursor.row + n).min(self.grid.rows().saturating_sub(1));
            }
            ('C', []) | ('a', []) => {
                let n = next_param().max(1);
                self.cursor.col = (self.cursor.col + n).min(self.grid.cols().saturating_sub(1));
            }
            ('D', []) => {
                let n = next_param().max(1);
                self.cursor.col = self.cursor.col.saturating_sub(n);
            }
            ('E', []) => {
                let n = next_param().max(1);
                self.cursor.row = (self.cursor.row + n).min(self.grid.rows().saturating_sub(1));
                self.cursor.col = 0;
            }
            ('F', []) => {
                let n = next_param().max(1);
                self.cursor.row = self.cursor.row.saturating_sub(n);
                self.cursor.col = 0;
            }
            ('G', []) | ('`', []) => {
                let n = next_param().max(1);
                self.cursor.col = (n - 1).min(self.grid.cols().saturating_sub(1));
            }
            ('H', []) | ('f', []) => {
                let row = next_param().max(1);
                let col = next_param().max(1);
                self.cursor.move_to(row - 1, col - 1);
                self.clamp_cursor();
            }
            ('J', []) => {
                let mode = next_param();
                self.clear_screen(mode);
            }
            ('K', []) => {
                let mode = next_param();
                self.clear_line(mode);
            }
            ('L', []) => {
                let n = next_param().max(1);
                self.insert_lines(n);
            }
            ('M', []) => {
                let n = next_param().max(1);
                self.delete_lines(n);
            }
            ('P', []) => {
                let n = next_param().max(1);
                self.delete_chars(n);
            }
            ('S', []) => {
                let n = next_param().max(1);
                self.scroll_up(n);
            }
            ('T', []) => {
                let n = next_param().max(1);
                self.scroll_down(n);
            }
            ('X', []) => {
                let n = next_param().max(1);
                self.erase_chars(n);
            }
            ('Z', []) => {
                let _n = next_param();
                let new_col = self.prev_tab_stop(self.cursor.col);
                self.cursor.col = new_col;
            }
            ('@', []) => {
                let n = next_param().max(1);
                self.insert_blank_chars(n);
            }
            ('d', []) => {
                let row = next_param().max(1);
                self.cursor.row = row - 1;
                self.clamp_cursor();
            }
            ('g', []) => {
                let mode = next_param();
                match mode {
                    0 => {
                        let col = self.cursor.col as usize;
                        if col < self.tab_stops.len() {
                            self.tab_stops[col] = false;
                        }
                    }
                    3 => {
                        self.tab_stops.iter_mut().for_each(|t| *t = false);
                    }
                    _ => {}
                }
            }
            ('r', []) => {
                let top = next_param();
                let bottom = next_param();
                self.set_scrolling_region(top, bottom);
            }
            ('m', []) => {
                let mut p = Vec::with_capacity(16);
                for param in params {
                    for &v in param {
                        p.push(v);
                    }
                }
                self.set_sgr(&p);
            }
            ('h', [b'?']) => {
                for param in params_iter {
                    if let Some(&mode) = param.first() {
                        self.dec_set_mode(mode as u32);
                    }
                }
            }
            ('l', [b'?']) => {
                for param in params_iter {
                    if let Some(&mode) = param.first() {
                        self.dec_reset_mode(mode as u32);
                    }
                }
            }
            ('n', []) => {}
            ('c', []) => {}
            ('s', []) => self.save_cursor_position(),
            ('u', []) => self.restore_cursor_position(),
            ('q', [b' ']) => {
                let style_id = next_param();
                match style_id {
                    0 | 1 => self.cursor.style = torvox_core::cursor::CursorStyle::Block,
                    2 => self.cursor.style = torvox_core::cursor::CursorStyle::Underline,
                    3 | 4 => self.cursor.style = torvox_core::cursor::CursorStyle::Bar,
                    5 | 6 => self.cursor.style = torvox_core::cursor::CursorStyle::Bar,
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.is_empty() || params[0].is_empty() {
            return;
        }
        match params[0] {
            b"0" | b"2" => {
                if params.len() >= 2 {
                    let title = String::from_utf8_lossy(params[1]).to_string();
                    self.title = Some(title);
                }
            }
            b"4" => {}
            b"10" | b"11" | b"12" => {}
            b"52" => {}
            b"104" => {}
            b"110" => {}
            b"111" => {}
            b"112" => {}
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        match (byte, intermediates) {
            (b'7', []) => self.save_cursor_position(),
            (b'8', []) => self.restore_cursor_position(),
            (b'D', []) => self.advance_line(),
            (b'E', []) => {
                self.advance_line();
                self.cursor.col = 0;
            }
            (b'H', []) => {
                let col = self.cursor.col as usize;
                if col < self.tab_stops.len() {
                    self.tab_stops[col] = true;
                }
            }
            (b'M', []) => {
                let row = self.effective_row();
                if row <= self.scroll_top {
                    self.grid
                        .scroll_down(self.scroll_top, self.scroll_bottom, self.grid.cols());
                } else {
                    self.cursor.row = self.cursor.row.saturating_sub(1);
                }
            }
            (b'c', []) => {
                if let Ok(new_state) = TerminalState::new(self.grid.rows(), self.grid.cols()) {
                    *self = new_state;
                }
            }
            (b'B', [b'(']) => self.charsets[0] = Charset::Ascii,
            (b'0', [b'(']) => self.charsets[0] = Charset::SpecialGraphics,
            (b'B', [b')']) => self.charsets[1] = Charset::Ascii,
            (b'0', [b')']) => self.charsets[1] = Charset::SpecialGraphics,
            (b'=', []) => {}
            (b'>', []) => {}
            (b'\\', []) => {}
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::VtParser;
    use proptest::prelude::*;
    use vte::Perform;

    fn make_state(rows: u32, cols: u32) -> TerminalState {
        TerminalState::new(rows, cols).unwrap()
    }

    fn parse(t: &mut TerminalState, bytes: &[u8]) {
        let mut parser = VtParser::new();
        parser.advance(t, bytes);
    }

    #[test]
    fn new_terminal_has_default_cursor() {
        let t = make_state(24, 80);
        assert_eq!(t.cursor.row, 0);
        assert_eq!(t.cursor.col, 0);
        assert!(t.cursor.visible);
    }

    #[test]
    fn new_terminal_has_correct_dimensions() {
        let t = make_state(24, 80);
        assert_eq!(t.rows(), 24);
        assert_eq!(t.cols(), 80);
    }

    #[test]
    fn print_single_char() {
        let mut t = make_state(24, 80);
        t.print('A');
        let cell = t.grid.cell(0, 0).unwrap();
        assert_eq!(cell.char, 'A');
        assert_eq!(t.cursor.col, 1);
    }

    #[test]
    fn print_advances_cursor() {
        let mut t = make_state(24, 80);
        t.print('H');
        t.print('i');
        assert_eq!(t.cursor.col, 2);
        assert_eq!(t.grid.cell(0, 0).unwrap().char, 'H');
        assert_eq!(t.grid.cell(0, 1).unwrap().char, 'i');
    }

    #[test]
    fn execute_linefeed() {
        let mut t = make_state(24, 80);
        assert_eq!(t.cursor.row, 0);
        t.execute(C0_LF);
        assert_eq!(t.cursor.row, 1);
    }

    #[test]
    fn execute_carriage_return() {
        let mut t = make_state(24, 80);
        t.cursor.col = 42;
        t.execute(C0_CR);
        assert_eq!(t.cursor.col, 0);
    }

    #[test]
    fn execute_backspace() {
        let mut t = make_state(24, 80);
        t.cursor.col = 5;
        t.execute(C0_BS);
        assert_eq!(t.cursor.col, 4);
    }

    #[test]
    fn execute_backspace_at_zero() {
        let mut t = make_state(24, 80);
        t.cursor.col = 0;
        t.execute(C0_BS);
        assert_eq!(t.cursor.col, 0);
    }

    #[test]
    fn execute_tab() {
        let mut t = make_state(24, 80);
        t.cursor.col = 0;
        t.execute(C0_HT);
        assert_eq!(t.cursor.col, 8);
    }

    #[test]
    fn cursor_movement_up() {
        let mut t = make_state(24, 80);
        t.cursor.row = 5;
        parse(&mut t, b"\x1b[A");
        assert_eq!(t.cursor.row, 4);
    }

    #[test]
    fn cursor_movement_up_with_param() {
        let mut t = make_state(24, 80);
        t.cursor.row = 10;
        parse(&mut t, b"\x1b[3A");
        assert_eq!(t.cursor.row, 7);
    }

    #[test]
    fn cursor_movement_up_clamps() {
        let mut t = make_state(24, 80);
        t.cursor.row = 0;
        parse(&mut t, b"\x1b[5A");
        assert_eq!(t.cursor.row, 0);
    }

    #[test]
    fn cursor_movement_down() {
        let mut t = make_state(24, 80);
        t.cursor.row = 5;
        parse(&mut t, b"\x1b[B");
        assert_eq!(t.cursor.row, 6);
    }

    #[test]
    fn cursor_movement_right() {
        let mut t = make_state(24, 80);
        t.cursor.col = 5;
        parse(&mut t, b"\x1b[C");
        assert_eq!(t.cursor.col, 6);
    }

    #[test]
    fn cursor_movement_left() {
        let mut t = make_state(24, 80);
        t.cursor.col = 5;
        parse(&mut t, b"\x1b[D");
        assert_eq!(t.cursor.col, 4);
    }

    #[test]
    fn cursor_position_absolute() {
        let mut t = make_state(24, 80);
        t.cursor.row = 10;
        t.cursor.col = 10;
        parse(&mut t, b"\x1b[3;5H");
        assert_eq!(t.cursor.row, 2);
        assert_eq!(t.cursor.col, 4);
    }

    #[test]
    fn cursor_position_default_params() {
        let mut t = make_state(24, 80);
        t.cursor.row = 10;
        t.cursor.col = 10;
        parse(&mut t, b"\x1b[H");
        assert_eq!(t.cursor.row, 0);
        assert_eq!(t.cursor.col, 0);
    }

    #[test]
    fn clear_screen_below() {
        let mut t = make_state(24, 80);
        t.grid.fill_cells(0, 'X', 0, 80);
        t.grid.fill_cells(1, 'Y', 0, 80);
        parse(&mut t, b"\x1b[J");
        assert_eq!(t.grid.cell(0, 0).unwrap().char, ' ');
        assert_eq!(t.grid.cell(1, 0).unwrap().char, ' ');
    }

    #[test]
    fn clear_screen_above() {
        let mut t = make_state(24, 80);
        t.cursor.row = 1;
        t.cursor.col = 5;
        t.grid.fill_cells(0, 'X', 0, 80);
        t.grid.fill_cells(1, 'Y', 0, 80);
        parse(&mut t, b"\x1b[1J");
        assert_eq!(t.grid.cell(0, 0).unwrap().char, ' ');
        assert_eq!(t.grid.cell(1, 5).unwrap().char, ' ');
        assert_eq!(t.grid.cell(1, 6).unwrap().char, 'Y');
    }

    #[test]
    fn clear_line_right() {
        let mut t = make_state(24, 80);
        t.grid.fill_cells(0, 'X', 0, 80);
        t.cursor.col = 40;
        parse(&mut t, b"\x1b[K");
        assert_eq!(t.grid.cell(0, 39).unwrap().char, 'X');
        assert_eq!(t.grid.cell(0, 40).unwrap().char, ' ');
    }

    #[test]
    fn clear_line_entire() {
        let mut t = make_state(24, 80);
        t.grid.fill_cells(0, 'X', 0, 80);
        t.cursor.col = 40;
        parse(&mut t, b"\x1b[2K");
        assert_eq!(t.grid.cell(0, 0).unwrap().char, ' ');
    }

    #[test]
    fn erase_chars() {
        let mut t = make_state(24, 80);
        t.grid.fill_cells(0, 'X', 0, 80);
        t.cursor.col = 10;
        parse(&mut t, b"\x1b[3X");
        assert_eq!(t.grid.cell(0, 9).unwrap().char, 'X');
        assert_eq!(t.grid.cell(0, 10).unwrap().char, ' ');
        assert_eq!(t.grid.cell(0, 11).unwrap().char, ' ');
        assert_eq!(t.grid.cell(0, 12).unwrap().char, ' ');
        assert_eq!(t.grid.cell(0, 13).unwrap().char, 'X');
    }

    #[test]
    fn scroll_up_region() {
        let mut t = make_state(24, 80);
        t.grid.fill_cells(0, 'A', 0, 80);
        t.grid.fill_cells(1, 'B', 0, 80);
        parse(&mut t, b"\x1b[1S");
        assert_eq!(t.grid.cell(0, 0).unwrap().char, 'B');
    }

    #[test]
    fn scroll_down_region() {
        let mut t = make_state(24, 80);
        t.grid.fill_cells(0, 'A', 0, 80);
        t.grid.fill_cells(1, 'B', 0, 80);
        t.cursor.row = 1;
        parse(&mut t, b"\x1b[1T");
        assert_eq!(t.grid.cell(0, 0).unwrap().char, ' ');
        assert_eq!(t.grid.cell(1, 0).unwrap().char, 'A');
    }

    #[test]
    fn insert_lines_basic() {
        let mut t = make_state(4, 10);
        t.grid.fill_cells(0, 'A', 0, 10);
        t.grid.fill_cells(1, 'B', 0, 10);
        t.grid.fill_cells(2, 'C', 0, 10);
        t.grid.fill_cells(3, 'D', 0, 10);
        parse(&mut t, b"\x1b[1L");
        assert_eq!(t.grid.cell(0, 0).unwrap().char, ' ');
        assert_eq!(t.grid.cell(1, 0).unwrap().char, 'A');
        assert_eq!(t.grid.cell(2, 0).unwrap().char, 'B');
    }

    #[test]
    fn delete_lines_basic() {
        let mut t = make_state(4, 10);
        t.grid.fill_cells(0, 'A', 0, 10);
        t.grid.fill_cells(1, 'B', 0, 10);
        t.grid.fill_cells(2, 'C', 0, 10);
        t.grid.fill_cells(3, 'D', 0, 10);
        parse(&mut t, b"\x1b[1M");
        assert_eq!(t.grid.cell(0, 0).unwrap().char, 'B');
        assert_eq!(t.grid.cell(1, 0).unwrap().char, 'C');
        assert_eq!(t.grid.cell(2, 0).unwrap().char, 'D');
        assert_eq!(t.grid.cell(3, 0).unwrap().char, ' ');
    }

    #[test]
    fn insert_blank_chars() {
        let mut t = make_state(1, 5);
        t.grid.fill_cells(0, 'X', 0, 5);
        t.cursor.col = 1;
        parse(&mut t, b"\x1b[2@");
        assert_eq!(t.grid.cell(0, 0).unwrap().char, 'X');
        assert_eq!(t.grid.cell(0, 1).unwrap().char, ' ');
        assert_eq!(t.grid.cell(0, 2).unwrap().char, ' ');
        assert_eq!(t.grid.cell(0, 3).unwrap().char, 'X');
    }

    #[test]
    fn delete_blank_chars() {
        let mut t = make_state(1, 5);
        t.grid.fill_cells(0, 'X', 0, 5);
        t.cursor.col = 1;
        parse(&mut t, b"\x1b[2P");
        assert_eq!(t.grid.cell(0, 0).unwrap().char, 'X');
        assert_eq!(t.grid.cell(0, 1).unwrap().char, 'X');
        assert_eq!(t.grid.cell(0, 2).unwrap().char, 'X');
        assert_eq!(t.grid.cell(0, 3).unwrap().char, ' ');
    }

    #[test]
    fn sgr_bold() {
        let mut t = make_state(24, 80);
        parse(&mut t, b"\x1b[1m");
        assert!(t.current_attrs.bold);
        assert!(!t.current_attrs.italic);
    }

    #[test]
    fn sgr_reset() {
        let mut t = make_state(24, 80);
        t.current_attrs.bold = true;
        t.current_attrs.italic = true;
        parse(&mut t, b"\x1b[0m");
        assert!(!t.current_attrs.bold);
        assert!(!t.current_attrs.italic);
    }

    #[test]
    fn sgr_foreground_color() {
        let mut t = make_state(24, 80);
        parse(&mut t, b"\x1b[31m");
        assert_eq!(t.fg_color, Color::new(128, 0, 0));
    }

    #[test]
    fn sgr_background_color() {
        let mut t = make_state(24, 80);
        parse(&mut t, b"\x1b[44m");
        assert_eq!(t.bg_color, Color::new(0, 0, 128));
    }

    #[test]
    fn sgr_truecolor_fg() {
        let mut t = make_state(24, 80);
        parse(&mut t, b"\x1b[38;2;255;128;0m");
        assert_eq!(t.fg_color, Color::new(255, 128, 0));
    }

    #[test]
    fn sgr_truecolor_bg() {
        let mut t = make_state(24, 80);
        parse(&mut t, b"\x1b[48;2;10;20;30m");
        assert_eq!(t.bg_color, Color::new(10, 20, 30));
    }

    #[test]
    fn sgr_256color_fg() {
        let mut t = make_state(24, 80);
        parse(&mut t, b"\x1b[38;5;196m");
        let expected = ansi_to_rgb(196);
        assert_eq!(
            t.fg_color,
            Color::new(expected[0], expected[1], expected[2])
        );
    }

    #[test]
    fn sgr_multiple_attributes() {
        let mut t = make_state(24, 80);
        parse(&mut t, b"\x1b[1;3;4m");
        assert!(t.current_attrs.bold);
        assert!(t.current_attrs.italic);
        assert!(t.current_attrs.underline);
    }

    #[test]
    fn sgr_fg_default() {
        let mut t = make_state(24, 80);
        t.fg_color = Color::new(255, 0, 0);
        parse(&mut t, b"\x1b[39m");
        assert_eq!(t.fg_color, Color::default());
    }

    #[test]
    fn sgr_bg_default() {
        let mut t = make_state(24, 80);
        t.bg_color = Color::new(255, 0, 0);
        parse(&mut t, b"\x1b[49m");
        assert_eq!(t.bg_color, Color::default());
    }

    #[test]
    fn sgr_bright_fg_colors() {
        let mut t = make_state(24, 80);
        parse(&mut t, b"\x1b[91m");
        let expected = ansi_to_rgb(9);
        assert_eq!(
            t.fg_color,
            Color::new(expected[0], expected[1], expected[2])
        );
    }

    #[test]
    fn sgr_bright_bg_colors() {
        let mut t = make_state(24, 80);
        parse(&mut t, b"\x1b[104m");
        let expected = ansi_to_rgb(12);
        assert_eq!(
            t.bg_color,
            Color::new(expected[0], expected[1], expected[2])
        );
    }

    #[test]
    fn save_restore_cursor() {
        let mut t = make_state(24, 80);
        t.cursor.move_to(5, 10);
        t.save_cursor_position();
        t.cursor.move_to(0, 0);
        assert_eq!(t.cursor.row, 0);
        t.restore_cursor_position();
        assert_eq!(t.cursor.row, 5);
        assert_eq!(t.cursor.col, 10);
    }

    #[test]
    fn scrolling_region() {
        let mut t = make_state(24, 80);
        parse(&mut t, b"\x1b[5;20r");
        assert_eq!(t.scroll_top, 4);
        assert_eq!(t.scroll_bottom, 20);
    }

    #[test]
    fn dec_set_origin_mode() {
        let mut t = make_state(24, 80);
        assert!(!t.origin_mode);
        t.dec_set_mode(6);
        assert!(t.origin_mode);
    }

    #[test]
    fn dec_set_wrap_around() {
        let mut t = make_state(24, 80);
        assert!(t.wrap_around);
        t.dec_reset_mode(7);
        assert!(!t.wrap_around);
    }

    #[test]
    fn dec_set_cursor_visible() {
        let mut t = make_state(24, 80);
        t.cursor.visible = false;
        t.dec_set_mode(25);
        assert!(t.cursor.visible);
    }

    #[test]
    fn dec_set_show_cursor_via_csi() {
        let mut t = make_state(24, 80);
        t.cursor.visible = false;
        parse(&mut t, b"\x1b[?25h");
        assert!(t.cursor.visible);
    }

    #[test]
    fn dec_reset_hide_cursor_via_csi() {
        let mut t = make_state(24, 80);
        t.cursor.visible = true;
        parse(&mut t, b"\x1b[?25l");
        assert!(!t.cursor.visible);
    }

    #[test]
    fn esc_save_restore_cursor() {
        let mut t = make_state(24, 80);
        t.cursor.move_to(5, 10);
        parse(&mut t, b"\x1b7");
        t.cursor.move_to(0, 0);
        parse(&mut t, b"\x1b8");
        assert_eq!(t.cursor.row, 5);
        assert_eq!(t.cursor.col, 10);
    }

    #[test]
    fn esc_ris_resets_state() {
        let mut t = make_state(24, 80);
        t.cursor.move_to(5, 10);
        t.current_attrs.bold = true;
        parse(&mut t, b"\x1bc");
        assert_eq!(t.cursor.row, 0);
        assert_eq!(t.cursor.col, 0);
        assert!(!t.current_attrs.bold);
    }

    #[test]
    fn osc_title() {
        let mut t = make_state(24, 80);
        t.osc_dispatch(&[b"2", b"my terminal"], false);
        assert_eq!(t.title.as_deref(), Some("my terminal"));
    }

    #[test]
    fn print_wraps_at_end_of_line() {
        let mut t = make_state(24, 3);
        t.print('A');
        t.print('B');
        assert_eq!(t.cursor.row, 0);
        assert_eq!(t.cursor.col, 2);
        t.print('C');
        assert_eq!(t.cursor.row, 1);
        assert_eq!(t.cursor.col, 0);
        assert_eq!(t.grid.cell(0, 0).unwrap().char, 'A');
        assert_eq!(t.grid.cell(0, 1).unwrap().char, 'B');
        assert_eq!(t.grid.cell(0, 2).unwrap().char, 'C');
        assert_eq!(t.grid.cell(1, 0).unwrap().char, ' ');
    }

    #[test]
    fn tab_stop_next() {
        let t = make_state(1, 40);
        assert_eq!(t.next_tab_stop(0), 8);
        assert_eq!(t.next_tab_stop(8), 16);
    }

    #[test]
    fn tab_stop_prev() {
        let t = make_state(1, 40);
        assert_eq!(t.prev_tab_stop(10), 8);
        assert_eq!(t.prev_tab_stop(8), 0);
        assert_eq!(t.prev_tab_stop(0), 0);
    }

    #[test]
    fn linefeed_at_bottom_of_scroll_region() {
        let mut t = make_state(4, 10);
        parse(&mut t, b"\x1b[2;3r");
        t.grid.fill_cells(1, 'A', 0, 10);
        t.grid.fill_cells(2, 'B', 0, 10);
        t.cursor.row = 2;
        t.execute(C0_LF);
        assert_eq!(t.grid.cell(1, 0).unwrap().char, 'B');
    }

    #[test]
    fn back_tab() {
        let mut t = make_state(1, 40);
        t.cursor.col = 20;
        parse(&mut t, b"\x1b[Z");
        assert_eq!(t.cursor.col, 16);
    }

    #[test]
    fn integration_print_text() {
        let mut t = make_state(24, 80);
        parse(&mut t, b"Hello, World!");
        for (i, ch) in "Hello, World!".chars().enumerate() {
            assert_eq!(t.grid.cell(0, i as u32).unwrap().char, ch);
        }
        assert_eq!(t.cursor.col, 13);
    }

    #[test]
    fn integration_bold_then_text() {
        let mut t = make_state(24, 80);
        parse(&mut t, b"\x1b[1mBold");
        assert!(t.current_attrs.bold);
        assert_eq!(t.grid.cell(0, 0).unwrap().char, 'B');
        assert!(t.grid.cell(0, 0).unwrap().attrs.bold);
        assert_eq!(t.grid.cell(0, 1).unwrap().char, 'o');
        assert!(t.grid.cell(0, 1).unwrap().attrs.bold);
    }

    #[test]
    fn integration_newline_moves_down() {
        let mut t = make_state(24, 80);
        parse(&mut t, b"Line1\r\nLine2");
        assert_eq!(t.grid.cell(0, 0).unwrap().char, 'L');
        assert_eq!(t.grid.cell(1, 0).unwrap().char, 'L');
        assert_eq!(t.grid.cell(1, 4).unwrap().char, '2');
    }

    #[test]
    fn integration_cursor_positioning() {
        let mut t = make_state(24, 80);
        parse(&mut t, b"AB\x1b[10;20HCD");
        assert_eq!(t.grid.cell(0, 0).unwrap().char, 'A');
        assert_eq!(t.grid.cell(0, 1).unwrap().char, 'B');
        assert_eq!(t.cursor.row, 9);
        assert_eq!(t.grid.cell(9, 19).unwrap().char, 'C');
        assert_eq!(t.grid.cell(9, 20).unwrap().char, 'D');
        assert_eq!(t.cursor.col, 21);
    }

    #[test]
    fn integration_color_and_text() {
        let mut t = make_state(24, 80);
        parse(&mut t, b"\x1b[31;44mRed on Blue");
        assert_eq!(t.fg_color, Color::new(128, 0, 0));
        assert_eq!(t.bg_color, Color::new(0, 0, 128));
        let cell = t.grid.cell(0, 0).unwrap();
        assert_eq!(cell.char, 'R');
        assert_eq!(cell.fg, Color::new(128, 0, 0));
        assert_eq!(cell.bg, Color::new(0, 0, 128));
    }

    #[test]
    fn integration_erase_display() {
        let mut t = make_state(24, 80);
        parse(&mut t, b"Hello\x1b[2J");
        assert_eq!(t.grid.cell(0, 0).unwrap().char, ' ');
    }

    #[test]
    fn integration_insert_mode() {
        let mut t = make_state(1, 10);
        t.grid.fill_cells(0, 'X', 0, 10);
        t.cursor.col = 2;
        t.insert_mode = true;
        t.print('!');
        assert_eq!(t.grid.cell(0, 2).unwrap().char, '!');
        assert_eq!(t.grid.cell(0, 3).unwrap().char, 'X');
    }

    #[test]
    fn grid_methods_cell_mut() {
        let mut g = Grid::new(3, 5);
        g.mark_clean();
        g.cell_mut(1, 2).unwrap().char = 'Z';
        assert_eq!(g.cell(1, 2).unwrap().char, 'Z');
        assert!(g.dirty().is_dirty(1));
        assert!(!g.dirty().is_dirty(0));
    }

    #[test]
    fn grid_methods_scroll_up() {
        let mut g = Grid::new(3, 5);
        g.fill_cells(0, 'A', 0, 5);
        g.fill_cells(1, 'B', 0, 5);
        g.fill_cells(2, 'C', 0, 5);
        g.scroll_up(0, 3, 5);
        assert_eq!(g.cell(0, 0).unwrap().char, 'B');
        assert_eq!(g.cell(1, 0).unwrap().char, 'C');
        assert_eq!(g.cell(2, 0).unwrap().char, ' ');
    }

    #[test]
    fn grid_methods_scroll_down() {
        let mut g = Grid::new(3, 5);
        g.fill_cells(0, 'A', 0, 5);
        g.fill_cells(1, 'B', 0, 5);
        g.fill_cells(2, 'C', 0, 5);
        g.scroll_down(0, 3, 5);
        assert_eq!(g.cell(0, 0).unwrap().char, ' ');
        assert_eq!(g.cell(1, 0).unwrap().char, 'A');
        assert_eq!(g.cell(2, 0).unwrap().char, 'B');
    }

    #[test]
    fn grid_methods_insert_lines() {
        let mut g = Grid::new(4, 5);
        g.fill_cells(0, 'A', 0, 5);
        g.fill_cells(1, 'B', 0, 5);
        g.fill_cells(2, 'C', 0, 5);
        g.fill_cells(3, 'D', 0, 5);
        g.insert_lines(1, 1, 4, 5);
        assert_eq!(g.cell(0, 0).unwrap().char, 'A');
        assert_eq!(g.cell(1, 0).unwrap().char, ' ');
        assert_eq!(g.cell(2, 0).unwrap().char, 'B');
        assert_eq!(g.cell(3, 0).unwrap().char, 'C');
    }

    #[test]
    fn grid_methods_delete_lines() {
        let mut g = Grid::new(4, 5);
        g.fill_cells(0, 'A', 0, 5);
        g.fill_cells(1, 'B', 0, 5);
        g.fill_cells(2, 'C', 0, 5);
        g.fill_cells(3, 'D', 0, 5);
        g.delete_lines(1, 1, 4, 5);
        assert_eq!(g.cell(0, 0).unwrap().char, 'A');
        assert_eq!(g.cell(1, 0).unwrap().char, 'C');
        assert_eq!(g.cell(2, 0).unwrap().char, 'D');
        assert_eq!(g.cell(3, 0).unwrap().char, ' ');
    }

    #[test]
    fn grid_methods_clear_cells() {
        let mut g = Grid::new(1, 10);
        g.fill_cells(0, 'X', 0, 10);
        g.clear_cells(0, 2, 5);
        assert_eq!(g.cell(0, 1).unwrap().char, 'X');
        assert_eq!(g.cell(0, 2).unwrap().char, ' ');
        assert_eq!(g.cell(0, 4).unwrap().char, ' ');
        assert_eq!(g.cell(0, 5).unwrap().char, 'X');
    }

    #[test]
    fn grid_methods_fill_cells() {
        let mut g = Grid::new(1, 10);
        g.fill_cells(0, 'Z', 3, 7);
        assert_eq!(g.cell(0, 2).unwrap().char, ' ');
        assert_eq!(g.cell(0, 3).unwrap().char, 'Z');
        assert_eq!(g.cell(0, 6).unwrap().char, 'Z');
        assert_eq!(g.cell(0, 7).unwrap().char, ' ');
    }

    proptest::proptest! {
        #[test]
        fn parser_never_panics(input in proptest::collection::vec(0u8..=255u8, 0..1000)) {
            let mut state = TerminalState::new(24, 80).unwrap();
            let mut parser = crate::parser::VtParser::new();
            parser.advance(&mut state, &input);
        }

        #[test]
        fn grid_dimensions_invariant(rows in 1u32..=200u32, cols in 1u32..=500u32) {
            let g = Grid::new(rows, cols);
            prop_assert_eq!(g.rows(), rows);
            prop_assert_eq!(g.cols(), cols);
        }

        #[test]
        fn dirty_mask_consistency(rows in 1u32..=200u32) {
            let mut mask = torvox_core::cell::DirtyMask::new(rows);
            prop_assert!(!mask.any_dirty());
            mask.mark_all(rows);
            for i in 0..rows {
                prop_assert!(mask.is_dirty(i));
            }
            mask.clear();
            prop_assert!(!mask.any_dirty());
        }

        #[test]
        fn scrollback_saves_scroll_top_lines(
            rows in 2u32..=20u32,
            cols in 1u32..=100u32,
            scroll_count in 1u32..=10u32,
        ) {
            let mut g = Grid::new(rows, cols);
            g.fill_cells(0, 'X', 0, cols);
            for _ in 0..scroll_count {
                g.scroll_up(0, rows, cols);
            }
            prop_assert!(g.scrollback_len() as u32 >= scroll_count.min(rows - 1));
        }

        #[test]
        fn scroll_region_scroll_up(rows in 2u32..=20u32, cols in 1u32..=100u32) {
            let mut state = TerminalState::new(rows, cols).unwrap();
            let mut parser = crate::parser::VtParser::new();
            state.grid.fill_cells(0, 'A', 0, cols);
            state.grid.fill_cells(1, 'B', 0, cols);
            parser.advance(&mut state, b"\x1b[1S");
            prop_assert_eq!(state.grid.cell(0, 0).unwrap().char, 'B');
            prop_assert_eq!(state.grid.cell(rows - 1, 0).unwrap().char, ' ');
        }
    }
}
