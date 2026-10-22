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
use libghostty_vt::search::{MatchBuffer, Search};
use libghostty_vt::selection::{Order, SelectWordOptions, Selection};
use libghostty_vt::style::PaletteIndex;
use libghostty_vt::terminal::{Mode, Point, PointCoordinate, PointSpace};

use super::commands::{Command, Query, RunConfig};
use super::types::*;
use flume::Sender;

/// 经通道发送值，失败时记日志。
fn try_send<T>(sender: &Sender<T>, value: T, context: &str) {
    if let Err(e) = sender.send(value) {
        log::error!("ghostty_terminal: {context}: channel send failed: {e}");
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
static LAST_CELL_FRAME_DROP_WARN_MS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
const CELL_FRAME_DROP_WARN_INTERVAL_MS: u64 = 5_000;

fn warn_drop_throttled(error: &flume::TrySendError<(Vec<CellData>, CursorInfo)>) {
    use std::sync::atomic::Ordering;
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    let last = LAST_CELL_FRAME_DROP_WARN_MS.load(Ordering::Relaxed);
    if now_ms.saturating_sub(last) >= CELL_FRAME_DROP_WARN_INTERVAL_MS
        && LAST_CELL_FRAME_DROP_WARN_MS
            .compare_exchange(last, now_ms, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
    {
        log::warn!(
            "ghostty_terminal: cell frame dropped ({error}) — further drops within {}ms are not logged",
            CELL_FRAME_DROP_WARN_INTERVAL_MS,
        );
    }
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

// ── GhosttyTerminal 的 impl 块 ────────────────────────────────
impl super::GhosttyTerminal {
    /// 处理单条查询。上游 `select_*` 查询会在查询路径内安装选区；成功安装时返回
    /// true，由 run 循环按 `Command::SetSelection` 同款规则失效行缓存并重推帧。
    pub(crate) fn process_query<'term>(
        query: Query,
        terminal: &mut Terminal<'term, '_>,
        all_text: &mut Option<String>,
        encoders: &mut Encoders,
        default_foreground: [f32; 4],
        default_background: [f32; 4],
        search: &mut Option<Search<'_, 'term>>,
        match_buffer: &mut MatchBuffer,
        extractors: &mut RenderExtractors<'term>,
    ) -> bool {
        let mut selection_installed = false;
        match query {
            Query::Rows(tx) => {
                try_send(
                    &tx,
                    grid_rows(terminal),
                    "ghostty_terminal: query channel send failed",
                );
            }
            Query::Cols(tx) => {
                try_send(
                    &tx,
                    grid_cols(terminal),
                    "ghostty_terminal: query channel send failed",
                );
            }
            Query::CursorX(tx) => {
                try_send(
                    &tx,
                    terminal.cursor_x().unwrap_or(DISCONNECTED_CURSOR_X as u16) as u32,
                    "ghostty_terminal: query channel send failed",
                );
            }
            Query::CursorY(tx) => {
                try_send(
                    &tx,
                    terminal.cursor_y().unwrap_or(DISCONNECTED_CURSOR_Y as u16) as u32,
                    "ghostty_terminal: query channel send failed",
                );
            }
            Query::CursorVisible(tx) => {
                // 上游取不到光标状态时按「不可见」处理，与查询通道断开时的回退一致：
                // 少画一个光标好过凭空显示一个，见 `DISCONNECTED_CURSOR_VISIBLE`。
                try_send(
                    &tx,
                    terminal
                        .is_cursor_visible()
                        .unwrap_or(DISCONNECTED_CURSOR_VISIBLE),
                    "query channel send failed",
                );
            }
            Query::Title(tx) => {
                try_send(
                    &tx,
                    terminal.title().unwrap_or("").to_string(),
                    "query channel send failed",
                );
            }
            Query::ModeGet(mode, tx) => {
                // 模式查询失败按「未启用」作答，但必须出声：静默 false 会让调用方
                // 把「读不到」当成「应用没开」，焦点上报与粘贴包装都会走错分支。
                let enabled = match terminal.mode(mode) {
                    Ok(enabled) => enabled,
                    Err(error) => {
                        log::error!("ghostty_terminal: mode query failed: {error}");
                        false
                    }
                };
                try_send(&tx, enabled, "ghostty_terminal: query channel send failed");
            }
            Query::ScrollbackLength(tx) => {
                let scrollback_rows = scrollback_len(terminal);
                // 热路径：滚动期间每次手势都会查一次，debug 级别会在 logcat 里
                // 刷出每秒十几行、并给渲染线程添一次同步写。降为 trace。
                log::trace!("ghostty_terminal: scrollback_rows query returned {scrollback_rows}");
                try_send(
                    &tx,
                    scrollback_rows,
                    "ghostty_terminal: query channel send failed",
                );
            }
            Query::ReadLineText { row, tx } => {
                try_send(
                    &tx,
                    Self::read_line_text_impl(terminal, row),
                    "query channel send failed",
                );
            }
            Query::RenderCursor(tx) => {
                // 只更新快照取光标视口，**不**走 build_cell_data：本查询要的只是
                // (row, col) 与可见性，而 build_cell_data 会把整张网格逐行抽一遍、
                // 写进跨帧行缓存、再 `snapshot.clean()` 标为已消费。为两个整数做这件事
                // 既浪费，又让「取光标」与「渲染」共用同一份可变抽取状态——此前本查询
                // 甚至会把抽取结果按调用处的默认色存进缓存。
                let (render_state, _, _, _) = extractors.parts();
                let visible_cursor = render_state
                    .update(terminal)
                    .map(|snapshot| cursor_in_viewport(&snapshot))
                    .ok()
                    .filter(|(_, _, _, visible)| *visible)
                    .map(|(row, col, _, _)| (row, col));
                try_send(&tx, visible_cursor, "query channel send failed");
            }
            Query::ReadVisibleText(tx) => {
                let rows = grid_rows(terminal);
                let scrollback_rows = scrollback_len(terminal);
                let mut text = String::new();
                for row in 0..rows {
                    // read_line_text_impl 要求绝对行号（回滚 + 视口）。
                    if let Some(line) = Self::read_line_text_impl(terminal, scrollback_rows + row) {
                        text.push_str(&line);
                        text.push('\n');
                    }
                }
                try_send(&tx, text, "ghostty_terminal: query channel send failed");
            }
            // 行域与 DumpGrid 相同（回滚顶部起连到视口末行），每格只读 codepoint。
            // 网格未变时复用上次产出：回滚填满时逐格 FFI 单趟即可越过
            // QUERY_TIMEOUT_MS，轮询方会读超时后的空值（表现为「内容不见了」）。
            Query::ReadAllText(tx) => {
                if all_text.is_none() {
                    let total_rows = scrollback_len(terminal) + grid_rows(terminal);
                    let mut text = String::new();
                    for row in 0..total_rows {
                        text.push_str(&Self::read_all_text_row_impl(terminal, row));
                        text.push('\n');
                    }
                    *all_text = Some(text);
                }
                try_send(
                    &tx,
                    all_text.clone().unwrap_or_default(),
                    "ghostty_terminal: query channel send failed",
                );
            }
            Query::SelectionText { start, end, tx } => {
                // Ghostty 原生的换行感知选区文本提取（termux
                // TerminalBuffer.getSelectedText 语义）：unwrap 拼接软换行行且
                // 不带 '\n'，trim 去掉尾部空白，格式化器内部把网格列映射为
                // 字符下标，故 CJK 宽字符不会被拆开。
                let text = Self::selection_text_impl(terminal, start, end);
                try_send(&tx, text, "selection text response send failed");
            }
            Query::SelectWordAt { row, col, tx } => {
                let bounds = Self::select_word_at_impl(terminal, row, col);
                selection_installed = bounds.is_some();
                try_send(&tx, bounds, "select word response send failed");
            }
            Query::SelectAll { tx } => {
                let bounds = Self::select_all_impl(terminal);
                selection_installed = bounds.is_some();
                try_send(&tx, bounds, "select all response send failed");
            }
            Query::HyperlinkAt { row, col, tx } => {
                let url = Self::hyperlink_at_impl(terminal, row, col);
                try_send(&tx, url, "hyperlink_at response send failed");
            }
            Query::WideCharTailCols { row, tx } => {
                try_send(
                    &tx,
                    Self::wide_char_tail_cols_impl(terminal, row),
                    "wide_char_tail_cols response send failed",
                );
            }
            Query::SearchSetNeedle { needle, tx } => {
                let total = Self::search_set_needle_impl(search, terminal, &needle);
                try_send(
                    &tx,
                    total,
                    "ghostty_terminal: search needle response send failed",
                );
            }
            Query::SearchSelect { forward, tx } => {
                let selection = Self::search_select_impl(search, terminal, forward);
                try_send(
                    &tx,
                    selection,
                    "ghostty_terminal: search select response send failed",
                );
            }
            Query::SearchViewportHighlights { tx } => {
                let highlights =
                    Self::search_viewport_highlights_impl(search, match_buffer, terminal);
                try_send(
                    &tx,
                    highlights,
                    "ghostty_terminal: search viewport highlights response send failed",
                );
            }
            Query::DumpGrid { tx } => {
                let dumped =
                    Self::build_dumped_grid(terminal, default_foreground, default_background);
                try_send(&tx, dumped, "ghostty_terminal: query channel send failed");
            }
            Query::TakeKittyGraphicsImage { id, tx } => {
                let kitty_graphics_data = (|| -> Option<KittyGraphicsImageData> {
                    let graphics = terminal.kitty_graphics().ok()?;
                    let image = graphics.image(id)?;
                    let (width, height, rgba) = Self::kitty_image_to_rgba(&image)?;
                    Some(KittyGraphicsImageData {
                        id,
                        width,
                        height,
                        data: rgba,
                    })
                })();
                try_send(
                    &tx,
                    kitty_graphics_data,
                    "ghostty_terminal: query channel send failed",
                );
            }
            Query::TakeKittyPlacements { tx } => {
                let placements = Self::collect_kitty_placements(terminal);
                try_send(
                    &tx,
                    placements,
                    "ghostty_terminal: query channel send failed",
                );
            }
            Query::EncodeMouseEvent {
                position,
                action,
                button,
                modifiers,
                cell_width,
                cell_height,
                tx,
            } => {
                let mut response = Vec::new();
                // 参考：zelland src-tauri/src/terminal.rs 的 `encode_mouse_event`——
                // 用 Ghostty 鼠标编码器配合渲染器的实时单元格尺寸，并在鼠标上报
                // 关闭时丢弃事件。编码器选项取自终端（跟踪模式 + 格式），故
                // SGR/X10/UTF-8 输出遵循应用的 DECSET 选择。
                Self::encode_mouse_event_into(
                    encoders,
                    terminal,
                    &mut response,
                    position,
                    match action {
                        1 => mouse::Action::Release,
                        2 => mouse::Action::Motion,
                        _ => mouse::Action::Press,
                    },
                    Some(Self::mouse_button(button)),
                    modifiers,
                    cell_width,
                    cell_height,
                );
                try_send(&tx, response, "mouse encode response send failed");
            }
            Query::EncodeMouseWheel {
                position,
                button,
                modifiers,
                cell_width,
                cell_height,
                repeat,
                tx,
            } => {
                let mut response = Vec::new();
                // 一个滚轮档 = press + release。`repeat` 档合并在这一次应答里：
                // 触摸拖动一次事件滚过几行就编几档，PTY 也只写一次。
                let wheel = Some(Self::mouse_button(button));
                for _ in 0..repeat.max(1) {
                    Self::encode_mouse_event_into(
                        encoders,
                        terminal,
                        &mut response,
                        position,
                        mouse::Action::Press,
                        wheel,
                        modifiers,
                        cell_width,
                        cell_height,
                    );
                    Self::encode_mouse_event_into(
                        encoders,
                        terminal,
                        &mut response,
                        position,
                        mouse::Action::Release,
                        wheel,
                        modifiers,
                        cell_width,
                        cell_height,
                    );
                }
                try_send(&tx, response, "mouse encode response send failed");
            }
            Query::EncodeKeyEvent {
                key,
                action,
                modifiers,
                text,
                unshifted,
                tx,
            } => {
                let (key_encoder, key_event) = &mut encoders.key;
                let mods = Mods::from_bits_retain(modifiers);
                key_encoder.set_options_from_terminal(terminal);
                key_event
                    .set_action(action)
                    .set_key(key)
                    .set_mods(mods)
                    // Shift 已体现在文本里（如 Shift+A 的 `A`）：不标为已消费，
                    // 编码器会再叠一层 Shift 修饰序列，终端收到的是两次 Shift+A。
                    .set_consumed_mods(if text.is_some() {
                        mods & Mods::SHIFT
                    } else {
                        Mods::empty()
                    })
                    .set_utf8(text.map(String::from))
                    // `key::Event` 跨事件复用而 `set_unshifted_codepoint` 没有清零入口：
                    // 只在有值时设置，上一键的码位会残留到本次。上游以 0 表示「未提供」
                    // （kitty 表格未命中时不猜码位、Ctrl 折叠不套用大写字母的基字母），
                    // 故缺省时显式写 0。
                    .set_unshifted_codepoint(unshifted.unwrap_or('\0'));
                let mut response = Vec::new();
                if let Err(error) = key_encoder.encode_to_vec(key_event, &mut response) {
                    log::warn!("ghostty_terminal: key encode failed: {error}");
                }
                try_send(&tx, response, "key encode response send failed");
            }
            Query::EncodePaste { data, tx } => {
                // 括号标志取活体模式 2004（DECSET 私有模式），与按键/鼠标编码
                // 同属终端状态感知路径。查不到按未启用作答（即不包裹），但记 error：
                // 这是粘贴注入的防线，失效必须是可诊断的，而不是一次静默降级。
                let bracketed = match terminal.mode(Mode::BRACKETED_PASTE) {
                    Ok(bracketed) => bracketed,
                    Err(error) => {
                        log::error!("ghostty_terminal: bracketed paste mode query failed: {error}");
                        false
                    }
                };
                try_send(
                    &tx,
                    encode_paste_data(&data, bracketed),
                    "paste encode response send failed",
                );
            }
        }
        selection_installed
    }

    /// 排空查询通道，使查询看到最新终端状态。返回真表示本批中有查询安装了选区，
    /// 调用方须按 `Command::SetSelection` 同款规则失效行缓存并重推帧。
    /// `all_text` 跨批次保留：网格变动由调用方置 `None`（与快照缓存同款失效时机）。
    pub(crate) fn drain_queries<'term>(
        query_receiver: &flume::Receiver<Query>,
        terminal: &mut Terminal<'term, '_>,
        all_text: &mut Option<String>,
        encoders: &mut Encoders,
        default_foreground: [f32; 4],
        default_background: [f32; 4],
        search: &mut Option<Search<'_, 'term>>,
        match_buffer: &mut MatchBuffer,
        extractors: &mut RenderExtractors<'term>,
    ) -> bool {
        let mut selection_installed = false;
        while let Ok(query) = query_receiver.try_recv() {
            // 累积而非覆盖：同批中选区安装查询之后若跟着普通查询（如 SelectionText），
            // 普通查询返回 false 会抹掉安装信号，导致行缓存不失效、反白帧不重推。
            selection_installed |= Self::process_query(
                query,
                terminal,
                all_text,
                encoders,
                default_foreground,
                default_background,
                search,
                match_buffer,
                extractors,
            );
        }
        selection_installed
    }

    pub(crate) fn run(config: RunConfig) {
        // 把整个函数体包进 `catch_unwind`，任何意外的 FFI panic
        // （例如来自 Ghostty 的 C 代码）只记日志，不会静默杀掉线程。
        // 此处用 `AssertUnwindSafe` 是安全的，因为：
        // - Terminal 因内部 C 指针而是 `!UnwindSafe`，但其 Drop
        //   实现会在 unwind 时调用 `ghostty_terminal_free`。
        // - panic 后线程总是退出，不存在二次使用。
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Self::run_inner(config)));
        if let Err(panic) = result {
            log::error!("ghostty_terminal: VT thread panicked: {panic:?}");
        }
    }

    /// 内层 run 循环（拆出来供 `catch_unwind` 调用）。
    fn run_inner(config: RunConfig) {
        let Ok(mut terminal) = Terminal::new(config.cols as u16, config.rows as u16) else {
            log::error!("ghostty_terminal: Terminal::new failed — thread exiting");
            return;
        };
        // 上游（git master）的 `Terminal::new` 已不再接收 options，
        // 回滚改用 `set_scrollback_max_lines` setter 配置。
        // 非零值即启用回滚（回滚关闭时 scrollback_rows 查询返回 0）。
        // 字节预算必须同步解除：上游默认字节上限先于行数触发，回滚
        // （DEFAULT_SCROLLBACK_LINES 行）会被拦腰截断到几百行。行数是唯一约束。
        if let Err(error) = terminal.set_scrollback_max_bytes(None) {
            log::error!("ghostty_terminal: set_scrollback_max_bytes failed: {error}");
        }
        if let Err(error) =
            terminal.set_scrollback_max_lines(Some(config.scrollback_lines as usize))
        {
            log::error!("ghostty_terminal: set_scrollback_max_lines failed: {error}");
        }

        // 初始化 Kitty Graphics Protocol（KGP）支持
        if let Err(error) = terminal.set_kitty_image_storage_limit(KGP_STORAGE_LIMIT) {
            log::error!("ghostty_terminal: set_kitty_image_storage_limit failed: {error}");
        }
        // PNG 载荷在进程内解码为 RGBA8（线程局部回调，须在 VT 线程注册）。
        // 用上游解码器：它按 libghostty 的分配器上限先取缓冲区再解码，
        // 未经该上限的尺寸不会被 PNG 头放大成巨额分配。
        if let Err(error) = libghostty_vt::kitty::graphics::set_png_decoder(Some(Box::new(
            libghostty_vt::kitty::graphics::RustPngDecoder::new(),
        ))) {
            log::error!("ghostty_terminal: set_png_decoder failed: {error}");
        }

        // 注册终端应答的 PTY 回写回调
        // （DECRPM 模式报告、DSR、DA 等）
        if let Err(error) = terminal.on_pty_write({
            let response_buffer = config.response_buffer.clone();
            move |_terminal, data| {
                if let Ok(mut guard) = response_buffer.lock() {
                    guard.push(data.to_vec());
                }
            }
        }) {
            log::error!("ghostty_terminal: on_pty_write callback registration failed: {error}");
        }
        // OSC 52 剪贴板写入改走上游回调：序列直达 Ghostty，事件经通道推送、
        // 由调用方在 flush 后收割。本仓不再自建 OSC 解析器；try_send 永不阻塞
        // VT 线程；OSC 52 读取请求（`?`）上游明确忽略，仍由 OutputProcessor
        // 的最小扫描器拦截（FR-036）。
        // 上游要求回调返回前必须 `reply`，否则视为拒绝写入并暂停 VT 流。
        // OSC 52 无应答时取值被丢弃，但调用本身不可省略。
        if let Err(error) = terminal.on_clipboard_write({
            let clipboard_tx = config.clipboard_tx.clone();
            move |_terminal, write| {
                // 选择器字母沿用 xterm 约定（Kotlin 侧原样透传）：c=剪贴板、p=主选区、s=次选区。
                let selection = match write.location() {
                    libghostty_vt::terminal::ClipboardLocation::Standard => "c",
                    libghostty_vt::terminal::ClipboardLocation::Primary => "p",
                    libghostty_vt::terminal::ClipboardLocation::Selection => "s",
                }
                .to_string();
                let mut fallback: Option<&[u8]> = None;
                let mut chosen: Option<&[u8]> = None;
                for content in write.contents() {
                    if fallback.is_none() {
                        fallback = Some(content.data);
                    }
                    if content.mime.starts_with("text/") {
                        chosen = Some(content.data);
                        break;
                    }
                }
                let payload = chosen.or(fallback).unwrap_or(&[]);
                // OSC 52 的负载由被测程序任意大，而下游是 `String` 事件、事件队列与
                // Kotlin 侧剪贴板：一份几十 MB 的负载会一路复制到 UI 线程并撑爆内存。
                // 上限取「远超任何真实剪贴板内容、又不至于拖垮进程」的量级，并在
                // 截断时显式记录——静默截断只会让用户拿到半个剪贴板却不知为何。
                let original_len = payload.len();
                let truncated = original_len > MAX_CLIPBOARD_PAYLOAD_BYTES;
                let payload = if truncated {
                    &payload[..MAX_CLIPBOARD_PAYLOAD_BYTES]
                } else {
                    payload
                };
                let text = String::from_utf8_lossy(payload).into_owned();
                if truncated {
                    log::warn!(
                        "OSC 52 clipboard payload truncated from {original_len} to {MAX_CLIPBOARD_PAYLOAD_BYTES} bytes"
                    );
                }
                // 通道满（`bounded(16)`）且消费侧（渲染暂停时停调）跟不上时丢弃——
                // 阻塞 VT 线程更糟。丢则记日志并回 `Busy`，不静默。
                if clipboard_tx.try_send((selection, text)).is_err() {
                    log::warn!("OSC 52 clipboard dropped: channel full");
                    write.reply(
                        Err(libghostty_vt::terminal::ClipboardWriteError::Busy),
                        false,
                    );
                } else {
                    write.reply(Ok(()), false);
                }
            }
        }) {
            log::error!(
                "ghostty_terminal: on_clipboard_write callback registration failed: {error}"
            );
        }
        // XTWINOPS 尺寸查询（CSI 14/16/18t）走上游 on_size 回调：行列取终端
        // 实时值，单元格像素取本层回填的共享几何（Resize 填默认，
        // SetCellPixelSize 填真实字形度量）。无锁读取，永不阻塞 VT 线程。
        if let Err(error) = terminal.on_size({
            let cell_size_px = config.cell_size_px.clone();
            move |terminal| {
                let (Ok(rows), Ok(cols)) = (terminal.rows(), terminal.cols()) else {
                    return None;
                };
                Some(libghostty_vt::terminal::SizeReportSize {
                    rows,
                    columns: cols,
                    cell_width: cell_size_px.0.load(Ordering::Acquire),
                    cell_height: cell_size_px.1.load(Ordering::Acquire),
                })
            }
        }) {
            log::error!("ghostty_terminal: on_size callback registration failed: {error}");
        }
        let mut default_background = Self::color_to_float(config.theme.palette.background);
        let mut default_foreground = Self::color_to_float(config.theme.palette.foreground);

        // 逐事件复用的上游编码器（见 `Encoders`）：每个终端只分配一次，编码器
        // 选项在每个事件前经 `set_options_from_terminal` 从终端重新同步。
        let mut encoders = match Encoders::new() {
            Ok(encoders) => encoders,
            Err(error) => {
                log::error!("ghostty_terminal: key encoder unavailable: {error} — thread exiting");
                return;
            }
        };

        // 上游 `search::Search` 的宿主：与终端同生命周期的局部变量，故搜索对象
        // 不必与终端自引用（`Search` 的生命周期挂在 `Terminal` 的分配器生命周期上，
        // 不占用借用）。首次搜索时惰性创建，此后复用同一份索引状态。
        let mut search: Option<Search<'_, '_>> = None;
        // 命中读出用的存储，跨查询复用（上游按需扩容）。
        let mut match_buffer = MatchBuffer::new();

        // 逐帧渲染迭代器与其行缓存：与终端同生命周期，不逐帧新建。上游 `RenderState`
        // 跨帧持有上次快照状态，逐帧新建会使 `rows = 0` 初值把每次 `update` 判为
        // 「维度变化 → 全量重建」，行级脏标记恒为全脏（实测命中率 0%）。
        // 创建失败即终止 VT 线程：缺了它们既取不到网格也读不到光标，
        // 以「本帧返回空网格」延后只会在第一次渲染时静默降级。
        let mut extractors = match RenderExtractors::new() {
            Some(extractors) => extractors,
            None => {
                log::error!("ghostty_terminal: render extractors unavailable — thread exiting");
                return;
            }
        };
        // 测试构建：换成该终端实例自己的计数句柄，使并行用例互不污染。
        #[cfg(test)]
        {
            extractors.cache_stats = config.cache_stats.clone();
        }

        // 初始主题：libghostty-vt 不处理 OSC 10/11 默认色转义（默认值归内嵌方所有，
        // OSC 4 调色板覆盖由上游处理），故走原生 setter。否则终端会保留内置的
        // xterm 调色板，主题颜色永远进不到网格里。
        Self::apply_theme(&mut terminal, &config.theme.palette);

        // 克隆（flume receiver 是 Arc 句柄）：使下面的命令循环仍能借用
        // `config`，无需部分移动。
        let query_receiver = config.query_receiver.clone();

        // 缓存上次构建的网格快照：当网格内容与滚动偏移自上一帧以来都没变时，
        // 跳过昂贵的逐格 ghostty FFI 重建。VT 线程单线程且顺序处理命令，
        // 标记 `grid_dirty` 与重建之间不存在竞争。
        let mut cached_snapshot: Option<Arc<GridSnapshot>> = None;
        // ── 自动推送 CellData ──
        // 用独立的脏标志，避免与旧的 GridSnapshot grid_dirty 追踪耦合。
        // 两个标志在 Write/Resize/SetTheme 时一起置位，但 grid_dirty 会在
        // TakeSnapshot 之后清除。
        let mut grid_dirty = true;
        // 全量文本（`Query::ReadAllText`）缓存。网格一变即失效，与 `grid_dirty` 同源：
        // 逐格走 ghostty FFI 的重建在回滚填满时单趟就会越过 QUERY_TIMEOUT_MS，
        // 未缓存时每次轮询都重建，查询方反复读到超时后的空值。
        let mut cached_all_text: Option<String> = None;
        // 自动推送路径的内容去重：`push_cell_data` 每轮循环（包括 50ms 空闲
        // 超时）都会构建整个网格，而渲染线程把收到的每批都当作
        // `is_new_data` → 每帧全量重绘。空闲终端会让渲染器满速空转
        // （SwiftShader 模拟器上实测 ~5 fps，真实 GPU 上也烧 CPU：
        // 「20fps 不可接受」）。当单元与光标自上次推送以来都没变时，
        // 直接跳过发送。
        let mut last_cell_data_push: Option<(Vec<CellData>, CursorInfo)> = None;
        // CellData 重建的开关：仅命令/查询导致状态变化后才重建。空闲 50ms tick
        // 不再全幅重建（2400+ 次 Ghostty FFI 调用），重复重建s只靠 last_push 去重
        // 抑制发送，CPU 清零。
        let mut cell_data_dirty = true;
        // 同步更新（模式 2026）的保持起点：引擎不会自行结束保持，
        // 程序崩溃或忘记 ESU 时由这里超时解锁（见 `synchronized_output_hold`）。
        let mut synchronized_output_held_since: Option<std::time::Instant> = None;

        'vt_loop: loop {
            // 等待有界通道里的下一条命令。用超时，以便在没有命令待处理时
            // （例如两次写入之间的查询）也周期性检查查询通道。tick 长度同时是
            // 查询的最坏延迟，调用方的超时窗口必须大于它（见 `VT_IDLE_TICK_MS`）。
            let mut pending =
                match config
                    .command_receiver
                    .recv_timeout(std::time::Duration::from_millis(
                        super::types::VT_IDLE_TICK_MS,
                    )) {
                    Ok(command) => Some(command),
                    Err(flume::RecvTimeoutError::Timeout) => {
                        // 没有待处理的有界命令——排空查询通道，让命令之间的查询
                        // 不必无限期等待。
                        if Self::drain_queries(
                            &query_receiver,
                            &mut terminal,
                            &mut cached_all_text,
                            &mut encoders,
                            default_foreground,
                            default_background,
                            &mut search,
                            &mut match_buffer,
                            &mut extractors,
                        ) {
                            // 查询内安装与 Command::SetSelection 同款失效：
                            // 反白改变每行内容，帧必须重推。
                            last_cell_data_push = None;
                            grid_dirty = true;
                            cell_data_dirty = true;
                            extractors.invalidate_row_cache();
                        }
                        // ── 自动推送 CellData（仅在真实状态变化后）──
                        if cell_data_dirty
                            && !synchronized_output_hold(
                                &mut terminal,
                                &mut synchronized_output_held_since,
                            )
                        {
                            Self::refresh_cell_data(
                                &config,
                                &terminal,
                                default_foreground,
                                default_background,
                                &mut last_cell_data_push,
                                &mut extractors,
                            );
                            cell_data_dirty = false;
                        }
                        continue;
                    }
                    Err(flume::RecvTimeoutError::Disconnected) => break,
                };
            // 整批命令处理完只做一次 cell-data 构建。poll_pty_output 每个渲染帧
            // 最多喂入 MAX_CHUNKS_PER_FRAME 条 Write 命令；若每条 Write 之后
            // 都重建整个网格（每次遍历 ~2444 次 Ghostty FFI 调用），SwiftShader
            // 上持续输出的渲染会被压到 ~6 fps——N 次构建里有 N−1 次在渲染线程
            // 看到之前就被覆盖了。中间状态有意合并——渲染线程只会消费最新状态。
            //
            // `grid_dirty` 仍由每条改动型命令就地置位（而非推迟到批尾）：
            // 同批到达的 TakeSnapshot 必须能观察到 FIFO 序上排在它之前的写入，
            // 并重建而不是提供陈旧缓存。
            let mut batch_dirty = false;
            let mut batch_acks: Vec<flume::Sender<()>> = Vec::new();
            while let Some(command) = pending.take() {
                match command {
                    Command::Write(data) => {
                        terminal.vt_write(&data);
                        mark_grid_dirty(&mut grid_dirty, &mut cached_all_text);
                        batch_dirty = true;
                    }
                    Command::FlushAck(tx) => {
                        // 扣到批尾构建完成才释放：`flush()` 的契约是返回时其前的每条命令
                        // ——包括 CellData 推送——都已处理完。测试会调用 flush()
                        // 再 receive_cell_data()；提前 ack 会与构建竞争，
                        // 把空通道交给它们。
                        batch_acks.push(tx);
                    }
                    Command::SetTheme(theme) => {
                        default_background = Self::color_to_float(theme.palette.background);
                        default_foreground = Self::color_to_float(theme.palette.foreground);
                        log::debug!(
                            "SetTheme: {} -> default_background={:?} default_foreground={:?}",
                            theme.name,
                            default_background,
                            default_foreground
                        );
                        // 用原生主题 API，而不是手写 OSC 10/11 序列：libghostty-vt 不处理
                        // OSC 默认色转义（默认值归内嵌方所有，OSC 4 覆盖由上游
                        // 处理），所以 OSC 方案会静默保留内置的 xterm 调色板。
                        // 这些 setter 写入的默认色正是上游单元颜色查询的解析基准。
                        Self::apply_theme(&mut terminal, &theme.palette);
                        mark_grid_dirty(&mut grid_dirty, &mut cached_all_text);
                        // 默认前景/背景参与每个单元的颜色解算，换主题后
                        // 缓存中的颜色逐行过时。
                        extractors.invalidate_row_cache();
                        batch_dirty = true;
                    }
                    Command::Resize { rows, cols } => {
                        // Ghostty 的 C API 取 u16 尺寸；越界值直接拒绝而非静默截断
                        // （恶意 Kotlin 调用方可能传入 >65535 而回绕）。
                        let (Ok(cols), Ok(rows)) = (u16::try_from(cols), u16::try_from(rows))
                        else {
                            log::error!(
                                "ghostty_terminal: resize rejected — dimensions out of u16 range"
                            );
                            continue;
                        };
                        if let Err(error) =
                            terminal.resize(cols, rows, DEFAULT_CELL_WIDTH, DEFAULT_CELL_HEIGHT)
                        {
                            // 网格没改成，但 `Session::resize` 已按「命令已入队」把缓存
                            // 尺寸发布成新值、并把 `grid_dirty` 清了——同尺寸的下次 resize
                            // 因此会短路，两者永久分歧。置位标记让那次重试真正发生。
                            config.resize_failed.store(true, Ordering::Release);
                            log::error!(
                                "ghostty_terminal: resize failed: {error}; grid lags the PTY — next resize retries"
                            );
                        } else {
                            config.resize_failed.store(false, Ordering::Release);
                            // 整网重调回填默认几何，XTWINOPS 应答与上游实际一致。
                            config
                                .cell_size_px
                                .0
                                .store(DEFAULT_CELL_WIDTH, Ordering::Release);
                            config
                                .cell_size_px
                                .1
                                .store(DEFAULT_CELL_HEIGHT, Ordering::Release);
                        }
                        mark_grid_dirty(&mut grid_dirty, &mut cached_all_text);
                        // 行数变化时行区间整体错位，缓存不可复用。
                        // `RowCache::covers` 也会拦下这一情形，这里显式失效
                        // 使失效时机与既有 `mark_grid_dirty` 保持同处。
                        extractors.invalidate_row_cache();
                        batch_dirty = true;
                    }
                    Command::SetCellPixelSize {
                        cell_width,
                        cell_height,
                    } => {
                        // 同行列重调，仅更新单元格像素几何（Kitty 放置/鼠标映射用）。
                        // 网格内容不变，不置脏，避免字体变化引发全量重绘。
                        let (Ok(cols), Ok(rows)) = (terminal.cols(), terminal.rows()) else {
                            continue;
                        };
                        if cell_width == 0 || cell_height == 0 {
                            continue;
                        }
                        if let Err(error) = terminal.resize(cols, rows, cell_width, cell_height) {
                            log::error!("ghostty_terminal: cell resize failed: {error}");
                        } else {
                            config.cell_size_px.0.store(cell_width, Ordering::Release);
                            config.cell_size_px.1.store(cell_height, Ordering::Release);
                        }
                    }
                    Command::ScrollViewport(delta) => {
                        // C ABI 返回 void；视口滚动失败只表现为无操作（网格不变），
                        // setScrollOffset 里的重试逻辑会在下次偏移变化时重发。
                        terminal
                            .scroll_viewport(libghostty_vt::terminal::ScrollViewport::Delta(delta));
                        // 显式滚动强制重推：视口内容可能逐字节相同（如已在底部），
                        // 内容去重会吞掉本次滑动的即时重绘。与 SetSelection 同款失效。
                        last_cell_data_push = None;
                        grid_dirty = true;
                        batch_dirty = true;
                    }
                    Command::Repaint => {
                        // 显式重推：内容可能与上次推送逐字节相同（切会话时终端本身毫无
                        // 变化），去重基线必须一并失效，否则这次请求被吞掉。
                        last_cell_data_push = None;
                        batch_dirty = true;
                    }
                    Command::Reset => {
                        // RIS 全重置：恢复初始状态并清空回滚；全部帧缓存失效。
                        // 显式清除终端持有的活动选区（幂等，不依赖上游副作用）。
                        terminal.reset();
                        let _ = terminal.set_selection(None);
                        cached_snapshot = None;
                        last_cell_data_push = None;
                        mark_grid_dirty(&mut grid_dirty, &mut cached_all_text);
                        extractors.invalidate_row_cache();
                        batch_dirty = true;
                    }
                    Command::SetSelection { start, end } => {
                        // 终端持有化：选区经 set_selection 安装为终端状态
                        //（上游转为跟踪引用，随滚动/输出/重排跟随文本）。
                        // 选区变化改变每行反白，需重推帧。
                        Self::install_selection_impl(&terminal, start, end);
                        last_cell_data_push = None;
                        grid_dirty = true;
                        extractors.invalidate_row_cache();
                        batch_dirty = true;
                    }
                    Command::ClearSelection => {
                        if terminal.set_selection(None).is_err() {
                            log::warn!("ghostty_terminal: clear selection failed");
                        }
                        last_cell_data_push = None;
                        grid_dirty = true;
                        extractors.invalidate_row_cache();
                        batch_dirty = true;
                    }
                    Command::TakeSnapshot { tx } => {
                        let needs_rebuild =
                            snapshot_needs_rebuild(grid_dirty, cached_snapshot.is_some());
                        let snapshot = if needs_rebuild {
                            let snap = Self::build_snapshot(
                                &terminal,
                                default_foreground,
                                default_background,
                            );
                            let cached = Arc::new(snap);
                            cached_snapshot = Some(Arc::clone(&cached));
                            grid_dirty = false;
                            cached
                        } else {
                            // 不变式：`needs_rebuild` 为 false 时 `cached_snapshot` 必为
                            // Some（上面第三个子句保证）。违反即渲染管线缺陷，直接崩溃
                            // 退出，不回退到空白网格（DESIGN 禁止掩盖错误）。
                            cached_snapshot
                                .as_ref()
                                .map(Arc::clone)
                                .expect("cached_snapshot 在 needs_rebuild=false 时必然存在")
                        };
                        try_send(
                            &tx,
                            snapshot,
                            "ghostty_terminal: command channel send failed",
                        );
                    }
                    Command::Terminate => break 'vt_loop,
                }
                pending = config.command_receiver.try_recv().ok();
            }
            // 整批处理完后排空待处理查询，让它们看到完全更新后的终端状态。
            if Self::drain_queries(
                &query_receiver,
                &mut terminal,
                &mut cached_all_text,
                &mut encoders,
                default_foreground,
                default_background,
                &mut search,
                &mut match_buffer,
                &mut extractors,
            ) {
                // 同上：查询内安装的选区需要纳入本批重推。
                last_cell_data_push = None;
                grid_dirty = true;
                batch_dirty = true;
                extractors.invalidate_row_cache();
            }
            // 整批只做一次 cell-data 构建——上面的每次状态改动都已完成，
            // 故这里反映的是积压队列的最终状态。`grid_dirty` 由每条改动型
            // 命令就地置位，并保持置位直到 TakeSnapshot 消费它为止
            // （快照缓存失效语义，与逐命令版本相同）。
            if batch_dirty
                && !synchronized_output_hold(&mut terminal, &mut synchronized_output_held_since)
            {
                Self::refresh_cell_data(
                    &config,
                    &terminal,
                    default_foreground,
                    default_background,
                    &mut last_cell_data_push,
                    &mut extractors,
                );
                cell_data_dirty = false;
            }
            // flush 应答只在上面的构建之后释放，保持 `flush()` 原有的
            // 逐命令顺序保证。
            for ack in batch_acks {
                try_send(&ack, (), "command channel send failed");
            }
        }
    }

    pub(crate) fn apply_style_to_snapshot(
        data: &mut CellSnapshot,
        style: &libghostty_vt::style::Style,
        terminal: &Terminal,
        default_foreground: [f32; 4],
        default_background: [f32; 4],
    ) {
        data.foreground = Self::resolve_style_color(terminal, &style.fg_color, default_foreground);
        data.background = Self::resolve_style_color(terminal, &style.bg_color, default_background);
        // SGR 58 下划线色：未设置时回退到解析后的前景（着色器旧 `deco = foreground` 语义）。
        data.underline_color =
            Self::resolve_style_color(terminal, &style.underline_color, data.foreground);
        data.bold = style.bold;
        data.dim = style.faint;
        data.italic = style.italic;
        data.strikethrough = style.strikethrough;
        data.overline = style.overline;
        data.blink = style.blink;
        data.hidden = style.invisible;
        data.underline = has_visible_underline(style.underline);
        data.double_underline = is_double_underline(style.underline);
        data.reverse = style.inverse;
    }

    /// `default_foreground`/`default_background` 必须与渲染路径
    /// [`Self::build_cell_data`] 收到的是同一对值（由 run 循环经 [`RunConfig`] 初始化、
    /// 经 `SetTheme` 更新）。转储是渲染的对照物：若这里另行从终端反查默认色，两条路径
    /// 一旦分歧，测试就会对着与 GPU 不同的颜色下断言。不设兜底——主题由构造参数给定，
    /// 不存在"查不到"。
    pub(crate) fn build_dumped_grid(
        terminal: &Terminal,
        default_foreground: [f32; 4],
        default_background: [f32; 4],
    ) -> DumpedGrid {
        let rows = grid_rows(terminal);
        let cols = grid_cols(terminal);
        let scrollback_rows = scrollback_len(terminal);

        let snapshot_at = |point: Point| {
            let mut data = CellSnapshot::default();
            if let Ok(point) = terminal.grid_ref(point) {
                if let Ok(cell) = point.cell() {
                    data.codepoint = cell.codepoint().unwrap_or(0);
                }
                if let Ok(style) = point.style() {
                    Self::apply_style_to_snapshot(
                        &mut data,
                        &style,
                        terminal,
                        default_foreground,
                        default_background,
                    );
                }
            }
            data
        };

        let mut visible = Vec::with_capacity((rows * cols) as usize);
        for row in 0..rows {
            for col in 0..cols {
                visible.push(snapshot_at(Point::Viewport(PointCoordinate {
                    x: col as u16,
                    y: row,
                })));
            }
        }

        let mut scrollback = Vec::with_capacity(scrollback_rows as usize);
        for scrollback_row in 0..scrollback_rows {
            let mut row_cells = Vec::with_capacity(cols as usize);
            for col in 0..cols {
                row_cells.push(snapshot_at(Point::History(PointCoordinate {
                    x: col as u16,
                    y: scrollback_row,
                })));
            }
            scrollback.push(row_cells);
        }

        DumpedGrid {
            rows,
            cols,
            visible,
            scrollback,
        }
    }

    pub(crate) fn byte_to_float(value: u8) -> f32 {
        value as f32 / 255.0
    }

    /// 向行数据推入空白（码点 0）单元。用于原始 ghostty 数据或样式
    /// 无法解析的单元。
    fn push_blank_cell(
        row_data: &mut Vec<CellData>,
        default_foreground: [f32; 4],
        default_background: [f32; 4],
        row: u32,
        col: u32,
        width: u32,
    ) {
        row_data.push(CellData {
            codepoint: 0,
            width,
            grapheme_extra: [0; MAX_GRAPHEME_EXTRA],
            foreground: default_foreground,
            background: default_background,
            underline_color: default_foreground,
            flags: 0,
            row,
            col,
        });
    }

    /// 单元占用的列数：`None` 表示 spacer 单元（宽字符的尾/头格），
    /// 它不产出 `CellData`，两列已由宽字符本身消耗。
    fn cell_columns(raw: &libghostty_vt::screen::Cell) -> Option<u32> {
        match raw.wide() {
            Ok(libghostty_vt::screen::CellWide::Wide) => Some(2),
            Ok(
                libghostty_vt::screen::CellWide::SpacerTail
                | libghostty_vt::screen::CellWide::SpacerHead,
            ) => None,
            _ => Some(1),
        }
    }

    /// 把 ghostty 光标可视样式映射为应用层光标样式。
    /// 空心块暂时按实心块渲染（细分形状留待以后）。
    /// 上游将来新增的未知样式回退为块：光标必须始终画出点什么
    /// （DESIGN 的极端崩溃不适用于每帧可见元素）。
    fn cursor_style_from_snapshot(
        snapshot: &libghostty_vt::render::Snapshot<'_, '_>,
    ) -> CursorStyle {
        use libghostty_vt::render::CursorVisualStyle;
        match snapshot.cursor_visual_style() {
            Ok(CursorVisualStyle::Bar) => CursorStyle::Bar,
            Ok(CursorVisualStyle::Underline) => CursorStyle::Underline,
            _ => CursorStyle::Block,
        }
    }

    /// 把单元颜色解析为 `[r, g, b, 1.0]` 浮点值；FFI 报错或颜色透明时
    /// 回退到单元默认值。
    fn cell_color(
        color: Result<Option<libghostty_vt::style::RgbColor>, libghostty_vt::error::Error>,
        default: [f32; 4],
    ) -> [f32; 4] {
        match color {
            Ok(Some(rgb)) => Self::color_to_float(rgb),
            _ => default,
        }
    }

    /// 用原生主题 API 设置默认背景/前景色与 16 色 ANSI 调色板。
    /// libghostty-vt 不处理 OSC 10/11 默认色转义（默认值归内嵌方所有，
    /// OSC 4 调色板覆盖由上游处理），故内嵌方必须直接推送主题色；
    /// 否则终端会保留内置的 xterm 调色板。
    fn apply_theme(
        terminal: &mut libghostty_vt::terminal::Terminal,
        palette: &crate::terminal::theme::Palette,
    ) {
        if terminal
            .set_default_bg_color(Some(palette.background))
            .is_err()
        {
            log::error!("ghostty_terminal: apply_theme failed to set background color");
        }
        if terminal
            .set_default_fg_color(Some(palette.foreground))
            .is_err()
        {
            log::error!("ghostty_terminal: apply_theme failed to set foreground color");
        }
        match terminal.default_color_palette() {
            Ok(mut colors) => {
                for (index, color) in ANSI_PALETTE_INDICES.iter().zip(palette.ansi.iter()) {
                    colors.set(*index, *color);
                }
                if terminal.set_default_color_palette(Some(colors)).is_err() {
                    log::error!("ghostty_terminal: apply_theme failed to set ANSI palette");
                }
            }
            Err(error) => {
                log::error!("ghostty_terminal: apply_theme failed to read ANSI palette: {error}");
            }
        }
    }

    /// 把 `StyleColor` 解析到终端**生效的** 256 色调色板（默认色加上 OSC 4
    /// 覆盖）；未设置或读不到时回退到 `default`。取代了历史上的静态
    /// 16 色表 + xterm 公式，后者会静默忽略 OSC 4 改色。
    pub(crate) fn resolve_style_color(
        terminal: &Terminal,
        color: &libghostty_vt::style::StyleColor,
        default: [f32; 4],
    ) -> [f32; 4] {
        match color {
            libghostty_vt::style::StyleColor::Rgb(c) => Self::color_to_float(*c),
            libghostty_vt::style::StyleColor::Palette(idx) => terminal
                .color_palette()
                .map(|palette| Self::color_to_float(palette.get(*idx)))
                .unwrap_or(default),
            _ => default,
        }
    }

    pub(crate) fn color_to_float(color: libghostty_vt::style::RgbColor) -> [f32; 4] {
        [
            Self::byte_to_float(color.r),
            Self::byte_to_float(color.g),
            Self::byte_to_float(color.b),
            1.0,
        ]
    }

    /// 按当前终端状态重建 CellData 并推给渲染线程的通道（未挂消费者时
    /// 为空操作）。Write/SetTheme/Resize/ScrollViewport 共用的帧推送块
    /// 只有这一个调用点。
    /// 终端网格变化后推送一份新的 cell-data 快照。
    /// [`Self::push_cell_data`] 的薄封装，补齐各命令处理器共用的
    /// 标准 config/状态接线。
    fn refresh_cell_data<'alloc>(
        config: &RunConfig,
        terminal: &libghostty_vt::terminal::Terminal<'alloc, '_>,
        default_foreground: [f32; 4],
        default_background: [f32; 4],
        last_push: &mut Option<(Vec<CellData>, CursorInfo)>,
        extractors: &mut RenderExtractors<'alloc>,
    ) {
        Self::push_cell_data(
            config.cell_data_tx.as_ref(),
            config.frame_wake.as_ref(),
            &ScreenMirrors {
                alt_screen: &config.alt_screen_active,
                mouse_tracking: &config.mouse_tracking_active,
            },
            terminal,
            default_foreground,
            default_background,
            last_push,
            extractors,
        );
    }

    fn push_cell_data<'alloc>(
        cell_data_tx: Option<&Sender<(Vec<CellData>, CursorInfo)>>,
        frame_wake: Option<&crate::terminal::session::OutputWake>,
        mirrors: &ScreenMirrors<'_>,
        terminal: &Terminal<'alloc, '_>,
        default_foreground: [f32; 4],
        default_background: [f32; 4],
        last_push: &mut Option<(Vec<CellData>, CursorInfo)>,
        extractors: &mut RenderExtractors<'alloc>,
    ) {
        if let Some(tx) = cell_data_tx
            && let Some(data) = Self::build_cell_data(
                terminal,
                default_foreground,
                default_background,
                mirrors,
                extractors,
            )
        {
            // 自上次推送以来什么都没变时跳过发送：循环定时器（50ms 空闲唤醒）
            // 会在终端毫无活动时重跑这条路径，而渲染线程把收到的每批都当作
            // 新数据 → 每帧全量重绘。把 bytemuck Pod 单元按原始字节（外加光标）
            // 比较，在 24x80 网格上约 150KB memcmp——微秒级，与它避免的
            // 全量渲染开销相比可忽略。
            let unchanged = match last_push {
                Some((last_cells, last_cursor)) => {
                    *last_cursor == data.1
                        && bytemuck::cast_slice::<CellData, u8>(last_cells)
                            == bytemuck::cast_slice::<CellData, u8>(&data.0)
                }
                None => false,
            };
            if unchanged {
                return;
            }
            // 去重基线只在帧真正入队后推进：通道满时本帧被丢弃，若同时推进基线，
            // 下一帧同一内容会命中 unchanged 而不再补发，屏幕将永久停留在旧帧。
            match tx.try_send((data.0.clone(), data.1)) {
                Ok(()) => {
                    *last_push = Some(data);
                    // 帧已可被取走：叫醒可能正睡在空闲闭锁上的渲染线程。
                    if let Some(wake) = frame_wake {
                        wake();
                    }
                }
                Err(error) => warn_drop_throttled(&error),
            }
        }
    }
}

impl super::GhosttyTerminal {
    /// 鼠标事件里的按钮编号 → 上游按钮。1/2 为右键/中键，3/4 为滚轮上/下，
    /// 其余（含 0）按左键处理：与 Android 的 `BUTTON_PRIMARY` 约定一致。
    fn mouse_button(button: u8) -> mouse::Button {
        match button {
            1 => mouse::Button::Right,
            2 => mouse::Button::Middle,
            3 => mouse::Button::Four,
            4 => mouse::Button::Five,
            _ => mouse::Button::Left,
        }
    }

    /// 把一个鼠标事件编码进 `out`。
    ///
    /// 编码器与事件跨事件复用：选项按终端当前状态重新同步，按钮/位置/修饰键
    /// 逐字段覆写。漏写任一字段都会让本次事件沿用上一次的取值。
    fn encode_mouse_event_into(
        encoders: &mut Encoders<'_>,
        terminal: &Terminal,
        out: &mut Vec<u8>,
        position: (f32, f32),
        action: mouse::Action,
        button: Option<mouse::Button>,
        modifiers: u16,
        cell_width: f32,
        cell_height: f32,
    ) {
        let Some((encoder, event)) = encoders.mouse.as_mut() else {
            log::warn!("ghostty_terminal: mouse encoder/event unavailable — dropping mouse event");
            return;
        };
        encoder.set_options_from_terminal(terminal);
        // 视口外 motion 只在有键按下时上报（上游 mouse_encode 口径）：
        // 按下/移动恒有接触（触摸 onTouchEvent 与持键鼠标移动），
        // 释放后无接触。缺省 false 会把滑出视口的拖动静默丢弃。
        encoder.set_any_button_pressed(!matches!(action, mouse::Action::Release));
        let cols = grid_cols(terminal);
        let rows = grid_rows(terminal);
        encoder.set_size(mouse::EncoderSize {
            screen_width: cols.saturating_mul(cell_width.max(1.0) as u32),
            screen_height: rows.saturating_mul(cell_height.max(1.0) as u32),
            cell_width: cell_width.max(1.0) as u32,
            cell_height: cell_height.max(1.0) as u32,
            padding_top: 0,
            padding_bottom: 0,
            padding_right: 0,
            padding_left: 0,
        });
        event.set_position(mouse::Position {
            x: position.0,
            y: position.1,
        });
        event.set_action(action);
        event.set_button(button);
        // 修饰键进编码器：Shift/Ctrl 点击在远端（vim/tmux/htop）必须与
        // 普通左键可区分，否则无法粘贴选择、无法扩展选区。
        event.set_mods(Mods::from_bits_retain(modifiers));
        if let Err(error) = encoder.encode_to_vec(event, out) {
            log::warn!("ghostty_terminal: mouse encode failed: {error}");
        }
    }

    /// 上游 Kitty 图像载荷归一化为 RGBA8：Ok(None) 为分片传输待定返回 None；
    /// RGB 直扩 alpha，灰度按亮度展开，PNG 经线程解码器已为 RGBA（按长度兜底）。
    fn kitty_image_to_rgba(
        image: &libghostty_vt::kitty::graphics::Image<'_>,
    ) -> Option<(u32, u32, Vec<u8>)> {
        use libghostty_vt::kitty::graphics::ImageFormat;
        let width = image.width().ok()?;
        let height = image.height().ok()?;
        let bytes = image.data().ok()??;
        let pixel_count = width.checked_mul(height)? as usize;
        let rgba = match image.format().ok()? {
            ImageFormat::Rgba => {
                if bytes.len() != pixel_count.checked_mul(4)? {
                    return None;
                }
                bytes.to_vec()
            }
            ImageFormat::Rgb => {
                if bytes.len() != pixel_count.checked_mul(3)? {
                    return None;
                }
                let (triples, _) = bytes.as_chunks::<3>();
                let mut out = Vec::with_capacity(pixel_count * 4);
                for triple in triples {
                    out.extend_from_slice(&[triple[0], triple[1], triple[2], 255]);
                }
                out
            }
            ImageFormat::Gray => {
                if bytes.len() != pixel_count {
                    return None;
                }
                let mut out = Vec::with_capacity(pixel_count * 4);
                for gray in bytes {
                    out.extend_from_slice(&[*gray, *gray, *gray, 255]);
                }
                out
            }
            ImageFormat::GrayAlpha => {
                if bytes.len() != pixel_count.checked_mul(2)? {
                    return None;
                }
                let (pairs, _) = bytes.as_chunks::<2>();
                let mut out = Vec::with_capacity(pixel_count * 4);
                for pair in pairs {
                    out.extend_from_slice(&[pair[0], pair[0], pair[0], pair[1]]);
                }
                out
            }
            ImageFormat::Png => {
                // 解码器输出 RGBA；按长度兜底 RGB（防御上游行为漂移）。
                if bytes.len() == pixel_count.checked_mul(4)? {
                    bytes.to_vec()
                } else if bytes.len() == pixel_count.checked_mul(3)? {
                    let (triples, _) = bytes.as_chunks::<3>();
                    let mut out = Vec::with_capacity(pixel_count * 4);
                    for triple in triples {
                        out.extend_from_slice(&[triple[0], triple[1], triple[2], 255]);
                    }
                    out
                } else {
                    return None;
                }
            }
            _ => return None,
        };
        Some((width, height, rgba))
    }

    /// 采集视口内全部可见 Kitty 放置（跳过虚拟占位与屏外项，按 z 排序）。
    /// 虚拟放置（unicode 占位符）由字形管线渲染，不进图像通道。
    fn collect_kitty_placements(terminal: &Terminal) -> Vec<KittyPlacementFrame> {
        use libghostty_vt::kitty::graphics::PlacementIterator;
        let Ok(graphics) = terminal.kitty_graphics() else {
            return Vec::new();
        };
        let Ok(mut iterator) = PlacementIterator::new() else {
            return Vec::new();
        };
        let Ok(mut placements) = iterator.update(&graphics) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        while let Some(placement) = placements.next() {
            match placement.is_virtual() {
                Ok(true) => continue,
                Ok(false) => {}
                // 读不到即按虚拟放置跳过：宁可少画一幅图，也不把不可见的放置画出来。
                Err(error) => {
                    log::warn!("kitty: is_virtual failed, skipping placement: {error}");
                    continue;
                }
            }
            let Ok(image_id) = placement.image_id() else {
                continue;
            };
            let Some(image) = graphics.image(image_id) else {
                log::debug!("kitty: 图像 {image_id} 已从 graphics 存储中消失，跳过");
                continue;
            };
            let Some((image_width, image_height, image_rgba)) = Self::kitty_image_to_rgba(&image)
            else {
                log::debug!("kitty: 图像 {image_id} 解码失败（格式或像素长度不支持），跳过");
                continue;
            };
            let Ok(info) = placement.placement_render_info(&image, terminal) else {
                continue;
            };
            if !info.viewport_visible
                || info.pixel_width == 0
                || info.pixel_height == 0
                || info.source_width == 0
                || info.source_height == 0
            {
                continue;
            }
            out.push(KittyPlacementFrame {
                image_id,
                viewport_col: info.viewport_col,
                viewport_row: info.viewport_row,
                pixel_width: info.pixel_width,
                pixel_height: info.pixel_height,
                source_x: info.source_x,
                source_y: info.source_y,
                source_width: info.source_width,
                source_height: info.source_height,
                // 偏移与 z 读不到即丢弃该放置：留 0 会把图像画到错误位置并
                // 破坏叠加次序，而那与「没画」同样不可接受，只是更难察觉。
                cell_offset_x: match placement.x_offset() {
                    Ok(offset) => offset,
                    Err(error) => {
                        log::warn!("kitty: x_offset failed, skipping placement: {error}");
                        continue;
                    }
                },
                cell_offset_y: match placement.y_offset() {
                    Ok(offset) => offset,
                    Err(error) => {
                        log::warn!("kitty: y_offset failed, skipping placement: {error}");
                        continue;
                    }
                },
                z: match placement.z() {
                    Ok(z) => z,
                    Err(error) => {
                        log::warn!("kitty: z failed, skipping placement: {error}");
                        continue;
                    }
                },
                image_width,
                image_height,
                image_rgba,
            });
        }
        out.sort_by_key(|frame| frame.z);
        out
    }
}

/// VT 线程每帧刷新的无锁屏幕状态镜像。
///
/// Android 输入路径在每一次触摸/滚轮事件里都要读它（决定「转发给远端」还是
/// 「滚本地回滚」），因此这些值绝不能靠查询通道获得：一次查询就要等 VT 线程
/// 从解析里抽身应答，终端正刷输出时会把 UI 线程按满一个 `QUERY_TIMEOUT_MS`。
pub(crate) struct ScreenMirrors<'a> {
    /// 备用屏（vim / less / htop）是否活动。
    pub(crate) alt_screen: &'a Arc<AtomicBool>,
    /// 鼠标上报（DECSET 1000/1002/1003）是否启用。
    pub(crate) mouse_tracking: &'a Arc<AtomicBool>,
}

impl super::GhosttyTerminal {
    /// 构建用于渲染的扁平全量 `CellData` 网格。
    ///
    /// 逐格抽取只对上游判脏的行执行，干净行整段复制 [`RowCache`] 的上一帧内容。
    /// 判脏依据是 [`RowIteration::dirty`]，它依赖跨帧复用的 [`RenderState`]——
    /// 本函数的迭代器由 [`Self::run`] 跨帧持有，且每次成功抽完后经
    /// [`Snapshot::clean`] 标记快照已消费，两者缺一则全部行恒为脏。
    ///
    /// 每帧的行数据全部读出后才 `clean`：上游要求整帧渲染完成后才可清脏，
    /// 提前标记会让本帧尚未读完的行被下一帧当作干净行复用。
    ///
    /// 跳过干净快照（网格级）由 `snapshot_needs_rebuild` 在上层完成，与行级判定
    /// 是同一决策的两层：网格未变时本函数根本不执行。
    pub(crate) fn build_cell_data<'alloc, 'cb>(
        terminal: &Terminal<'alloc, 'cb>,
        default_foreground: [f32; 4],
        default_background: [f32; 4],
        mirrors: &ScreenMirrors<'_>,
        extractors: &mut RenderExtractors<'alloc>,
    ) -> Option<(Vec<CellData>, CursorInfo)> {
        // VT 线程每推一帧就同步这个无锁的备用屏镜像。Android 输入路径在每次
        // 触摸滚动时无锁读取它，以决定手势是否转发给远端
        // （Haven 研究：altScreen 滚轮消费）。
        mirrors.alt_screen.store(
            terminal
                .active_screen()
                .is_ok_and(|screen| screen == libghostty_vt::screen::Screen::Alternate),
            Ordering::Release,
        );
        // 鼠标上报开关同样每帧镜像：输入路径（触摸拖动、外接滚轮）每次事件都要
        // 决定「转发给远端还是滚本地回滚」，走查询通道会把 UI 线程钉在 VT 线程
        // 的一次应答上；读原子量是纯本地判断。
        mirrors.mouse_tracking.store(
            terminal.is_mouse_tracking().unwrap_or(false),
            Ordering::Release,
        );
        let rows = grid_rows(terminal);
        let cols = grid_cols(terminal);
        let size = (rows * cols) as usize;

        // 计数句柄须在 `parts()` 的独占借用之前取一份副本：`extractors` 在整个
        // 抽取期间已被可变借用，此处再借即冲突。句柄内部是 `Arc`，副本共享
        // 同一组原子量。
        #[cfg(test)]
        let cache_stats = extractors.cache_stats.clone();
        let (render_state, row_iter, cell_iter, row_cache) = extractors.parts();

        let snapshot = match render_state.update(terminal) {
            Ok(render_snapshot) => render_snapshot,
            Err(update_error) => {
                log::error!("build_cell_data: render_state.update failed: {update_error}");
                row_cache.invalidate();
                return None;
            }
        };

        let mut row_iter_impl = match row_iter.update(&snapshot) {
            Ok(row_iterator) => row_iterator,
            Err(row_error) => {
                log::error!("build_cell_data: row_iter.update failed: {row_error}");
                row_cache.invalidate();
                return None;
            }
        };

        // 缓存行数与网格不一致（尺寸变更/重置/首次抽取）时不可复用：
        // 区间按行数划分，错位复用会把别行的内容写进本行。
        // 旧帧缓冲经 `take_previous` 移出后即由本帧输出重建，故不留悬空副本。
        let reusable_rows = row_cache.covers(rows);
        let (previous_cells, previous_ranges) = if reusable_rows {
            row_cache.take_previous()
        } else {
            (Vec::new(), Vec::new())
        };
        let mut data = Vec::with_capacity(size);
        let mut current_row = 0u32;
        let mut extraction_complete = true;
        // 本次抽取的行级分流结果，仅测试读取；生产构建不声明，零开销。
        #[cfg(test)]
        let (mut reused_rows, mut extracted_rows) = (0u64, 0u64);
        // 无选区时跳过每行一次 selection FFI。读失败按「无选区」作答并出声：
        // 若真读成 false 而终端确有选区，本帧的反白会整帧消失且无处可查。
        let selection_active = match terminal.selection() {
            Ok(selected) => selected.is_some(),
            Err(error) => {
                log::error!("build_cell_data: selection query failed: {error}");
                false
            }
        };

        while let Some(row) = row_iter_impl.next() {
            // 行级脏判定：缓存不可复用（尺寸变更/首次）时全部行逐格抽取；
            // 上游报该行干净时整段复制上一帧该行——内容逐字节相同，省下
            // 一次逐单元 FFI 往返。判读失败按「脏」处理：宁可多抽，
            // 不可拿可能失配的缓存冒充本帧内容。
            let row_dirty = !reusable_rows
                || match row.dirty() {
                    Ok(dirty) => dirty,
                    Err(error) => {
                        log::warn!("build_cell_data: row dirty query failed: {error}");
                        true
                    }
                };
            if !row_dirty {
                data.extend_from_slice(RowCache::row_slice(
                    &previous_cells,
                    &previous_ranges,
                    current_row,
                ));
                #[cfg(test)]
                {
                    reused_rows += 1;
                }
                current_row += 1;
                continue;
            }
            #[cfg(test)]
            {
                extracted_rows += 1;
            }

            // 行级选区范围（无选区时为 None）：每行一次 FFI，避免逐单元格查询。
            let row_selection = if selection_active {
                row.selection().ok().flatten()
            } else {
                None
            };

            let mut row_data = Vec::with_capacity(cols as usize);
            let mut cell_iter_impl = match cell_iter.update(row) {
                Ok(ci) => ci,
                Err(error) => {
                    log::warn!("build_cell_data: cell_iter.update failed: {error}");
                    // 该行已不完整，不得以缓存版本冒充：整帧作废并清空缓存。
                    row_cache.invalidate();
                    extraction_complete = false;
                    break;
                }
            };

            let mut current_col = 0u32;
            // CellRun 式逐行样式缓存（termlib CellRun.kt）：共享同一 style_id 的
            // 连续单元只解析一次 style/前景/背景；扁平 CellData 输出不变，
            // 但该 run 内跳过了逐单元的 FFI 调用（style/前景/背景）。
            let mut cached_style_id: Option<libghostty_vt::style::Id> = None;
            let mut cached_foreground = default_foreground;
            let mut cached_background = default_background;
            let mut cached_underline = default_foreground;
            let mut cached_flags = 0u32;

            while let Some(cell) = cell_iter_impl.next() {
                let raw = match cell.raw_cell() {
                    Ok(raw_cell) => raw_cell,
                    Err(_) => {
                        // `raw_cell()` 失败则宽度无从得知（宽度只存在于 raw cell），
                        // 只能按 1 列推进——这是该失败路径的固有上限。
                        Self::push_blank_cell(
                            &mut row_data,
                            default_foreground,
                            default_background,
                            current_row,
                            current_col,
                            1,
                        );
                        current_col += 1;
                        continue;
                    }
                };

                let style_id = raw.style_id().ok();
                let (_style, foreground, background, underline_color, flags) = if style_id.is_some()
                    && style_id == cached_style_id
                {
                    // 同一样式 run：复用已缓存的解析颜色。
                    (
                        None,
                        cached_foreground,
                        cached_background,
                        cached_underline,
                        cached_flags,
                    )
                } else {
                    match cell.style() {
                        Ok(style) => {
                            let foreground = Self::cell_color(cell.fg_color(), default_foreground);
                            let background = Self::cell_color(cell.bg_color(), default_background);
                            let underline = Self::resolve_style_color(
                                terminal,
                                &style.underline_color,
                                foreground,
                            );
                            let style_flags = Self::pack_style_flags(&style);
                            cached_style_id = style_id;
                            cached_foreground = foreground;
                            cached_background = background;
                            cached_underline = underline;
                            cached_flags = style_flags;
                            (Some(style), foreground, background, underline, style_flags)
                        }
                        Err(_) => {
                            // 宽度仍可从 raw cell 读到，必须照真实宽度推进：
                            // 宽字符（中日韩）按 1 列推进会让本行其后每一个单元的
                            // `col` 左移一格，且行长度不再等于 cols。
                            // spacer（None）：两列已由宽字符本身消耗，直接跳过。
                            if let Some(width) = Self::cell_columns(&raw) {
                                Self::push_blank_cell(
                                    &mut row_data,
                                    default_foreground,
                                    default_background,
                                    current_row,
                                    current_col,
                                    width,
                                );
                                current_col += width;
                            }
                            continue;
                        }
                    }
                };

                let codepoint = raw.codepoint().unwrap_or(0);

                // 跳过 Ghostty 为宽字符发出的 spacer 单元（SpacerTail、SpacerHead）。
                // 它们没有内容，若照常推进会让 `current_col` 错位，
                // 导致其后所有单元右移一列。
                // spacer 单元不产出 CellData 条目。
                // `current_col` 保持不变——两列已由宽字符本身消耗。
                let Some(width) = Self::cell_columns(&raw) else {
                    continue;
                };

                let mut grapheme_extra = [0u32; MAX_GRAPHEME_EXTRA];
                if let Ok(graphemes) = cell.graphemes() {
                    for (grapheme_index, &codepoint) in graphemes
                        .iter()
                        .enumerate()
                        .skip(1)
                        .take(MAX_GRAPHEME_EXTRA)
                    {
                        grapheme_extra[grapheme_index - 1] = codepoint as u32;
                    }
                }

                // 终端持有选区的行内反白（经典反白：前景背景互换，与覆盖层旧语义一致；
                // 下划线色同步取反白后的前景，保证选中文本的下划线仍可见）。
                let (foreground, background, underline_color) =
                    if row_selection.as_ref().is_some_and(|range| {
                        let col = current_col as u16;
                        col >= range.start_x && col <= range.end_x
                    }) {
                        (background, foreground, background)
                    } else {
                        (foreground, background, underline_color)
                    };

                row_data.push(CellData {
                    codepoint,
                    width,
                    grapheme_extra,
                    foreground,
                    background,
                    underline_color,
                    flags,
                    row: current_row,
                    col: current_col,
                });
                current_col += width;
            }
            data.extend_from_slice(&row_data);
            current_row += 1;
        }
        // 行迭代在此自然结束：它持有 `RowIterator` 的可变借用与快照的共享借用，
        // 而后续的 `Snapshot::clean` 与读光标都只取 `&self`，两者不冲突——
        // 无需提前 drop（`RowIteration` 不实现 `Drop`，显式 drop 只延长借用而无作用）。

        // 抽取中断：该帧网格不完整，不得把半张网格推给渲染线程，也不留作缓存。
        if !extraction_complete {
            row_cache.invalidate();
            return None;
        }

        // 本帧完整后才刷新缓存与标记快照已消费：
        // · 缓存按 `build_row_ranges` 的行边界划分，行数与内容不符即无法复用；
        // · 上游要求整帧完成后才可 `clean`，提前标记会让下一帧误判未读出的行为干净。
        //
        // 一次整网格拷贝（约 150KB/24×80）换下一帧少走 22 行的逐单元 FFI 往返
        // （实测同一 `RenderState` 跨帧复用时，改一行只判脏该行与末行）：
        // 净收益为正，故此处不做「全干净帧免拷贝」之类的特化——该路径本就
        // 很少进入（`build_cell_data` 由 `batch_dirty`/`cell_data_dirty` 双重守卫），
        // 而特化会引入一条需要单独证明正确的相等性判断。
        row_cache.store(&data, rows);
        if let Err(clean_error) = snapshot.clean() {
            log::warn!("build_cell_data: snapshot clean failed: {clean_error}");
        }
        #[cfg(test)]
        cache_stats.record(reused_rows, extracted_rows);

        let cursor_style = Self::cursor_style_from_snapshot(&snapshot);
        // 光标取视口相对坐标：口径与判据见 [cursor_in_viewport]。
        let (cursor_row, cursor_col, cursor_at_wide_tail, cursor_visible) =
            cursor_in_viewport(&snapshot);
        Some((
            data,
            CursorInfo {
                row: cursor_row,
                col: cursor_col,
                at_wide_tail: cursor_at_wide_tail,
                visible: cursor_visible,
                style: cursor_style,
                scrollback_length: scrollback_len(terminal),
                rows,
                cols,
                kitty_generation: terminal
                    .kitty_graphics()
                    .ok()
                    .and_then(|graphics| graphics.generation().ok())
                    .unwrap_or(0),
            },
        ))
    }

    /// 把样式属性打包成与 `cell.wgsl` 着色器布局一致的位掩码：
    /// 第 0 位=bold、1=italic、2=reverse、3=underline、
    /// 4=blink、5=strikethrough、6=overline、7=dim、8=double_underline
    /// （blink 会带上，着色器忽略它；留待将来使用）。
    fn pack_style_flags(style: &libghostty_vt::style::Style) -> u32 {
        use crate::terminal::ghostty_terminal::cell_flags;
        let mut flags = 0u32;
        if style.bold {
            flags |= 1 << cell_flags::BOLD;
        }
        if style.italic {
            flags |= 1 << cell_flags::ITALIC;
        }
        if style.inverse {
            flags |= 1 << cell_flags::REVERSE;
        }
        if style.blink {
            flags |= 1 << cell_flags::BLINK;
        }
        if has_visible_underline(style.underline) {
            flags |= 1 << cell_flags::UNDERLINE;
        }
        if style.strikethrough {
            flags |= 1 << cell_flags::STRIKETHROUGH;
        }
        if style.overline {
            flags |= 1 << cell_flags::OVERLINE;
        }
        if style.faint {
            flags |= 1 << cell_flags::FAINT;
        }
        if is_double_underline(style.underline) {
            flags |= 1 << cell_flags::DOUBLE_UNDERLINE;
        }
        flags
    }

    pub(crate) fn build_snapshot(
        terminal: &Terminal,
        default_foreground: [f32; 4],
        default_background: [f32; 4],
    ) -> GridSnapshot {
        let rows = grid_rows(terminal);
        let cols = grid_cols(terminal);
        let size = (rows * cols) as usize;
        let mut cells = Vec::with_capacity(size);

        // 独立的 RenderState 与迭代器：快照路径每次整帧重建，不做行级缓存，
        // 故不复用 `build_cell_data` 跨帧持有的那一套。上游的脏状态按实例保存
        // （见 render.rs 模块文档「dirty tracking」），两套实例互不消费对方的
        // 脏标记——共用会使行级判定读到已被快照路径 `clean` 清掉的状态。
        let (mut render_state, mut row_iter, mut cell_iter) = match create_render_iterators() {
            Some(iterator_bundle) => iterator_bundle,
            None => return GridSnapshot::fallback(rows, cols),
        };

        let snapshot = match render_state.update(terminal) {
            Ok(render_snapshot) => render_snapshot,
            Err(update_error) => {
                log::error!("build_snapshot: render_state.update failed: {update_error}");
                return GridSnapshot::fallback(rows, cols);
            }
        };

        let mut row_iter_impl = match row_iter.update(&snapshot) {
            Ok(row_iterator) => row_iterator,
            Err(row_error) => {
                log::error!("build_snapshot: row_iter.update failed: {row_error}");
                return GridSnapshot::fallback(rows, cols);
            }
        };

        // ── CellIterator 循环 ──
        // 先用 RowIterator 遍历全部可见行，再用 CellIterator 遍历每行的
        // 全部单元。取代了逐单元的 grid_ref。
        while let Some(row) = row_iter_impl.next() {
            let mut cell_iter_impl = match cell_iter.update(row) {
                Ok(cell_iterator) => cell_iterator,
                Err(error) => {
                    log::warn!("build_snapshot: cell_iter.update failed: {error}");
                    break;
                }
            };

            while let Some(cell) = cell_iter_impl.next() {
                let raw = match cell.raw_cell() {
                    Ok(raw_cell) => raw_cell,
                    Err(error) => {
                        log::warn!("build_snapshot: raw_cell failed: {error}");
                        cells.push(CellSnapshot {
                            foreground: default_foreground,
                            background: default_background,
                            ..CellSnapshot::default()
                        });
                        continue;
                    }
                };

                let style = match cell.style() {
                    Ok(cell_style) => cell_style,
                    Err(error) => {
                        log::warn!("build_snapshot: cell style failed: {error}");
                        cells.push(CellSnapshot {
                            foreground: default_foreground,
                            background: default_background,
                            ..CellSnapshot::default()
                        });
                        continue;
                    }
                };

                let codepoint = raw.codepoint().unwrap_or(0);

                // 快照路径保留占位：与 CellData 跳过 Spacer 不同，快照按网格索引逐格存放，宽字符占位格须保留 width=1 条目以对齐行列。仅测试行使。
                let width = match raw.wide() {
                    Ok(libghostty_vt::screen::CellWide::Wide) => 2,
                    _ => 1,
                };

                let graphemes: Vec<u32> = match cell.graphemes() {
                    Ok(grapheme_slice) if grapheme_slice.len() <= MAX_GRAPHEME_CLUSTERS => {
                        grapheme_slice
                            .iter()
                            .map(|&codepoint| codepoint as u32)
                            .collect()
                    }
                    Ok(grapheme_slice) => grapheme_slice
                        .iter()
                        .take(MAX_GRAPHEME_CLUSTERS)
                        .map(|&codepoint| codepoint as u32)
                        .collect(),
                    Err(_) => vec![codepoint],
                };

                let foreground = Self::cell_color(cell.fg_color(), default_foreground);
                let background = Self::cell_color(cell.bg_color(), default_background);
                let underline_color =
                    Self::resolve_style_color(terminal, &style.underline_color, foreground);

                cells.push(CellSnapshot {
                    codepoint,
                    graphemes,
                    foreground,
                    background,
                    underline_color,
                    bold: style.bold,
                    dim: style.faint,
                    italic: style.italic,
                    underline: has_visible_underline(style.underline),
                    reverse: style.inverse,
                    strikethrough: style.strikethrough,
                    blink: style.blink,
                    hidden: style.invisible,
                    overline: style.overline,
                    double_underline: is_double_underline(style.underline),
                    width,
                });
            }
        }

        // 视口相对的光标坐标（与按视口行存放的单元行一致）：口径见 [cursor_in_viewport]。
        let (cursor_row, cursor_col, _, cursor_visible) = cursor_in_viewport(&snapshot);

        let dirty = vec![true; (rows as usize) * (cols as usize)];

        GridSnapshot {
            rows,
            cols,
            cursor_row,
            cursor_col,
            cursor_visible,
            cursor_style: Self::cursor_style_from_snapshot(&snapshot),
            cells,
            dirty,

            // 标题读失败不静默留空：会话列表会把该会话显示成无名，而空标题与
            // 「应用本来就没设标题」不可区分。
            title: match terminal.title() {
                Ok(title) => title.to_string(),
                Err(error) => {
                    log::error!("ghostty_terminal: title query failed: {error}");
                    String::new()
                }
            },
            scrollback_length: scrollback_len(terminal),
        }
    }

    /// 纯文本单行：跳过 codepoint 0（宽字符尾格与空格），行尾去空白。
    ///
    /// 与 [`Self::read_line_text_impl`] 的区别是空单元不占位。全量文本面向
    /// 「这一行写了什么」，语义与 [`Self::build_dumped_grid`] 的消费侧一致。
    pub(crate) fn read_all_text_row_impl(terminal: &Terminal, row: u32) -> String {
        let cols = grid_cols(terminal);
        let mut text = String::new();
        for col in 0..cols {
            let codepoint = cell_codepoint(terminal, row, col);
            if let Some(character) = char::from_u32(codepoint).filter(|&c| c != '\0') {
                text.push(character);
            }
        }
        text.trim_end().to_string()
    }

    /// 单行文本，**每个网格列恰好一个字符**：codepoint 为 0 的空单元与宽字符尾格
    /// 都以空格占位。
    ///
    /// 字符下标即列号，故供需要按列寻址的消费方使用（选区拖动经 JNI 的
    /// `scrollbackLine` 取行文本后按列换算）。搜索高亮不用此读口：它按列填充会把宽字符拆成「字 + 空格」，
    /// CJK 多字命中因而无法定位到整字，见 [`Self::selection_endpoint_columns`]。
    pub(crate) fn read_line_text_impl(terminal: &Terminal, row: u32) -> Option<String> {
        let cols = grid_cols(terminal);
        let mut text = String::new();
        for col in 0..cols {
            // 每个网格列必须**恰好**产出一个字符：读不到单元时若少产一个字符，
            // 其后所有列号都会左移一格，且偏移会经软换行段累加继续放大。
            let codepoint = cell_codepoint(terminal, row, col);
            match char::from_u32(codepoint) {
                Some(character) if codepoint != 0 => text.push(character),
                _ => text.push(' '),
            }
        }
        let trimmed = text.trim_end().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }

    /// 经 Ghostty 原生格式化器做换行感知的选区文本提取。
    ///
    /// 参考：termux-app TerminalBuffer.getSelectedText（joinBackLines）加
    /// TerminalRow.findStartOfColumn。格式化器的 `unwrap` 拼接软换行行且不带
    /// '\n'，`trim` 去掉尾部空白；网格列内部映射为字符下标，故 CJK 宽字符
    /// 不会被拆开（代理对不产生列到字符的漂移）。
    ///
    /// 坐标是绝对网格行（0 = 回滚顶部；视口自 `scrollback_rows` 起）。
    /// 回移行经 Point::History 解析，视口行经 Point::Viewport（等价于带绝对
    /// y 的 Point::Screen）。选区无效时返回空串。
    pub(crate) fn selection_text_impl(
        terminal: &Terminal,
        start: (u32, u32),
        end: (u32, u32),
    ) -> String {
        // 绝对网格行（0 = 回滚顶部）→ Point 的空间解析统一见 absolute_point。
        let start_point = Self::absolute_point(terminal, start.0, start.1);
        let end_point = Self::absolute_point(terminal, end.0, end.1);
        let (Ok(start_gref), Ok(end_gref)) =
            (terminal.grid_ref(start_point), terminal.grid_ref(end_point))
        else {
            return String::new();
        };
        let selection =
            libghostty_vt::selection::Selection::new(start_gref, end_gref, SELECTION_RECTANGLE);
        let mut formatter = match libghostty_vt::fmt::Formatter::new(
            terminal,
            libghostty_vt::fmt::FormatterOptions::new()
                .with_unwrap(true)
                .with_trim(true)
                .with_selection(&selection),
        ) {
            Ok(initialized_formatter) => initialized_formatter,
            Err(error) => {
                log::error!("ghostty_terminal: formatter new failed: {error}");
                return String::new();
            }
        };
        match formatter.format_alloc(None) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(error) => {
                log::error!("ghostty_terminal: formatter format failed: {error}");
                String::new()
            }
        }
    }

    /// 该行中作为宽字符后半格（`SpacerTail`）的列号，升序。
    ///
    /// 宽字符占几列只有网格知道：行文本里尾格与真空白同为 `' '`，据此反推必然出错
    /// （见 change design 第 1 节）。`SpacerHead` 不计入——它出现在软换行续行的行首，
    /// 该行前一列属于折行前的上一行，把它当尾格会把选区跨过折行边界。
    pub(crate) fn wide_char_tail_cols_impl(terminal: &Terminal, row: u32) -> Vec<u32> {
        (0..grid_cols(terminal))
            .filter(|&col| {
                terminal
                    .grid_ref(Self::absolute_point(terminal, row, col))
                    .and_then(|point| point.cell())
                    .is_ok_and(|cell| {
                        matches!(cell.wide(), Ok(libghostty_vt::screen::CellWide::SpacerTail))
                    })
            })
            .collect()
    }

    /// 绝对网格行（0 = 回滚顶部）→ Point：唯一前向映射规则，列钳制到网格宽度。
    ///
    /// 恒用 [`PointSpace::Screen`]/`Point::Screen`（上游 `.screen` 的原点恒为
    /// `pages.first`，即回滚顶部的绝对行）。`Point::Viewport` 的原点随
    /// `pages.viewport` 滚动而移动，把它当绝对坐标会在上滚 `s` 行后整体错位 `s` 行。
    /// 供 selection_text_impl、install_selection_impl 与上游选择派生共用。
    fn absolute_point(terminal: &Terminal, row: u32, col: u32) -> Point {
        let clamped_col = col.min(grid_cols(terminal).saturating_sub(1)) as u16;
        Point::Screen(PointCoordinate {
            x: clamped_col,
            y: row,
        })
    }

    /// 绝对网格坐标 → gref：三处选择派生与超链接查询的公共读口，
    /// 越界/无效坐标一律 None（调用方据此返回"无该格"）。
    fn grid_ref_at<'a>(terminal: &'a Terminal, row: u32, col: u32) -> Option<GridRef<'a>> {
        terminal
            .grid_ref(Self::absolute_point(terminal, row, col))
            .ok()
    }

    /// gref → 绝对网格坐标：absolute_point 的逆，单次 Screen 空间回读
    /// （`.screen` 原点与 `absolute_point` 同一），无法表达该格时返回 None。
    fn absolute_coordinate(terminal: &Terminal, grid_ref: &GridRef) -> Option<(u32, u32)> {
        let coordinate = terminal
            .point_from_grid_ref(grid_ref, PointSpace::Screen)
            .ok()??;
        Some((coordinate.y, u32::from(coordinate.x)))
    }

    /// 上游派生选区的公共收尾（design 决策 1 的顺序）：to_ordered(Forward)
    /// 取序 → gref 反解为绝对坐标 → set_selection 安装 → 回传界限。
    /// 反解或安装失败时返回 None，绝不回传未安装到终端的界限。
    fn installed_bounds(
        terminal: &Terminal,
        selection: Selection<'_>,
    ) -> Option<((u32, u32), (u32, u32))> {
        let ordered = selection.to_ordered(terminal, Order::Forward).ok()?;
        let start = Self::absolute_coordinate(terminal, &ordered.start())?;
        let end = Self::absolute_coordinate(terminal, &ordered.end())?;
        if terminal.set_selection(Some(&ordered)).is_err() {
            log::warn!("ghostty_terminal: install derived selection failed");
            return None;
        }
        Some((start, end))
    }

    /// 上游词选语义（`Terminal::select_word`，Ghostty 默认词边界码位）：
    /// 派生该格所属词的选区、安装并回传有序绝对界限。
    pub(crate) fn select_word_at_impl(
        terminal: &Terminal,
        row: u32,
        col: u32,
    ) -> Option<((u32, u32), (u32, u32))> {
        let grid_ref = Self::grid_ref_at(terminal, row, col)?;
        let selection = terminal
            .select_word(SelectWordOptions::new(grid_ref))
            .ok()??;
        Self::installed_bounds(terminal, selection)
    }

    /// 上游全选语义（`Terminal::select_all`，“all selectable terminal
    /// content”）：界限不含尾部空行/空列（design 决策 2，由 cargo 测试钉住）。
    pub(crate) fn select_all_impl(terminal: &Terminal) -> Option<((u32, u32), (u32, u32))> {
        let selection = terminal.select_all().ok()??;
        Self::installed_bounds(terminal, selection)
    }

    /// 将绝对网格坐标的选区安装为终端持有的活动选区（跟踪引用）。
    /// 与 selection_text_impl 同一坐标系：绝对网格行（0 = 回滚顶部）。
    /// 无效端点时静默忽略（调用方已标记脏并重推，保持帧一致）。
    pub(crate) fn install_selection_impl(terminal: &Terminal, start: (u32, u32), end: (u32, u32)) {
        let (Ok(start_ref), Ok(end_ref)) = (
            terminal.grid_ref(Self::absolute_point(terminal, start.0, start.1)),
            terminal.grid_ref(Self::absolute_point(terminal, end.0, end.1)),
        ) else {
            return;
        };
        let selection =
            libghostty_vt::selection::Selection::new(start_ref, end_ref, SELECTION_RECTANGLE);
        if terminal.set_selection(Some(&selection)).is_err() {
            log::warn!("ghostty_terminal: install selection failed");
        }
    }

    /// 查询某网格单元处的 OSC 8 超链接 URI（等价 termux TerminalView 的
    /// openLinkAt；ghostty 侧用 cell.has_hyperlink + hyperlink_uri）。
    pub(crate) fn hyperlink_at_impl(terminal: &Terminal, row: u32, col: u32) -> Option<String> {
        let cols = grid_cols(terminal);
        let Ok(total_rows) = terminal.total_rows() else {
            log::error!("ghostty_terminal: total_rows failed during hyperlink hit test");
            return None;
        };
        if col >= cols || row >= total_rows as u32 {
            return None;
        }
        let grid_ref = Self::grid_ref_at(terminal, row, col)?;
        let cell = grid_ref.cell().ok()?;
        match cell.has_hyperlink() {
            Ok(true) => {}
            Ok(false) => return None,
            // 读不到时当「无链接」，但必须出声：静默 false 会让用户点不动链接
            // 且无从判断是坐标没落在链接上还是读取失败。
            Err(error) => {
                log::warn!("ghostty_terminal: has_hyperlink failed: {error}");
                return None;
            }
        }
        // 超长 URI（>4KiB）按上游 OutOfSpace{required} 重试，避免截断长链接。
        let mut buf = [0u8; HYPERLINK_INLINE_BUFFER_SIZE];
        match grid_ref.hyperlink_uri(&mut buf) {
            Ok(0) => None,
            Ok(len) => Some(String::from_utf8_lossy(&buf[..len]).into_owned()),
            Err(libghostty_vt::error::Error::OutOfSpace { required }) => {
                // 超过上限的 URI 直接丢弃并出声：截断的链接指向错误地址，
                // 比没有链接更糟；上限内才值得第二次精确读取。
                if required == 0 || required > MAX_HYPERLINK_URI_BYTES {
                    log::warn!(
                        "ghostty_terminal: hyperlink URI unusable (required {required} bytes), dropping link"
                    );
                    return None;
                }
                let mut grown = vec![0u8; required];
                match grid_ref.hyperlink_uri(&mut grown) {
                    Ok(0) => None,
                    Ok(grown_len) => {
                        Some(String::from_utf8_lossy(&grown[..grown_len]).into_owned())
                    }
                    Err(error) => {
                        log::warn!("ghostty_terminal: hyperlink URI reread failed: {error}");
                        None
                    }
                }
            }
            Err(error) => {
                log::warn!("ghostty_terminal: hyperlink URI read failed: {error}");
                None
            }
        }
    }

    /// 搜索 needle 长度上限：超长 needle 判为无命中，避免上游搜索器为不可能命中的
    /// 长串付出代价。唯一真源，Kotlin 侧经 `NativeBridge.searchQueryMaxChars()` 取值
    /// 截断输入，不再自带副本（改这里即两侧同步生效）。
    pub(crate) const MAX_SEARCH_QUERY_CHARS: usize = 128;

    /// 行内最后一个有内容的列的开区间上界：行尾未写下的填充不计，高亮不越过它。
    ///
    /// 自右向左扫。宽字符的尾格自身无 codepoint，故命中尾格即以 `col + 1` 收尾
    /// （它与前一格的首列同属一个宽字符）。
    fn row_content_columns(terminal: &Terminal, row: u32) -> u32 {
        for col in (0..grid_cols(terminal)).rev() {
            let Ok(grid_ref) = terminal.grid_ref(Self::absolute_point(terminal, row, col)) else {
                continue;
            };
            let Ok(cell) = grid_ref.cell() else { continue };
            if matches!(cell.wide(), Ok(CellWide::SpacerTail)) {
                return col + 1;
            }
            if cell_codepoint(terminal, row, col) != 0 {
                return col + Self::cell_columns(&cell).unwrap_or(1);
            }
        }
        0
    }

    /// 选区端点单元的 `(行, 首列, 开区间上界)`：宽字符覆盖两列，端点落在它的头/尾格
    /// 时起点回退到该宽字符的首列。端点格读不出时返回 None。
    fn selection_endpoint_columns(
        terminal: &Terminal,
        grid_ref: &GridRef,
    ) -> Option<(u32, u32, u32)> {
        let (row, col) = Self::absolute_coordinate(terminal, grid_ref)?;
        let cell = grid_ref.cell().ok()?;
        let start_col = match cell.wide() {
            Ok(CellWide::SpacerTail | CellWide::SpacerHead) => col.saturating_sub(1),
            _ => col,
        };
        let columns = match cell.wide() {
            Ok(CellWide::Wide | CellWide::SpacerTail | CellWide::SpacerHead) => 2,
            _ => 1,
        };
        Some((row, start_col, start_col + columns))
    }

    /// 上游命中的 [`Selection`] → 按物理行拆分的绝对网格区间。
    ///
    /// 上游按软换行把跨行命中表达为一条选区，本仓的高亮按行表达：首行自命中起点到
    /// 行内容末尾，末行自 0 到命中终点，中间整行覆盖，且每行末尾按行内容裁剪。
    /// 端点可能反向（上游保留选择方向），按行优先序对调即得正向区间。
    fn search_matches_from_selection(
        terminal: &Terminal,
        selection: &Selection,
        matches: &mut Vec<SearchMatch>,
    ) {
        let Some((start_row, start_col, start_limit)) =
            Self::selection_endpoint_columns(terminal, &selection.start())
        else {
            return;
        };
        let Some((end_row, end_col, end_limit)) =
            Self::selection_endpoint_columns(terminal, &selection.end())
        else {
            return;
        };
        let ((first_row, first_col), (last_row, last_limit)) =
            if (start_row, start_col) <= (end_row, end_col) {
                ((start_row, start_col), (end_row, end_limit))
            } else {
                ((end_row, end_col), (start_row, start_limit))
            };
        for row in first_row..=last_row {
            let content_columns = Self::row_content_columns(terminal, row);
            let row_start = if row == first_row { first_col } else { 0 };
            let row_end = if row == last_row {
                last_limit
            } else {
                content_columns
            };
            let start_col = row_start.min(content_columns);
            let end_col = row_end.min(content_columns);
            if start_col < end_col {
                matches.push(SearchMatch {
                    row,
                    start_col,
                    end_col,
                });
            }
        }
    }

    /// 设置 needle 并同步跑完搜索（`feed` + `tick` 至 `Complete`），返回命中总数。
    ///
    /// 搜索对象随 VT 线程存活：首次调用时创建，之后复用同一份索引状态，故连续
    /// 查询不必重建。needle 超上限不截断而是判为无命中——截断会让用户搜到另一个
    /// 串的结果。
    fn search_set_needle_impl<'term>(
        search: &mut Option<Search<'_, 'term>>,
        terminal: &mut Terminal<'term, '_>,
        needle: &str,
    ) -> usize {
        if search.is_none() {
            match Search::new(terminal) {
                Ok(created) => *search = Some(created),
                Err(error) => {
                    log::error!("ghostty_terminal: search::Search::new failed: {error}");
                    return 0;
                }
            }
        }
        let Some(active) = search.as_mut() else {
            return 0;
        };
        let needle = if needle.chars().count() > Self::MAX_SEARCH_QUERY_CHARS {
            log::warn!(
                "ghostty_terminal: search needle over {} chars ignored",
                Self::MAX_SEARCH_QUERY_CHARS
            );
            ""
        } else {
            needle
        };
        if let Err(error) = active.set_needle(terminal, needle.as_bytes()) {
            log::error!("ghostty_terminal: search set_needle failed: {error}");
            return 0;
        }
        if let Err(error) = active.run(terminal) {
            log::error!("ghostty_terminal: search run failed: {error}");
            return 0;
        }
        match active.total_matches() {
            Ok(total) => total,
            Err(error) => {
                log::error!("ghostty_terminal: search total_matches failed: {error}");
                0
            }
        }
    }

    /// 选中下一个（`forward = true`，朝更旧内容）或上一个命中，返回选中态。
    ///
    /// 视口滚动由上游按滚动策略完成（`Scroll::IfNeeded`），本仓不重复实现定位。
    fn search_select_impl<'term>(
        search: &mut Option<Search<'_, 'term>>,
        terminal: &mut Terminal<'term, '_>,
        forward: bool,
    ) -> Option<SearchSelection> {
        let active = search.as_mut()?;
        let selected = if forward {
            active.select_next(terminal)
        } else {
            active.select_prev(terminal)
        };
        match selected {
            Ok(true) => {}
            Ok(false) => return None,
            Err(error) => {
                log::error!("ghostty_terminal: search select failed: {error}");
                return None;
            }
        }
        let total = match active.total_matches() {
            Ok(total) => total,
            Err(error) => {
                log::error!("ghostty_terminal: search total_matches failed: {error}");
                return None;
            }
        };
        let index = active.selected_index().ok().flatten()?;
        let snapshot = match active.snapshot(terminal) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                log::error!("ghostty_terminal: search snapshot failed: {error}");
                return None;
            }
        };
        let selected_match = match snapshot.selected_match() {
            Ok(Some(selected_match)) => selected_match,
            Ok(None) => return None,
            Err(error) => {
                log::error!("ghostty_terminal: search selected_match failed: {error}");
                return None;
            }
        };
        let mut matches = Vec::new();
        Self::search_matches_from_selection(snapshot.terminal(), &selected_match, &mut matches);
        Some(SearchSelection {
            index,
            total,
            matched: matches.first()?.clone(),
        })
    }

    /// 视口高亮输入：一次 `snapshot` 同时取视口命中与选中命中（同一快照内一致，
    /// 避免两次读取之间输出到达导致选中态与命中集不同源），回滚长度供渲染侧折算
    /// 绝对行到网格行。
    fn search_viewport_highlights_impl<'term>(
        search: &mut Option<Search<'_, 'term>>,
        match_buffer: &mut MatchBuffer,
        terminal: &mut Terminal<'term, '_>,
    ) -> SearchViewportHighlights {
        let scrollback_lines = scrollback_len(terminal);
        let Some(active) = search.as_mut() else {
            return SearchViewportHighlights {
                matches: Vec::new(),
                selected: None,
                scrollback_lines,
            };
        };
        let snapshot = match active.snapshot(terminal) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                log::error!("ghostty_terminal: search snapshot failed: {error}");
                return SearchViewportHighlights {
                    matches: Vec::new(),
                    selected: None,
                    scrollback_lines,
                };
            }
        };
        let selected = match snapshot.selected_match() {
            Ok(Some(selected_match)) => {
                let mut selected_matches = Vec::new();
                Self::search_matches_from_selection(
                    snapshot.terminal(),
                    &selected_match,
                    &mut selected_matches,
                );
                selected_matches.first().cloned()
            }
            Ok(None) => None,
            Err(error) => {
                log::error!("ghostty_terminal: search selected_match failed: {error}");
                None
            }
        };
        let Ok(viewport_matches) = snapshot.viewport_matches(match_buffer) else {
            log::error!("ghostty_terminal: search viewport_matches failed");
            return SearchViewportHighlights {
                matches: Vec::new(),
                selected,
                scrollback_lines,
            };
        };
        let mut matches = Vec::new();
        for selection in viewport_matches {
            Self::search_matches_from_selection(snapshot.terminal(), &selection, &mut matches);
        }
        SearchViewportHighlights {
            matches,
            selected,
            scrollback_lines,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::ghostty_terminal::GhosttyTerminal;
    use libghostty_vt::style::{Style, StyleColor, Underline};

    /// 回归钉：单元帧被丢弃的告警必须限频。
    ///
    /// 渲染暂停期刻意不消费通道（`receive` 是破坏性取数，取走的帧永不呈现），
    /// 通道于是被填满，此后每次推送都失败——实测设备上每秒约 40 条、持续到恢复为止。
    /// 逐条记无诊断价值却挤爆 logcat 并持续吃 CPU。
    ///
    /// 判别力：限频窗口内无论调用多少次，只应放行第一次。本例断言「连续 50 次里
    /// 放行次数为 1」；去掉限频（每次都记）即失败。
    #[test]
    fn cell_frame_drop_warning_is_throttled() {
        use std::sync::atomic::Ordering;
        // 从零起算，确保第一次调用一定落在窗口内（否则静态原子被别的用例推进过）。
        LAST_CELL_FRAME_DROP_WARN_MS.store(0, Ordering::Relaxed);
        let error = flume::bounded::<(Vec<CellData>, CursorInfo)>(0)
            .0
            .try_send((
                Vec::new(),
                CursorInfo {
                    row: 0,
                    col: 0,
                    at_wide_tail: false,
                    visible: false,
                    style: CursorStyle::Block,
                    scrollback_length: 0,
                    rows: 0,
                    cols: 0,
                    kitty_generation: 0,
                },
            ))
            .expect_err("bounded(0) must reject every send");
        let mut logged = 0;
        for _ in 0..50 {
            // 直接数「本进程内本条是否真的被写出」不可行（log 无测试挂钩），
            // 故改为驱动限频闸门本身：把窗口设为极大，除首次外必然全部落空。
            let before = LAST_CELL_FRAME_DROP_WARN_MS.load(Ordering::Relaxed);
            warn_drop_throttled(&error);
            if LAST_CELL_FRAME_DROP_WARN_MS.load(Ordering::Relaxed) != before {
                logged += 1;
            }
        }
        assert_eq!(
            logged, 1,
            "限频窗口内只应放行一次告警，实际放行 {logged} 次"
        );
    }

    #[test]
    fn byte_to_float_scales_255() {
        assert_eq!(GhosttyTerminal::byte_to_float(0), 0.0);
        assert_eq!(GhosttyTerminal::byte_to_float(255), 1.0);
        assert!((GhosttyTerminal::byte_to_float(128) - 128.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn byte_color_to_float_includes_opaque_alpha() {
        let color = GhosttyTerminal::color_to_float(libghostty_vt::style::RgbColor {
            r: 13,
            g: 188,
            b: 121,
        });
        assert!((color[0] - 13.0 / 255.0).abs() < 1e-6);
        assert!((color[1] - 188.0 / 255.0).abs() < 1e-6);
        assert!((color[2] - 121.0 / 255.0).abs() < 1e-6);
        assert_eq!(color[3], 1.0, "alpha must be opaque");
    }

    /// OSC 4 调色板覆盖必须生效：有效调色板（含覆盖）而非静态表决定渲染色。
    #[test]
    fn osc4_palette_override_reaches_dumped_grid() {
        let mut terminal = GhosttyTerminal::new(5, 20, 100).expect("terminal");
        // 调色板索引 1 改为纯绿，再以红色（索引 1）写字：看到的必须是绿色。
        terminal.vt_write(b"\x1b]4;1;#00ff00\x07\x1b[31mX");
        terminal.flush();
        let dumped = terminal.dump_grid();
        let cell = &dumped.visible[0];
        assert_eq!(cell.codepoint, 'X' as u32);
        assert_eq!(
            cell.foreground,
            GhosttyTerminal::color_to_float(libghostty_vt::style::RgbColor { r: 0, g: 255, b: 0 }),
            "OSC 4 override must win over the static palette"
        );
    }

    /// SGR 58 下划线色进入快照（未设置时回退前景）。
    #[test]
    fn sgr58_underline_color_reaches_dumped_grid() {
        let mut terminal = GhosttyTerminal::new(5, 20, 100).expect("terminal");
        terminal.vt_write(b"\x1b[4m\x1b[58;2;255;0;0mU");
        terminal.flush();
        let dumped = terminal.dump_grid();
        let cell = &dumped.visible[0];
        assert!(cell.underline, "SGR 4 must set underline");
        assert_eq!(
            cell.underline_color,
            GhosttyTerminal::color_to_float(libghostty_vt::style::RgbColor { r: 255, g: 0, b: 0 }),
            "SGR 58 must set the underline color"
        );
    }

    #[test]
    fn underline_color_falls_back_to_foreground() {
        let mut terminal = GhosttyTerminal::new(5, 20, 100).expect("terminal");
        terminal.vt_write(b"\x1b[4m\x1b[31mV");
        terminal.flush();
        let dumped = terminal.dump_grid();
        let cell = &dumped.visible[0];
        assert!(cell.underline);
        assert_eq!(
            cell.underline_color, cell.foreground,
            "unset SGR 58 must fall back to the resolved foreground"
        );
    }

    /// SGR 53 上划线进入快照（与 SGR 58 用例对称的端到端覆盖）。
    #[test]
    fn sgr53_overline_reaches_dumped_grid() {
        let mut terminal = GhosttyTerminal::new(5, 20, 100).expect("terminal");
        terminal.vt_write(b"\x1b[53mO");
        terminal.flush();
        let dumped = terminal.dump_grid();
        let cell = &dumped.visible[0];
        assert!(cell.overline, "SGR 53 must set overline");
    }

    /// SGR 3 斜体进入快照：斜体缺失首先排除 VT 层，回应对“斜体无法显示”。
    #[test]
    fn sgr3_italic_reaches_dumped_grid() {
        let mut terminal = GhosttyTerminal::new(5, 20, 100).expect("terminal");
        terminal.vt_write(b"\x1b[3mI");
        terminal.flush();
        let dumped = terminal.dump_grid();
        let cell = &dumped.visible[0];
        assert_eq!(cell.codepoint, 'I' as u32);
        assert!(cell.italic, "SGR 3 must set italic");
    }

    /// SGR 31 红色前景进入快照：颜色缺失首先排除 VT 层，回应对“颜色只显示背景”。
    #[test]
    fn sgr31_red_foreground_reaches_dumped_grid() {
        let mut terminal = GhosttyTerminal::new(5, 20, 100).expect("terminal");
        terminal.vt_write(b"\x1b[31mR");
        terminal.flush();
        let dumped = terminal.dump_grid();
        let cell = &dumped.visible[0];
        assert_eq!(cell.codepoint, 'R' as u32);
        // 默认主题调色板索引 1，非纯红：断言有效调色板色即证明链路。
        // 色值取自默认调色板单源，不手写。
        assert_eq!(
            cell.foreground,
            GhosttyTerminal::color_to_float(crate::terminal::theme::default_dark().palette.ansi[1]),
            "SGR 31 must resolve to palette index 1"
        );
    }

    fn style_with_flags() -> Style {
        Style {
            bold: true,
            italic: true,
            faint: true,
            blink: false,
            inverse: true,
            invisible: false,
            strikethrough: true,
            overline: true,
            underline: Underline::Double,
            fg_color: StyleColor::None,
            bg_color: StyleColor::None,
            underline_color: StyleColor::None,
        }
    }

    #[test]
    fn pack_style_flags_sets_expected_bits() {
        let flags = GhosttyTerminal::pack_style_flags(&style_with_flags());
        assert_ne!(flags & (1 << cell_flags::BOLD), 0, "bold bit");
        assert_ne!(flags & (1 << cell_flags::ITALIC), 0, "italic bit");
        assert_ne!(flags & (1 << cell_flags::REVERSE), 0, "reverse bit");
        assert_ne!(flags & (1 << cell_flags::UNDERLINE), 0, "underline bit");
        assert_ne!(
            flags & (1 << cell_flags::DOUBLE_UNDERLINE),
            0,
            "double underline bit"
        );
        assert_ne!(
            flags & (1 << cell_flags::STRIKETHROUGH),
            0,
            "strikethrough bit"
        );
        assert_ne!(flags & (1 << cell_flags::OVERLINE), 0, "overline bit");
        assert_ne!(flags & (1 << cell_flags::FAINT), 0, "faint bit");
    }

    #[test]
    fn pack_style_flags_double_underline_sets_underline_too() {
        // 双下划线必须同时置上普通下划线位，着色器的高亮路径才会
        // 把该单元当作带下划线处理。
        let flags = GhosttyTerminal::pack_style_flags(&style_with_flags());
        let both = (1 << cell_flags::UNDERLINE) | (1 << cell_flags::DOUBLE_UNDERLINE);
        assert_eq!(flags & both, both);
    }

    #[test]
    fn pack_style_flags_default_style_is_zero() {
        let flags = GhosttyTerminal::pack_style_flags(&Style::default());
        assert_eq!(flags, 0);
    }

    #[test]
    fn pack_style_flags_plain_underline_sets_only_underline_bit() {
        let style = Style {
            underline: Underline::Single,
            ..Default::default()
        };
        let flags = GhosttyTerminal::pack_style_flags(&style);
        assert_eq!(flags, 1 << cell_flags::UNDERLINE);
    }

    #[test]
    fn pack_style_flags_curly_dashed_dotted_keep_underline_without_double() {
        // 对标上游下划线样式回归：Curly/Dashed/Dotted 必须保留下划线，
        // 且不得误置双下划线位。
        for underline in [Underline::Curly, Underline::Dashed, Underline::Dotted] {
            let style = Style {
                underline,
                ..Default::default()
            };
            let flags = GhosttyTerminal::pack_style_flags(&style);
            assert_eq!(
                flags & (1 << cell_flags::UNDERLINE),
                1 << cell_flags::UNDERLINE,
                "underline bit must survive"
            );
            assert_eq!(
                flags & (1 << cell_flags::DOUBLE_UNDERLINE),
                0,
                "double bit must stay clear"
            );
        }
    }
}
