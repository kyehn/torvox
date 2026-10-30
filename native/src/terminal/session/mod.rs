//! 会话编排器：串接 PTY 读取、VT 解析与进程等待。
mod lifecycle;
mod output;
mod pty_master;
mod spawn;

use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};

use flume::Receiver;
use thiserror::Error;

use crate::terminal::ghostty_terminal::GhosttyTerminal;
use crate::terminal::output_processor::OutputProcessor;
#[cfg(test)]
use crate::terminal::pty::Pty;
use crate::terminal::pty::PtyError;

pub(crate) use lifecycle::join_with_timeout;
pub use pty_master::PtyMaster;

/// 读线程每读到一块新输出后调用一次的唤醒钩子。
///
/// 会话本身不关心宿主的渲染循环，只负责「有新字节了」这一个事实；
/// 把钩子做成 `Arc<dyn Fn>` 让宿主（Android FFI 层）把 PTY 输出接到自己的
/// 渲染唤醒源上，而不必在会话里引入平台类型或轮询线程。
pub type OutputWake = Arc<dyn Fn() + Send + Sync>;
/// 子进程退出码槽位。三态而非 `Option<i32>`：等待线程写入与「已退出但码无从取得」
/// 必须可区分——前者要继续等，后者必须上报（否则退出事件永不到达，会话表泄漏）。
enum ExitCodeSlot {
    /// 子进程尚未退出，等待线程未写入。
    Pending,
    /// 等待线程已取得的真实退出码（含 128+信号 的信号死亡）。
    Code(i32),
    /// 子进程确已退出，但 `waitpid` 失败或返回了预期外状态：码未知。
    Unknown,
}

/// 可以上报给宿主的退出结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportedExit {
    /// 等待线程取得的真实退出码（信号死亡为 128+信号号）。
    Code(i32),
    /// 子进程已退出但退出码无从取得（`waitpid` 失败或状态预期外）。
    ///
    /// 单独一态而不是塞个 0 或 -1：调用方据此显示「退出码未知」，而不是把一次查不到
    /// 原因的死亡说成正常退出。序列化到 JNI 时为 JSON `null`。
    Unknown,
}
/// 回滚行数固定值（与 Termux 默认 transcript-rows=2000 一致）。
/// PROHIBITED 禁止「终端回滚行数」设置，因此既无产品入口也无 FFI 入参通道。
pub(crate) const DEFAULT_SCROLLBACK_LINES: u32 = 2000;

/// PTY 输出通道缓冲块数：读取线程与 VT 线程之间的背压边界，满时读取线程阻塞
/// 而非堆积无界内存。
const OUTPUT_CHANNEL_BOUND: usize = 128;
/// 会话操作期间可能出现的错误。
#[derive(Debug, Error)]
pub enum SessionError {
    #[error("pty error: {0}")]
    Pty(#[from] PtyError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ghostty error: {0}")]
    Ghostty(String),
    #[error("ghostty terminal error: {0}")]
    Terminal(#[from] crate::terminal::ghostty_terminal::TerminalError),
    #[error("session closed")]
    Closed,
    #[error("invalid dimensions (zero or out of u16 range)")]
    InvalidDimensions,
}

/// resize 结果：Ghostty 网格是否接受了命令。
/// `Applied` = PTY 与网格都已缩放；`Dropped` = PTY winsize 已变但网格命令被丢弃
/// （通道满 / VT 线程卡住）。`Dropped` 期间本会话缓存的网格尺寸仍是旧值，
/// 且已置 `grid_dirty` 使下一次 resize（即使尺寸相同）重试修复；因此调用方
/// 可以照常发布新尺寸，但不得把 `self.grid_size()` 当作已生效的权威值。
pub enum ResizeOutcome {
    Applied,
    Dropped,
}

impl SessionError {
    /// 底层写入以 EAGAIN/EWOULDBLOCK 失败时为真，即非阻塞主端的 PTY 缓冲区已满
    /// （子进程未读取）。此时调用方按 xterm 语义丢弃该输入而非当作错误上报。
    pub fn is_would_block(&self) -> bool {
        matches!(self, SessionError::Io(io_error) if io_error.kind() == std::io::ErrorKind::WouldBlock)
    }
}
/// 终端会话：串接 PTY 读取、VT 解析与进程等待。
///
/// 生命周期：`Spawned`（PTY 建立、线程启动）→ `Running` ⇄ `Idle`/`Paused`（有输出即回到
/// `Running`）→ `Exited`（EOF/退出）→ `Cleaned`（`cleanup_resources`）。
pub struct Session {
    pty: PtyMaster,
    terminal: GhosttyTerminal,
    output_processor: OutputProcessor,
    output_tx: flume::Sender<Vec<u8>>,
    output_rx: Receiver<Vec<u8>>,
    // ── 事件状态（Kotlin 经 push_event 轮询） ────────────────────────
    exited: Arc<AtomicBool>,
    /// 后台（非活跃）会话的 Exit 事件推入事件队列后置位，使 pollEvent 的逐帧扫描
    /// 只上报一次。
    exit_reported: Arc<AtomicBool>,
    /// 待上报的 OSC 52 剪贴板写入（FIFO）：`drain_callback_events` 收割，
    /// `poll_clipboard` 取走队首。队列而非单槽——渲染暂停（设置页/输入法弹出）
    /// 期间 `pollEvent` 停调，单槽会让后一次写入覆盖前一次，用户丢剪贴板内容。
    ///
    /// 深度**不**受 `EVENT_CHANNEL_CAPACITY` 约束：那只约束上游回调通道，而
    /// `drain_callback_events` 每帧把通道**排空**进本队列、`poll_clipboard` 每帧只
    /// 取一条，于是程序以快于每帧一条的速率发 OSC 52 时净增速可达每帧十余条，
    /// 队列无界增长。故显式封顶并丢最旧（见 `MAX_CLIPBOARD_TEXT_QUEUE`）。
    clipboard_text: Arc<Mutex<VecDeque<String>>>,
    /// 待处理的 OSC 52 剪贴板读取请求：所请求的 selection 名。
    ///
    /// FIFO：`poll_pty_output` 收割，JNI 层**对每个会话**（含后台）逐帧取走全部并
    /// 转发给宿主应用，经 [`Session::answer_clipboard_read`] 写回应答。队列而非
    /// 单槽——同帧/块内多个读请求必须全部作答，单槽 last-wins 会让被挤掉的请求
    /// 永不作答。深度由「每帧必被排空」约束：只排活跃会话时，后台程序持续发读
    /// 请求会让本队列无界增长（进一条、从不取），故后台分支同样收割。
    clipboard_read: Arc<Mutex<VecDeque<String>>>,

    /// 本帧未写完的 VT 应答（DECRPM/DSR/DA 等），FIFO。
    ///
    /// 独占性来自会话锁而非某个专属线程：`pollEvent` 会逐帧扫描**全部**会话，
    /// 故本字段由当帧持住本会话锁的那个线程独占（通常是活跃会话的渲染线程）。
    ///
    /// VT 引擎的应答缓冲是无界的 `Vec`（上游回调逐条 push），而回写每次可等到
    /// `WRITE_BACK_TIMEOUT`：于是一个 8KiB 输出块里的上万个 `CSI 6n` 查询会在同一帧
    /// 产生上万个应答，`drain_pty_write_back` 逐条同步写——子进程不读 stdin 时
    /// 每条都等满 50ms，**全程持会话锁**，把按帧取同锁的渲染与看门狗一起拖住。
    /// 故每帧只写有限条，写不完的留在这里下帧续写（见
    /// [`Session::drain_pty_write_back`]）。
    pty_write_back_queue: VecDeque<Vec<u8>>,

    // ── 线程生命周期 ─────────────────────────────────────────────────
    reader_handle: Option<std::thread::JoinHandle<()>>,
    wait_handle: Option<std::thread::JoinHandle<()>>,

    // ── 运行态 ───────────────────────────────────────────────────────
    /// 来自 waitpid 的退出码；进程运行中为 [ExitCodeSlot::Pending]。
    exit_code: Arc<Mutex<ExitCodeSlot>>,

    // ── 缓存的网格尺寸 ───────────────────────────────────────────────
    /// 最近已知的终端网格尺寸，spawn 与成功 resize 时更新。`ffi::switch_session_inner`
    /// 无锁读取以刷新缓存的网格尺寸，**不**发起阻塞式查询 RPC（在注册表写锁内查询会让
    /// 所有会话操作最多阻塞 2×`QUERY_TIMEOUT_MS`）。
    terminal_rows: AtomicU32,
    terminal_cols: AtomicU32,
    /// PTY 已缩放但网格 resize 命令被丢弃时置位，下次成功缩放网格后清除；
    /// 避免尺寸短路永久掩盖 PTY/网格不一致。
    ///
    grid_dirty: AtomicBool,

    /// 本会话上次已应用的滚动偏移。若按单一全局值计算增量，切换会话后旧会话的渲染
    /// 线程带着另一会话的偏移恢复时就会错误移动旧会话视口。
    ///
    /// 存在会话内而非注册表项内：滚动与重置两条路径都要在**注册表读锁**下取会话锁
    /// （会话锁可被 PTY 写入按住数秒），挂一个跨阻塞调用的注册表写锁会把整台终端冻住。
    last_scroll_offset: AtomicI64,
}

impl Session {
    /// 以已构造的 PTY 创建会话，不启动读取/等待线程（PTY I/O 由调用方驱动）。
    /// 仅供测试配合 `MockPty` 使用；生产构造入口是 [`Session::spawn`]。
    #[cfg(test)]
    pub fn with_pty_for_test(
        pty: Box<dyn Pty>,
        rows: u32,
        cols: u32,
    ) -> Result<Self, SessionError> {
        Self::spawn_with_pty(
            pty,
            rows,
            cols,
            None,
            Some(std::env::temp_dir()),
            DEFAULT_SCROLLBACK_LINES,
            None,
        )
    }

    /// 往本会话的输出通道注入一段 PTY 输出（等价于 `pty_read` 收到数据）。
    ///
    /// 仅供测试：`MockPty` 的输出侧是 `/dev/null`（见其 `try_clone_reader_fd`），
    /// 没有注入口，故「程序输出了什么」这条最常见的测试前提无法表达。
    #[cfg(test)]
    pub(crate) fn inject_output_for_test(&self, bytes: &[u8]) {
        self.output_tx
            .send(bytes.to_vec())
            .expect("output_tx send must succeed");
    }
    pub fn write(&mut self, data: &[u8]) -> Result<(), SessionError> {
        self.pty.write(data)
    }

    /// 克隆 PTY 写入面，供调用方在**释放会话锁之后**写入（见 [PtyMaster]）。
    ///
    /// 退出判定与写入都在 [PtyMaster::write] 内，调用方无须为此单持一次会话锁。
    pub fn pty_master(&self) -> PtyMaster {
        self.pty.clone()
    }

    /// 把终端缩放到给定行列数，超出 PTY ioctl 的 u16 范围时拒绝。网格命令被丢弃时
    /// PTY winsize 仍已更新（ioctl 已成功），但调用方不得把新尺寸作为权威值发布。
    pub fn resize(&mut self, rows: u32, cols: u32) -> Result<ResizeOutcome, SessionError> {
        // 0 行/0 列不是合法网格：与越界同判据拒绝，不把无意义尺寸递给 PTY 与 Ghostty。
        let (Ok(rows), Ok(cols)) = (u16::try_from(rows), u16::try_from(cols)) else {
            return Err(SessionError::InvalidDimensions);
        };
        if rows == 0 || cols == 0 {
            return Err(SessionError::InvalidDimensions);
        }
        // 相同尺寸直接短路，**除非**上一次网格 resize 被丢弃——此时缓存尺寸已与网格
        // 不符，必须重发相同尺寸以修复分歧。
        // `resize_failed` 与 `grid_dirty` 同为「不得按同尺寸短路」的判据：前者来自
        // 命令**已入队但 VT 线程上失败**，后者来自入队即被丢弃。两者的共同后果都是
        // PTY/缓存尺寸已更新而网格仍是旧尺寸。
        let dirty = self.grid_dirty.load(Ordering::Acquire) || self.terminal.resize_failed();
        if !dirty && (rows as u32, cols as u32) == self.grid_size() {
            return Ok(ResizeOutcome::Applied);
        }
        self.pty.resize(rows, cols)?;
        if !self.terminal.resize(rows as u32, cols as u32) {
            // PTY winsize 已变但 Ghostty 网格未变（命令被丢弃）。缓存保留旧尺寸并置
            // `grid_dirty`，使下次 resize 事件（即使尺寸相同）重试而非短路。
            self.grid_dirty.store(true, Ordering::Release);
            log::warn!(
                "session: ghostty grid resize to {rows}x{cols} dropped; PTY updated, grid lags — retry on next resize"
            );
            return Ok(ResizeOutcome::Dropped);
        }
        self.grid_dirty.store(false, Ordering::Release);
        // 入队即发布：VT 尚未应用新网格。帧装配不得读这组缓存
        // （`CursorInfo.rows/cols` 与 CellData 同源，）；
        // 它们只服务像素换算、去重短路与 Kotlin 查询。
        self.terminal_rows.store(rows as u32, Ordering::Release);
        self.terminal_cols.store(cols as u32, Ordering::Release);
        Ok(ResizeOutcome::Applied)
    }

    /// 经 TIOCSWINSZ 更新 PTY winsize 的像素字段（`ws_xpixel`/`ws_ypixel`）并保留当前行列。
    /// 像素感知的程序（`icat`、全屏 TUI）从 TIOCGWINSZ 读像素尺寸，为 0 时会回退到错误的
    /// 默认单元格尺寸。行列必须保留，因为 TIOCSWINSZ 会替换整个结构体。
    pub fn set_pixel_size(&self, width: u16, height: u16) -> Result<(), SessionError> {
        self.pty.set_pixel_size(width, height)?;
        // 同步单元格像素几何到终端（Kitty 放置几何依赖它；失败仅日志，不阻断 PTY）。
        let (rows, cols) = self.grid_size();
        if rows > 0 && cols > 0 && width > 0 && height > 0 {
            let cell_width = (u32::from(width) / cols).max(1);
            let cell_height = (u32::from(height) / rows).max(1);
            self.terminal.set_cell_pixel_size(cell_width, cell_height);
        }
        Ok(())
    }

    /// RIS 全重置：恢复终端初始状态并清空回滚（侧边面板“重置终端”按钮）。
    ///
    /// OSC 52 读取扫描器一并归位：它是 VT 之外的第二份解析状态，不跟着 RIS 重置
    /// 就会把重置后的头一段输出当成未闭合的读取请求吞掉（见 [`OutputProcessor::reset`]）。
    pub fn reset_terminal(&mut self) {
        self.output_processor.reset();
        // 清空本会话的续写队列：RIS 之前排队的应答带着旧网格的行列号，重置后再补发
        // 等于让终端状态与刚重建的网格对不上。VT 侧 `reset` 是异步投递的，引擎缓冲
        // 里尚未收割的应答仍可能随后到达——那属于「重置后仍排队」而非本队列残留。
        self.pty_write_back_queue.clear();
        self.terminal.reset();
        // VT 视口已归零：同步清零本会话的滚动记账，否则下一次滚动会按 stale 值算出
        // 错误 delta 误滚视图。
        self.last_scroll_offset.store(0, Ordering::Release);
    }

    /// 上次已应用的滚动偏移（滚动路径经 [`GhosttyTerminal::scroll_viewport`] 推进）。
    pub fn last_scroll_offset(&self) -> i64 {
        self.last_scroll_offset.load(Ordering::Acquire)
    }

    /// 推进滚动记账。仅在 `scroll_viewport` 确认投递后才调用——发送失败时保持旧值，
    /// 下次以相同目标调用才能重试该增量而非静默丢弃。
    pub fn set_last_scroll_offset(&self, offset: i64) {
        self.last_scroll_offset.store(offset, Ordering::Release);
    }

    /// 无锁读取最近已知的网格尺寸（spawn/resize 时更新）。绝不阻塞：VT 线程的权威尺寸
    /// 只能经查询 RPC 获得，而持有注册表写锁的调用方必须避开它。
    pub fn grid_size(&self) -> (u32, u32) {
        (
            self.terminal_rows.load(Ordering::Acquire),
            self.terminal_cols.load(Ordering::Acquire),
        )
    }
    pub fn terminal(&self) -> &GhosttyTerminal {
        &self.terminal
    }

    pub fn terminal_mut(&mut self) -> &mut GhosttyTerminal {
        &mut self.terminal
    }
}

#[cfg(test)]
mod tests;
