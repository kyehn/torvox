/// A single match from search_all_in_scrollback.
/// Row and column positions are byte offsets in the line text.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchMatch {
    pub row: u32,
    pub start_col: u32,
    pub end_col: u32,
}

/// Cursor style enum — replaces the deleted terminal_core::cursor::CursorStyle.
/// Ghostty is the single source of truth for cursor style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorStyle {
    #[default]
    Default,
    Block,
    Bar,
    Underline,
}

/// Selection mode — used by the renderer for selection rendering.
/// Replaces the deleted terminal_core::selection::SelectionMode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionMode {
    #[default]
    Char,
    Word,
    Line,
    Semantic,
    Block,
}

/// Check if a character is East Asian Wide (double-width CJK).
/// Replaces the deleted terminal_core::unicode::is_wide.
pub fn is_wide(c: char) -> bool {
    let cp = c as u32;
    // East Asian Width — W / F characters as defined by Unicode Annex #11.
    // This covers CJK, Hangul, fullwidth forms, and related blocks.
    matches!(cp,
        0x1100..=0x115F | // Hangul Jamo
        0x2329..=0x232A | // Angle brackets
        0x2E80..=0x2EFF | // CJK Radicals Supplement
        0x2F00..=0x2FDF | // Kangxi Radicals
        0x2FF0..=0x2FFF | // Ideographic Description Characters
        0x3000..=0x303E | // CJK Symbols & Punctuation
        0x3041..=0x3096 | // Hiragana
        0x3099..=0x30FF | // Katakana
        0x3105..=0x312F | // Bopomofo
        0x3131..=0x318E | // Hangul Compatibility Jamo
        0x3190..=0x31E3 | // Kanbun, CJK Strokes, Bopomofo Extended
        0x31F0..=0x321E | // Katakana Phonetic Extensions
        0x3220..=0x3247 | // Enclosed CJK Letters
        0x3250..=0x4DBF | // CJK Extension A
        0x4E00..=0xA4CF | // CJK Unified Ideographs + Yi
        0xA960..=0xA97C | // Hangul Jamo Extended-A
        0xAC00..=0xD7A3 | // Hangul Syllables
        0xF900..=0xFAFF | // CJK Compatibility Ideographs
        0xFE10..=0xFE19 | // Vertical Forms
        0xFE30..=0xFE6B | // CJK Compatibility Forms
        0xFF01..=0xFF60 | // Fullwidth Forms
        0xFFE0..=0xFFE6 | // Fullwidth Signs
        0x1B000..=0x1B0FF | // Kana Supplement
        0x1B100..=0x1B12F | // Kana Extended-A
        0x1F200..=0x1F2FF | // Enclosed Ideographic Supplement
        0x20000..=0x2FFFF | // CJK Extension B+
        0x30000..=0x3FFFF   // CJK Extension G+
    )
}

/// Cell data — the per-cell payload transported from the Session thread
/// (where it's produced via Ghostty CellIterator) to the Render thread
/// (where it's converted to CellInstance for GPU upload).
///
/// This is a fixed-size bytemuck struct (80 bytes) so Vec<CellData> can be
/// sent across a flume channel with zero copying overhead per cell.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CellData {
    /// Primary codepoint (typically the only one).
    pub codepoint: u32,
    /// Cell width: 1 = normal, 2 = wide (CJK, emoji).
    pub width: u32,
    /// Reserved grapheme-cluster continuation codepoints (7 extras).
    /// Most cells have zero extras; `codepoint` alone suffices for ASCII.
    pub grapheme_extra: [u32; 7],
    /// Resolved foreground color as [R, G, B, A] in 0..1.
    pub fg_color: [f32; 4],
    /// Resolved background color as [R, G, B, A] in 0..1.
    pub bg_color: [f32; 4],
    /// Packed style flags (reserved for bold/italic/underline/etc bitmask).
    pub flags: u32,
    /// Grid row (for screen-space position computation on render thread).
    pub row: u32,
    /// Grid column.
    pub col: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cell_data_size() {
        assert_eq!(std::mem::size_of::<CellData>(), 80);
    }
    #[test]
    fn cell_data_is_bytemuck() {
        // Compile-time check: CellData implements Pod + Zeroable
        fn _assert_pod_zeroable<T: bytemuck::Pod + bytemuck::Zeroable>() {}
        _assert_pod_zeroable::<CellData>();
    }
}

/// Render snapshot of the terminal grid.
/// Built on the terminal thread; consumed by the renderer thread.
#[derive(Clone, Debug, Default)]
pub struct GridSnapshot {
    pub rows: u32,
    pub cols: u32,
    pub cursor_row: u32,
    pub cursor_col: u32,
    pub cursor_visible: bool,
    pub cursor_style: CursorStyle,
    pub cells: Vec<CellSnapshot>,
    pub dirty: Vec<bool>,
    pub kgp_placements: Vec<KgpPlacement>,
    pub title: String,
    pub scrollback_length: u32,
    pub sync_active: bool,
}

/// A Kitty Graphics Protocol (KGP) placement for rendering.
#[derive(Clone, Debug)]
pub struct KgpPlacement {
    pub image_id: u32,
    pub placement_id: u32,
    pub row: i32,
    pub col: i32,
    pub z: u8,
}

/// Raw pixel data for a KGP image (RGBA8).
#[derive(Clone, Debug)]
pub struct KgpImageData {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl GridSnapshot {
    pub fn fallback(rows: u32, cols: u32) -> Self {
        let count = (rows * cols) as usize;
        Self {
            rows,
            cols,
            cells: vec![CellSnapshot::default(); count],
            dirty: vec![true; count],
            cursor_row: DISCONNECTED_CURSOR_Y,
            cursor_col: DISCONNECTED_CURSOR_X,
            cursor_visible: DISCONNECTED_CURSOR_VISIBLE,
            cursor_style: Default::default(),
            kgp_placements: Vec::new(),
            title: String::new(),
            scrollback_length: 0,
            sync_active: false,
        }
    }
    pub fn cell_at(&self, row: u32, col: u32) -> &CellSnapshot {
        let idx = (row * self.cols + col) as usize;
        if idx >= self.cells.len() {
            return &DEFAULT_CELL;
        }
        &self.cells[idx]
    }
    pub fn uri_at(&self, row: u32, col: u32) -> Option<&str> {
        if row >= self.rows || col >= self.cols {
            return None;
        }
        let idx = (row * self.cols + col) as usize;
        self.cells.get(idx).and_then(|c| c.uri.as_deref())
    }
}

/// A snapshot of the entire terminal grid for serialization across FFI boundaries.
pub struct DumpedGrid {
    pub rows: u32,
    pub cols: u32,
    pub visible: Vec<CellSnapshot>,
    pub scrollback: Vec<Vec<CellSnapshot>>,
}

/// Semantic classification of terminal content for clipboard copy behavior.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SemanticContent {
    /// Normal terminal output.
    #[default]
    Output,
    /// User-typed input.
    Input,
    /// Command prompt text.
    Prompt,
}

/// A snapshot of a single terminal cell for serialization across FFI.
#[derive(Clone, Debug, Default)]
pub struct CellSnapshot {
    pub codepoint: u32,
    pub graphemes: Vec<u32>,
    pub foreground: [f32; 4],
    pub background: [f32; 4],
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub reverse: bool,
    pub strikethrough: bool,
    pub blink: bool,
    pub hidden: bool,
    pub uri: Option<String>,
    pub semantic: SemanticContent,
    pub overline: bool,
    pub double_underline: bool,
    pub width: u8,
}

pub(crate) const COMMAND_CHANNEL_CAPACITY: usize = 1024;
pub(crate) const QUERY_TIMEOUT_MS: u64 = 500;
pub(crate) const DISCONNECTED_ROWS: u32 = 24;
pub(crate) const DISCONNECTED_COLS: u32 = 80;
pub(crate) const DISCONNECTED_CURSOR_X: u32 = 0;
pub(crate) const DISCONNECTED_CURSOR_Y: u32 = 0;
pub(crate) const DISCONNECTED_CURSOR_VISIBLE: bool = true;
pub(crate) const DISCONNECTED_MODE_ORIGIN: bool = false;
pub(crate) const DISCONNECTED_MODE_AUTOWRAP: bool = false;
pub(crate) const DISCONNECTED_TITLE: &str = "";
pub(crate) const DISCONNECTED_SCROLLBACK: u32 = 0;
static DEFAULT_CELL: CellSnapshot = CellSnapshot {
    codepoint: 0,
    graphemes: Vec::new(),
    foreground: [0.0; 4],
    background: [0.0; 4],
    bold: false,
    dim: false,
    italic: false,
    underline: false,
    reverse: false,
    strikethrough: false,
    blink: false,
    hidden: false,
    uri: None,
    semantic: SemanticContent::Output,
    overline: false,
    double_underline: false,
    width: 1,
};
pub(crate) const KGP_STORAGE_LIMIT: u64 = 64 * 1024 * 1024;
pub(crate) const MAX_GRAPHEME_CLUSTERS: usize = 8;
pub(crate) const DEFAULT_CELL_WIDTH: u32 = 8;
pub(crate) const DEFAULT_CELL_HEIGHT: u32 = 16;
