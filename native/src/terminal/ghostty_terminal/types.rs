/// [`GhosttyTerminal`](crate::terminal::ghostty_terminal::GhosttyTerminal) 构造错误。
/// 唯一可能失败的是启动 VT 线程；运行时查询失败不致命，只回退为默认值。
#[derive(Debug, thiserror::Error)]
pub enum TerminalError {
    #[error("failed to spawn terminal thread: {0}")]
    /// VT 线程启动失败：底层系统调用返回错误。
    Spawn(#[from] std::io::Error),
}

/// 搜索的单个匹配。`row` 为绝对网格行（0 = 回滚顶部）；`start_col`/`end_col`
/// 是网格列区间，`end_col` 为开区间上界，与渲染高亮所用的 `CellData.col` 对齐。
/// 跨软换行行的命中按行拆成多条，故同一命中的多行各占一条。
#[derive(Debug, Clone, PartialEq)]
pub struct SearchMatch {
    /// 命中的绝对网格行号（0 = 回滚区顶部）。
    pub row: u32,
    /// 命中的起始列（含）。
    pub start_col: u32,
    /// 命中的结束列（不含，开区间上界）。
    pub end_col: u32,
}

/// 搜索选中态：命中序号（上游口径，0 = 最新命中）、命中总数与选中命中的区间。
#[derive(Debug, Clone, PartialEq)]
pub struct SearchSelection {
    /// 选中命中的序号（0 = 最新命中）。
    pub index: usize,
    /// 命中总数。
    pub total: usize,
    /// 当前选中命中的区间。
    pub matched: SearchMatch,
}

/// 绘制视口高亮所需的全部输入：视口命中、当前选中命中与回滚长度。
///
/// 行号均为绝对网格行（0 = 回滚顶部）；视口折算由渲染侧用 `scrollback_lines`
/// 与视图的滚动偏移完成，故终端侧不必知道视口位置。
///
/// `selected` 是选中命中占据的**全部物理行**：一个命中可跨软换行，只取首行会让
/// 续行按「其余命中」上色，当前项在同一处命中里被劈成两种颜色。
#[derive(Debug, Clone, PartialEq)]
pub struct SearchViewportHighlights {
    /// 视口内全部命中，按行序排列。
    pub matches: Vec<SearchMatch>,
    /// 选中命中占据的全部物理行（一个命中可跨软换行，故可能多行）。
    pub selected: Vec<SearchMatch>,
    /// 回滚总行数，供渲染侧把绝对行号折算成视口行号。
    pub scrollback_lines: u32,
}

/// 光标样式。以 Ghostty（DECSCUSR）为单一来源，快照转换 1:1 映射上游视觉样式，
/// 仅空心块暂按实心块渲染。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorStyle {
    #[default]
    /// 实心块光标（DECSCUSR 1、2；缺省形态）。
    Block,
    /// 竖线光标（DECSCUSR 5、6）。
    Bar,
    /// 下划线光标（DECSCUSR 3、4）。
    Underline,
}

/// `CellData::flags` 的位定义，是样式打包器、GPU 单元构建器与 `cell.wgsl` 共享的
/// 单一来源。须与 `pack_style_flags` 和 `shaders/cell.wgsl` 保持一致（着色器读
/// 3/5/6/7/8 位作装饰；第 4 位 blink 与第 9 位 conceal 仅透传信息，着色器忽略）。
pub mod cell_flags {
    /// 粗体位（第 0 位）。
    pub const BOLD: u32 = 0;
    /// 斜体位（第 1 位）。
    pub const ITALIC: u32 = 1;
    /// 反白位（第 2 位）。
    pub const REVERSE: u32 = 2;
    /// 下划线位（第 3 位）。
    pub const UNDERLINE: u32 = 3;
    /// 闪烁位（第 4 位）：仅透传信息，着色器忽略。
    pub const BLINK: u32 = 4;
    /// 删除线位（第 5 位）。
    pub const STRIKETHROUGH: u32 = 5;
    /// 上划线位（第 6 位）。
    pub const OVERLINE: u32 = 6;
    /// 暗淡位（第 7 位）。
    pub const FAINT: u32 = 7;
    /// 双下划线位（第 8 位）。
    pub const DOUBLE_UNDERLINE: u32 = 8;
    /// 隐藏位（第 9 位）：仅透传信息，着色器忽略。
    pub const CONCEAL: u32 = 9;
}

/// 光标信息：与 `CellData` 一同发送的终端光标状态，供同帧渲染。由 `build_cell_data`
/// 产生，渲染线程以 `CellCursor` 消费。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorInfo {
    /// 光标所在的网格行号。
    pub row: u32,
    /// 光标所在的网格列号。
    pub col: u32,
    /// 光标落在宽字符的**尾列**（上游 `CursorViewport::at_wide_tail`）。此时 `col`
    /// 指向的是宽字符的第二格，而单元数据只为该宽字符发出一条 `col = col - 1` 的
    /// 记录——渲染侧须据此把光标认到那条记录上，否则整帧不生成任何光标四边形。
    pub at_wide_tail: bool,
    /// 光标是否可见；隐藏时仍给出位置。
    pub visible: bool,
    /// 光标形状。
    pub style: CursorStyle,
    /// 回滚长度：搭载在单元格数据通道上，渲染线程无需同步 `scrollback_length()` RPC。
    pub scrollback_length: u32,
    /// VT 视口顶部的绝对行号（可滚动区行空间：0 = 回滚区顶部）。
    ///
    /// 与 `scrollback_length`（回滚行数）配套：活动区顶行 = `scrollback_length`，
    /// 视口行号落在 `0..=scrollback_length`。视口行号**锚在内容上**——回滚浏览
    /// 期间新输出只把活动区底边推远，视口行号不动。
    ///
    /// 宿主的「离实时视图多少行」会随输出漂移（浏览期间每来一行输出就多一行），
    /// 不能用来定位视口；本字段才是视口位置的唯一真源。
    pub viewport_row: u32,
    /// 产出本帧时 VT 已处理的最后一个滚动请求序号（0 = 从未处理过滚动命令）。
    ///
    /// 渲染线程拿它与宿主已下发的最新序号比较：本帧序号更小即说明该帧早于
    /// 在途的滚动命令（视口仍是旧的），此刻呈现会用旧视口配新余量。
    pub scroll_request_id: u64,
    /// 产出本帧时 VT 网格的真实行列：与 `CellData` 同源，渲染线程不得再读会话侧
    /// `terminal_rows/cols` 原子缓存——后者在 resize 入队时提前发布，
    /// VT 尚未应用新网格时两者不一致，收缩帧会被 `build_row_ranges` 判空丢弃。
    pub rows: u32,
    /// 产出本帧时的网格列数（与同批 `CellData` 同源）。
    pub cols: u32,
    /// Kitty 图像存储生成戳（上游 `Graphics::generation`；0 = 从未写入）。
    /// 它只随图像内容增删而变化：图集重传按此门控，滚动/移动放置不应触发重传。
    pub kitty_generation: u64,
    /// Kitty 放置生成戳（上游 `RenderState::image_placements_generation`）。
    /// 放置被增删、移动、随滚动/重排都会变化，是「需要重查放置」的唯一可靠信号；
    /// 图像存储生成戳在 ED2 撤下放置时不变，只看它会漏掉首次 clear 的图像残留。
    pub kitty_placement_generation: u64,
}

/// 逐单元载荷：会话线程（经 Ghostty `CellIterator` 产生）→ 渲染线程（转为 `CellInstance`
/// 上传 GPU）。定长 bytemuck 结构（96 字节），故 `Vec<CellData>` 过 flume 通道
/// 时每单元零拷贝。
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CellData {
    /// 主码点（通常即唯一码点）。
    pub codepoint: u32,
    /// 单元宽度：1 = 常规，2 = 宽（CJK、emoji）。
    pub width: u32,
    /// 字素簇续接码点，多数单元为空；ASCII 只需 `codepoint`。
    pub grapheme_extra: [u32; MAX_GRAPHEME_EXTRA],
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
    /// 网格列号（渲染线程据此算屏幕空间位置）。
    pub col: u32,
}

/// 依 `CellData.row` 把扁平行主序的 `cell_data` 切片划分为每行范围。
/// 宽字符/间隔单元被终端省略，故各行长度不同；
/// 无单元的行映射为空范围。存在超出 `rows` 的
/// 单元（或输入非行主序）时返回 `None`——
/// 属陈旧/失配数据，不可用于增量构建。
///
/// 置于终端层而非渲染层：入参与返回值都只由 `CellData` 的行号约定决定，
/// 渲染侧的实例构建与终端侧的逐行缓存共用同一判据。
pub(crate) fn build_row_ranges(
    cell_data: &[CellData],
    rows: u32,
) -> Option<Vec<std::ops::Range<usize>>> {
    let mut ranges: Vec<std::ops::Range<usize>> = (0..rows as usize).map(|_| 0..0).collect();
    let mut current = 0usize;
    for (r, range) in ranges.iter_mut().enumerate() {
        let start = current;
        while current < cell_data.len() && cell_data[current].row as usize == r {
            current += 1;
        }
        *range = start..current;
    }
    if current < cell_data.len() {
        return None;
    }
    Some(ranges)
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
        fn assert_pod_zeroable<T: bytemuck::Pod + bytemuck::Zeroable>() {}
        assert_pod_zeroable::<CellData>();
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
            cell_flags::CONCEAL,
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
    /// 网格行数。
    pub rows: u32,
    /// 网格列数。
    pub cols: u32,
    /// 光标所在行号。
    pub cursor_row: u32,
    /// 光标所在列号。
    pub cursor_col: u32,
    /// 光标是否可见。
    pub cursor_visible: bool,
    /// 光标形状。
    pub cursor_style: CursorStyle,
    /// 逐单元快照，按行主序排列。
    pub cells: Vec<CellSnapshot>,
    /// 逐单元脏标记：置位表示该单元自上次快照以来有变化。
    pub dirty: Vec<bool>,
    /// 终端标题（OSC 0/2 上报）。
    pub title: String,
    /// 回滚区行数。
    pub scrollback_length: u32,
}

/// KGP 图像的原始像素数据（RGBA8）。
#[derive(Clone, Debug)]
pub struct KittyGraphicsImageData {
    /// 图像 id（KGP 协议内的传输序号）。
    pub id: u32,
    /// 图像像素宽。
    pub width: u32,
    /// 图像像素高。
    pub height: u32,
    /// RGBA8 像素数据，长度为 `width * height * 4`。
    pub data: Vec<u8>,
}

/// 单个 Kitty 放置的可渲染几何 + RGBA8 像素（VT 线程采集，渲染线程组装图集）。
/// 坐标为视口相对网格列/行（可为负，表示顶部滚出部分）；像素尺寸为上游
/// `placement_render_info` 解算值；source 矩形在此按上游口径原样携带，越界部分的
/// 钳制在组装图集时统一做（见 `render::kitty::clamp_source`）——采集侧再钳一次
/// 只会让「声明尺寸」与「实际像素」两处判据分叉。
#[derive(Clone, Debug)]
pub struct KittyPlacementFrame {
    /// 所属图像 id。
    pub image_id: u32,
    /// 放置左缘的视口相对列号（可为负，表示左半部分已滚出）。
    pub viewport_col: i32,
    /// 放置上缘的视口相对行号（可为负，表示上半部分已滚出）。
    pub viewport_row: i32,
    /// 放置解算出的像素宽。
    pub pixel_width: u32,
    /// 放置解算出的像素高。
    pub pixel_height: u32,
    /// 取源矩形左缘（像素）。
    pub source_x: u32,
    /// 取源矩形上缘（像素）。
    pub source_y: u32,
    /// 取源矩形宽（像素）。
    pub source_width: u32,
    /// 取源矩形高（像素）。
    pub source_height: u32,
    /// 放置相对单元格的横向像素偏移。
    pub cell_offset_x: u32,
    /// 放置相对单元格的纵向像素偏移。
    pub cell_offset_y: u32,
    /// 层叠次序：值越大越靠前。
    pub z: i32,
    /// 所属图像的像素宽。
    pub image_width: u32,
    /// 所属图像的像素高。
    pub image_height: u32,
    /// 图像像素经 `Arc` 共享：Unicode 占位符按放置分组时，同一图像可能被
    /// 多个可见块引用，逐块复制整幅 RGBA 会成倍放大内存。
    pub image_rgba: std::sync::Arc<[u8]>,
}

impl GridSnapshot {
    /// 构造「全部单元空白且全部标脏」的占位快照。
    ///
    /// 用于终端尚未产出首帧时：网格尺寸先行可用，渲染线程不会拿到空快照。
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
            cursor_style: CursorStyle::default(),
            title: String::new(),
            scrollback_length: 0,
        }
    }
}

/// 整个终端网格的快照，用于跨 FFI 边界序列化。
pub struct DumpedGrid {
    /// 网格行数。
    pub rows: u32,
    /// 网格列数。
    pub cols: u32,
    /// 活动区逐单元快照。
    pub visible: Vec<CellSnapshot>,
    /// 回滚区逐行快照（每行一个单元序列）。
    pub scrollback: Vec<Vec<CellSnapshot>>,
}

/// 单个终端单元的快照，用于跨 FFI 序列化。
#[derive(Clone, Debug, Default)]
pub struct CellSnapshot {
    /// 主码点。
    pub codepoint: u32,
    /// 字素簇续接码点（ASCII 恒为空）。
    pub graphemes: Vec<u32>,
    /// 前景色 [R, G, B, A]，取值 0..1。
    pub foreground: [f32; 4],
    /// 背景色 [R, G, B, A]，取值 0..1。
    pub background: [f32; 4],
    /// 解算后的 SGR 58 下划线色（回退到 `foreground`）。
    pub underline_color: [f32; 4],
    /// 是否粗体（SGR 1）。
    pub bold: bool,
    /// 是否暗淡（SGR 2）。
    pub dim: bool,
    /// 是否斜体（SGR 3）。
    pub italic: bool,
    /// 是否下划线（SGR 4）。
    pub underline: bool,
    /// 是否反白（SGR 7）。
    pub reverse: bool,
    /// 是否删除线（SGR 9）。
    pub strikethrough: bool,
    /// 是否闪烁（SGR 5）。
    pub blink: bool,
    /// 是否隐藏（SGR 8）。
    pub hidden: bool,
    /// 是否上划线（SGR 53）。
    pub overline: bool,
    /// 是否双下划线（SGR 21）。
    pub double_underline: bool,
    /// 单元宽度：1 = 常规，2 = 宽。
    pub width: u8,
}

pub(crate) const COMMAND_CHANNEL_CAPACITY: usize = 1024;
/// 上游 OSC 9/777 通知回调通道容量（桌面通知，低频；满则丢弃，VT 线程永不阻塞）。
///
/// 与剪贴板通道分开计量：两者都是上游回调的入口，但语义无关（通知可被程序
/// 洪泛，剪贴板写入则有协议级应答），混在一条通道里会让洪泛的通知挤掉剪贴板。
pub(crate) const NOTIFICATION_CHANNEL_CAPACITY: usize = 16;
/// VT 查询通道容量（无界语义由有界 256 + `try_send` 回退实现）。
pub(crate) const QUERY_CHANNEL_CAPACITY: usize = 256;
/// 上游 OSC 回调事件通道容量（剪贴板写入，低频；满则丢弃，VT 线程永不阻塞）。
pub(crate) const EVENT_CHANNEL_CAPACITY: usize = 16;
pub(crate) const QUERY_TIMEOUT_MS: u64 = 500;
/// VT 线程空闲时等待下一条命令的时长。
///
/// VT 线程同时等待命令通道与查询通道（任一就绪即醒），故查询的应答延迟与本值无关：
/// 本值只决定「无命令无查询时」多久做一次周期职责（排空残留查询、推送单元数据）。
/// 但命令批处理期间不排空查询（批尾才应答），一次查询最坏要等一整批命令处理完，
/// 故调用方的查询窗口仍须显著大于本值（见 `Session::FOCUS_MODE_QUERY_TIMEOUT_MS`）。
pub(crate) const VT_IDLE_TICK_MS: u64 = 50;
/// 构造函数等待 VT 线程进入其闭包的时限。超过即判定线程调度异常。
/// 与 [`QUERY_TIMEOUT_MS`] 同量级：正常调度是微秒级，超时只可能来自系统过载。
pub(crate) const VT_READY_TIMEOUT_MS: u64 = 500;
/// `flush()` 等待 VT 线程排空积压的上限；须远大于 debug 构建下突发写入的正常排空
/// 时间（数百毫秒），静默 5s 即视为 VT 线程确实卡死。
pub(crate) const FLUSH_TIMEOUT_SECS: u64 = 5;
pub(crate) const DISCONNECTED_ROWS: u32 = 24;
pub(crate) const DISCONNECTED_COLS: u32 = 80;
pub(crate) const DISCONNECTED_CURSOR_X: u32 = 0;
pub(crate) const DISCONNECTED_CURSOR_Y: u32 = 0;
/// 查询通道断开时的光标可见性回退。
///
/// 取 `false`（隐藏）而非 `true`：`true` 会在会话已经失联时凭空画出一个块状光标，
/// 让「会话没了」看起来像「光标闪了一下」；隐藏只是少画一块，帧仍照常呈现，
/// 排查时看到的正是真正的情况。
pub(crate) const DISCONNECTED_CURSOR_VISIBLE: bool = false;
pub(crate) const DISCONNECTED_TITLE: &str = "";
/// 查询通道断开时的工作目录回退：空串表示「未知」，与「shell 从未上报」同义。
pub(crate) const DISCONNECTED_PWD: &str = "";
pub(crate) const DISCONNECTED_SCROLLBACK: u32 = 0;
pub(crate) const KGP_STORAGE_LIMIT: u64 = 64 * 1024 * 1024;
/// 单格字素簇的码位上限：主码位与续接码位合计。
pub(crate) const MAX_GRAPHEME_CLUSTERS: usize = 8;
/// 续接码位槽数：主码位已由 `CellData::codepoint` 携带，故比整簇少一。
pub(crate) const MAX_GRAPHEME_EXTRA: usize = MAX_GRAPHEME_CLUSTERS - 1;
pub(crate) const DEFAULT_CELL_WIDTH: u32 = 8;
pub(crate) const DEFAULT_CELL_HEIGHT: u32 = 16;

/// Kitty Unicode 占位符码位（U+10EEEE）。
///
/// 该码位在网格里只承载图像 id 与放置 id 的编码，不是有形的字形：
/// 文本层遇到它必须跳过，否则会画出豆腐块盖住图像。
pub(crate) const KITTY_PLACEHOLDER: u32 = 0x10_EEEE;
