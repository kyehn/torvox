//! Cell, Color, Attrs, and DirtyMask — the terminal's atomic display unit.
//!
//! # Requirements
//! - [FR-004](crate) — ANSI color palette
//! - [FR-013](crate) — DirtyMask tracking
//! - [NFR-010](crate) — Rust nightly compatibility
use serde::{Deserialize, Serialize};

#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
/// A single terminal cell with character, colors, and attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    pub char: char,
    pub foreground: Color,
    pub background: Color,
    pub attrs: Attrs,
    pub width: u8,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            char: ' ',
            foreground: Color::default(),
            background: Color::default(),
            attrs: Attrs::default(),
            width: 1,
        }
    }
}

/// RGBA color value.
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Default for Color {
    fn default() -> Self {
        Self {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        }
    }
}

/// Text attributes (bold, italic, underline, etc.).
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Attrs {
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub double_underline: bool,
    pub reverse: bool,
    pub strikethrough: bool,
    pub blink: bool,
    pub hidden: bool,
    pub overline: bool,
    pub protected: bool,
    pub double_width: bool,
    pub double_height_top: bool,
    pub double_height_bottom: bool,
}

const BITS_PER_PARTITION: u32 = 64;

/// Bitmask tracking which rows need re-rendering.
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirtyMask {
    partitions: alloc::vec::Vec<u64>,
}

/// A bit-packed mask tracking which rows have been modified.
///
/// ```
/// use torvox_core::cell::DirtyMask;
///
/// let mut mask = DirtyMask::new(80);
/// assert!(!mask.any_dirty());
///
/// mask.mark(5);
/// assert!(mask.is_dirty(5));
/// assert!(!mask.is_dirty(0));
///
/// mask.clear();
/// assert!(!mask.any_dirty());
///
/// mask.mark_all(80);
/// assert!(mask.is_dirty(79));
/// ```
impl DirtyMask {
    pub fn new(total_rows: u32) -> Self {
        let num_partitions = (total_rows as usize).div_ceil(BITS_PER_PARTITION as usize);
        Self {
            partitions: alloc::vec![0u64; num_partitions.max(1)],
        }
    }

    pub fn is_dirty(&self, row: u32) -> bool {
        let (part, bit) = Self::partition_index(row);
        self.partitions
            .get(part)
            .is_some_and(|p| *p & (1 << bit) != 0)
    }

    pub fn mark(&mut self, row: u32) {
        let (part, bit) = Self::partition_index(row);
        if let Some(p) = self.partitions.get_mut(part) {
            *p |= 1 << bit;
        }
    }

    pub fn mark_all(&mut self, rows: u32) {
        let num_partitions = (rows as usize).div_ceil(BITS_PER_PARTITION as usize);
        self.partitions.clear();
        self.partitions.resize(num_partitions.max(1), !0u64);
        let remainder = rows % BITS_PER_PARTITION;
        if remainder != 0
            && let Some(last) = self.partitions.last_mut()
        {
            *last = (1u64 << remainder) - 1;
        }
    }

    pub fn clear(&mut self) {
        self.partitions.fill(0);
    }

    pub fn any_dirty(&self) -> bool {
        self.partitions.iter().any(|&p| p != 0)
    }

    pub fn resize(&mut self, total_rows: u32) {
        let num_partitions = (total_rows as usize).div_ceil(BITS_PER_PARTITION as usize);
        self.partitions.resize(num_partitions.max(1), 0);
    }

    fn partition_index(row: u32) -> (usize, u32) {
        let part = (row / BITS_PER_PARTITION) as usize;
        let bit = row % BITS_PER_PARTITION;
        (part, bit)
    }
}

/// Create a color from RGB components (alpha defaults to 255).
///
/// ```
/// use torvox_core::cell::Color;
///
/// let red = Color::new(255, 0, 0);
/// assert_eq!(red.r, 255);
/// assert_eq!(red.g, 0);
/// assert_eq!(red.b, 0);
/// assert_eq!(red.a, 255);
///
/// let a = Color::new(200, 100, 50);
/// let b = Color::new(100, 200, 50);
/// let c = a.saturating_add(&b);
/// assert_eq!(c, Color::new(255, 255, 100));
///
/// let d = Color::new(100, 200, 50).saturating_mul(2);
/// assert_eq!(d, Color::new(200, 255, 100));
/// ```
impl Color {
    /// Create a new opaque RGB color.
    pub fn new(red: u8, green: u8, blue: u8) -> Self {
        Self {
            r: red,
            g: green,
            b: blue,
            a: 255,
        }
    }

    /// Construct a Color from an ANSI 256-color palette index.
    pub fn from_ansi(index: u8) -> Self {
        let [r, g, b] = crate::ansi::ansi_to_rgb(index);
        Self { r, g, b, a: 255 }
    }

    /// Component-wise saturating addition of two colors.
    pub fn saturating_add(&self, other: &Color) -> Color {
        Color {
            r: self.r.saturating_add(other.r),
            g: self.g.saturating_add(other.g),
            b: self.b.saturating_add(other.b),
            a: self.a.saturating_add(other.a),
        }
    }

    /// Component-wise saturating multiplication by a scalar.
    pub fn saturating_mul(&self, scalar: u8) -> Color {
        Color {
            r: self.r.saturating_mul(scalar),
            g: self.g.saturating_mul(scalar),
            b: self.b.saturating_mul(scalar),
            a: self.a.saturating_mul(scalar),
        }
    }
}

/// A terminal cell with character, colors, and attributes.
///
/// Default cell is a space with white-on-white colors and no attributes.
///
/// ```
/// use torvox_core::cell::{Cell, Color};
///
/// let cell = Cell::default();
/// assert_eq!(cell.char, ' ');
/// assert_eq!(cell.foreground, Color::new(255, 255, 255));
/// assert_eq!(cell.background, Color::new(255, 255, 255));
/// assert_eq!(cell.width, 1);
///
/// let x = Cell::with_char('X');
/// assert_eq!(x.char, 'X');
/// assert_eq!(x.width, 1);
/// ```
impl Cell {
    pub fn with_char(character: char) -> Self {
        Self {
            char: character,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck_macros::quickcheck;

    #[test]
    fn cell_default_is_space() {
        let c = Cell::default();
        assert_eq!(c.char, ' ');
        assert_eq!(c.foreground, Color::default());
        assert_eq!(c.background, Color::default());
    }

    #[test]
    fn cell_with_char() {
        let c = Cell::with_char('X');
        assert_eq!(c.char, 'X');
        assert_eq!(c.foreground, Color::default());
    }

    #[test]
    fn color_new() {
        let c = Color::new(255, 128, 0);
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 128);
        assert_eq!(c.b, 0);
        assert_eq!(c.a, 255);
    }

    #[test]
    fn color_from_ansi() {
        let c = Color::from_ansi(1);
        assert_eq!(c.r, 128);
        assert_eq!(c.g, 0);
        assert_eq!(c.b, 0);
    }

    #[test]
    fn attrs_default_all_false() {
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
    }

    #[test]
    fn dirty_mask_ops() {
        let mut m = DirtyMask::new(24);
        assert!(!m.any_dirty());
        m.mark(5);
        assert!(m.is_dirty(5));
        assert!(!m.is_dirty(0));
        m.clear();
        assert!(!m.any_dirty());
    }

    #[test]
    fn dirty_mask_mark_all() {
        let mut m = DirtyMask::new(80);
        m.mark_all(80);
        assert!(m.any_dirty());
        for i in 0..80 {
            assert!(m.is_dirty(i));
        }
        assert!(!m.is_dirty(80));
    }

    #[test]
    fn dirty_mask_large_row_count() {
        let mut m = DirtyMask::new(200);
        m.mark(0);
        m.mark(63);
        m.mark(64);
        m.mark(199);
        assert!(m.is_dirty(0));
        assert!(m.is_dirty(63));
        assert!(m.is_dirty(64));
        assert!(m.is_dirty(199));
        assert!(!m.is_dirty(65));
        assert!(!m.is_dirty(198));
    }

    #[test]
    fn dirty_mask_resize() {
        let mut m = DirtyMask::new(24);
        m.mark(5);
        m.resize(200);
        assert!(m.is_dirty(5));
        assert!(!m.is_dirty(100));
        m.mark(150);
        assert!(m.is_dirty(150));
    }

    #[test]
    fn dirty_mask_zero_rows_has_no_dirty() {
        let m = DirtyMask::new(0);
        assert!(!m.any_dirty());
    }

    #[test]
    fn dirty_mask_single_row() {
        let mut m = DirtyMask::new(1);
        m.mark(0);
        assert!(m.is_dirty(0));
        assert!(m.any_dirty());
        m.clear();
        assert!(!m.any_dirty());
    }

    #[test]
    fn dirty_mask_exactly_64_rows_one_partition() {
        let mut m = DirtyMask::new(64);
        for i in 0..64 {
            m.mark(i);
        }
        for i in 0..64 {
            assert!(m.is_dirty(i));
        }
    }

    #[test]
    fn dirty_mask_65_rows_two_partitions() {
        let mut m = DirtyMask::new(65);
        m.mark(0);
        m.mark(63);
        m.mark(64);
        assert!(m.is_dirty(0));
        assert!(m.is_dirty(63));
        assert!(m.is_dirty(64));
        assert!(!m.is_dirty(1));
        assert!(!m.is_dirty(62));
        assert!(!m.is_dirty(65));
    }

    #[test]
    fn dirty_mask_mark_all_exact_partition_boundary() {
        let mut m = DirtyMask::new(64);
        m.mark_all(64);
        for i in 0..64 {
            assert!(m.is_dirty(i));
        }
    }

    #[test]
    fn dirty_mask_mark_all_partial_last_partition() {
        let mut m = DirtyMask::new(100);
        m.mark_all(100);
        for i in 0..100 {
            assert!(m.is_dirty(i));
        }
        assert!(!m.is_dirty(100));
    }

    #[test]
    fn dirty_mask_mark_all_one_row() {
        let mut m = DirtyMask::new(1);
        m.mark_all(1);
        assert!(m.is_dirty(0));
    }

    #[test]
    fn dirty_mask_resize_smaller_keeps_early_marks() {
        let mut m = DirtyMask::new(200);
        m.mark(0);
        m.mark(50);
        m.resize(10);
        assert!(m.is_dirty(0));
        assert!(m.is_dirty(50));
        assert!(!m.is_dirty(9));
    }

    #[test]
    fn dirty_mask_resize_larger_starts_clean() {
        let mut m = DirtyMask::new(10);
        m.mark(5);
        m.resize(1000);
        assert!(m.is_dirty(5));
        assert!(!m.is_dirty(500));
    }

    #[test]
    fn dirty_mask_mark_idempotent() {
        let mut m = DirtyMask::new(100);
        m.mark(42);
        m.mark(42);
        m.mark(42);
        assert!(m.is_dirty(42));
    }

    #[test]
    fn dirty_mask_mark_all_then_clear() {
        let mut m = DirtyMask::new(200);
        m.mark_all(200);
        assert!(m.any_dirty());
        m.clear();
        assert!(!m.any_dirty());
        for i in 0..200 {
            assert!(!m.is_dirty(i));
        }
    }

    #[test]
    fn dirty_mask_partial_clear() {
        let mut m = DirtyMask::new(100);
        m.mark(10);
        m.mark(20);
        m.clear();
        assert!(!m.is_dirty(10));
        assert!(!m.is_dirty(20));
    }

    #[test]
    fn color_from_ansi_all_16() {
        for i in 0..16u8 {
            let c = Color::from_ansi(i);
            let expected = Color {
                r: crate::ansi::ansi_to_rgb(i)[0],
                g: crate::ansi::ansi_to_rgb(i)[1],
                b: crate::ansi::ansi_to_rgb(i)[2],
                a: 255,
            };
            assert_eq!(c, expected);
        }
    }

    #[test]
    fn color_from_ansi_all_216_cube() {
        for i in 16..232u8 {
            let c = Color::from_ansi(i);
            let expected = Color {
                r: crate::ansi::ansi_to_rgb(i)[0],
                g: crate::ansi::ansi_to_rgb(i)[1],
                b: crate::ansi::ansi_to_rgb(i)[2],
                a: 255,
            };
            assert_eq!(c, expected);
        }
    }

    #[test]
    fn color_from_ansi_all_24_grayscale() {
        for i in 232..=255u8 {
            let c = Color::from_ansi(i);
            let expected = Color {
                r: crate::ansi::ansi_to_rgb(i)[0],
                g: crate::ansi::ansi_to_rgb(i)[1],
                b: crate::ansi::ansi_to_rgb(i)[2],
                a: 255,
            };
            assert_eq!(c, expected);
        }
    }

    #[test]
    fn color_from_ansi_alpha_always_255() {
        for i in 0..=255u8 {
            let c = Color::from_ansi(i);
            assert_eq!(c.a, 255);
        }
    }

    #[test]
    fn color_default_is_white_opaque() {
        let c = Color::default();
        assert_eq!(c, Color::new(255, 255, 255));
    }

    #[test]
    fn color_equality() {
        assert_ne!(Color::new(1, 2, 3), Color::new(1, 2, 4));
        assert_ne!(Color::new(1, 2, 3), Color::new(2, 2, 3));
    }

    #[test]
    fn cell_equality_full() {
        let c1 = Cell {
            char: 'A',
            foreground: Color::new(1, 2, 3),
            background: Color::new(4, 5, 6),
            attrs: Attrs {
                bold: true,
                ..Default::default()
            },
            width: 1,
        };
        let c2 = c1;
        assert_eq!(c1, c2);
    }

    #[test]
    fn cell_inequality_different_char() {
        let c1 = Cell::with_char('A');
        let c2 = Cell::with_char('B');
        assert_ne!(c1, c2);
    }

    #[test]
    fn cell_inequality_different_width() {
        let c1 = Cell {
            char: 'A',
            width: 1,
            ..Default::default()
        };
        let c2 = Cell {
            char: 'A',
            width: 2,
            ..Default::default()
        };
        assert_ne!(c1, c2);
    }

    #[test]
    fn cell_inequality_different_attrs() {
        let mut c1 = Cell::with_char('A');
        c1.attrs.bold = true;
        let c2 = Cell::with_char('A');
        assert_ne!(c1, c2);
    }

    #[test]
    fn cell_with_char_keeps_default_colors() {
        let c = Cell::with_char('日');
        assert_eq!(c.char, '日');
        assert_eq!(c.foreground, Color::default());
        assert_eq!(c.background, Color::default());
        assert_eq!(c.width, 1);
    }

    #[test]
    fn attrs_serde_json_roundtrip() {
        let a = Attrs {
            bold: true,
            dim: false,
            italic: true,
            underline: false,
            double_underline: true,
            reverse: false,
            strikethrough: true,
            blink: false,
            hidden: true,
            overline: false,
            protected: true,
            double_width: false,
            double_height_top: false,
            double_height_bottom: false,
        };
        let json = serde_json::to_string(&a).expect("ser");
        let back: Attrs = serde_json::from_str(&json).expect("de");
        assert_eq!(a, back);
    }

    #[test]
    fn color_serde_json_roundtrip() {
        let c = Color::new(10, 20, 30);
        let json = serde_json::to_string(&c).expect("ser");
        let back: Color = serde_json::from_str(&json).expect("de");
        assert_eq!(c, back);
    }

    #[test]
    fn cell_serde_json_roundtrip() {
        let c = Cell {
            char: 'X',
            foreground: Color::new(1, 2, 3),
            background: Color::new(4, 5, 6),
            attrs: Attrs {
                bold: true,
                italic: true,
                ..Default::default()
            },
            width: 2,
        };
        let json = serde_json::to_string(&c).expect("ser");
        let back: Cell = serde_json::from_str(&json).expect("de");
        assert_eq!(c, back);
    }

    #[test]
    fn dirty_mask_serde_json_roundtrip() {
        let mut m = DirtyMask::new(100);
        m.mark(0);
        m.mark(50);
        m.mark(99);
        let json = serde_json::to_string(&m).expect("ser");
        let back: DirtyMask = serde_json::from_str(&json).expect("de");
        for i in 0..100 {
            assert_eq!(m.is_dirty(i), back.is_dirty(i));
        }
    }

    #[test]
    fn dirty_mask_clone_preserves_state() {
        let mut m = DirtyMask::new(50);
        m.mark(10);
        m.mark(20);
        let m2 = m.clone();
        assert!(m2.is_dirty(10));
        assert!(m2.is_dirty(20));
        assert!(!m2.is_dirty(11));
    }

    #[test]
    fn attrs_all_true() {
        let a = Attrs {
            bold: true,
            dim: true,
            italic: true,
            underline: true,
            double_underline: true,
            reverse: true,
            strikethrough: true,
            blink: true,
            hidden: true,
            overline: true,
            protected: true,
            double_width: true,
            double_height_top: true,
            double_height_bottom: true,
        };
        let json = serde_json::to_string(&a).unwrap();
        let back: Attrs = serde_json::from_str(&json).unwrap();
        assert_eq!(a, back);
    }

    #[test]
    fn dirty_mask_partition_boundary_63_64() {
        let mut m = DirtyMask::new(128);
        m.mark(63);
        m.mark(64);
        assert!(m.is_dirty(63));
        assert!(m.is_dirty(64));
        assert!(!m.is_dirty(62));
        assert!(!m.is_dirty(65));
    }

    #[test]
    fn dirty_mask_clear_then_mark() {
        let mut m = DirtyMask::new(10);
        m.mark(0);
        m.mark(5);
        m.clear();
        m.mark(7);
        assert!(!m.is_dirty(0));
        assert!(!m.is_dirty(5));
        assert!(m.is_dirty(7));
    }

    #[test]
    fn color_saturating_add_wraps_at_255() {
        let a = Color {
            r: 200,
            g: 200,
            b: 200,
            a: 255,
        };
        let b = Color {
            r: 100,
            g: 100,
            b: 100,
            a: 255,
        };
        let c = a.saturating_add(&b);
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 255);
        assert_eq!(c.b, 255);
        assert_eq!(c.a, 255);
    }

    #[test]
    fn color_saturating_add_small_values() {
        let a = Color {
            r: 10,
            g: 20,
            b: 30,
            a: 255,
        };
        let b = Color {
            r: 5,
            g: 10,
            b: 15,
            a: 0,
        };
        let c = a.saturating_add(&b);
        assert_eq!(c.r, 15);
        assert_eq!(c.g, 30);
        assert_eq!(c.b, 45);
        assert_eq!(c.a, 255);
    }

    #[test]
    fn color_saturating_add_zero() {
        let a = Color {
            r: 100,
            g: 50,
            b: 25,
            a: 128,
        };
        let zero = Color {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        };
        let c = a.saturating_add(&zero);
        assert_eq!(c, a);
    }

    #[test]
    fn color_saturating_mul_saturates() {
        let c = Color {
            r: 100,
            g: 50,
            b: 25,
            a: 255,
        };
        let d = c.saturating_mul(3);
        assert_eq!(d.r, 255);
        assert_eq!(d.g, 150);
        assert_eq!(d.b, 75);
        assert_eq!(d.a, 255);
    }

    #[test]
    fn color_saturating_mul_by_one() {
        let c = Color {
            r: 100,
            g: 50,
            b: 25,
            a: 200,
        };
        let d = c.saturating_mul(1);
        assert_eq!(d, c);
    }

    #[test]
    fn color_saturating_mul_by_zero() {
        let c = Color {
            r: 100,
            g: 50,
            b: 25,
            a: 200,
        };
        let d = c.saturating_mul(0);
        assert_eq!(d.r, 0);
        assert_eq!(d.g, 0);
        assert_eq!(d.b, 0);
        assert_eq!(d.a, 0);
    }

    #[test]
    fn color_saturating_mul_all_channels_saturate() {
        let c = Color {
            r: 200,
            g: 200,
            b: 200,
            a: 200,
        };
        let d = c.saturating_mul(2);
        assert_eq!(d.r, 255);
        assert_eq!(d.g, 255);
        assert_eq!(d.b, 255);
        assert_eq!(d.a, 255);
    }

    #[test]
    fn dirty_mask_mark_out_of_range_partition_untouched() {
        // Row 200 maps to partition 3 (200/64), while a 5-row mask has only partition 0.
        // Marking rows in non-existent partitions is a silent no-op.
        let mut m = DirtyMask::new(5);
        m.mark(200);
        assert!(
            !m.any_dirty(),
            "marking row in non-existent partition sets no bit"
        );
    }

    #[test]
    fn dirty_mask_mark_existing_partition_sets_bit() {
        // Row 10 maps to partition 0 (10/64), which exists even for a 5-row mask.
        // The mark succeeds even though 10 >= total_rows(5) — the mask does not track
        // per-partition capacity.
        let mut m = DirtyMask::new(5);
        m.mark(10);
        assert!(
            m.any_dirty(),
            "marking row in existing partition must set a bit"
        );
    }

    #[test]
    fn dirty_mask_new_with_zero_rows_creates_empty_mask() {
        let m = DirtyMask::new(0);
        assert!(!m.any_dirty(), "zero-row mask must start clean");
        assert_eq!(m, DirtyMask::new(0), "all zero-row masks must be equal");
    }

    #[test]
    fn color_from_argb_via_fields() {
        let color = Color {
            a: 128,
            r: 255,
            g: 128,
            b: 64,
        };
        assert_eq!(color.r, 255, "red component must match");
        assert_eq!(color.g, 128, "green component must match");
        assert_eq!(color.b, 64, "blue component must match");
        assert_eq!(color.a, 128, "alpha component must match");
    }

    #[test]
    fn color_to_argb_via_fields() {
        let color = Color {
            r: 10,
            g: 20,
            b: 30,
            a: 200,
        };
        let (alpha, red, green, blue) = (color.a, color.r, color.g, color.b);
        assert_eq!(alpha, 200, "extracted alpha must match");
        assert_eq!(red, 10, "extracted red must match");
        assert_eq!(green, 20, "extracted green must match");
        assert_eq!(blue, 30, "extracted blue must match");
    }

    #[test]
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

    #[test]
    fn attrs_bold_set_and_read() {
        let mut a = Attrs::default();
        assert!(!a.bold);
        a.bold = true;
        assert!(a.bold);
    }

    #[test]
    fn attrs_dim_set_and_read() {
        let mut a = Attrs::default();
        a.dim = true;
        assert!(a.dim);
        a.dim = false;
        assert!(!a.dim);
    }

    #[test]
    fn attrs_italic_set_and_read() {
        let mut a = Attrs::default();
        a.italic = true;
        assert!(a.italic);
    }

    #[test]
    fn attrs_underline_set_and_read() {
        let mut a = Attrs::default();
        a.underline = true;
        assert!(a.underline);
    }

    #[test]
    fn attrs_strikethrough_set_and_read() {
        let mut a = Attrs::default();
        a.strikethrough = true;
        assert!(a.strikethrough);
    }

    #[test]
    fn attrs_blink_set_and_read() {
        let mut a = Attrs::default();
        a.blink = true;
        assert!(a.blink);
    }

    #[test]
    fn attrs_hidden_set_and_read() {
        let mut a = Attrs::default();
        a.hidden = true;
        assert!(a.hidden);
    }

    #[test]
    fn attrs_reverse_set_and_read() {
        let mut a = Attrs::default();
        a.reverse = true;
        assert!(a.reverse);
    }

    #[test]
    fn attrs_overline_set_and_read() {
        let mut a = Attrs::default();
        a.overline = true;
        assert!(a.overline);
    }

    #[test]
    fn attrs_double_underline_set_and_read() {
        let mut a = Attrs::default();
        a.double_underline = true;
        assert!(a.double_underline);
    }

    #[test]
    fn attrs_protected_set_and_read() {
        let mut a = Attrs::default();
        a.protected = true;
        assert!(a.protected);
    }

    #[test]
    fn attrs_all_fields_set_and_serde() {
        let a = Attrs {
            bold: true,
            dim: true,
            italic: true,
            underline: true,
            double_underline: true,
            reverse: true,
            strikethrough: true,
            blink: true,
            hidden: true,
            overline: true,
            protected: true,
            double_width: true,
            double_height_top: true,
            double_height_bottom: true,
        };
        let json = serde_json::to_string(&a).unwrap();
        let back: Attrs = serde_json::from_str(&json).unwrap();
        assert_eq!(a, back);
    }

    #[quickcheck]
    fn prop_color_from_ansi_bounds(index: u8) -> bool {
        let c = Color::from_ansi(index);
        c.a == 255
    }

    #[quickcheck]
    fn prop_color_saturating_add_bounds(r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8) -> bool {
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
            a: 0,
        };
        let _ = a.saturating_add(&b);
        true
    }

    #[quickcheck]
    fn prop_color_saturating_mul_bounds(r: u8, g: u8, b: u8, factor: u8) -> bool {
        let a = Color { r, g, b, a: 255 };
        let _ = a.saturating_mul(factor);
        true
    }

    #[quickcheck]
    fn prop_attrs_first_eight_roundtrip(
        bold: bool,
        dim: bool,
        italic: bool,
        underline: bool,
        double_underline: bool,
        reverse: bool,
        strikethrough: bool,
        blink: bool,
    ) -> bool {
        let a = Attrs {
            bold,
            dim,
            italic,
            underline,
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
    fn prop_attrs_last_six_roundtrip(
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

    #[quickcheck]
    fn prop_cell_equality_reflexive(char_code: u32, r: u8, g: u8, b: u8, width: u8) -> bool {
        let cell = Cell {
            char: char::from_u32(char_code & 0x10FFFF).unwrap_or(' '),
            foreground: Color { r, g, b, a: 255 },
            background: Color { r, g, b, a: 255 },
            width: if width == 0 { 1 } else { width % 3 + 1 },
            ..Default::default()
        };
        cell == cell
    }

    #[quickcheck]
    fn prop_dirty_mask_new_not_dirty(rows: u8) -> bool {
        let m = DirtyMask::new(rows as u32);
        !m.any_dirty()
    }

    #[test]
    fn color_serde_with_alias_fg() {
        let json = r#"{"r":255,"g":0,"b":0,"a":255}"#;
        let color: Color = serde_json::from_str(json).unwrap();
        assert_eq!(color, Color::new(255, 0, 0));
    }

    #[test]
    fn color_serde_with_alias_bg() {
        let json = r#"{"r":0,"g":255,"b":0,"a":255}"#;
        let color: Color = serde_json::from_str(json).unwrap();
        assert_eq!(color, Color::new(0, 255, 0));
    }

    #[test]
    fn dirty_mask_mark_cross_partition_boundary() {
        let mut m = DirtyMask::new(128);
        m.mark(63);
        m.mark(64);
        m.mark(65);
        assert!(m.is_dirty(63));
        assert!(m.is_dirty(64));
        assert!(m.is_dirty(65));
        assert!(!m.is_dirty(62));
        assert!(!m.is_dirty(66));
    }

    #[test]
    fn dirty_mask_mark_all_exact_partition_and_one_more() {
        let mut m = DirtyMask::new(65);
        m.mark_all(65);
        for i in 0..65 {
            assert!(m.is_dirty(i));
        }
        assert!(!m.is_dirty(65));
    }

    #[test]
    fn cell_with_char_preserves_foreground_alias() {
        let c = Cell::with_char('X');
        assert_eq!(c.foreground.r, 255);
        assert_eq!(c.foreground.g, 255);
        assert_eq!(c.foreground.b, 255);
    }

    #[test]
    fn cell_width_default_is_1() {
        let c = Cell::with_char('A');
        assert_eq!(c.width, 1);
    }
}
