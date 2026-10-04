use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::{Arc, Mutex};

use flume::{Receiver, Sender};

use super::types::*;

/// 调用方发往 VT 线程的命令，按序处理。
///
/// 通道**有界**，VT 线程卡住时内存不会无限增长；发送方用 `try_send` 并回退到缓存值。
/// 无状态查询归 [`Query`]，两条通道使动作顺序与查询延迟互不干扰。
pub enum Command {
    Write(Vec<u8>),
    FlushAck(Sender<()>),
    SetTheme {
        background: [u8; 3],
        foreground: [u8; 3],
        ansi: [[u8; 3]; 16],
    },
    Resize {
        rows: u32,
        cols: u32,
    },
    /// 更新终端单元格像素尺寸（Kitty 图像几何与鼠标像素映射依赖它）。
    /// 由 PTY 像素尺寸除以网格行列得出；VT 线程以当前行列重调 resize。
    SetCellPixelSize {
        cell_width: u32,
        cell_height: u32,
    },
    /// 取当前视口网格快照。回滚浏览走 CellData 通道（`ScrollViewport` +
    /// `receive_cell_data`），本命令只覆盖当前视口，因此没有偏移参数。
    TakeSnapshot {
        tx: Sender<Arc<GridSnapshot>>,
    },
    /// 按增量滚动终端视口（向上为负），供应用浏览回滚。
    ScrollViewport(isize),
    /// 安装终端持有的线性活动选区（跟踪网格引用，随滚动/输出/重排跟随文本）。
    /// 坐标为绝对网格行（0 = 回滚顶部）与列。
    SetSelection {
        start: (u32, u32),
        end: (u32, u32),
    },
    ClearSelection,
    /// RIS 全重置：恢复终端初始状态并清空回滚（侧边面板“重置终端”按钮）。
    Reset,
    Terminate,
}

/// 由 VT 线程 `process_query` 应答的无状态查询。
///
/// 走 `query_tx` 通道（有界 256，`try_send` + 回退）：查询突发（如设置面板遍历所有模式）
/// 既不阻塞调用方也不延迟命令通道上的有序动作；VT 线程在每条命令后与空闲超时后抽取。
///
/// 注意：查询在命令之间抽取，故查询可能先于先发的动作（Write/Resize）被应答，两条通道
/// 之间无顺序保证。必须观察先前状态变更的动作应走命令通道（如 `Command::TakeSnapshot`）。
pub enum Query {
    Rows(Sender<u32>),
    Cols(Sender<u32>),
    CursorX(Sender<u32>),
    CursorY(Sender<u32>),
    CursorVisible(Sender<bool>),
    Title(Sender<String>),
    ModeGet(u16, u8, Sender<bool>),
    ScrollbackLength(Sender<u32>),
    ReadLineText {
        row: u32,
        tx: Sender<Option<String>>,
    },
    /// 经 `build_cell_data` 读取的光标视口 (row, col)，即渲染线程消费的同一数据源，
    /// 可断言与 GPU 绘制一致的坐标。光标隐藏或构建失败时为 None。
    RenderCursor(Sender<Option<(u32, u32)>>),
    ReadVisibleText(Sender<String>),
    /// 用 Ghostty 原生格式化器提取选中文本：软换行行被合并（不加 '\n'）并去尾随空白。
    /// 列端点是网格列，格式化器内部自行映射到字符下标（宽字符安全）。
    SelectionText {
        /// 网格行号（绝对；ghostty 语义下回滚行为负偏移，调用方传 `Point::Screen` 坐标）。
        start: (u32, u32),
        end: (u32, u32),
        tx: Sender<String>,
    },
    /// 用上游 `Terminal::select_word`（Ghostty 词边界规则）在绝对网格单元导出选区并安装到
    /// 终端，返回有序边界 (start, end)（绝对网格行，0 = 回滚顶部）；无选区时 None。
    SelectWordAt {
        row: u32,
        col: u32,
        tx: Sender<Option<((u32, u32), (u32, u32))>>,
    },
    /// 用上游 `Terminal::select_line` 导出整行选区并安装，返回有序边界。
    SelectLineAt {
        row: u32,
        col: u32,
        tx: Sender<Option<((u32, u32), (u32, u32))>>,
    },
    /// 用上游 `Terminal::select_all` 导出全部可选内容并安装，返回有序边界
    /// （上游语义：边界不含尾部空白行列）。
    SelectAll {
        tx: Sender<Option<((u32, u32), (u32, u32))>>,
    },
    /// 查询网格单元处的 OSC 8 超链接 URI（无则 None）。
    HyperlinkAt {
        row: u32,
        col: u32,
        tx: Sender<Option<String>>,
    },
    /// 该行中作为宽字符后半格（SpacerTail）的列号，升序。宽字符占几列只有网格知道
    /// ——行文本里尾格与真空白同为 `' '`，故吸附判定不可由行文本反推（见 design 第 1 节）。
    /// 一次取整行而非逐列查询：手柄拖动的每个 MOVE 都要吸附，逐列 RPC 会把发往 VT
    /// 线程的同步查询压到每个触摸帧上。
    WideCharTailCols {
        row: u32,
        tx: Sender<Vec<u32>>,
    },
    SearchInScrollbackAll {
        query: String,
        case_sensitive: bool,
        tx: Sender<Vec<SearchMatch>>,
    },
    DumpGrid {
        tx: Sender<DumpedGrid>,
    },
    TakeKittyGraphicsImage {
        id: u32,
        tx: Sender<Option<KittyGraphicsImageData>>,
    },
    /// 采集全部可见 Kitty 放置（含几何 + RGBA），供渲染线程组装图集。
    TakeKittyPlacements {
        tx: Sender<Vec<KittyPlacementFrame>>,
    },
    KeyEncode {
        key_code: u32,
        modifiers: u16,
        action: u8,
        unicode_char: u32,
        unshifted_char: u32,
        tx: Sender<Vec<u8>>,
    },
    /// 用 Ghostty 鼠标编码器把鼠标事件编码为终端转义序列（按终端状态选 SGR/X10/UTF-8）。
    /// `position` 为表面像素，`cell_width`/`cell_height` 为实时单元格像素尺寸以便像素→单元映射。
    /// 鼠标上报关闭或编码失败时返回空 Vec。
    EncodeMouseEvent {
        position: (f32, f32),
        action: u8,
        button: u8,
        /// 上游 `key.Mods` 原始位（`Mods::from_bits_retain` 直接消费）：
        /// Shift/Ctrl 点击到达 vim/tmux 时必须与普通左键可区分。
        modifiers: u16,
        cell_width: f32,
        cell_height: f32,
        tx: Sender<Vec<u8>>,
    },
}

pub(crate) struct RunConfig {
    pub(crate) command_receiver: Receiver<Command>,
    pub(crate) query_receiver: Receiver<Query>,
    pub(crate) rows: u32,
    pub(crate) cols: u32,
    pub(crate) scrollback_lines: u32,
    pub(crate) background_color: [u8; 3],
    pub(crate) foreground_color: [u8; 3],
    pub(crate) ansi_colors: [[u8; 3]; 16],
    pub(crate) response_buffer: Arc<Mutex<Vec<Vec<u8>>>>,
    /// 备用屏状态的无锁镜像，由 VT 线程每帧更新，供输入路径免阻塞 RPC 检出。
    pub(crate) alt_screen_active: Arc<AtomicBool>,
    /// 上游 OSC 52 回调事件通道（VT 线程推送，调用方轮询）：剪贴板写入。
    /// 有界丢弃——VT 线程永不阻塞；Kotlin 经 session 锁存槽读取。
    pub(crate) clipboard_tx: flume::Sender<(String, String)>,
    /// 上游 BEL 回调事件通道（VT 线程推送，调用方轮询）：每次振铃一个空消息。
    /// 有界丢弃——VT 线程永不阻塞；单帧多响在会话锁存处合并为一。
    pub(crate) bell_tx: flume::Sender<()>,
    /// 当前单元格像素几何（XTWINOPS 14/16t 应答用）：Resize 回填默认值，
    /// SetCellPixelSize 回填真实字形度量；回调经此共享，无锁读取。
    pub(crate) cell_size_px: Arc<(AtomicU32, AtomicU32)>,
    /// 可选通道：设置后 VT 线程在网格变化时自动构建并推送 `Vec<CellData>`，
    /// 即线程拆分架构的数据路径（会话线程 → Vec<CellData> → 渲染线程）。
    pub(crate) cell_data_tx: Option<flume::Sender<(Vec<CellData>, CursorInfo)>>,
}
