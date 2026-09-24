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

/// Cursor style. Ghostty is the single source of truth for cursor style
/// (DECSCUSR); the snapshot conversion maps the upstream visual style
/// 1:1, except hollow block which renders as solid block for now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorStyle {
    #[default]
    Block,
    /// 竖线光标（DECSCUSR 5、6）。
    Bar,
    /// 下划线光标（DECSCUSR 3、4）。
    Underline,
}

/// Bit positions in `CellData::flags`, the single source of truth shared by
/// the style packer (`ghostty_terminal::internal::pack_style_flags`), the GPU
/// cell builder (`render::cell_builder`)
/// and the shader `cell.wgsl`. Keep in sync with `pack_style_flags` and
/// `shaders/cell.wgsl` (which reads bits 3/5/6/7/8 for decorations;
/// bit 4 blink is carried for information, the shader ignores it).
pub mod cell_flags {
    pub const BOLD: u32 = 0;
    pub const ITALIC: u32 = 1;
    pub const REVERSE: u32 = 2;
    pub const UNDERLINE: u32 = 3;
    pub const BLINK: u32 = 4;
    pub const STRIKETHROUGH: u32 = 5;
    pub const OVERLINE: u32 = 6;
    pub const FAINT: u32 = 7;
    pub const DOUBLE_UNDERLINE: u32 = 8;
}

/// Cursor info — terminal cursor state sent alongside CellData for
/// same-frame cursor rendering. Produced by build_cell_data, consumed
/// by the render thread as CellCursor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorInfo {
    pub row: u32,
    pub col: u32,
    pub visible: bool,
    pub style: CursorStyle,
    /// Scrollback length — piggy-backed on the cell data channel so the
    /// render thread never needs a synchronous scrollback_length() RPC.
    pub scrollback_length: u32,
    /// Kitty 图像存储生成戳（上游 `Graphics::generation`；0 = 从未写入）。
    /// 生成戳不变时放置集合与图像像素相同，渲染线程跳过放置查询；
    /// 滚动/缩放仍需重算几何（上游语义），由滚动长度与网格尺寸门控。
    pub kitty_generation: u64,
}

/// Cell data — the per-cell payload transported from the Session thread
/// (where it's produced via Ghostty CellIterator) to the Render thread
/// (where it's converted to CellInstance for GPU upload).
///
/// This is a fixed-size bytemuck struct (96 bytes) so `Vec<CellData>` can be
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
    pub foreground: [f32; 4],
    /// Resolved background color as [R, G, B, A] in 0..1.
    pub background: [f32; 4],
    /// Resolved underline (SGR 58) color as [R, G, B, A] in 0..1.
    /// Falls back to the resolved foreground when the cell sets no explicit
    /// underline color, matching the shader's historic `deco_color = foreground`.
    pub underline_color: [f32; 4],
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
        assert_eq!(std::mem::size_of::<CellData>(), 96);
    }
    #[test]
    fn cell_data_is_bytemuck() {
        // Compile-time check: CellData implements Pod + Zeroable
        fn _assert_pod_zeroable<T: bytemuck::Pod + bytemuck::Zeroable>() {}
        _assert_pod_zeroable::<CellData>();
    }
    #[test]
    fn cell_flags_bits_are_disjoint_and_reserve_bit_four() {
        // 对标上游闪烁/下划线位重叠回归：已定义位必须互不重叠。
        let defined_bits = [
            cell_flags::BOLD,
            cell_flags::ITALIC,
            cell_flags::REVERSE,
            cell_flags::UNDERLINE,
            cell_flags::BLINK,
            cell_flags::STRIKETHROUGH,
            cell_flags::OVERLINE,
            cell_flags::FAINT,
            cell_flags::DOUBLE_UNDERLINE,
        ];
        let mut used_mask = 0u32;
        for bit in defined_bits {
            assert!(bit < 32, "样式位必须在单个 u32 内：{bit}");
            assert_eq!(used_mask & (1 << bit), 0, "样式位重叠：{bit}");
            used_mask |= 1 << bit;
        }
        // blink 占位 4：下划线形状字段不得与其交叠（上游真实踩坑）。
        assert_eq!(cell_flags::BLINK, 4);
        // 着色器装饰掩码与位定义一致（cell.wgsl 读取 8/32/64/128/256）。
        assert_eq!(1 << cell_flags::UNDERLINE, 8);
        assert_eq!(1 << cell_flags::BLINK, 16);
        assert_eq!(1 << cell_flags::STRIKETHROUGH, 32);
        assert_eq!(1 << cell_flags::OVERLINE, 64);
        assert_eq!(1 << cell_flags::FAINT, 128);
        assert_eq!(1 << cell_flags::DOUBLE_UNDERLINE, 256);
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

/// 单个 Kitty 放置的可渲染几何 + RGBA8 像素（VT 线程采集，渲染线程组装图集）。
/// 坐标为视口相对网格列/行（可为负，表示顶部滚出部分）；像素尺寸为上游
/// placement_render_info 解算值；source 矩形已按 Kitty 语义钳制到图像边界。
#[derive(Clone, Debug)]
pub struct KittyPlacementFrame {
    pub image_id: u32,
    pub viewport_col: i32,
    pub viewport_row: i32,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub source_x: u32,
    pub source_y: u32,
    pub source_width: u32,
    pub source_height: u32,
    pub cell_offset_x: u32,
    pub cell_offset_y: u32,
    pub z: i32,
    pub image_width: u32,
    pub image_height: u32,
    pub image_rgba: Vec<u8>,
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
}

/// A snapshot of the entire terminal grid for serialization across FFI boundaries.
pub struct DumpedGrid {
    pub rows: u32,
    pub cols: u32,
    pub visible: Vec<CellSnapshot>,
    pub scrollback: Vec<Vec<CellSnapshot>>,
}

/// A snapshot of a single terminal cell for serialization across FFI.
#[derive(Clone, Debug, Default)]
pub struct CellSnapshot {
    pub codepoint: u32,
    pub graphemes: Vec<u32>,
    pub foreground: [f32; 4],
    pub background: [f32; 4],
    /// Resolved SGR 58 underline color (falls back to `foreground`).
    pub underline_color: [f32; 4],
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub reverse: bool,
    pub strikethrough: bool,
    pub blink: bool,
    pub hidden: bool,
    pub overline: bool,
    pub double_underline: bool,
    pub width: u8,
}

pub(crate) const COMMAND_CHANNEL_CAPACITY: usize = 1024;
/// VT 查询通道容量（无界语义由有界 256 + try_send 回退实现）。
pub(crate) const QUERY_CHANNEL_CAPACITY: usize = 256;
/// 单元格帧通道容量（渲染线程逐帧消费，满则丢弃旧帧）。
pub(crate) const CELL_DATA_CHANNEL_CAPACITY: usize = 4;
/// 上游 OSC 回调事件通道容量（剪贴板写入、振铃，低频；满则丢弃，VT 线程永不阻塞）。
pub(crate) const EVENT_CHANNEL_CAPACITY: usize = 16;
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
pub(crate) const DISCONNECTED_TITLE: &str = "";
pub(crate) const DISCONNECTED_SCROLLBACK: u32 = 0;
static DEFAULT_CELL: CellSnapshot = CellSnapshot {
    codepoint: 0,
    graphemes: Vec::new(),
    foreground: [0.0; 4],
    background: [0.0; 4],
    underline_color: [0.0; 4],
    bold: false,
    dim: false,
    italic: false,
    underline: false,
    reverse: false,
    strikethrough: false,
    blink: false,
    hidden: false,
    overline: false,
    double_underline: false,
    width: 1,
};
pub(crate) const KGP_STORAGE_LIMIT: u64 = 64 * 1024 * 1024;
pub(crate) const MAX_GRAPHEME_CLUSTERS: usize = 8;
pub(crate) const DEFAULT_CELL_WIDTH: u32 = 8;
pub(crate) const DEFAULT_CELL_HEIGHT: u32 = 16;
