/// Errors returned by [GhosttyTerminal](crate::terminal::ghostty_terminal::GhosttyTerminal) construction.
///
/// The only fallible step is spawning the VT thread; runtime query failures
/// are non-fatal and surface as fallback values, not errors (see
/// `public_api::query`).
#[derive(Debug, thiserror::Error)]
pub enum TerminalError {
    #[error("failed to spawn terminal thread: {0}")]
    Spawn(#[from] std::io::Error),
}

/// A single match from search_all_in_scrollback.
/// Row is a scrollback row; start_col/end_col are character columns in the
/// line (NOT byte offsets) — they align with CellData.col used by the
/// renderer's highlight pass.
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

/// Bit positions in `CellData::flags`, the single source of truth shared by
/// the style packer (`ghostty_terminal::internal::pack_style_flags`), the GPU
/// cell builder (`render::cell_builder`), the CPU fallback (`render::cpu_frame`)
/// and the shader `cell.wgsl`. Keep in sync with `pack_style_flags` and
/// `shaders/cell.wgsl` (which reads bits 3/5/6/7/8 for decorations).
pub mod cell_flags {
    pub const BOLD: u32 = 0;
    pub const ITALIC: u32 = 1;
    pub const REVERSE: u32 = 2;
    pub const UNDERLINE: u32 = 3;
    pub const STRIKETHROUGH: u32 = 5;
    pub const OVERLINE: u32 = 6;
    pub const FAINT: u32 = 7;
    pub const DOUBLE_UNDERLINE: u32 = 8;
}

/// Cursor info — terminal cursor state sent alongside CellData for
/// same-frame cursor rendering. Produced by build_cell_data, consumed
/// by the render thread as CellCursor.
#[derive(Debug, Clone, Copy)]
pub struct CursorInfo {
    pub row: u32,
    pub col: u32,
    pub visible: bool,
    pub style: CursorStyle,
}

/// Cell data — the per-cell payload transported from the Session thread
/// (where it's produced via Ghostty CellIterator) to the Render thread
/// (where it's converted to CellInstance for GPU upload).
///
/// This is a fixed-size bytemuck struct (80 bytes) so `Vec<CellData>` can be
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
    /// Packed style flags; bit positions are defined by [`cell_flags`]
    /// (bold/italic/reverse/underline/strikethrough/overline/faint/double
    /// underline), packed by `pack_style_flags` and consumed by the GPU and
    /// CPU cell builders plus `shaders/cell.wgsl`.
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
    pub title: String,
    pub scrollback_length: u32,
    pub sync_active: bool,
}

/// Raw pixel data for a KGP image (RGBA8).
#[derive(Clone, Debug)]
pub struct KittyGraphicsImageData {
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
/// How long `flush()` waits for the VT thread to drain its backlog before
/// giving up. Must be far above legitimate burst-write drain times in
/// debug builds (hundreds of ms); 5s of silence means the VT thread is
/// genuinely wedged.
pub(crate) const FLUSH_TIMEOUT_SECS: u64 = 5;
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
