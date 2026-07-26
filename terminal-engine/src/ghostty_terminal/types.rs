/// A single match from search_all_in_scrollback.
/// Row and column positions are byte offsets in the line text.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchMatch {
    pub row: u32,
    pub start_col: u32,
    pub end_col: u32,
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
    pub cursor_style: terminal_core::cursor::CursorStyle,
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
