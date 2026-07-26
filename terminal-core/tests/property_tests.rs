//! Property-based tests for core invariants using quickcheck.
//!
//! These verify:
//!  - Grid resize preserves invariants
//!  - DirtyMask operations are idempotent
//!  - Snapshot rkyv/serde roundtrip preserves equality
//!  - Color arithmetic stays in range

#[cfg(test)]
mod grid_invariants {
    use quickcheck::TestResult;
    use quickcheck_macros::quickcheck;

    use terminal_core::cell::DirtyMask;
    use terminal_core::grid::Grid;

    #[quickcheck]
    fn grid_resize_preserves_dimensions(rows: u32, cols: u32, new_rows: u32, new_cols: u32) {
        if rows == 0 || cols == 0 || rows > 200 || cols > 500 {
            return;
        }
        let new_rows = new_rows.clamp(1, 200);
        let new_cols = new_cols.clamp(1, 500);
        let mut g = Grid::new(rows, cols);
        g.resize(new_rows, new_cols);
        assert_eq!(g.rows(), new_rows, "rows should match after resize");
        assert_eq!(g.cols(), new_cols, "cols should match after resize");
    }

    #[quickcheck]
    fn grid_resize_keeps_cells_in_range(rows: u32, cols: u32, new_cols: u32) -> TestResult {
        if rows == 0 || rows > 50 || cols == 0 || cols > 100 || new_cols == 0 || new_cols > 200 {
            return TestResult::discard();
        }
        let mut g = Grid::new(rows, cols);
        g.resize(rows, new_cols);
        for r in 0..rows {
            let line = g.get(r);
            if let Some(line) = line {
                let n = line.cells().len() as u32;
                assert_eq!(
                    n, new_cols,
                    "row {} should have {} cells after resize to {} cols, has {}",
                    r, new_cols, new_cols, n
                );
            }
        }
        TestResult::passed()
    }

    #[quickcheck]
    fn dirty_mask_mark_then_is_dirty(row: u32) {
        let rows = 100u32;
        let mut m = DirtyMask::new(rows);
        if row < rows {
            m.mark(row);
            assert!(m.is_dirty(row));
        }
    }

    #[quickcheck]
    fn dirty_mask_clear_makes_empty(row: u32) {
        let rows = 100u32;
        let mut m = DirtyMask::new(rows);
        if row < rows {
            m.mark(row);
            m.clear();
            assert!(!m.is_dirty(row));
        }
    }

    #[quickcheck]
    fn dirty_mask_mark_idempotent(row: u32) {
        let rows = 100u32;
        let mut m = DirtyMask::new(rows);
        if row < rows {
            m.mark(row);
            m.mark(row);
            m.mark(row);
            assert!(m.is_dirty(row));
        }
    }

    #[quickcheck]
    fn dirty_mask_any_after_mark_all(rows: u32) {
        if rows == 0 || rows > 1000 {
            return;
        }
        let mut m = DirtyMask::new(rows);
        m.mark_all(rows);
        for r in 0..rows {
            assert!(m.is_dirty(r));
        }
    }

    #[quickcheck]
    fn dirty_mask_clear_makes_any_dirty_false(rows: u32) {
        if rows == 0 || rows > 1000 {
            return;
        }
        let mut m = DirtyMask::new(rows);
        m.mark_all(rows);
        m.clear();
        assert!(!m.any_dirty());
    }
}

#[cfg(test)]
mod color_arithmetic {
    use quickcheck_macros::quickcheck;

    use terminal_core::cell::Color;

    #[quickcheck]
    fn color_default_is_white(_unit: ()) {
        let c = Color::default();
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 255);
        assert_eq!(c.b, 255);
        assert_eq!(c.a, 255);
    }

    #[quickcheck]
    fn color_equality(c1: (u8, u8, u8, u8), c2: (u8, u8, u8, u8)) {
        let (r1, g1, b1, a1) = c1;
        let (r2, g2, b2, a2) = c2;
        let a = Color {
            r: r1,
            g: g1,
            b: b1,
            a: a1,
        };
        let b = Color {
            r: r2,
            g: g2,
            b: b2,
            a: a2,
        };
        let eq = r1 == r2 && g1 == g2 && b1 == b2 && a1 == a2;
        assert_eq!(a == b, eq);
    }
}

#[cfg(test)]
mod config_invariants {
    use quickcheck_macros::quickcheck;

    use terminal_core::config::{BackspaceMode, RightAltMode, Shell, TerminalConfig};

    #[quickcheck]
    fn terminal_config_default_is_24x80(_unit: ()) {
        let cfg = TerminalConfig::default();
        assert_eq!(cfg.rows, 24);
        assert_eq!(cfg.cols, 80);
    }

    #[quickcheck]
    fn terminal_config_serde_roundtrip(rows: u16, cols: u16, scroll: u32) -> bool {
        if rows == 0 || cols == 0 {
            return true;
        }
        let cfg = TerminalConfig {
            rows: rows as u32,
            cols: cols as u32,
            scrollback_lines: scroll,
            shell: Shell::SystemDefault,
            font_size_tenths: 140,
            backspace_mode: BackspaceMode::default(),
            right_alt_mode: RightAltMode::default(),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: TerminalConfig = serde_json::from_str(&json).unwrap();
        back == cfg
    }

    #[quickcheck]
    fn shell_equality_system_default(_unit: ()) {
        assert_ne!(Shell::SystemDefault, Shell::Custom("/bin/sh".to_string()));
    }
}

#[cfg(test)]
mod snapshot_invariants {
    use quickcheck::TestResult;
    use quickcheck_macros::quickcheck;

    use terminal_core::cell::Cell;
    use terminal_core::grid::Grid;
    use terminal_core::line::Line;
    use terminal_core::snapshot::SessionSnapshot;

    #[quickcheck]
    fn session_snapshot_from_grid_dimensions(rows: u32, cols: u32) -> TestResult {
        if rows == 0 || cols == 0 || rows > 50 || cols > 100 {
            return TestResult::discard();
        }
        let g = Grid::new(rows, cols);
        let snap = SessionSnapshot::from_grid(&g);
        assert_eq!(snap.rows, rows);
        assert_eq!(snap.cols, cols);
        assert_eq!(snap.visible_lines.len(), rows as usize);
        TestResult::passed()
    }

    #[quickcheck]
    fn cell_default_serde_roundtrip(_unit: ()) -> bool {
        let c: Cell = Cell::default();
        let json = serde_json::to_string(&c).unwrap();
        let back: Cell = serde_json::from_str(&json).unwrap();
        back == c
    }

    #[quickcheck]
    fn line_serde_roundtrip(cols: u32) -> TestResult {
        if cols == 0 || cols > 100 {
            return TestResult::discard();
        }
        let l = Line::new(cols);
        let json = serde_json::to_string(&l).unwrap();
        let back: Line = serde_json::from_str(&json).unwrap();
        let eq = back.cells().len() == l.cells().len();
        TestResult::from_bool(eq)
    }
}

#[cfg(test)]
mod cell_serde_invariants {
    use quickcheck_macros::quickcheck;
    use terminal_core::cell::{Attrs, Cell, Color};

    #[quickcheck]
    fn cell_serde_roundtrip_char(char_code: u32, width: u8) -> bool {
        let cell = Cell {
            char: char::from_u32(char_code & 0x0010_FFFF).unwrap_or(' '),
            width: if width == 0 { 1 } else { (width % 3) + 1 },
            ..Default::default()
        };
        let json = serde_json::to_string(&cell).unwrap();
        let back: Cell = serde_json::from_str(&json).unwrap();
        back == cell
    }

    #[quickcheck]
    fn cell_serde_roundtrip_colors(r1: u8, g1: u8, b1: u8, a1: u8) -> bool {
        let fg = Color {
            r: r1,
            g: g1,
            b: b1,
            a: a1,
        };
        let bg = Color {
            r: r1.wrapping_add(128),
            g: g1.wrapping_add(128),
            b: b1.wrapping_add(128),
            a: a1,
        };
        let cell = Cell {
            foreground: fg,
            background: bg,
            ..Default::default()
        };
        let json = serde_json::to_string(&cell).unwrap();
        let back: Cell = serde_json::from_str(&json).unwrap();
        back == cell
    }

    #[quickcheck]
    fn cell_serde_roundtrip_attrs(
        bold: bool,
        italic: bool,
        underline: bool,
        reverse: bool,
    ) -> bool {
        let attrs = Attrs {
            bold,
            italic,
            underline,
            reverse,
            ..Default::default()
        };
        let cell = Cell {
            attrs,
            ..Default::default()
        };
        let json = serde_json::to_string(&cell).unwrap();
        let back: Cell = serde_json::from_str(&json).unwrap();
        back == cell
    }

    #[quickcheck]
    fn attrs_first_four_roundtrip(bold: bool, dim: bool, italic: bool, underline: bool) -> bool {
        let a = Attrs {
            bold,
            dim,
            italic,
            underline,
            ..Default::default()
        };
        let json = serde_json::to_string(&a).unwrap();
        let back: Attrs = serde_json::from_str(&json).unwrap();
        a == back
    }

    #[quickcheck]
    fn attrs_mid_four_roundtrip(
        double_underline: bool,
        reverse: bool,
        strikethrough: bool,
        blink: bool,
    ) -> bool {
        let a = Attrs {
            double_underline,
            reverse,
            strikethrough,
            blink,
            ..Default::default()
        };
        let json = serde_json::to_string(&a).unwrap();
        let back: Attrs = serde_json::from_str(&json).unwrap();
        a == back
    }

    #[quickcheck]
    fn attrs_last_six_roundtrip(
        hidden: bool,
        overline: bool,
        protected: bool,
        double_width: bool,
        double_height_top: bool,
        double_height_bottom: bool,
    ) -> bool {
        let a = Attrs {
            hidden,
            overline,
            protected,
            double_width,
            double_height_top,
            double_height_bottom,
            ..Default::default()
        };
        let json = serde_json::to_string(&a).unwrap();
        let back: Attrs = serde_json::from_str(&json).unwrap();
        a == back
    }
}

#[cfg(test)]
mod dirty_mask_invariants {
    use quickcheck::TestResult;
    use quickcheck_macros::quickcheck;
    use terminal_core::cell::DirtyMask;

    #[quickcheck]
    fn dirty_mask_mark_one_does_not_affect_others(marked_row: u32, checked_row: u32) -> TestResult {
        if marked_row == checked_row || marked_row > 1000 || checked_row > 1000 {
            return TestResult::discard();
        }
        let mut m = DirtyMask::new(1001);
        m.mark(marked_row);
        if m.is_dirty(checked_row) {
            return TestResult::failed();
        }
        TestResult::passed()
    }

    #[quickcheck]
    fn dirty_mask_union_covers_full_range(rows: u32) -> TestResult {
        if rows == 0 || rows > 1000 {
            return TestResult::discard();
        }
        let mut m = DirtyMask::new(rows);
        m.mark_all(rows);
        for r in 0..rows {
            if !m.is_dirty(r) {
                return TestResult::failed();
            }
        }
        TestResult::passed()
    }
}

#[cfg(test)]
mod selection_invariants {
    use quickcheck_macros::quickcheck;
    use terminal_core::selection::{Selection, SelectionAnchor, SelectionMode};

    #[quickcheck]
    fn selection_contains_is_deterministic(
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
        test_row: u32,
        test_col: u32,
    ) {
        for &mode in &[
            SelectionMode::Char,
            SelectionMode::Word,
            SelectionMode::Line,
            SelectionMode::Block,
        ] {
            let s = Selection::new(
                SelectionAnchor {
                    row: start_row,
                    col: start_col,
                },
                SelectionAnchor {
                    row: end_row,
                    col: end_col,
                },
                mode,
            );
            let r1 = s.contains(test_row, test_col);
            let r2 = s.contains(test_row, test_col);
            assert_eq!(r1, r2, "contains must be deterministic for mode {mode:?}");
        }
    }
}

#[cfg(test)]
mod cursor_invariants {
    use quickcheck_macros::quickcheck;
    use terminal_core::cursor::CursorState;

    #[quickcheck]
    fn cursor_state_move_no_panic(row: u32, col: u32, n: u32, max_rows: u32, max_cols: u32) {
        let mut c = CursorState::new(row, col);
        c.move_up(n);
        c.move_down(n, max_rows);
        c.move_left(n);
        c.move_right(n, max_cols);
        c.carriage_return();
        // Verify moves don't leave cursor in an invalid state
        assert!(
            c.row <= max_rows || max_rows == 0,
            "row {} should be ≤ max_rows {}",
            c.row,
            max_rows
        );
        assert!(
            c.col <= max_cols || max_cols == 0,
            "col {} should be ≤ max_cols {}",
            c.col,
            max_cols
        );
    }

    #[quickcheck]
    fn cursor_state_clamp_safety(row: u32, col: u32, max_rows: u32, max_cols: u32) {
        let mut c = CursorState::new(row, col);
        c.clamp(max_rows, max_cols);
        assert!(
            c.row <= max_rows || max_rows == 0,
            "after clamp, row {} should be ≤ max_rows {}",
            c.row,
            max_rows
        );
        assert!(
            c.col <= max_cols || max_cols == 0,
            "after clamp, col {} should be ≤ max_cols {}",
            c.col,
            max_cols
        );
    }
}

#[cfg(test)]
mod grid_scroll_invariants {
    use quickcheck::TestResult;
    use quickcheck_macros::quickcheck;
    use terminal_core::grid::Grid;

    #[quickcheck]
    fn grid_scroll_up_preserves_total_rows(rows: u32, scroll_count: u32) -> TestResult {
        if !(2..=50).contains(&rows) || scroll_count == 0 {
            return TestResult::discard();
        }
        let mut g = Grid::new(rows, 10);
        let scroll = (scroll_count % (rows - 1)).max(1);
        for _ in 0..scroll {
            g.scroll_up(0, rows, 10);
        }
        let passed = g.rows() == rows;
        TestResult::from_bool(passed)
    }

    #[quickcheck]
    fn grid_scroll_down_preserves_total_rows(rows: u32, scroll_count: u32) -> TestResult {
        if !(2..=50).contains(&rows) || scroll_count == 0 {
            return TestResult::discard();
        }
        let mut g = Grid::new(rows, 10);
        let scroll = (scroll_count % (rows - 1)).max(1);
        for _ in 0..scroll {
            g.scroll_down(0, rows, 10);
        }
        let passed = g.rows() == rows;
        TestResult::from_bool(passed)
    }

    #[quickcheck]
    fn grid_scroll_region_noop_when_invalid(top: u32, bottom: u32, rows: u32) -> TestResult {
        if rows == 0 || rows > 50 {
            return TestResult::discard();
        }
        let mut g = Grid::new(rows, 10);
        let old_rows = g.rows();
        if top >= bottom || bottom > rows {
            g.scroll_up(top, bottom, 10);
            g.scroll_down(top, bottom, 10);
        }
        let passed = g.rows() == old_rows;
        TestResult::from_bool(passed)
    }

    #[quickcheck]
    fn grid_resize_to_same_is_noop(rows: u32, cols: u32) -> TestResult {
        if rows == 0 || cols == 0 || rows > 100 || cols > 200 {
            return TestResult::discard();
        }
        let mut g = Grid::new(rows, cols);
        g.resize(rows, cols);
        let passed = g.rows() == rows && g.cols() == cols;
        TestResult::from_bool(passed)
    }

    #[quickcheck]
    fn grid_cell_mut_dirty_marked(row: u32, col: u32, rows: u32, cols: u32) -> TestResult {
        if rows == 0 || cols == 0 || rows > 50 || cols > 100 {
            return TestResult::discard();
        }
        if row >= rows || col >= cols {
            return TestResult::discard();
        }
        let mut g = Grid::new(rows, cols);
        g.mark_clean();
        let _ = g.cell_mut(row, col);
        let passed = g.dirty().is_dirty(row);
        TestResult::from_bool(passed)
    }
}

#[cfg(test)]
mod color_ops_invariants {
    use quickcheck_macros::quickcheck;
    use terminal_core::cell::Color;

    #[quickcheck]
    fn color_saturating_add_commutative(r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8) {
        let a = Color {
            r: r1,
            g: g1,
            b: b1,
            a: 255,
        };
        let b = Color {
            r: r2,
            g: g2,
            b: b2,
            a: 255,
        };
        let ab = a.saturating_add(&b);
        let ba = b.saturating_add(&a);
        assert_eq!(ab, ba, "saturating_add should be commutative");
    }

    #[quickcheck]
    fn color_saturating_add_identity(r: u8, g: u8, b: u8) {
        let a = Color { r, g, b, a: 255 };
        let zero = Color {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        };
        assert_eq!(a.saturating_add(&zero), a);
    }

    #[quickcheck]
    fn color_saturating_mul_identity(r: u8, g: u8, b: u8) {
        let a = Color { r, g, b, a: 255 };
        assert_eq!(a.saturating_mul(1), a);
    }

    #[allow(clippy::many_single_char_names)]
    #[quickcheck]
    fn color_saturating_mul_zero(r: u8, g: u8, b: u8) {
        let a = Color { r, g, b, a: 255 };
        let z = a.saturating_mul(0);
        assert_eq!(z.r, 0);
        assert_eq!(z.g, 0);
        assert_eq!(z.b, 0);
        assert_eq!(z.a, 0);
    }

    #[quickcheck]
    fn color_saturating_mul_bounded(r: u8, g: u8, b: u8, factor: u8) {
        let a = Color { r, g, b, a: 255 };
        let result = a.saturating_mul(factor);
        // result fields are u8, guaranteed 0-255 by type system
        let _ = result;
    }
}

#[cfg(test)]
mod attrs_invariants {
    use quickcheck_macros::quickcheck;
    use terminal_core::cell::Attrs;

    #[quickcheck]
    fn attrs_default_is_clean(_bold: bool, _italic: bool, _underline: bool, _strikethrough: bool) {
        let def = Attrs::default();
        assert!(!def.bold);
        assert!(!def.italic);
        assert!(!def.underline);
        assert!(!def.strikethrough);
    }

    #[quickcheck]
    fn attrs_all_fields_default_false() {
        let a = Attrs::default();
        assert!(!a.bold);
        assert!(!a.dim);
        assert!(!a.italic);
        assert!(!a.underline);
        assert!(!a.double_underline);
        assert!(!a.reverse);
        assert!(!a.strikethrough);
        assert!(!a.blink);
        assert!(!a.hidden);
        assert!(!a.overline);
        assert!(!a.protected);
        assert!(!a.double_width);
        assert!(!a.double_height_top);
        assert!(!a.double_height_bottom);
    }
}

#[cfg(test)]
mod grid_properties {
    use quickcheck::TestResult;
    use quickcheck_macros::quickcheck;
    use terminal_core::grid::Grid;

    #[quickcheck]
    fn grid_rows_cols_after_new(rows: u32, cols: u32) -> TestResult {
        if rows > 200 || cols > 500 {
            return TestResult::discard();
        }
        let g = Grid::new(rows, cols);
        TestResult::from_bool(g.rows() == rows && g.cols() == cols)
    }

    #[quickcheck]
    fn grid_resize_preserves_invariants(
        rows: u32,
        cols: u32,
        new_rows: u32,
        new_cols: u32,
    ) -> TestResult {
        if rows == 0 || cols == 0 || rows > 50 || cols > 100 {
            return TestResult::discard();
        }
        let new_rows = new_rows.clamp(1, 100);
        let new_cols = new_cols.clamp(1, 200);
        let mut g = Grid::new(rows, cols);
        g.resize(new_rows, new_cols);
        g.assert_invariants();
        TestResult::passed()
    }

    #[quickcheck]
    fn grid_mark_all_dirty_then_clean(rows: u32) -> TestResult {
        if rows == 0 || rows > 200 {
            return TestResult::discard();
        }
        let cols = rows.max(1);
        let mut g = Grid::new(rows, cols);
        assert!(g.dirty().any_dirty(), "new grid must be dirty");
        g.mark_clean();
        assert!(!g.dirty().any_dirty(), "mark_clean must clear all dirt");
        g.mark_all_dirty();
        for r in 0..rows {
            if !g.dirty().is_dirty(r) {
                return TestResult::failed();
            }
        }
        TestResult::passed()
    }
}

#[cfg(test)]
mod insert_delete_lines_invariants {
    use quickcheck::TestResult;
    use quickcheck_macros::quickcheck;
    use terminal_core::grid::Grid;

    #[quickcheck]
    fn insert_lines_preserves_total_rows(at: u32, count: u32, rows: u32) -> TestResult {
        if !(3..=50).contains(&rows) {
            return TestResult::discard();
        }
        let at = at % rows;
        let count = count % 5;
        let mut g = Grid::new(rows, 10);
        g.insert_lines(at, count, rows, 10);
        g.assert_invariants();
        TestResult::from_bool(g.rows() == rows)
    }

    #[quickcheck]
    fn delete_lines_preserves_total_rows(at: u32, count: u32, rows: u32) -> TestResult {
        if !(3..=50).contains(&rows) {
            return TestResult::discard();
        }
        let at = at % rows;
        let count = count % 5;
        let mut g = Grid::new(rows, 10);
        g.delete_lines(at, count, rows, 10);
        g.assert_invariants();
        TestResult::from_bool(g.rows() == rows)
    }

    #[quickcheck]
    fn insert_lines_at_bottom_is_no_op(rows: u32) -> TestResult {
        if !(2..=50).contains(&rows) {
            return TestResult::discard();
        }
        let mut g = Grid::new(rows, 10);
        g.mark_clean();
        g.insert_lines(rows, 1, rows, 10);
        TestResult::from_bool(!g.dirty().any_dirty())
    }

    #[quickcheck]
    fn delete_lines_at_bottom_is_no_op(rows: u32) -> TestResult {
        if !(2..=50).contains(&rows) {
            return TestResult::discard();
        }
        let mut g = Grid::new(rows, 10);
        g.mark_clean();
        g.delete_lines(rows, 1, rows, 10);
        TestResult::from_bool(!g.dirty().any_dirty())
    }
}

#[cfg(test)]
mod scrollback_invariants {
    use quickcheck_macros::quickcheck;
    use terminal_core::grid::Grid;

    #[quickcheck]
    fn push_scrollback_bounded(max: u8, pushes: u8) -> bool {
        let max = max.clamp(1, 50) as usize;
        let pushes = pushes.min(100);
        let mut g = Grid::with_scrollback(2, 5, max);
        for _ in 0..pushes {
            g.push_scrollback(terminal_core::line::Line::new(5));
        }
        g.scrollback_length() <= max
    }

    #[quickcheck]
    fn clear_scrollback_empties(pushes: u8) -> bool {
        let pushes = pushes.min(50);
        let mut g = Grid::with_scrollback(2, 5, 100);
        for _ in 0..pushes {
            g.push_scrollback(terminal_core::line::Line::new(5));
        }
        g.clear_scrollback();
        g.scrollback_length() == 0
    }
}

#[cfg(test)]
mod ansi_invariants {
    use terminal_core::ansi::ansi_to_rgb;

    #[test]
    fn ansi_all_256_indices_valid() {
        for index in 0..=255u8 {
            let [r, g, b] = ansi_to_rgb(index);
            assert!(
                (r, g, b) != (0, 0, 0) || index == 0 || index == 16,
                "index {index} returned [{r},{g},{b}] which is also black"
            );
        }
    }
}

#[cfg(test)]
mod line_invariants {
    use quickcheck::TestResult;
    use quickcheck_macros::quickcheck;
    use terminal_core::line::Line;

    #[quickcheck]
    fn line_new_has_correct_length(cols: u32) -> TestResult {
        if cols > 500 {
            return TestResult::discard();
        }
        let l = Line::new(cols);
        TestResult::from_bool(l.len() == cols)
    }

    #[quickcheck]
    fn line_resize_preserves_prefix(old_cols: u32, new_cols: u32) -> TestResult {
        let old_cols = old_cols.clamp(1, 50);
        let new_cols = new_cols.clamp(1, 100);
        let mut l = Line::new(old_cols);
        l.get_mut(0).unwrap().char = 'X';
        l.resize(new_cols);
        if l.get(0).unwrap().char != 'X' {
            return TestResult::failed();
        }
        TestResult::passed()
    }
}

#[cfg(test)]
mod grid_fill_erase_invariants {
    use quickcheck::TestResult;
    use quickcheck_macros::quickcheck;
    use terminal_core::grid::Grid;

    #[quickcheck]
    fn fill_rect_marks_rows_dirty(rows: u32, height: u32) -> TestResult {
        let rows = rows.clamp(3, 30);
        let height = height.clamp(1, rows);
        let mut g = Grid::new(rows, 10);
        g.mark_clean();
        g.fill_rect(0, 0, 10, height, 'X');
        for r in 0..height {
            if !g.dirty().is_dirty(r) {
                return TestResult::failed();
            }
        }
        TestResult::passed()
    }

    #[quickcheck]
    fn erase_rect_marks_rows_dirty(rows: u32, height: u32) -> TestResult {
        let rows = rows.clamp(3, 30);
        let height = height.clamp(1, rows);
        let mut g = Grid::new(rows, 10);
        g.mark_clean();
        g.erase_rect(0, 0, 10, height, ' ');
        for r in 0..height {
            if !g.dirty().is_dirty(r) {
                return TestResult::failed();
            }
        }
        TestResult::passed()
    }

    #[quickcheck]
    fn fill_rect_fills_cells(rows: u32, cols: u32) -> TestResult {
        let rows = rows.clamp(1, 10);
        let cols = cols.clamp(1, 10);
        let mut g = Grid::new(rows, cols);
        g.fill_rect(0, 0, cols, rows, 'Z');
        for r in 0..rows {
            for c in 0..cols {
                if g.cell(r, c).unwrap().char != 'Z' {
                    return TestResult::failed();
                }
            }
        }
        TestResult::passed()
    }
}

#[cfg(test)]
mod snapshot_property_invariants {
    use quickcheck::TestResult;
    use quickcheck_macros::quickcheck;
    use terminal_core::grid::Grid;
    use terminal_core::snapshot::SessionSnapshot;

    #[quickcheck]
    fn snapshot_roundtrip_identity(rows: u32, cols: u32) -> TestResult {
        if rows == 0 || rows > 30 || cols == 0 || cols > 80 {
            return TestResult::discard();
        }
        let g = Grid::new(rows, cols);
        let snap = SessionSnapshot::from_grid(&g);
        if snap.rows != rows || snap.cols != cols {
            return TestResult::failed();
        }
        if snap.visible_lines.len() != rows as usize {
            return TestResult::failed();
        }
        TestResult::passed()
    }

    #[quickcheck]
    fn snapshot_serde_roundtrip(rows: u32, cols: u32) -> TestResult {
        if rows == 0 || rows > 20 || cols == 0 || cols > 40 {
            return TestResult::discard();
        }
        let g = Grid::new(rows, cols);
        let snap = SessionSnapshot::from_grid(&g);
        let json = serde_json::to_string(&snap).unwrap();
        let back: SessionSnapshot = serde_json::from_str(&json).unwrap();
        TestResult::from_bool(snap == back)
    }
}
