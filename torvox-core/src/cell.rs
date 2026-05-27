use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    pub char: char,
    pub fg: Color,
    pub bg: Color,
    pub attrs: Attrs,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            char: ' ',
            fg: Color::default(),
            bg: Color::default(),
            attrs: Attrs::default(),
        }
    }
}

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
}

const BITS_PER_PARTITION: u32 = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirtyMask {
    partitions: alloc::vec::Vec<u64>,
}

impl DirtyMask {
    pub fn new(total_rows: u32) -> Self {
        let num_partitions = (total_rows as usize).div_ceil(BITS_PER_PARTITION as usize);
        Self {
            partitions: alloc::vec![0u64; num_partitions.max(1)],
        }
    }

    pub fn is_dirty(&self, row: u32) -> bool {
        let (part, bit) = self.partition_index(row);
        self.partitions
            .get(part)
            .is_some_and(|p| *p & (1 << bit) != 0)
    }

    pub fn mark(&mut self, row: u32) {
        let (part, bit) = self.partition_index(row);
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
        for p in &mut self.partitions {
            *p = 0;
        }
    }

    pub fn any_dirty(&self) -> bool {
        self.partitions.iter().any(|&p| p != 0)
    }

    pub fn resize(&mut self, total_rows: u32) {
        let num_partitions = (total_rows as usize).div_ceil(BITS_PER_PARTITION as usize);
        self.partitions.resize(num_partitions.max(1), 0);
    }

    fn partition_index(&self, row: u32) -> (usize, u32) {
        let part = (row / BITS_PER_PARTITION) as usize;
        let bit = row % BITS_PER_PARTITION;
        (part, bit)
    }
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub fn from_ansi(index: u8) -> Self {
        let [r, g, b] = crate::ansi::ansi_to_rgb(index);
        Self { r, g, b, a: 255 }
    }
}

impl Cell {
    pub fn with_char(c: char) -> Self {
        Self {
            char: c,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_default_is_space() {
        let c = Cell::default();
        assert_eq!(c.char, ' ');
        assert_eq!(c.fg, Color::default());
        assert_eq!(c.bg, Color::default());
    }

    #[test]
    fn cell_with_char() {
        let c = Cell::with_char('X');
        assert_eq!(c.char, 'X');
        assert_eq!(c.fg, Color::default());
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
}
