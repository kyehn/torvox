use super::GhosttyTerminal;

use std::sync::Arc;
#[cfg(test)]
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, Ordering};

use libghostty_vt::Terminal;
use libghostty_vt::key::{self, Mods};
use libghostty_vt::mouse;
use libghostty_vt::paste;
use libghostty_vt::render::{CellIterator, RenderState, RowIterator, Snapshot};
use libghostty_vt::screen::{CellWide, GridRef};
use libghostty_vt::search::{MatchBuffer, Scroll, Search};
use libghostty_vt::selection::{Order, SelectWordOptions, Selection};
use libghostty_vt::style::PaletteIndex;
use libghostty_vt::terminal::{Mode, Point, PointCoordinate, PointSpace};

use super::commands::{Command, Query, RunConfig};
use super::types::*;
use flume::Sender;

/// 经通道发送值，失败时记日志。
fn try_send<T>(sender: &Sender<T>, value: T, context: &str) {
    if let Err(error) = sender.send(value) {
        log::error!("ghostty_terminal: {context}: channel send failed: {error}");
    }
}

/// 上游 `paste::encode` 的输出缓冲管理：输出恒比输入长（bracketed 包裹与
/// 控制字节替换），先按输入长度加裕量一次成形；不足时按上游回告的 `required`
/// 精确扩容。`required` 为 0 是上游未填值，此时翻倍避免原地空转。
/// 其他错误记日志后返回空（调用方丢弃本次粘贴）。
fn encode_paste_data(data: &[u8], bracketed: bool) -> Vec<u8> {
    let mut working = data.to_vec();
    let mut output = vec![0u8; data.len() + 16];
    loop {
        match paste::encode(&mut working, bracketed, &mut output) {
            Ok(written) => return output[..written].to_vec(),
            Err(libghostty_vt::Error::OutOfSpace { required }) => {
                let grown = if required > output.len() {
                    required
                } else {
                    output.len().saturating_mul(2)
                };
                output.resize(grown, 0);
            }
            Err(error) => {
                log::warn!("ghostty_terminal: paste encode failed: {error}");
                return Vec::new();
            }
        }
    }
}

// ── 自由函数 ────────────────────────────────────────────────────

/// OSC 8 超链接内联缓冲（4KiB）：常见 URI 一次取回，超长按需增长。
const HYPERLINK_INLINE_BUFFER_SIZE: usize = 4096;
/// 超链接 URI 上限（64KiB）：`OutOfSpace{required}` 重试的封顶，避免无界分配。
const MAX_HYPERLINK_URI_BYTES: usize = 64 * 1024;
/// OSC 52 剪贴板负载上限（1MiB）：远超任何真实剪贴板内容，又不至于让一份异常负载
/// 拖垮进程内存（负载经 `String` 事件 → 事件队列 → Kotlin 剪贴板逐层复制）。
const MAX_CLIPBOARD_PAYLOAD_BYTES: usize = 1024 * 1024;
/// 选区形状：块选通道已移除（无入口、无 JNI 参数），选区固定为线性，故 rectangle 恒假。
/// 具名而非就地写 `false`：两处安装/格式化入口共用该口径，就地字面量会让
/// 「将来若引入块选」这件事看不出还需要改几个点。
const SELECTION_RECTANGLE: bool = false;

/// 16 个标准 ANSI 调色板索引，按 xterm 顺序（常规色在前，亮色在后）。
/// `libghostty_vt::Palette` 只暴露具名的 `PaletteIndex` 常量，
/// 故须显式把主题色映射到这些索引上。
const ANSI_PALETTE_INDICES: [PaletteIndex; 16] = [
    PaletteIndex::BLACK,
    PaletteIndex::RED,
    PaletteIndex::GREEN,
    PaletteIndex::YELLOW,
    PaletteIndex::BLUE,
    PaletteIndex::MAGENTA,
    PaletteIndex::CYAN,
    PaletteIndex::WHITE,
    PaletteIndex::BRIGHT_BLACK,
    PaletteIndex::BRIGHT_RED,
    PaletteIndex::BRIGHT_GREEN,
    PaletteIndex::BRIGHT_YELLOW,
    PaletteIndex::BRIGHT_BLUE,
    PaletteIndex::BRIGHT_MAGENTA,
    PaletteIndex::BRIGHT_CYAN,
    PaletteIndex::BRIGHT_WHITE,
];

/// VT 线程等待到的事件：命令通道或查询通道就绪。
///
/// 两条通道必须一起等：只等命令通道时，查询要等命令批尾或空闲节拍
/// （`VT_IDLE_TICK_MS`）到点才被排空——空闲终端上每次查询都等满一个节拍，
/// 渲染线程的 kitty 放置查询每等满一节拍就丢掉一帧（真机实测 ~54ms 慢帧）。
enum VtEvent {
    /// 命令通道就绪；`Err` 表示通道断开（会话销毁）。
    Command(Result<Command, flume::RecvError>),
    /// 查询通道就绪；`Err` 表示通道断开（会话销毁）。
    Query(Result<Query, flume::RecvError>),
}

/// VT 线程中与终端借用无关的可变状态。
///
/// 终端、行提取器与搜索对象之间存在借用关系（`Search` 的生命周期挂在终端的
/// 分配器上），不能收进同一个结构体；其余量没有此约束，集中存放便于按职责
/// 拆分命令处理。
struct CommandState {
    /// 主题默认背景色（`Command::SetTheme` 更新）。
    default_background: [f32; 4],
    /// 主题默认前景色（`Command::SetTheme` 更新）。
    default_foreground: [f32; 4],
    /// 缓存上次构建的网格快照：当网格内容与滚动偏移自上一帧以来都没变时，
    /// 跳过昂贵的逐格 ghostty FFI 重建。VT 线程单线程且顺序处理命令，
    /// 标记 `grid_dirty` 与重建之间不存在竞争。
    cached_snapshot: Option<Arc<GridSnapshot>>,
    /// 全量文本（`Query::ReadAllText`）缓存。网格一变即失效，与 `grid_dirty` 同源：
    /// 逐格走 ghostty FFI 的重建在回滚填满时单趟就会越过 QUERY_TIMEOUT_MS，
    /// 未缓存时每次轮询都重建，查询方反复读到超时后的空值。
    cached_all_text: Option<String>,
    /// 自动推送路径的内容去重：`push_cell_data` 每轮循环（包括 50ms 空闲
    /// 超时）都会构建整个网格，而渲染线程把收到的每批都当作
    /// `is_new_data` → 每帧全量重绘。空闲终端会让渲染器满速空转
    /// （SwiftShader 模拟器上实测 ~5 fps，真实 GPU 上也烧 CPU：
    /// 「20fps 不可接受」）。当单元与光标自上次推送以来都没变时，
    /// 直接跳过发送。
    last_cell_data_push: Option<(Vec<CellData>, CursorInfo)>,
    /// 网格是否已变更（快照缓存的失效标记）：与 `cached_all_text` 同源失效，
    /// 但它会在 `TakeSnapshot` 消费后清除。
    grid_dirty: bool,
    /// CellData 重建的开关：仅命令/查询导致状态变化后才重建。空闲 50ms tick
    /// 不再全幅重建（2400+ 次 Ghostty FFI 调用），重复重建只靠
    /// `last_cell_data_push` 去重抑制发送，CPU 清零。
    cell_data_dirty: bool,
    /// 同步更新（模式 2026）的保持起点：引擎不会自行结束保持，
    /// 程序崩溃或忘记 ESU 时由这里超时解锁（见 `synchronized_output_hold`）。
    synchronized_output_held_since: Option<std::time::Instant>,
}

impl CommandState {
    /// 以主题调色板初始化：网格与 CellData 均视为已脏，快照与文本缓存为空。
    fn new(palette: &crate::terminal::theme::Palette) -> Self {
        Self {
            default_background: GhosttyTerminal::color_to_float(palette.background),
            default_foreground: GhosttyTerminal::color_to_float(palette.foreground),
            cached_snapshot: None,
            cached_all_text: None,
            last_cell_data_push: None,
            grid_dirty: true,
            cell_data_dirty: true,
            synchronized_output_held_since: None,
        }
    }
}

/// 一批命令的批级状态。
#[derive(Default)]
struct BatchState {
    /// 本批是否已有内容变更（输出/主题/尺寸/选区/重置）。纯视口滚动才能
    /// 平移行缓存；若先有内容变更再滚动，缓存相对新网格已过期，必须整帧重建。
    content_changed: bool,
    /// 批尾统一释放的 flush 应答。
    acks: Vec<flume::Sender<()>>,
}

/// 单条命令的处理结果。
enum CommandOutcome {
    /// 继续处理本批的下一条命令。
    Continue,
    /// 结束本批：不再取下一条命令（尺寸被拒等情形），留到下一轮唤醒再处理。
    EndBatch,
    /// 终止 VT 线程。
    Terminate,
}

/// 查询处理资源：编码器、搜索索引与 Kitty 放置缓存。
///
/// 与终端借用无关（`Search` 只以 `PhantomData` 关联终端的分配器生命周期），
/// 故可整体传递，避免每个处理函数都带一长串参数。
struct QueryContext<'term> {
    /// 逐事件复用的上游编码器（见 [`Encoders`]）：每个终端只分配一次，编码器
    /// 选项在每个事件前经 `set_options_from_terminal` 从终端重新同步。
    encoders: Encoders<'term>,
    /// 上游 `search::Search` 的宿主：与终端同生命周期的局部变量，故搜索对象
    /// 不必与终端自引用（`Search` 的生命周期挂在 `Terminal` 的分配器生命周期上，
    /// 不占用借用）。首次搜索时惰性创建，此后复用同一份索引状态。
    search: Option<Search<'term, 'term>>,
    /// 命中读出用的存储，跨查询复用（上游按需扩容）。
    match_buffer: MatchBuffer,
    /// Kitty 放置查询缓存：跨帧复用图像解码与未变化行的占位符扫描。
    kitty_cache: KittyPlacementCache,
}

impl<'term> QueryContext<'term> {
    /// 编码器分配失败即返回错误：没有键编码器等于收不到任何按键，属不可降级。
    fn new() -> Result<Self, libghostty_vt::Error> {
        Ok(Self {
            encoders: Encoders::new()?,
            search: None,
            match_buffer: MatchBuffer::new(),
            kitty_cache: KittyPlacementCache::default(),
        })
    }
}

/// 终端网格列数：ghostty 断开时回退 DISCONNECTED_COLS。调用方一律经此取值，
/// 不重复写 `unwrap_or` 链。
fn grid_cols(terminal: &Terminal) -> u32 {
    terminal.cols().unwrap_or(DISCONNECTED_COLS as u16) as u32
}

/// 终端网格行数：ghostty 断开时回退 DISCONNECTED_ROWS（与 `grid_cols` 对称）。
fn grid_rows(terminal: &Terminal) -> u32 {
    terminal.rows().unwrap_or(DISCONNECTED_ROWS as u16) as u32
}

/// 回滚行数：ghostty 断开时为 0（与 `grid_rows` 对称）。
/// FFI 报错时记下原因再按 0 处理——静默吞掉会让「回滚量=0」的下游断言
/// 只能猜错误来源（本机 cargo 全量套件曾撞上一次，无法归因）。
fn scrollback_len(terminal: &Terminal) -> u32 {
    terminal.scrollback_rows().unwrap_or_else(|error| {
        log::warn!("ghostty_terminal: scrollback_rows query failed: {error}");
        0
    }) as u32
}

/// 单格码点直读：调用方保证列在界内，不逐格查网格宽度。
fn cell_codepoint(terminal: &Terminal, row: u32, col: u32) -> u32 {
    terminal
        .grid_ref(Point::Screen(PointCoordinate {
            x: col as u16,
            y: row,
        }))
        .ok()
        .and_then(|point| point.cell().ok())
        .and_then(|cell| cell.codepoint().ok())
        .unwrap_or(0)
}

/// 内容变脏：网格与全量文本缓存同源失效，调用方再按需置批量脏。
fn mark_grid_dirty(grid_dirty: &mut bool, cached_all_text: &mut Option<String>) {
    *grid_dirty = true;
    *cached_all_text = None;
}

/// 下划线是否需要绘制：除无下划线外五种形状均绘制。
fn has_visible_underline(underline: libghostty_vt::style::Underline) -> bool {
    matches!(
        underline,
        libghostty_vt::style::Underline::Single
            | libghostty_vt::style::Underline::Double
            | libghostty_vt::style::Underline::Curly
            | libghostty_vt::style::Underline::Dashed
            | libghostty_vt::style::Underline::Dotted
    )
}

/// 逐帧渲染迭代器与其宿主行缓存。
///
/// 三者与 VT 线程同生命周期，不逐帧新建：上游 [`RenderState`] 跨帧持有上次快照
/// 状态，逐帧新建会使 `rows = 0` 初值把每次 [`RenderState::update`] 判为
/// 「维度变化 → 全量重建」，行级脏标记恒为全脏。`RowCache` 只服务于
/// [`Self::build_cell_data`]（快照路径整帧重建，不做行级缓存）。
pub(crate) struct RenderExtractors<'alloc> {
    render_state: RenderState<'alloc>,
    row_iter: RowIterator<'alloc>,
    cell_iter: CellIterator<'alloc>,
    row_cache: RowCache,
    /// 上一批命令只有视口滚动时记录的实际行偏移（终端语义：向下为正）。
    /// 下一帧据此平移 `RowCache` 并只逐格抽取露出的边缘行；任何内容变更
    /// 或缓存失效都会清空它，避免拿平移后的旧内容冒充新内容。
    scroll_shift: Option<isize>,
    /// VT 已处理的最后一个滚动请求序号（0 = 从未处理过滚动命令）。
    ///
    /// 随帧回传给渲染侧：帧携带的序号小于宿主已下发的最新序号即说明该帧
    /// 早于在途的滚动命令，呈现它会用旧视口配新余量。
    scroll_request_id: u64,
    /// 行级分流计数，仅测试使用：按终端实例隔离，避免并行测试互相污染。
    #[cfg(test)]
    pub(crate) cache_stats: CacheStats,
}

impl<'alloc> RenderExtractors<'alloc> {
    /// 创建失败即记日志并返回 `None`：缺了它们既取不到网格也读不到光标，
    /// 以「本帧返回空网格」延后只会在第一次渲染时静默降级。
    pub(crate) fn new() -> Option<Self> {
        let render_state = match RenderState::new() {
            Ok(initialized_state) => initialized_state,
            Err(state_error) => {
                log::error!("RenderExtractors: RenderState::new() failed: {state_error}");
                return None;
            }
        };
        let row_iter = match RowIterator::new() {
            Ok(initialized_rows) => initialized_rows,
            Err(row_error) => {
                log::error!("RenderExtractors: RowIterator::new() failed: {row_error}");
                return None;
            }
        };
        let cell_iter = match CellIterator::new() {
            Ok(initialized_cells) => initialized_cells,
            Err(cell_error) => {
                log::error!("RenderExtractors: CellIterator::new() failed: {cell_error}");
                return None;
            }
        };
        Some(Self {
            render_state,
            row_iter,
            cell_iter,
            row_cache: RowCache::new(),
            scroll_shift: None,
            scroll_request_id: 0,
            // 测试构建下由 `run_inner` 从 `RunConfig` 换成该终端实例的句柄；
            // 生产构建无此字段，故此处为占位。
            #[cfg(test)]
            cache_stats: CacheStats::default(),
        })
    }

    /// 三个迭代器字段作为可变引用一次性解出：`build_cell_data` 需要同时独占
    /// 借用它们（上游 `RowIteration` 借用 `RowIterator` 与 `Snapshot`，
    /// `CellIteration` 借用 `RowIteration`），逐字段借用会与 `row_cache`
    /// 的可变借用冲突。
    fn parts(
        &mut self,
    ) -> (
        &mut RenderState<'alloc>,
        &mut RowIterator<'alloc>,
        &mut CellIterator<'alloc>,
        &mut RowCache,
    ) {
        (
            &mut self.render_state,
            &mut self.row_iter,
            &mut self.cell_iter,
            &mut self.row_cache,
        )
    }

    /// 行缓存整体失效（网格尺寸变化、终端重置、主题变更、选区安装或清除）。
    /// 上游脏状态按实例保存，故本方法只清本仓缓存，`RenderState` 仍如实反映
    /// 终端的新脏状态——它会在下一次 `update` 时看到尺寸变化或全量脏标记。
    pub(crate) fn invalidate_row_cache(&mut self) {
        self.row_cache.invalidate();
        self.scroll_shift = None;
    }

    /// 记录一次视口滚动实际移动的行数（终端语义：向下为正）。
    ///
    /// 同一批内多次滚动累加；`None` 表示本次滚动没有可平移的缓存
    /// （例如已在底部，或内容刚变更），下一帧走完整抽取。
    pub(crate) fn note_scroll(&mut self, delta: isize) {
        self.scroll_shift = Some(self.scroll_shift.unwrap_or(0).saturating_add(delta));
    }

    /// 内容变更后丢弃待平移量：平移帧会跳过上游脏标记直接复用整行，
    /// 若输出发生在滚动之后，旧缓存相对新网格已过期，必须回到逐行脏判定。
    fn clear_scroll_shift(&mut self) {
        self.scroll_shift = None;
    }

    /// 取走平移量：只对紧随其后的那一帧生效，避免陈旧偏移在后续帧继续套用。
    fn take_scroll_shift(&mut self) -> Option<isize> {
        self.scroll_shift.take()
    }

    /// 记下刚处理的滚动请求序号：随下一帧回传给渲染侧。
    fn note_scroll_request(&mut self, request_id: u64) {
        self.scroll_request_id = request_id;
    }
}

/// 上一帧的扁平 `CellData` 与其按行划分的区间。
///
/// 干净行整段复制自这里，故行边界必须由 [`build_row_ranges`] 依
/// `CellData.row` 划分而非按 `cols` 反推：宽字符被上游省略，各行单元数不同。
pub(crate) struct RowCache {
    cells: Vec<CellData>,
    ranges: Vec<std::ops::Range<usize>>,
}

/// 行级路径的可观测计数，仅测试使用。
///
/// 行级增量与全量重建产出逐字节相同，内容断言无法区分二者（增量正确但从未
/// 命中时，用例照样全绿）；这两个计数是「行级路径确实生效」的唯一判别力来源。
///
/// 按终端实例隔离而非进程全局：测试并行执行，全局计数会被同进程其他终端的
/// 抽取淹没，「本帧复用 0 行」这类断言因此不可判别。句柄经 [`RunConfig`] 传入，
/// 测试从自己的终端句柄取同一实例。
#[cfg(test)]
#[derive(Clone, Default)]
pub(crate) struct CacheStats(Arc<(AtomicU64, AtomicU64)>);

#[cfg(test)]
impl CacheStats {
    pub(crate) fn record(&self, reused: u64, extracted: u64) {
        self.0.0.fetch_add(reused, Ordering::Relaxed);
        self.0.1.fetch_add(extracted, Ordering::Relaxed);
    }

    /// 取出并清零：返回自上次取出以来本终端的复用行数与逐格行数。
    pub(crate) fn take(&self) -> (u64, u64) {
        (
            self.0.0.swap(0, Ordering::Relaxed),
            self.0.1.swap(0, Ordering::Relaxed),
        )
    }
}

impl RowCache {
    pub(crate) fn new() -> Self {
        Self {
            cells: Vec::new(),
            ranges: Vec::new(),
        }
    }

    /// 缓存不覆盖本次抽取的网格尺寸时无法复用（行区间按行数划分）。
    pub(crate) fn covers(&self, rows: u32) -> bool {
        self.ranges.len() == rows as usize
    }

    /// 交出缓存内容与行边界，供本帧抽取期间充当干净行来源。
    ///
    /// 取走而非克隆：本帧结束后 [`Self::store`] 用本帧输出重建，无需保留旧缓冲。
    /// 克隆 24×80 网格（约 150KB）会把省下的逐单元 FFI 往返换成与被否方案同型的
    /// 拷贝成本。
    fn take_previous(&mut self) -> (Vec<CellData>, Vec<std::ops::Range<usize>>) {
        (
            std::mem::take(&mut self.cells),
            std::mem::take(&mut self.ranges),
        )
    }

    /// 按视口滚动平移上一帧的行缓存。
    ///
    /// `delta` 为终端语义（向下为正）：新行 `i` 复用旧行 `i + delta`，
    /// 越界行留下空区间，由本帧逐格抽取补齐。返回逐行「可直接复用」标记；
    /// 平移后单元的行号同步改成新行号，列与样式保持不变（内容整体移动）。
    fn shift_previous(
        cells: &mut Vec<CellData>,
        ranges: &mut Vec<std::ops::Range<usize>>,
        delta: isize,
        rows: u32,
    ) -> Vec<bool> {
        let mut shifted_cells = Vec::with_capacity(cells.len());
        let mut shifted_ranges = Vec::with_capacity(rows as usize);
        let mut reusable = vec![false; rows as usize];
        for row in 0..rows {
            let old_row = row as isize + delta;
            if old_row >= 0 && old_row < rows as isize {
                let old_range = ranges.get(old_row as usize).cloned().unwrap_or(0..0);
                let start = shifted_cells.len();
                shifted_cells.extend_from_slice(&cells[old_range]);
                for cell in &mut shifted_cells[start..] {
                    cell.row = row;
                }
                shifted_ranges.push(start..shifted_cells.len());
                reusable[row as usize] = true;
            } else {
                shifted_ranges.push(shifted_cells.len()..shifted_cells.len());
            }
        }
        *cells = shifted_cells;
        *ranges = shifted_ranges;
        reusable
    }

    /// 记录本帧输出与其行边界。行边界推导失败时保持缓存为空：
    /// 错位的区间会把别行的内容写进本行，比不缓存更糟。
    ///
    /// 拷贝一整份本帧网格换取后续帧免去逐单元 FFI 往返——只在本帧确有脏行
    /// （即确有新内容）时才走到这里。
    pub(crate) fn store(&mut self, data: &[CellData], rows: u32) {
        match crate::terminal::ghostty_terminal::build_row_ranges(data, rows) {
            Some(ranges) => {
                self.cells = data.to_vec();
                self.ranges = ranges;
            }
            None => {
                log::error!("build_cell_data: row ranges mismatch — cache invalidated");
                self.invalidate();
            }
        }
    }

    /// 该网格行在缓冲中的区间；行数不匹配时返回空切片，
    /// 调用方据此走逐格抽取而非误用错位区间。
    fn row_slice<'a>(
        cells: &'a [CellData],
        ranges: &[std::ops::Range<usize>],
        row: u32,
    ) -> &'a [CellData] {
        match ranges.get(row as usize) {
            Some(range) => &cells[range.clone()],
            None => &[],
        }
    }

    /// 整体失效：下一帧必须逐格重建。用于网格尺寸变化、重置、主题或选区变更
    /// ——这些情形下缓存内容与新网格不可比。
    pub(crate) fn invalidate(&mut self) {
        self.cells.clear();
        self.ranges.clear();
    }
}

/// 视口相对的光标位置与可见性（网格行、网格列、是否可见）。
///
/// 一律取 [`Snapshot::cursor_viewport`] 而非活动屏 `cursor_y()`：用户把回滚滚进视口后
/// 活动屏行号与视口行分叉，光标会匹配到错误的网格行（真机上表现为光标「块」按滚动量
/// 向右下跳约一格）。光标所在页不在可见视口内时上游返回 `None`（如大幅滚动），
/// 此时不画光标。
///
/// `build_cell_data` 与 `build_snapshot` 两条抽取路径共用本函数：两份判读一旦分叉，
/// 同一终端状态下 CellData 与快照会给出不同的光标行。
fn cursor_in_viewport(snapshot: &Snapshot<'_, '_>) -> (u32, u32, bool, bool) {
    match snapshot.cursor_viewport() {
        Ok(Some(cursor_viewport)) => (
            cursor_viewport.y as u32,
            cursor_viewport.x as u32,
            cursor_viewport.at_wide_tail,
            snapshot
                .cursor_visible()
                .unwrap_or(DISCONNECTED_CURSOR_VISIBLE),
        ),
        Ok(None) | Err(_) => (0, 0, false, false),
    }
}

/// 单元帧被丢弃告警的限频。
///
/// 渲染暂停期（设置页/输入法弹出）**刻意不消费**通道——`receive` 是破坏性取数，
/// 暂停帧取走后永不呈现，恢复时 VT 的去重基线也不会重推，那正是「IME 弹出时输入
/// 不可见」的主因。帧于是留在通道里，而通道有界：终端继续输出就把它填满，此后
/// **每一次**推送都失败。实测设备上这是每秒约 40 条、持续到恢复为止的告警洪水。
///
/// 逐条记没有任何诊断价值（每一条都只是同一个「通道满」），却会挤爆 logcat 并持续
/// 吃 CPU。故限频；恢复后通道排空时自然不再命中限频，首条即重新可见。
///
/// 闸门与渲染侧慢帧诊断共用（`crate::slow_log`）：窗口更长的差异只体现为窗口
/// 长度，间隔判定仍只此一处——私有一份时口径分叉，间隔下限便失去保证。
const CELL_FRAME_DROP_WARN_INTERVAL_MS: u64 = 5_000;
static CELL_FRAME_DROP_WARN_THROTTLE: crate::slow_log::SlowLogThrottle =
    crate::slow_log::SlowLogThrottle::with_interval(CELL_FRAME_DROP_WARN_INTERVAL_MS);

/// 记一条单元帧丢弃告警；返回真表示本次真的写出了告警（未被限频压住）。
fn warn_drop_throttled(error: &flume::TrySendError<(Vec<CellData>, CursorInfo)>) -> bool {
    if !CELL_FRAME_DROP_WARN_THROTTLE.should_log() {
        return false;
    }
    log::warn!(
        "ghostty_terminal: cell frame dropped ({error}) — further drops within {}ms are not logged",
        CELL_FRAME_DROP_WARN_INTERVAL_MS,
    );
    true
}

/// 同步更新（模式 2026）的发布门控：BSU（`2026h`）到 ESU（`2026l`）之间不发布
/// 中间帧，渲染线程继续显示进入保持前的那一帧；ESU 的写入重新置脏并在批尾发布
/// 最终帧。
///
/// 引擎没有时钟、也不会自行结束保持——程序崩溃或忘记发 ESU 时屏幕会永久冻结。
/// 上游建议宿主超时解锁，这里取 1 秒：超时后复位模式并立即发布当前帧（日志留痕）。
/// 返回 `true` 表示仍处于保持中、本帧应跳过发布。
fn synchronized_output_hold(
    terminal: &mut Terminal,
    held_since: &mut Option<std::time::Instant>,
) -> bool {
    let active = match terminal.mode(Mode::SYNC_OUTPUT) {
        Ok(active) => active,
        Err(error) => {
            log::error!("ghostty_terminal: sync output mode query failed: {error}");
            false
        }
    };
    if !active {
        *held_since = None;
        return false;
    }
    let started = held_since.get_or_insert_with(std::time::Instant::now);
    if started.elapsed() < SYNC_OUTPUT_HOLD_TIMEOUT {
        return true;
    }
    log::warn!(
        "ghostty_terminal: synchronized output held for over {}ms without release; forcing frame",
        SYNC_OUTPUT_HOLD_TIMEOUT.as_millis()
    );
    if let Err(error) = terminal.set_mode(Mode::SYNC_OUTPUT, false) {
        log::error!("ghostty_terminal: sync output mode release failed: {error}");
    }
    *held_since = None;
    false
}

/// 保持超过该时长即视为程序已放弃 ESU（上游推荐 1 秒）。
const SYNC_OUTPUT_HOLD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);

/// 是否为双下划线：单独成函数，与绘制判定同源，避免三处散落比较。
fn is_double_underline(underline: libghostty_vt::style::Underline) -> bool {
    underline == libghostty_vt::style::Underline::Double
}

/// 创建三个逐帧渲染迭代器。
/// 任一创建失败即记日志并返回 `None`。
///
/// 迭代器与 VT 线程同生命周期（见 [`Self::run`] 的持有点），不逐帧新建：上游
/// [`RenderState`] 跨帧持有上次快照状态，逐帧新建会使其 `rows = 0` 初值把每次
/// [`RenderState::update`] 判为「维度变化」而重建全部行，行级脏跟踪恒为全脏。
fn create_render_iterators() -> Option<(
    RenderState<'static>,
    RowIterator<'static>,
    CellIterator<'static>,
)> {
    let render_state = match RenderState::new() {
        Ok(initialized_state) => initialized_state,
        Err(state_error) => {
            log::error!("create_render_iterators: RenderState::new() failed: {state_error}");
            return None;
        }
    };
    let row_iter = match RowIterator::new() {
        Ok(initialized_rows) => initialized_rows,
        Err(row_error) => {
            log::error!("create_render_iterators: RowIterator::new() failed: {row_error}");
            return None;
        }
    };
    let cell_iter = match CellIterator::new() {
        Ok(initialized_cells) => initialized_cells,
        Err(cell_error) => {
            log::error!("create_render_iterators: CellIterator::new() failed: {cell_error}");
            return None;
        }
    };
    Some((render_state, row_iter, cell_iter))
}

/// 决定 VT 线程是否必须从终端重建网格快照，而非克隆上一次建好的缓存快照。
///
/// 仅在网格内容变化（`grid_dirty`，由 `Command::Write` / `Resize` / `SetTheme`
/// 与 `Command::ScrollViewport` 置位）或尚无缓存时重建。两者都不成立时网格内容与缓存
/// 逐字节相同，复用它不会产出陈旧帧，同时省下约 1920 次逐单元 ghostty FFI 调用。
///
/// 注意签名里没有「滚动偏移」参数：`TakeSnapshot` 只覆盖当前视口，回滚浏览走 CellData
/// 通道，故视口偏移变化由 `ScrollViewport` 自行置脏来覆盖，而不是在此比较。
pub(crate) fn snapshot_needs_rebuild(grid_dirty: bool, has_cache: bool) -> bool {
    grid_dirty || !has_cache
}

/// 逐事件复用的上游编码器（每个终端一份）。`Encoder` 在事件之间保留状态，
/// `Event` 复用使热路径不再每次分配。编码器的选项每个事件前经
/// `set_options_from_terminal` 从终端重新同步；`Event` 的字段没有回溯入口，
/// 故 [`Query::EncodeKeyEvent`] 的每个字段都必须无条件覆写，漏写即以上一键的值
/// 充当本次输入。
///
/// 鼠标协议是可选特性：编码器分配失败只记日志并置 `None`（事件被丢弃）。
/// 键盘不是——没有键编码器就等于收不到任何按键，属不可降级，故键编码器分配
/// 失败由 [`Encoders::new`] 返回错误并终止 VT 线程，不静默降级。
pub(crate) struct Encoders<'alloc> {
    mouse: Option<(mouse::Encoder<'alloc>, mouse::Event<'alloc>)>,
    key: (key::Encoder<'alloc>, key::Event<'alloc>),
}

impl Encoders<'_> {
    fn new() -> Result<Self, libghostty_vt::Error> {
        Ok(Self {
            mouse: match (mouse::Encoder::new(), mouse::Event::new()) {
                (Ok(encoder), Ok(event)) => Some((encoder, event)),
                (Err(error), _) | (_, Err(error)) => {
                    log::warn!("ghostty_terminal: mouse encoder unavailable: {error}");
                    None
                }
            },
            key: (key::Encoder::new()?, key::Event::new()?),
        })
    }
}
mod commands;
mod dispatch;
mod kitty;
mod mouse_input;
mod rendering;
mod run;
mod selection;
#[cfg(test)]
mod tests;

pub(crate) use kitty::KittyPlacementCache;
