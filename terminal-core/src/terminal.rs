//! TerminalState — cursor, modes, tabs, and VT state machine.
//!
//! # Requirements
//! - [FR-021](crate) — Terminal: state machine
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::sgr::SgrAttribute;

/// Standard VT100 tab stop interval (every 8 columns)
const TAB_STOP_INTERVAL: u16 = 8;

/// Terminal state for VT protocol conformance
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalState {
    /// Cursor position (row, col) - 0-indexed
    pub cursor_row: u16,
    pub cursor_col: u16,
    /// Saved cursor row for the DECSC/DECRC save/restore sequence.
    pub saved_cursor_row: u16,
    /// Saved cursor column for the DECSC/DECRC save/restore sequence.
    pub saved_cursor_col: u16,
    /// Tab stop positions across the terminal width.
    pub tab_stops: Vec<bool>,
    /// DEC private mode flags (e.g., DEC cursor keys mode, autowrap, origin mode, etc.).
    pub dec_modes: Vec<u16>,
    /// Origin mode (DECOM) - cursor constrained to scrolling region
    pub origin_mode: bool,
    /// Auto-wrap mode (DECAWM)
    pub auto_wrap: bool,
    /// Text cursor enable (DECTCEM)
    pub cursor_visible: bool,
    /// Alternate screen buffer
    pub alternate_screen: bool,
    /// Bracketed paste mode
    pub bracketed_paste: bool,
    /// Scrolling region defined as (top, bottom) row boundaries.
    pub scrolling_region: Option<(u16, u16)>,
    /// Terminal window title, set by OSC 0 or OSC 2.
    pub title: Option<String>,
    /// Terminal icon title, set by OSC 1.
    pub icon_title: Option<String>,
    /// Current SGR attributes (bold, italic, underline, colors, etc.).
    pub sgr_attributes: Vec<SgrAttribute>,
}

impl TerminalState {
    /// Create a new terminal state with given dimensions
    ///
    /// ```
    /// use terminal_core::terminal::TerminalState;
    ///
    /// let state = TerminalState::new(24, 80);
    /// assert_eq!(state.cursor_row, 0);
    /// assert_eq!(state.cursor_col, 0);
    /// assert!(state.cursor_visible);
    /// assert!(state.auto_wrap);
    /// assert_eq!(state.alternate_screen, false);
    /// ```
    pub fn new(_rows: u16, cols: u16) -> Self {
        let mut tab_stops = Vec::with_capacity(cols as usize + 1);
        for i in 0..=cols {
            tab_stops.push(i % TAB_STOP_INTERVAL == 0);
        }

        Self {
            cursor_row: 0,
            cursor_col: 0,
            saved_cursor_row: 0,
            saved_cursor_col: 0,
            tab_stops,
            dec_modes: Vec::new(),
            origin_mode: false,
            auto_wrap: true,
            cursor_visible: true,
            alternate_screen: false,
            bracketed_paste: false,
            scrolling_region: None,
            title: None,
            icon_title: None,
            sgr_attributes: Vec::new(),
        }
    }

    /// Apply SGR attributes to terminal state
    pub fn apply_sgr(&mut self, attrs: &[SgrAttribute]) {
        if attrs.is_empty() {
            self.sgr_attributes.clear();
            return;
        }
        for attr in attrs {
            match attr {
                SgrAttribute::Reset => {
                    self.sgr_attributes.clear();
                    return;
                }
                _ => {
                    self.sgr_attributes.retain(|existing| {
                        core::mem::discriminant(existing) != core::mem::discriminant(attr)
                    });
                    self.sgr_attributes.push(*attr);
                }
            }
        }
    }

    /// Save cursor position (DECSC)
    ///
    /// ```
    /// use terminal_core::terminal::TerminalState;
    ///
    /// let mut state = TerminalState::new(24, 80);
    /// state.cursor_row = 15;
    /// state.cursor_col = 40;
    /// state.save_cursor();
    /// state.cursor_row = 0;
    /// state.cursor_col = 0;
    /// state.restore_cursor();
    /// assert_eq!(state.cursor_row, 15);
    /// assert_eq!(state.cursor_col, 40);
    /// ```
    pub fn save_cursor(&mut self) {
        self.saved_cursor_row = self.cursor_row;
        self.saved_cursor_col = self.cursor_col;
    }

    /// Restore cursor position (DECRC)
    pub fn restore_cursor(&mut self) {
        self.cursor_row = self.saved_cursor_row;
        self.cursor_col = self.saved_cursor_col;
    }

    /// Set or clear a DEC private mode.
    pub fn set_dec_mode(&mut self, mode: u16, enabled: bool) {
        if enabled {
            if !self.dec_modes.contains(&mode) {
                self.dec_modes.push(mode);
            }
        } else {
            if let Some(pos) = self.dec_modes.iter().position(|&m| m == mode) {
                self.dec_modes.remove(pos);
            }
        }
    }

    /// Check if a DEC private mode is enabled.
    pub fn is_dec_mode(&self, mode: u16) -> bool {
        self.dec_modes.contains(&mode)
    }

    /// Move cursor to position with bounds checking
    pub fn move_cursor(&mut self, row: u16, col: u16, rows: u16, cols: u16) {
        let (mut target_row, mut target_col) = (row, col);

        if self.origin_mode
            && let Some((top, _bottom)) = self.scrolling_region
        {
            target_row = target_row.saturating_add(top);
        }

        target_row = target_row.min(rows.saturating_sub(1));
        target_col = target_col.min(cols.saturating_sub(1));

        self.cursor_row = target_row;
        self.cursor_col = target_col;
    }

    /// When origin mode (DECOM) is active, clamp `row` to the scrolling region.
    /// When origin mode is inactive or no region is set, clamp to valid screen bounds.
    fn clamp_row_for_origin(&self, row: u16, total_rows: u16) -> u16 {
        if self.origin_mode
            && let Some((top, bottom)) = self.scrolling_region
        {
            return row.clamp(top, bottom);
        }
        row.min(total_rows.saturating_sub(1))
    }

    /// Move cursor up by `count` rows.
    pub fn cursor_up(&mut self, count: u16, total_rows: u16) {
        let count = count.max(1);
        self.cursor_row =
            self.clamp_row_for_origin(self.cursor_row.saturating_sub(count), total_rows);
    }

    /// Move cursor down by `count` rows.
    pub fn cursor_down(&mut self, count: u16, total_rows: u16) {
        let count = count.max(1);
        self.cursor_row =
            self.clamp_row_for_origin(self.cursor_row.saturating_add(count), total_rows);
    }

    /// Move cursor forward by `count` columns.
    pub fn cursor_forward(&mut self, count: u16, total_columns: u16) {
        let count = count.max(1);
        self.cursor_col = self
            .cursor_col
            .saturating_add(count)
            .min(total_columns.saturating_sub(1));
    }

    /// Move cursor back by `count` columns.
    pub fn cursor_back(&mut self, count: u16) {
        let count = count.max(1);
        self.cursor_col = self.cursor_col.saturating_sub(count);
    }

    /// Move cursor to next line (column 0), down by `count` rows.
    pub fn cursor_next_line(&mut self, count: u16, total_rows: u16) {
        let count = count.max(1);
        self.cursor_row =
            self.clamp_row_for_origin(self.cursor_row.saturating_add(count), total_rows);
        self.cursor_col = 0;
    }

    /// Move cursor to previous line, up by `count` rows.
    pub fn cursor_prev_line(&mut self, count: u16, total_rows: u16) {
        let count = count.max(1);
        self.cursor_row =
            self.clamp_row_for_origin(self.cursor_row.saturating_sub(count), total_rows);
    }

    /// Set cursor horizontal absolute position
    pub fn cursor_horizontal_absolute(&mut self, col: u16, cols: u16) {
        self.cursor_col = col.min(cols.saturating_sub(1));
    }

    /// Set cursor position (1-indexed per VT spec, converted to 0-indexed).
    pub fn cursor_position(&mut self, row: u16, col: u16, rows: u16, cols: u16) {
        self.move_cursor(row.saturating_sub(1), col.saturating_sub(1), rows, cols);
    }

    /// Horizontal tab
    pub fn cursor_horizontal_tab(&mut self, cols: u16) {
        let current_col = self.cursor_col as usize;
        let max_col = cols.saturating_sub(1) as usize;
        let mut next_tab = None;

        for i in (current_col + 1)..=max_col {
            if self.tab_stops.get(i).copied().unwrap_or(false) {
                next_tab = Some(i as u16);
                break;
            }
        }

        if let Some(tab_col) = next_tab {
            self.cursor_col = tab_col;
        } else {
            self.cursor_col = cols.saturating_sub(1);
        }
    }

    /// Horizontal tab back
    pub fn cursor_horizontal_tab_back(&mut self, _cols: u16) {
        let mut prev_tab = None;

        for i in (0..self.cursor_col as usize).rev() {
            if self.tab_stops.get(i).copied().unwrap_or(false) {
                prev_tab = Some(i as u16);
                break;
            }
        }

        if let Some(tab_col) = prev_tab {
            self.cursor_col = tab_col;
        } else {
            self.cursor_col = 0;
        }
    }

    /// Vertical position absolute
    pub fn cursor_vertical_absolute(&mut self, row: u16, rows: u16) {
        self.cursor_row = row.min(rows.saturating_sub(1));
    }

    /// Erase in display. Cursor position is preserved per VT spec.
    /// Grid erasure is handled by the caller; this method only updates
    /// terminal state flags if needed.
    pub fn erase_in_display(&mut self, _mode: u8, _rows: u16, _cols: u16) {}

    /// Erase in line. Cursor position is preserved per VT spec.
    /// Grid erasure is handled by the caller.
    pub fn erase_in_line(&mut self, _mode: u8) {}

    /// Insert count blank lines at cursor. Cursor stays put.
    /// Actual line insertion is handled by the caller.
    pub fn insert_lines(&mut self, _count: u16, _total_rows: u16) {}

    /// Delete count lines at cursor. Cursor stays put.
    /// Actual line deletion is handled by the caller.
    pub fn delete_lines(&mut self, _count: u16, _total_rows: u16) {}

    /// Insert count blank characters at cursor. Cursor stays put.
    /// Character insertion is handled by the caller.
    pub fn insert_characters(&mut self, _count: u16) {}

    /// Delete count characters at cursor. Cursor stays put.
    /// Character deletion is handled by the caller.
    pub fn delete_characters(&mut self, _count: u16) {}

    /// Scroll up count lines within scrolling region.
    /// Scrolling is handled by the caller.
    pub fn scroll_up(&mut self, _count: u16, _total_rows: u16) {}

    /// Scroll down count lines within scrolling region.
    /// Scrolling is handled by the caller.
    pub fn scroll_down(&mut self, _count: u16, _total_rows: u16) {}

    /// Erase count characters starting at cursor (replaces with blanks).
    /// Character erasure is handled by the caller.
    pub fn erase_characters(&mut self, _count: u16) {}

    /// Repeat the character preceding the cursor count times.
    /// Character repetition is handled by the caller.
    pub fn repeat_character(&mut self, _count: u16) {}

    /// Set scrolling region
    pub fn set_scrolling_region(&mut self, top: u16, bottom: u16) {
        self.scrolling_region = Some((top, bottom));
    }

    /// Reset scrolling region
    pub fn reset_scrolling_region(&mut self) {
        self.scrolling_region = None;
    }

    /// Set origin mode
    pub fn set_origin_mode(&mut self, enabled: bool) {
        self.origin_mode = enabled;
    }

    /// Set auto-wrap mode
    pub fn set_auto_wrap(&mut self, enabled: bool) {
        self.auto_wrap = enabled;
    }

    /// Set cursor visibility
    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor_visible = visible;
    }

    /// Set alternate screen buffer
    pub fn set_alternate_screen(&mut self, enabled: bool) {
        self.alternate_screen = enabled;
    }

    /// Set bracketed paste mode
    pub fn set_bracketed_paste(&mut self, enabled: bool) {
        self.bracketed_paste = enabled;
    }

    /// Set window title
    pub fn set_title(&mut self, title: &str) {
        self.title = Some(title.to_string());
    }

    /// Set icon title
    pub fn set_icon_title(&mut self, title: &str) {
        self.icon_title = Some(title.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn erase_in_display_mode_zero_does_not_move_cursor() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 5;
        state.cursor_col = 10;
        state.erase_in_display(0, 24, 80);
        assert_eq!(state.cursor_row, 5);
        assert_eq!(state.cursor_col, 10);
    }

    #[test]
    fn erase_in_display_mode_one_does_not_move_cursor() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 5;
        state.cursor_col = 10;
        state.erase_in_display(1, 24, 80);
        assert_eq!(state.cursor_row, 5);
        assert_eq!(state.cursor_col, 10);
    }

    #[test]
    fn erase_in_display_mode_two_does_not_move_cursor() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 5;
        state.cursor_col = 10;
        state.erase_in_display(2, 24, 80);
        assert_eq!(state.cursor_row, 5);
        assert_eq!(state.cursor_col, 10);
    }

    #[test]
    fn erase_in_line_mode_zero_does_not_move_cursor() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 5;
        state.cursor_col = 10;
        state.erase_in_line(0);
        assert_eq!(state.cursor_row, 5);
        assert_eq!(state.cursor_col, 10);
    }

    #[test]
    fn erase_in_line_mode_one_does_not_move_cursor() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 5;
        state.cursor_col = 10;
        state.erase_in_line(1);
        assert_eq!(state.cursor_row, 5);
        assert_eq!(state.cursor_col, 10);
    }

    #[test]
    fn erase_in_line_mode_two_does_not_move_cursor() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 5;
        state.cursor_col = 10;
        state.erase_in_line(2);
        assert_eq!(state.cursor_row, 5);
        assert_eq!(state.cursor_col, 10);
    }

    #[test]
    fn cursor_move_stays_within_bounds() {
        let mut state = TerminalState::new(24, 80);
        state.move_cursor(30, 100, 24, 80);
        assert!(state.cursor_row < 24);
        assert!(state.cursor_col < 80);
    }

    #[test]
    fn cursor_up_at_top_stays_at_zero() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 0;
        state.cursor_up(5, 24);
        assert_eq!(state.cursor_row, 0);
    }

    #[test]
    fn cursor_up_clamps() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 3;
        state.cursor_up(10, 24);
        assert_eq!(state.cursor_row, 0);
    }

    #[test]
    fn cursor_down_clamps() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 20;
        state.cursor_down(10, 24);
        assert_eq!(state.cursor_row, 23);
    }

    #[test]
    fn cursor_horizontal_tab_advances() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_col = 0;
        state.cursor_horizontal_tab(80);
        assert!(state.cursor_col > 0);
    }

    #[test]
    fn cursor_horizontal_tab_clamps_to_end() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_col = 78;
        state.cursor_horizontal_tab(80);
        assert_eq!(
            state.cursor_col, 79,
            "tab past last stop must clamp to right margin"
        );
    }

    #[test]
    fn cursor_horizontal_tab_no_stop_moves_to_right_margin() {
        let mut state = TerminalState::new(24, 80);
        state.tab_stops = vec![false; 80];
        state.cursor_col = 5;
        state.cursor_horizontal_tab(80);
        assert_eq!(
            state.cursor_col, 79,
            "no tab stop ahead must move to right margin (col 79)"
        );
    }

    #[test]
    fn cursor_horizontal_tab_back_moves_to_left_margin() {
        let mut state = TerminalState::new(24, 80);
        state.tab_stops = vec![false; 80];
        state.cursor_col = 10;
        state.cursor_horizontal_tab_back(80);
        assert_eq!(
            state.cursor_col, 0,
            "no tab stop behind must move to left margin (col 0)"
        );
    }

    #[test]
    fn cursor_horizontal_tab_back_does_not_underflow() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_col = 0;
        state.cursor_horizontal_tab_back(80);
        assert_eq!(state.cursor_col, 0);
    }

    #[test]
    fn set_scrolling_region_roundtrip() {
        let mut state = TerminalState::new(24, 80);
        assert!(state.scrolling_region.is_none());
        state.set_scrolling_region(2, 22);
        assert_eq!(state.scrolling_region, Some((2, 22)));
        state.reset_scrolling_region();
        assert!(state.scrolling_region.is_none());
    }

    #[test]
    fn mode_toggles() {
        let mut state = TerminalState::new(24, 80);
        assert!(!state.origin_mode);
        state.set_origin_mode(true);
        assert!(state.origin_mode);

        assert!(state.auto_wrap);
        state.set_auto_wrap(false);
        assert!(!state.auto_wrap);

        assert!(state.cursor_visible);
        state.set_cursor_visible(false);
        assert!(!state.cursor_visible);

        assert!(!state.alternate_screen);
        state.set_alternate_screen(true);
        assert!(state.alternate_screen);

        assert!(!state.bracketed_paste);
        state.set_bracketed_paste(true);
        assert!(state.bracketed_paste);
    }

    #[test]
    fn title_and_icon_title() {
        let mut state = TerminalState::new(24, 80);
        assert!(state.title.is_none());
        assert!(state.icon_title.is_none());
        state.set_title("My Terminal");
        assert_eq!(state.title.as_deref(), Some("My Terminal"));
        state.set_icon_title("My Icon");
        assert_eq!(state.icon_title.as_deref(), Some("My Icon"));
        state.set_title("Updated");
        assert_eq!(state.title.as_deref(), Some("Updated"));
    }

    #[test]
    fn save_restore_cursor_roundtrip() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 15;
        state.cursor_col = 40;
        state.save_cursor();
        state.cursor_row = 0;
        state.cursor_col = 0;
        state.restore_cursor();
        assert_eq!(state.cursor_row, 15, "row should be restored to 15");
        assert_eq!(state.cursor_col, 40, "col should be restored to 40");
    }

    #[test]
    fn save_restore_cursor_overwrites_previous() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 5;
        state.cursor_col = 10;
        state.save_cursor();
        state.cursor_row = 20;
        state.cursor_col = 70;
        state.save_cursor();
        state.cursor_row = 0;
        state.cursor_col = 0;
        state.restore_cursor();
        assert_eq!(
            state.cursor_row, 20,
            "should restore to second save, not first"
        );
        assert_eq!(state.cursor_col, 70, "col should restore to second save");
    }

    #[test]
    fn cursor_forward_clamps_to_right() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_col = 70;
        state.cursor_forward(20, 80);
        assert_eq!(state.cursor_col, 79, "should clamp to cols-1");
    }

    #[test]
    fn cursor_forward_from_zero() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_col = 0;
        state.cursor_forward(5, 80);
        assert_eq!(state.cursor_col, 5, "should advance by 5");
    }

    #[test]
    fn cursor_back_clamps_to_zero() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_col = 3;
        state.cursor_back(10);
        assert_eq!(state.cursor_col, 0, "should clamp to 0");
    }

    #[test]
    fn cursor_back_normal_move() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_col = 20;
        state.cursor_back(5);
        assert_eq!(state.cursor_col, 15, "should move back by 5");
    }

    #[test]
    fn cursor_next_line_wraps_to_next_row() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 5;
        state.cursor_col = 40;
        state.cursor_next_line(1, 24);
        assert_eq!(state.cursor_row, 6, "should move to next row");
        assert_eq!(state.cursor_col, 0, "should reset col to 0");
    }

    #[test]
    fn cursor_next_line_at_bottom_does_not_overflow() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 23;
        state.cursor_next_line(1, 24);
        assert_eq!(state.cursor_row, 23, "should clamp at last row");
        assert_eq!(state.cursor_col, 0, "should still reset col to 0");
    }

    #[test]
    fn cursor_prev_line_at_top_does_not_underflow() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 0;
        state.cursor_prev_line(1, 24);
        assert_eq!(state.cursor_row, 0, "should clamp at row 0");
    }

    #[test]
    fn cursor_prev_line_normal_move() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 10;
        state.cursor_prev_line(3, 24);
        assert_eq!(state.cursor_row, 7, "should move up by 3");
        assert_eq!(state.cursor_row, 7, "should move up by 3");
    }

    #[test]
    fn cursor_horizontal_absolute_sets_col() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_col = 50;
        state.cursor_horizontal_absolute(10, 80);
        assert_eq!(
            state.cursor_col, 10,
            "should set col to param directly (clamped to cols-1)"
        );
    }

    #[test]
    fn cursor_horizontal_absolute_clamps() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_horizontal_absolute(200, 80);
        assert_eq!(state.cursor_col, 79, "should clamp to cols-1");
    }

    #[test]
    fn cursor_vertical_absolute_sets_row() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 10;
        state.cursor_vertical_absolute(5, 24);
        assert_eq!(
            state.cursor_row, 5,
            "should set row to param directly (clamped to rows-1)"
        );
    }

    #[test]
    fn cursor_vertical_absolute_clamps() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_vertical_absolute(100, 24);
        assert_eq!(state.cursor_row, 23, "should clamp to rows-1");
    }

    #[test]
    fn is_dec_mode_matches_set_dec_mode() {
        let mut state = TerminalState::new(24, 80);
        assert!(!state.is_dec_mode(25), "mode 25 should start unset");
        state.set_dec_mode(25, true);
        assert!(
            state.is_dec_mode(25),
            "mode 25 should be set after set(true)"
        );
        state.set_dec_mode(25, false);
        assert!(
            !state.is_dec_mode(25),
            "mode 25 should be unset after set(false)"
        );
    }

    #[test]
    fn set_dec_mode_multiple_independent() {
        let mut state = TerminalState::new(24, 80);
        state.set_dec_mode(6, true);
        state.set_dec_mode(7, true);
        state.set_dec_mode(25, true);
        assert!(state.is_dec_mode(6), "mode 6 should be set");
        assert!(state.is_dec_mode(7), "mode 7 should be set");
        assert!(state.is_dec_mode(25), "mode 25 should be set");
        state.set_dec_mode(6, false);
        assert!(!state.is_dec_mode(6), "mode 6 should be unset");
        assert!(state.is_dec_mode(7), "mode 7 should still be set");
        assert!(state.is_dec_mode(25), "mode 25 should still be set");
    }

    #[test]
    fn set_dec_mode_idempotent() {
        let mut state = TerminalState::new(24, 80);
        state.set_dec_mode(7, true);
        state.set_dec_mode(7, true);
        state.set_dec_mode(7, true);
        let count = state.dec_modes.iter().filter(|&&m| m == 7).count();
        assert_eq!(
            count, 1,
            "setting mode 7 three times should produce exactly one entry"
        );
        state.set_dec_mode(7, false);
        assert!(!state.is_dec_mode(7), "mode 7 should be unset");
    }

    #[test]
    fn no_op_stubs_do_not_panic() {
        let mut state = TerminalState::new(24, 80);
        state.insert_characters(5);
        state.delete_characters(3);
        state.erase_characters(7);
        state.repeat_character(2);
        state.scroll_up(3, 24);
        state.scroll_down(2, 24);
        state.insert_lines(1, 24);
        state.delete_lines(1, 24);
    }

    #[test]
    fn erase_in_display_does_not_change_cursor() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 12;
        state.cursor_col = 40;
        for mode in 0..=3u8 {
            state.erase_in_display(mode, 24, 80);
            assert_eq!(
                state.cursor_row, 12,
                "erase_in_display mode {} should not change cursor_row",
                mode
            );
            assert_eq!(
                state.cursor_col, 40,
                "erase_in_display mode {} should not change cursor_col",
                mode
            );
        }
    }

    #[test]
    fn erase_in_line_does_not_change_cursor() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 12;
        state.cursor_col = 40;
        for mode in 0..=2u8 {
            state.erase_in_line(mode);
            assert_eq!(
                state.cursor_row, 12,
                "erase_in_line mode {} should not change cursor_row",
                mode
            );
            assert_eq!(
                state.cursor_col, 40,
                "erase_in_line mode {} should not change cursor_col",
                mode
            );
        }
    }

    #[test]
    fn cursor_up_moves_correctly() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 15;
        state.cursor_up(3, 24);
        assert_eq!(state.cursor_row, 12, "should move up by 3");
    }

    #[test]
    fn cursor_down_moves_correctly() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 15;
        state.cursor_down(3, 24);
        assert_eq!(state.cursor_row, 18, "should move down by 3");
    }

    #[test]
    fn cursor_down_does_not_overflow_u16() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 23;
        state.cursor_down(u16::MAX, 24);
        assert_eq!(
            state.cursor_row, 23,
            "saturating_add prevents u16 overflow panic and clamps to last row"
        );
    }

    #[test]
    fn cursor_forward_does_not_overflow_u16() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_col = 79;
        state.cursor_forward(u16::MAX, 80);
        assert_eq!(
            state.cursor_col, 79,
            "saturating_add prevents u16 overflow panic and clamps to last col"
        );
    }

    #[test]
    fn cursor_next_line_does_not_overflow_u16() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 23;
        state.cursor_next_line(u16::MAX, 24);
        assert_eq!(
            state.cursor_row, 23,
            "saturating_add prevents u16 overflow panic and clamps to last row"
        );
        assert_eq!(state.cursor_col, 0, "col resets to 0");
    }

    #[test]
    fn cursor_position_sets_exact_location() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_position(5, 10, 24, 80);
        assert_eq!(state.cursor_row, 4, "1-indexed row 5 → 0-indexed 4");
        assert_eq!(state.cursor_col, 9, "1-indexed col 10 → 0-indexed 9");
    }

    #[test]
    fn cursor_position_clamps() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_position(200, 200, 24, 80);
        assert_eq!(state.cursor_row, 23, "should clamp row to rows-1");
        assert_eq!(state.cursor_col, 79, "should clamp col to cols-1");
    }

    #[test]
    fn set_scrolling_region_stores_values() {
        let mut state = TerminalState::new(24, 80);
        state.set_scrolling_region(5, 20);
        assert_eq!(state.scrolling_region, Some((5, 20)));
    }

    #[test]
    fn reset_scrolling_region_clears() {
        let mut state = TerminalState::new(24, 80);
        state.set_scrolling_region(5, 20);
        state.reset_scrolling_region();
        assert_eq!(state.scrolling_region, None);
    }

    #[test]
    fn cursor_forward_beyond_end_wraps_no_further() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_col = 75;
        state.cursor_forward(5, 80);
        assert_eq!(state.cursor_col, 79, "forward past end clamps to cols-1");
    }

    #[test]
    fn cursor_horizontal_tab_back_at_zero_stays() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_col = 0;
        state.cursor_horizontal_tab_back(80);
        assert_eq!(
            state.cursor_col, 0,
            "tab back at col 0 should not underflow"
        );
    }

    #[test]
    fn set_title_empty_string() {
        let mut state = TerminalState::new(24, 80);
        state.set_title("");
        assert_eq!(state.title.as_deref(), Some(""));
        state.set_title("real title");
        assert_eq!(state.title.as_deref(), Some("real title"));
    }

    #[test]
    fn set_title_unicode() {
        let mut state = TerminalState::new(24, 80);
        state.set_title("终端标题 🖥️");
        assert_eq!(state.title.as_deref(), Some("终端标题 🖥️"));
    }

    #[test]
    fn scrolling_region_boundary_values() {
        let mut state = TerminalState::new(24, 80);
        state.set_scrolling_region(1, 1);
        assert_eq!(state.scrolling_region, Some((1, 1)));
        state.set_scrolling_region(24, 24);
        assert_eq!(state.scrolling_region, Some((24, 24)));
    }

    #[test]
    fn dec_mode_set_toggle_repeatedly() {
        let mut state = TerminalState::new(24, 80);
        for _ in 0..100 {
            state.set_dec_mode(7, true);
            state.set_dec_mode(7, false);
        }
        assert!(
            !state.is_dec_mode(7),
            "after 100 toggles off, should be off"
        );
        let count = state.dec_modes.iter().filter(|&&m| m == 7).count();
        assert_eq!(count, 0, "should have zero entries after all toggles off");
    }

    #[test]
    fn cursor_next_line_preserves_existing_position_on_overflow() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 23;
        state.cursor_col = 40;
        state.cursor_next_line(1, 24);
        assert_eq!(state.cursor_row, 23, "should not overflow past last row");
        assert_eq!(state.cursor_col, 0, "should still reset col to 0");
    }

    #[test]
    fn cursor_prev_line_large_count_clamps() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 3;
        state.cursor_prev_line(100, 24);
        assert_eq!(state.cursor_row, 0, "should clamp to 0 with large count");
    }

    #[test]
    fn apply_sgr_multiple_batches_accumulate() {
        use crate::sgr::SgrAttribute;
        let mut state = TerminalState::new(24, 80);
        state.apply_sgr(&[SgrAttribute::Bold(true)]);
        assert!(matches!(state.sgr_attributes[0], SgrAttribute::Bold(true)));
        state.apply_sgr(&[SgrAttribute::Italic(true)]);
        assert_eq!(
            state.sgr_attributes.len(),
            2,
            "SGR accumulates across calls"
        );
        assert!(matches!(state.sgr_attributes[0], SgrAttribute::Bold(true)));
        assert!(matches!(
            state.sgr_attributes[1],
            SgrAttribute::Italic(true)
        ));
        state.apply_sgr(&[SgrAttribute::Underline(crate::sgr::UnderlineStyle::Single)]);
        assert_eq!(
            state.sgr_attributes.len(),
            3,
            "each call adds, not replaces"
        );
        state.apply_sgr(&[]);
        assert!(
            state.sgr_attributes.is_empty(),
            "empty slice clears attributes"
        );
    }

    #[test]
    fn cursor_position_zero_params_default() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_position(0, 0, 24, 80);
        assert_eq!(state.cursor_row, 0, "row 0 should stay 0");
        assert_eq!(state.cursor_col, 0, "col 0 should stay 0");
    }

    #[test]
    fn apply_sgr_accumulates_attributes() {
        use crate::sgr::SgrAttribute;
        let mut state = TerminalState::new(24, 80);
        state.apply_sgr(&[SgrAttribute::Bold(true)]);
        assert_eq!(
            state.sgr_attributes.len(),
            1,
            "should have 1 attribute after first call"
        );
        state.apply_sgr(&[SgrAttribute::Italic(true)]);
        assert_eq!(
            state.sgr_attributes.len(),
            2,
            "should have 2 attributes after accumulation"
        );
        state.apply_sgr(&[]);
        assert!(
            state.sgr_attributes.is_empty(),
            "empty apply_sgr should clear"
        );
    }

    #[test]
    fn cursor_forward_and_back_roundtrip() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_col = 40;
        state.cursor_forward(10, 80);
        assert_eq!(state.cursor_col, 50);
        state.cursor_back(10);
        assert_eq!(state.cursor_col, 40, "forward+back should return to start");
    }

    #[test]
    fn cursor_up_down_roundtrip() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 15;
        state.cursor_down(5, 24);
        assert_eq!(state.cursor_row, 20);
        state.cursor_up(5, 24);
        assert_eq!(state.cursor_row, 15, "down+up should return to start");
    }

    #[test]
    fn cursor_next_prev_line_roundtrip() {
        let mut state = TerminalState::new(24, 80);
        state.cursor_row = 10;
        state.cursor_col = 50;
        state.cursor_next_line(1, 24);
        assert_eq!(state.cursor_row, 11);
        assert_eq!(state.cursor_col, 0, "next_line resets col");
        state.cursor_prev_line(1, 24);
        assert_eq!(state.cursor_row, 10, "prev_line returns to row");
    }
}
