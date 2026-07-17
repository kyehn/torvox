//! Verus formal verification annotations for torvox-core.
//!
//! Only compiled under `#[cfg(verus_only)]` (i.e. during `cargo verus verify`).
//! Normal `cargo build` / `cargo test` skips this module entirely.

use crate::cell::Color;
use crate::cursor::CursorState;
use crate::selection::{Selection, SelectionMode};

verus! {

/// Verify that `Color::saturating_add` computes component-wise saturating
/// u8 addition and that every component of the result is ≤ 255.
pub exec fn verify_color_saturating_add() {
    let a = Color { r: 100, g: 200, b: 50, a: 10 };
    let b = Color { r: 200, g: 100, b: 50, a: 10 };
    let c = a.saturating_add(&b);
    assert(c.r == 255);  // 100+200 saturates
    assert(c.g == 255);  // 200+100 saturates
    assert(c.b == 100);  // 50+50 = 100
    assert(c.a == 20);   // 10+10 = 20
    assert(c.r <= 255 && c.g <= 255 && c.b <= 255 && c.a <= 255);
}

/// Verify that saturating_add is commutative: a+b == b+a.
pub exec fn verify_color_saturating_add_commutative() {
    let a = Color { r: 100, g: 200, b: 50, a: 10 };
    let b = Color { r: 200, g: 100, b: 50, a: 10 };
    let ab = a.saturating_add(&b);
    let ba = b.saturating_add(&a);
    assert(ab == ba);
}

/// Verify that `Color::saturating_mul` is bounded.
pub exec fn verify_color_saturating_mul() {
    let a = Color { r: 100, g: 200, b: 50, a: 10 };
    let c = a.saturating_mul(2);
    assert(c.r == 200);
    assert(c.g == 255);  // saturates
    assert(c.b == 100);
    assert(c.a == 20);
    assert(c.r <= 255 && c.g <= 255 && c.b <= 255 && c.a <= 255);
}

/// Verify that saturating_mul with scalar 1 is identity.
pub exec fn verify_color_saturating_mul_identity() {
    let a = Color { r: 100, g: 200, b: 50, a: 10 };
    let c = a.saturating_mul(1);
    assert(c == a);
}

/// Verify that `CursorState::clamp` keeps row and col within bounds.
pub exec fn verify_cursor_clamp_bounds() {
    let mut c = CursorState { row: 100, col: 200 };
    let max_rows = 24u32;
    let max_cols = 80u32;
    c.clamp(max_rows, max_cols);
    assert(c.row < max_rows);
    assert(c.col < max_cols);
}

/// Verify that clamp with zero bounds saturates to zero.
pub exec fn verify_cursor_clamp_zero_bounds() {
    let mut c = CursorState { row: 5, col: 5 };
    c.clamp(0, 0);
    assert(c.row == 0);
    assert(c.col == 0);
}

/// Verify that `Selection::is_ordered` returns true when start < end.
pub exec fn verify_selection_is_ordered_forward() {
    let sel = Selection {
        start: crate::selection::SelectionAnchor { row: 1, col: 5 },
        end: crate::selection::SelectionAnchor { row: 3, col: 2 },
        mode: SelectionMode::Char,
    };
    assert(sel.is_ordered());
}

/// Verify that `Selection::is_ordered` returns false when start > end.
pub exec fn verify_selection_is_ordered_reverse() {
    let sel = Selection {
        start: crate::selection::SelectionAnchor { row: 5, col: 2 },
        end: crate::selection::SelectionAnchor { row: 3, col: 2 },
        mode: SelectionMode::Char,
    };
    assert(!sel.is_ordered());
}

/// Verify that `Selection::contains` returns true for a point inside the
/// selection range in Char mode.
pub exec fn verify_selection_contains_in_range_char() {
    let sel = Selection {
        start: crate::selection::SelectionAnchor { row: 1, col: 5 },
        end: crate::selection::SelectionAnchor { row: 3, col: 10 },
        mode: SelectionMode::Char,
    };
    // A point strictly inside
    assert(sel.contains(2, 7));
    // Start point
    assert(sel.contains(1, 5));
    // End point
    assert(sel.contains(3, 10));
}

/// Verify that `Selection::contains` returns false for a point outside the
/// selection range in Char mode.
pub exec fn verify_selection_contains_out_of_range() {
    let sel = Selection {
        start: crate::selection::SelectionAnchor { row: 1, col: 5 },
        end: crate::selection::SelectionAnchor { row: 3, col: 10 },
        mode: SelectionMode::Char,
    };
    // Before start
    assert(!sel.contains(0, 0));
    // After end
    assert(!sel.contains(5, 5));
}

/// Verify Line mode contains behavior.
pub exec fn verify_selection_contains_line_mode() {
    let sel = Selection {
        start: crate::selection::SelectionAnchor { row: 2, col: 0 },
        end: crate::selection::SelectionAnchor { row: 4, col: 0 },
        mode: SelectionMode::Line,
    };
    assert(sel.contains(2, 50));
    assert(sel.contains(3, 10));
    assert(sel.contains(4, 99));
    assert(!sel.contains(1, 0));
    assert(!sel.contains(5, 0));
}

/// Verify Block mode contains behavior.
pub exec fn verify_selection_contains_block_mode() {
    let sel = Selection {
        start: crate::selection::SelectionAnchor { row: 1, col: 5 },
        end: crate::selection::SelectionAnchor { row: 3, col: 10 },
        mode: SelectionMode::Block,
    };
    assert(sel.contains(2, 7));
    assert(sel.contains(1, 5));
    assert(sel.contains(3, 10));
    assert(!sel.contains(0, 7));
    assert(!sel.contains(2, 3));
    assert(!sel.contains(2, 11));
}

} // verus!
