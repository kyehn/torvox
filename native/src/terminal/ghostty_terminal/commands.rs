use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};
use std::sync::{Arc, Mutex};

use flume::{Receiver, Sender};
use libghostty_vt::key;

use super::frame_mailbox::FrameMailbox;
use super::types::*;

/// 调用方发往 VT 线程的命令，按序处理。
///
/// 通道**有界**，VT 线程卡住时内存不会无限增长；发送方用 `try_send` 并回退到缓存值。
/// 无状态查询归 [`Query`]，两条通道使动作顺序与查询延迟互不干扰。
pub enum Command {
    Write(Vec<u8>),
    FlushAck(Sender<()>),
    SetTheme(&'static crate::terminal::theme::Theme),
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
    /// 取当前视口网格快照。回滚浏览走单元帧邮箱（`ScrollViewport` +
    /// `receive_cell_data`），本命令只覆盖当前视口，因此没有偏移参数。
    TakeSnapshot {
        tx: Sender<Arc<GridSnapshot>>,
    },
    /// 按增量滚动终端视口（向上为负），供应用浏览回滚。
    ///
    /// `request_id` 是本次请求的序号（进程级单调递增）：VT 处理命令后把序号随帧
    /// 回传，渲染侧据此判断收到的帧是否早于在途的滚动命令。
    ScrollViewport {
        delta: isize,
        request_id: u64,
    },
    /// 安装终端持有的线性活动选区（跟踪网格引用，随滚动/输出/重排跟随文本）。
    /// 坐标为绝对网格行（0 = 回滚顶部）与列。
    SetSelection {
        start: (u32, u32),
        end: (u32, u32),
    },
    ClearSelection,
    /// RIS 全重置：恢复终端初始状态并清空回滚（侧边面板“重置终端”按钮）。
    Reset,
    /// 请求按当前状态重推一帧 CellData（不改变终端状态）。
    ///
    /// 用于累加器里的像素属于别人而本会话又没有新输出的场合（切会话、换 surface）：
    /// 重推的帧才让渲染侧把整幅重画一遍，否则屏幕停在上一个会话的内容上。
    Repaint,
    Terminate,
}

/// 滚动请求序号：进程级单调递增（1 起）。
///
/// 序号不复用：会话重启后新请求的序号恒大于重启前的任何序号，渲染侧
/// 「帧序号 < 在途请求序号 ⇒ 帧早于该次滚动」的判定不会把重启前的帧误判为最新。
static SCROLL_REQUEST_SEQ: AtomicU64 = AtomicU64::new(0);

/// 分配下一个滚动请求序号。
pub fn next_scroll_request_id() -> u64 {
    SCROLL_REQUEST_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
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
    ModeGet(libghostty_vt::terminal::Mode, Sender<bool>),
    ScrollbackLength(Sender<u32>),
    ReadLineText {
        row: u32,
        tx: Sender<Option<String>>,
    },
    /// 经 `build_cell_data` 读取的光标视口 (row, col)，即渲染线程消费的同一数据源，
    /// 可断言与 GPU 绘制一致的坐标。光标隐藏或构建失败时为 None。
    RenderCursor(Sender<Option<(u32, u32)>>),
    ReadVisibleText(Sender<String>),
    /// 视口 + 回滚区以换行拼接的全量文本。仅读 codepoint，不构建前景/背景/下划线
    /// 等样式，故与 [`Query::DumpGrid`] 相比每格少三次跨语言调用。
    ReadAllText(Sender<String>),
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
    /// 设置搜索 needle 并同步跑完搜索，返回命中总数；空 needle 清除并归零。
    ///
    /// 匹配、软换行跨行、增量同步与滚动策略全部由上游 `search::Search` 承担，
    /// `search` 对象随 VT 线程存活，跨查询复用同一份索引状态。
    SearchSetNeedle {
        needle: String,
        tx: Sender<usize>,
    },
    /// 选中下一个（`forward = true`，朝更旧内容）或上一个命中，返回选中态；
    /// 无命中时 None。选中时不移动视口（`Scroll::None`），居中由宿主完成。
    SearchSelect {
        forward: bool,
        tx: Sender<Option<SearchSelection>>,
    },
    /// 绘制视口高亮所需的全部输入：视口命中、选中命中与回滚长度。
    SearchViewportHighlights {
        tx: Sender<SearchViewportHighlights>,
    },
    DumpGrid {
        tx: Sender<DumpedGrid>,
    },
    TakeKittyGraphicsImage {
        id: u32,
        tx: Sender<Option<KittyGraphicsImageData>>,
    },
    /// 采集全部可见 Kitty 放置（含几何 + RGBA），供渲染线程组装图集。
    ///
    /// `row_signatures` 是按视口行下标的占位符布局签名（0 = 该行无占位符），
    /// 由渲染线程从本帧 `CellData` 算出：命中缓存的行不再逐格扫描 FFI。
    TakeKittyPlacements {
        row_signatures: Option<std::sync::Arc<[u64]>>,
        tx: Sender<Vec<KittyPlacementFrame>>,
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
    /// 一次往返编码多档滚轮：`repeat` 个 press+release 对拼进同一个缓冲区。
    /// 触摸拖动一个事件可能滚过数行，逐行发查询会把 UI 线程按在 VT 线程上
    /// （每行两次应答，且每次都可能排在整个 `QUERY_TIMEOUT_MS` 之后）；
    /// 合并成一次往返后，每次手势事件只阻塞一次，PTY 也只写一次。
    EncodeMouseWheel {
        position: (f32, f32),
        /// 3 = 滚轮上（更旧），4 = 滚轮下（更新），与 [`Self::EncodeMouseEvent`] 同号。
        button: u8,
        modifiers: u16,
        cell_width: f32,
        cell_height: f32,
        /// 编码几档，调用方保证 ≥1。
        repeat: u32,
        tx: Sender<Vec<u8>>,
    },
    /// 用上游 `key::Encoder` 把按键事件编码为终端转义序列。DECCKM、keypad 应用
    /// 模式、modifyOtherKeys、kitty 键盘协议与 backarrow key mode 全部由
    /// `set_options_from_terminal` 从终端状态接管，调用方无需查询任何模式。
    ///
    /// 编码必须在 VT 线程完成：`set_options_from_terminal` 需要 `&Terminal`，
    /// 而 `Terminal` 只存在于该线程。
    EncodeKeyEvent {
        key: key::Key,
        action: key::Action,
        /// 上游 `key::Mods` 原始位（`Mods::from_bits_retain` 直接消费）。
        modifiers: u16,
        /// 平台按当前布局与修饰状态生成的文本（如 Shift+A 的 `A`）。无文本的键
        /// （方向键、回车、单独的修饰键）传 `None`——上游文档要求 C0 控制字符与
        /// U+007F 一律以 `None` 表达，由编码器按逻辑键推导。
        text: Option<char>,
        /// 该键无修饰时的码点；kitty 键盘协议据此区分 Shift 前后的大小写档位。
        unshifted: Option<char>,
        tx: Sender<Vec<u8>>,
    },
    /// 用上游 `paste::encode` 把剪贴板文本编码为 PTY 写入字节。bracketed 标志
    /// 取自终端模式 2004 的活体状态：启用时包裹 `ESC[200~`/`ESC[201~` 并保留换行，
    /// 未启用时换行转回车；控制字节一律由上游中化为空格。
    ///
    /// 编码必须在 VT 线程完成：模式状态只存在于该线程的终端里。
    /// 不取 `Terminal::paste`：其拒绝流程要求确认对话框（PROHIBITED 明禁）。
    EncodePaste {
        /// 剪贴板 UTF-8 字节（调用方已截断）。
        data: Vec<u8>,
        tx: Sender<Vec<u8>>,
    },
}

pub(crate) struct RunConfig {
    pub(crate) command_receiver: Receiver<Command>,
    pub(crate) query_receiver: Receiver<Query>,
    pub(crate) rows: u32,
    pub(crate) cols: u32,
    pub(crate) scrollback_lines: u32,
    /// 初态主题：调色板经 [`crate::terminal::theme`] 单源解析，`SetTheme` 换主题后
    /// 以命令携带的新主题为准。
    pub(crate) theme: &'static crate::terminal::theme::Theme,
    /// Kitty 文件介质允许的临时目录（客户端 `t=t` 的路径须落在这里）。
    /// 与子进程 `TMPDIR` 同源；`None` 时上游拒绝全部临时文件传输。
    pub(crate) kitty_temp_dir: Option<std::path::PathBuf>,
    pub(crate) response_buffer: Arc<Mutex<Vec<Vec<u8>>>>,
    /// 备用屏状态的无锁镜像，由 VT 线程每帧更新，供输入路径免阻塞 RPC 检出。
    pub(crate) alt_screen_active: Arc<AtomicBool>,
    /// 鼠标上报（DECSET 1000/1002/1003）是否启用的无锁镜像，同样每帧刷新。
    /// 触摸拖动每个事件都要判一次「远端要不要滚轮」，走查询通道会把 UI 线程
    /// 钉在 VT 线程的一次应答上；读原子量则是纯本地判断。
    pub(crate) mouse_tracking_active: Arc<AtomicBool>,
    /// VT 线程置位表示 `Terminal::resize` 失败（网格未真正改尺寸）。
    pub(crate) resize_failed: Arc<AtomicBool>,
    /// 上游 OSC 52 回调事件通道（VT 线程推送，调用方轮询）：剪贴板写入。
    /// 有界丢弃——VT 线程永不阻塞；Kotlin 经 session 锁存槽读取。
    pub(crate) clipboard_tx: flume::Sender<(String, String)>,
    /// 当前单元格像素几何（XTWINOPS 14/16t 应答用）：Resize 回填默认值，
    /// SetCellPixelSize 回填真实字形度量；回调经此共享，无锁读取。
    pub(crate) cell_size_px: Arc<(AtomicU32, AtomicU32)>,
    /// 单元帧邮箱：VT 线程在网格变化时构建并写入 `Vec<CellData>`，
    /// 即线程拆分架构的数据路径（会话线程 → Vec<CellData> → 渲染线程）。
    pub(crate) cell_data_mailbox: Arc<FrameMailbox>,
    /// 帧入队后的唤醒回调（会话侧接渲染循环的条件变量）。渲染线程只知道
    /// 「有输出」时会被叫醒，但那一拍 VT 线程可能还没解析完、邮箱里没有新帧，
    /// 于是它重新睡满空闲闭锁——真机上表现为静置 5s 后后台输出晚 0..500ms 上屏。
    /// 帧真正入队时再叫一次，屏幕上的延迟就只剩一帧。
    pub(crate) frame_wake: Option<crate::terminal::session::OutputWake>,
    /// 行级抽取的可观测计数，仅测试使用：生产构建无此字段。
    #[cfg(test)]
    pub(crate) cache_stats: super::internal::CacheStats,
    /// Kitty 占位符扫描计数，仅测试使用。
    #[cfg(test)]
    pub(crate) kitty_stats: super::internal::CacheStats,
}
