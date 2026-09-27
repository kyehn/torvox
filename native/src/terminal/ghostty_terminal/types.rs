/// [`GhosttyTerminal`](crate::terminal::ghostty_terminal::GhosttyTerminal) 构造错误。
/// 唯一可能失败的是启动 VT 线程；运行时查询失败不致命，只回退为默认值。
#[derive(Debug, thiserror::Error)]
pub enum TerminalError {
    #[error("failed to spawn terminal thread: {0}")]
    Spawn(#[from] std::io::Error),
}

/// `search_all_in_scrollback` 的单个匹配。`row` 为回滚行号；`start_col`/`end_col`
/// 是行内字符列（**非**字节偏移），与渲染高亮所用的 `CellData.col` 对齐。
#[derive(Debug, Clone, PartialEq)]
pub struct SearchMatch {
    pub row: u32,
    pub start_col: u32,
    pub end_col: u32,
}

/// 光标样式。以 Ghostty（DECSCUSR）为单一来源，快照转换 1:1 映射上游视觉样式，
/// 仅空心块暂按实心块渲染。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorStyle {
    #[default]
    Block,
    /// 竖线光标（DECSCUSR 5、6）。
    Bar,
    /// 下划线光标（DECSCUSR 3、4）。
    Underline,
}

/// `CellData::flags` 的位定义，是样式打包器、GPU 单元构建器与 `cell.wgsl` 共享的
/// 单一来源。须与 `pack_style_flags` 和 `shaders/cell.wgsl` 保持一致（着色器读
/// 3/5/6/7/8 位作装饰；第 4 位 blink 仅透传信息，着色器忽略）。
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

/// 光标信息：与 CellData 一同发送的终端光标状态，供同帧渲染。由 `build_cell_data`
/// 产生，渲染线程以 `CellCursor` 消费。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorInfo {
    pub row: u32,
    pub col: u32,
    pub visible: bool,
    pub style: CursorStyle,
    /// 回滚长度：搭载在单元格数据通道上，渲染线程无需同步 `scrollback_length()` RPC。
    pub scrollback_length: u32,
    /// Kitty 图像存储生成戳（上游 `Graphics::generation`；0 = 从未写入）。
    /// 生成戳不变时放置集合与图像像素相同，渲染线程跳过放置查询；
    /// 滚动/缩放仍需重算几何（上游语义），由滚动长度与网格尺寸门控。
    pub kitty_generation: u64,
}

/// 逐单元载荷：会话线程（经 Ghostty CellIterator 产生）→ 渲染线程（转为 CellInstance
/// 上传 GPU）。定长 bytemuck 结构（96 字节），故 `Vec<CellData>` 过 flume 通道
/// 时每单元零拷贝。
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CellData {
    /// 主码点（通常即唯一码点）。
    pub codepoint: u32,
    /// 单元宽度：1 = 常规，2 = 宽（CJK、emoji）。
    pub width: u32,
    /// 字素簇续接码点（预留 7 个），多数单元为空；ASCII 只需 `codepoint`。
    pub grapheme_extra: [u32; 7],
    /// 前景色 [R, G, B, A]，取值 0..1。
    pub foreground: [f32; 4],
    /// 背景色 [R, G, B, A]，取值 0..1。
    pub background: [f32; 4],
    /// 下划线（SGR 58）色 [R, G, B, A]，取值 0..1；单元未显式设置时回退到前景色，
    /// 与着色器沿用的 `deco_color = foreground` 一致。
    pub underline_color: [f32; 4],
    /// 打包的样式标志，位定义见 [`cell_flags`]，由 `pack_style_flags` 打包。
    pub flags: u32,
    /// 网格行号（渲染线程据此算屏幕空间位置）。
    pub row: u32,
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

/// 终端网格的渲染快照：终端线程构建，渲染线程消费。
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

/// KGP 图像的原始像素数据（RGBA8）。
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

/// 整个终端网格的快照，用于跨 FFI 边界序列化。
pub struct DumpedGrid {
    pub rows: u32,
    pub cols: u32,
    pub visible: Vec<CellSnapshot>,
    pub scrollback: Vec<Vec<CellSnapshot>>,
}

/// 单个终端单元的快照，用于跨 FFI 序列化。
#[derive(Clone, Debug, Default)]
pub struct CellSnapshot {
    pub codepoint: u32,
    pub graphemes: Vec<u32>,
    pub foreground: [f32; 4],
    pub background: [f32; 4],
    /// 解算后的 SGR 58 下划线色（回退到 `foreground`）。
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
/// `flush()` 等待 VT 线程排空积压的上限；须远大于 debug 构建下突发写入的正常排空
/// 时间（数百毫秒），静默 5s 即视为 VT 线程确实卡死。
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
