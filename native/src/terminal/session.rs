//! 会话编排器：串接 PTY 读取、VT 解析与进程等待。
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::fs::File;
use std::io::Read;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};
use std::time::Duration;

use flume::{Receiver, bounded};
use thiserror::Error;

use crate::terminal::ghostty_terminal::GhosttyTerminal;
use crate::terminal::output_processor::OutputProcessor;
use crate::terminal::pty::{Pty, PtyError, PtyPair};
use crate::terminal::shell_env::ShellEnv;

/// 读线程每读到一块新输出后调用一次的唤醒钩子。
///
/// 会话本身不关心宿主的渲染循环，只负责「有新字节了」这一个事实；
/// 把钩子做成 `Arc<dyn Fn>` 让宿主（Android FFI 层）把 PTY 输出接到自己的
/// 渲染唤醒源上，而不必在会话里引入平台类型或轮询线程。
pub type OutputWake = Arc<dyn Fn() + Send + Sync>;

const READ_BUF_SIZE: usize = 8192;

/// 拆卸会话时 SIGHUP 与 SIGKILL 之间的宽限期，同时作为读/等待线程的 join 超时。
const TRAILING_EXIT_GRACE: Duration = Duration::from_millis(50);
/// 信号致死退出码基数（shell 惯例：128 + 信号码）。
const SIGNAL_EXIT_BASE: i32 = 128;

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
/// `read(2)` 失败后读取线程应采取的动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReaderErrorAction {
    /// 重试读取——瞬时条件（EINTR），保持循环存活。
    Retry,
    /// 停止读取——PTY 已消失（EIO = 从端关闭）或错误致命。
    Stop,
}

/// 把 `read(2)` 错误归类为重试/停止决策。纯函数，使读取循环分支无需真实 PTY 即可测试。
fn read_error_action(raw_os_error: Option<i32>) -> ReaderErrorAction {
    match raw_os_error {
        Some(code) if code == libc::EINTR || code == libc::EAGAIN => ReaderErrorAction::Retry,
        _ => ReaderErrorAction::Stop,
    }
}

/// 选择收尾信号的目标：`kill(-pgid)` 覆盖整个前台进程组，使 shell 的子进程一并退出。
/// 组号下界 MUST 大于 1——`-1` 会把信号发给本进程有权发信号的所有进程；组号也 MUST NOT
/// 是本进程自身的组——子进程 fork 后尚未 `setsid` 时与本进程同组，组杀会连带杀死自己
/// （设备实证 SIGKILL 自杀）。本进程组号取不到时一律退化为直杀子进程：确认不了就不组杀。
/// 纯函数，使这条 guard 无需真实子进程即可测试。
fn exit_signal_target(
    pid: nix::unistd::Pid,
    child_pgid: Option<nix::unistd::Pid>,
    own_pgid: Option<nix::unistd::Pid>,
) -> nix::unistd::Pid {
    let Some(own_pgid) = own_pgid else {
        return pid;
    };
    child_pgid
        .filter(|pgid| pgid.as_raw() > 1 && *pgid != own_pgid)
        .map_or(pid, |pgid| nix::unistd::Pid::from_raw(-pgid.as_raw()))
}

/// 读取线程在 `poll` 中停驻多久后重新检查退出标志。改用 poll 后输出延迟依旧很低，
/// 而 PTY 空闲时不再空转 CPU。
const READ_POLL_TIMEOUT_MS: i32 = 100;

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

/// PTY 主端：可克隆的写入面，与 [Session] 解耦。
///
/// 为什么必须能脱离会话锁使用：`Pty::write_all` 在子进程不读 stdin 时最多等
/// `pty::WRITE_DRAIN_TIMEOUT`（5s），且每有部分写入就重置期限，故持锁时间**无上界**；
/// 而会话锁每帧都被渲染与事件收割取得——持锁写入会把整台终端（渲染、事件、输入）
/// 冻住同样长的时间。写入只需主端 fd，故句柄单独克隆、锁外完成。
///
/// 两把锁的分工是刻意的：
/// - **读侧无锁**：`resize` / `set_pixel_size` / `child_pid` 只是 ioctl 或取字段，
///   直接经 `Arc<dyn Pty>` 完成。它们一旦排在无上界的写入后面，UI 线程上的
///   旋转/输入法变化就会跟着一起卡住。
/// - **仅写入之间互斥**：同一 PTY 的字节序要求写入彼此串行，故另设一把只锁写入
///   的锁——它挡不住别的线程，但也不会被只读路径拖住。
#[derive(Clone)]
pub struct PtyMaster {
    /// 只读路径直接经它拿 `&dyn Pty`，不取任何锁（`Pty` 为 `Send + Sync`）。
    pty: Arc<dyn Pty>,
    /// 只串行化写入之间的字节序，与读侧无关。
    write_order: Arc<Mutex<()>>,
    exited: Arc<AtomicBool>,
    /// 子进程 pid，构造期取一次。
    ///
    /// `request_exit` 必须在**不取互斥量**的前提下读到它：那条路经 `destroySession`
    /// 跑在 UI 线程上，而互斥量此刻可能被 `PtyMaster::write` 持住——后者要等子进程
    /// 消费，且每有部分写入就重置期限，故持锁时间**无上界**。子进程退出后该路若被
    /// 堵住，`Session::drop` 的 `join_with_timeout` 随之超时并分离等待线程。
    ///
    /// 进程组在 spawn 之后不再改变，故这个值与经 `Pty::child_pid` 现取完全等价。
    child_pid: nix::unistd::Pid,
}

/// 待上报 OSC 52 剪贴板写入的上限。渲染暂停期间（设置页/输入法弹出）`poll_clipboard`
/// 停调，而程序可继续发 OSC 52；超出即丢最旧，使内存不随「上游发得多快」增长。
const MAX_CLIPBOARD_TEXT_QUEUE: usize = 8;

/// 单帧最多回写多少条 VT 应答。
///
/// 上界而非下界：正常终端一帧内至多一两条（DSR 一次、DA 一次），而病态输出
/// （`printf '\033[6n'` 循环）一帧能产生上万条。写完即止，剩余的留在
/// [`Session::pty_write_back_queue`] 下帧续写。它与
/// `WRITE_BACK_TIMEOUT` 的墙钟闸各管一件事：条数封顶内存与单帧工作量，
/// 墙钟封顶**持会话锁**的时间——主端满时每条都会等满排空期限，只靠条数封顶
/// 仍能按住渲染与看门狗近一秒。
const MAX_PTY_WRITES_PER_FRAME: usize = 16;

/// 待回写 VT 应答的队列上限。
///
/// 每帧只写走 [`MAX_PTY_WRITES_PER_FRAME`] 条，生产速率远高于它时（子进程不读
/// stdin，病态程序又持续查询）队列净增长，故显式封顶丢最旧——应答是请求-响应
/// 语义，不被消费的应答不会被回头读，无限保留只是白占内存。取值给足突发余量：
/// 一整帧的输出块（10 × 8KiB）能产生的 DSR 应答不足其四分之一。
const MAX_PENDING_PTY_WRITES: usize = 256;

impl PtyMaster {
    fn new(pty: Box<dyn Pty>, exited: Arc<AtomicBool>) -> Self {
        let child_pid = pty.child_pid();
        Self {
            pty: Arc::from(pty),
            write_order: Arc::new(Mutex::new(())),
            exited,
            child_pid,
        }
    }

    /// 子进程 pid：不取互斥量，故可在写入进行中安全调用。
    pub fn child_pid(&self) -> nix::unistd::Pid {
        self.child_pid
    }

    /// 更新 PTY winsize。**不取锁**：只是 ioctl，与无上界的写入无关。
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), crate::terminal::pty::PtyError> {
        self.pty.resize(rows, cols)
    }

    /// 更新 TIOCSWINSZ 的像素字段。**不取锁**，同上。
    pub fn set_pixel_size(
        &self,
        width: u16,
        height: u16,
    ) -> Result<(), crate::terminal::pty::PtyError> {
        self.pty.set_pixel_size(width, height)
    }

    /// 以给定排空期限写入。**取写入锁**以保字节序——VT 应答与用户输入共用主端。
    pub fn write_with_timeout(
        &self,
        data: &[u8],
        drain_timeout: std::time::Duration,
    ) -> std::io::Result<()> {
        let _order = self.pty_write_order();
        self.pty.write_all(data, drain_timeout, true).1
    }

    /// [`Self::write_with_timeout`] 的带回写条数版：返回**已写出的字节数**（含出错
    /// 时的部分写），调用方据此只把未写出的部分退回队列。VT 应答续写用；用户输入走
    /// 上面那条——它不容忍重发，多写一次就是多打一行命令。
    ///
    /// 排空期限同时是**写入锁**的取锁期限：逐帧续写传进来的是整帧剩余墙钟预算，
    /// 而用户输入传满额。上一条路径的写入锁在别处被等满 `WRITE_DRAIN_TIMEOUT` 时，
    /// 不设限地等锁会让「整段持会话锁以墙钟为界」变成「以条数 × 排空期限为界」。
    ///
    /// `None` 表示期限内没取到写入锁，调用方须把整条退回队列下帧续写。
    pub fn write_with_timeout_counted(
        &self,
        data: &[u8],
        drain_timeout: std::time::Duration,
    ) -> Option<(usize, std::io::Result<()>)> {
        let _order = self.write_order.try_lock_for(drain_timeout)?;
        Some(self.pty.write_all(data, drain_timeout, false))
    }

    fn pty_write_order(&self) -> parking_lot::MutexGuard<'_, ()> {
        self.write_order.lock()
    }

    /// 写入子进程 stdin。会话已退出即拒绝（`SessionError::Closed`）——
    /// 退出后写入只会得到 EIO，读起来像「写失败」而非「会话已关」。
    pub fn write(&self, data: &[u8]) -> Result<(), SessionError> {
        if self.exited.load(Ordering::Acquire) {
            return Err(SessionError::Closed);
        }
        self.write_with_timeout(data, crate::terminal::pty::WRITE_DRAIN_TIMEOUT)
            .map_err(SessionError::Io)
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
        Self::spawn_with_pty(pty, rows, cols, None, Some(std::env::temp_dir()))
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

    /// 以默认主题创建会话。`cwd` 可选，覆盖子进程初始工作目录；
    /// `None` 则 chdir 到 `env.working_directory`。cwd 不存在或非目录时由子进程记 stderr
    /// 但不使启动失败（与 shell 行为一致）。
    pub fn spawn(
        shell: &str,
        rows: u32,
        cols: u32,
        env: &ShellEnv,
        cwd: Option<&Path>,
        output_wake: Option<OutputWake>,
    ) -> Result<Self, SessionError> {
        log::info!("Session::spawn: shell='{shell}', rows={rows}, cols={cols}, cwd={cwd:?}");
        // 预先拒绝非法尺寸（与 `resize` 同一判据）：0 行/0 列会让 Ghostty 网格与 PTY
        // winsize 同时变成无意义值，下方的 `as u16` 还会静默截断越界尺寸，
        // 导致缓存的 grid_size 与 PTY 不一致。
        if rows == 0 || cols == 0 || u16::try_from(rows).is_err() || u16::try_from(cols).is_err() {
            return Err(SessionError::InvalidDimensions);
        }
        let mut pty = match PtyPair::spawn(shell, rows as u16, cols as u16, env, cwd) {
            Ok(pty_pair) => {
                log::info!("Session::spawn: PtyPair::spawn OK");
                pty_pair
            }
            Err(spawn_error) => {
                log::info!("Session::spawn: PtyPair::spawn error: {spawn_error}");
                return Err(spawn_error.into());
            }
        };
        match pty.set_nonblocking() {
            Ok(()) => log::info!("Session::spawn: set_nonblocking OK"),
            Err(nonblocking_error) => {
                log::info!("Session::spawn: set_nonblocking error: {nonblocking_error}");
                return Err(nonblocking_error.into());
            }
        }

        log::info!("Session::spawn: cloning master fd for reader");
        // 安全：dup 发生在 `try_clone_reader_fd` 内部（位于允许 `unsafe` 的 pty.rs）。
        // 返回的是自有安全句柄，经 `std::fs::File` 读取，此处无需 `unsafe` 块。
        let reader_fd = pty.try_clone_reader_fd().map_err(SessionError::Io)?;
        let mut read_file = File::from(reader_fd);

        let child_pid = pty.child_pid();
        // execve 失败回报管道的读端：等待线程在子进程退出后据此判定 execve
        // 是否真失败——退出码不能承担这件事（shell 被 SIGHUP 杀掉时退出码 129
        // 与「errno=29」的编码撞车）。
        let exec_failure_reader = pty.take_exec_failure_reader();

        let mut session = match Self::spawn_with_pty(
            Box::new(pty) as Box<dyn Pty>,
            rows,
            cols,
            output_wake.clone(),
            env.tmp_dir().or_else(|| Some(std::env::temp_dir())),
        ) {
            Ok(session) => session,
            Err(inner_error) => {
                // `read_file` 在此被丢弃，其 fd 随之安全关闭。
                return Err(inner_error);
            }
        };

        let exited = session.exited.clone();
        let output_tx = session.output_tx.clone();
        let output_wake = output_wake;

        log::info!("Session::spawn: spawning reader thread");
        let exited_read = exited.clone();
        let reader_handle = std::thread::spawn(move || {
            let mut read_buf = [0u8; READ_BUF_SIZE];
            let poll_fd = read_file.as_raw_fd();
            loop {
                if exited_read.load(Ordering::Acquire) {
                    log::info!("reader thread: exiting due to exited flag");
                    break;
                }
                let mut poll_fd = libc::pollfd {
                    fd: poll_fd,
                    events: libc::POLLIN,
                    revents: 0,
                };
                // SAFETY: `poll` 是 POSIX 系统调用；`poll_fd` 是有效且已初始化的
                // `pollfd`，其 `fd` 是 `read_file` 持有的存活读取 fd。`poll` 只读这些
                // 输入并回写 `revents`。这是读取线程中仅剩的 `unsafe`，且未绕过
                // `Pty` 抽象（fd 由 `try_clone_reader_fd` 取得）。
                let poll_result = unsafe {
                    libc::poll(&mut poll_fd as *mut libc::pollfd, 1, READ_POLL_TIMEOUT_MS)
                };
                match poll_result.cmp(&0) {
                    std::cmp::Ordering::Greater => {}
                    std::cmp::Ordering::Equal => continue,
                    std::cmp::Ordering::Less => {
                        if std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
                            continue;
                        }
                        log::info!("reader thread: poll error: {poll_result}");
                        exited_read.store(true, Ordering::Release);
                        break;
                    }
                }
                match read_file.read(&mut read_buf) {
                    Ok(0) => {
                        log::info!("reader thread: EOF from PTY");
                        exited_read.store(true, Ordering::Release);
                        break;
                    }
                    Ok(bytes_read) => {
                        // 原样投递：字节级清洗的唯一归属是 `pty_write`
                        // （见 public_api 的说明），读取线程不再重复剥离。
                        let data = read_buf[..bytes_read].to_vec();
                        if output_tx.send(data).is_err() {
                            log::info!("reader thread: output channel closed");
                            break;
                        }
                        // 数据已入队再唤醒：等待方被叫醒后立刻能读到这块输出，
                        // 不会出现「先醒再空转一拍」。
                        if let Some(wake) = output_wake.as_ref() {
                            wake();
                        }
                    }
                    Err(read_error) => match read_error_action(read_error.raw_os_error()) {
                        ReaderErrorAction::Retry => {}
                        ReaderErrorAction::Stop => {
                            if read_error.raw_os_error() == Some(libc::EIO) {
                                log::info!("reader thread: PTY EOF (slave closed, EIO)");
                            } else {
                                log::info!("reader thread: read error: {read_error}");
                            }
                            exited_read.store(true, Ordering::Release);
                            break;
                        }
                    },
                }
            }
            // `read_file`（及其 fd）在此被丢弃，随之安全关闭。
        });

        let exit_code = session.exit_code.clone();
        let exited_wait = exited.clone();
        let wait_handle = std::thread::spawn(move || {
            log::info!("wait thread: waiting for child pid={child_pid}");
            let result = nix::sys::wait::waitpid(child_pid, None);
            // execve 真失败只有子进程自己知道：它把 errno 写进了回报管道。
            // 退出码不作数——正常收尾（组杀 SIGHUP）时 shell 退出码是 129，
            // 与「errno=29」的旧编码撞车，会把正常路径误报成 execve 失败。
            if let Some(reader) = exec_failure_reader
                && let Some(exec_errno) = crate::terminal::pty::read_exec_failure_errno(&reader)
            {
                log::error!("wait thread: execve failed: errno={exec_errno}");
            }
            log::info!("wait thread: child exited: {result:?}");
            match result {
                Ok(nix::sys::wait::WaitStatus::Exited(_, code)) => {
                    *exit_code.lock() = ExitCodeSlot::Code(code);
                }
                // Shell 被信号杀死（Ctrl+\、kill -9）：上报惯例的 128 + 信号码，
                // 避免 UI 把信号死亡当成干净的退出码 0。
                Ok(nix::sys::wait::WaitStatus::Signaled(_, signal, _)) => {
                    *exit_code.lock() = ExitCodeSlot::Code(SIGNAL_EXIT_BASE + signal as i32);
                }
                Ok(other) => {
                    // 未传 WUNTRACED/WCONTINUED，正常只可能是 Exited 或 Signaled；
                    // 真到这里说明等待语义变了，不能让退出码永远缺席（那会让退出事件
                    // 永远不上报、会话表泄漏），故显式记为「未知」。
                    log::error!("wait thread: unexpected wait status {other:?}; exit code unknown");
                    *exit_code.lock() = ExitCodeSlot::Unknown;
                }
                Err(error) => {
                    // 同上：`waitpid` 失败时真实退出码无从取得。记为错误并置「未知」，
                    // 由上报侧（`take_reported_exit_code`）把它当作「已退出但码未知」。
                    log::error!("wait thread: waitpid failed: {error}");
                    *exit_code.lock() = ExitCodeSlot::Unknown;
                }
            }
            exited_wait.store(true, Ordering::Release);
        });

        session.reader_handle = Some(reader_handle);
        session.wait_handle = Some(wait_handle);

        Ok(session)
    }

    fn spawn_with_pty(
        pty: Box<dyn Pty>,
        rows: u32,
        cols: u32,
        frame_wake: Option<OutputWake>,
        kitty_temp_dir: Option<std::path::PathBuf>,
    ) -> Result<Self, SessionError> {
        log::info!("Session::spawn_with_pty: creating Arc/Channel");
        let exited = Arc::new(AtomicBool::new(false));
        let exit_reported = Arc::new(AtomicBool::new(false));
        let clipboard_text = Arc::new(Mutex::new(VecDeque::new()));
        let clipboard_read = Arc::new(Mutex::new(VecDeque::new()));
        let (output_tx, output_rx) = bounded::<Vec<u8>>(OUTPUT_CHANNEL_BOUND);
        // 管道分支：原始 PTY 输出的次要消费者（日志、追踪）。

        // 同一份唤醒既用于 PTY 到达，也用于帧入队：渲染线程在「有字节但还没帧」
        // 的那一拍醒来后可以立刻再睡，等帧真正入队时被第二次叫醒。
        let terminal = GhosttyTerminal::new_with_frame_wake_and_temp_dir(
            rows,
            cols,
            DEFAULT_SCROLLBACK_LINES,
            crate::terminal::theme::default_dark(),
            frame_wake,
            kitty_temp_dir,
        )
        .map_err(SessionError::Terminal)?;

        Ok(Self {
            pty: PtyMaster::new(pty, exited.clone()),
            terminal,
            output_processor: OutputProcessor::new(),
            output_tx,
            output_rx,
            exited,
            exit_reported,
            clipboard_text,
            clipboard_read,
            pty_write_back_queue: VecDeque::new(),
            reader_handle: None,
            wait_handle: None,
            exit_code: Arc::new(Mutex::new(ExitCodeSlot::Pending)),
            terminal_rows: AtomicU32::new(rows),
            terminal_cols: AtomicU32::new(cols),
            grid_dirty: AtomicBool::new(false),
            last_scroll_offset: AtomicI64::new(0),
        })
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

    /// 每会话帧处理的最大 VT 输出块数，用于 PTY 输出洪水时限制渲染线程延迟。
    const MAX_CHUNKS_PER_FRAME: u32 = 10;

    /// [`Self::focus_event`] 中 DECSET 1004 模式查询的超时。调用方持会话锁，
    /// 而该锁每帧都被渲染与事件收割取得，故 VT 线程卡住时必须快速失败。
    /// PTY 写入在同一把锁内，其排空上限是 `pty::WRITE_DRAIN_TIMEOUT`。
    const FOCUS_MODE_QUERY_TIMEOUT_MS: u64 = 4 * crate::terminal::ghostty_terminal::VT_IDLE_TICK_MS;

    /// 处理来自 PTY 读取线程的终端输出：读取 VT 输出、更新终端状态并排空回写应答。
    /// 处理过任何 VT 数据时返回 true。
    pub(crate) fn poll_pty_output(&mut self, max_chunks: u32) -> bool {
        let mut count = 0u32;
        while let Ok(data) = self.output_rx.try_recv() {
            let snap = self.output_processor.process(&data);

            for selection in snap.clipboard_reads {
                self.clipboard_read.lock().push_back(selection);
            }
            self.terminal.pty_write(&snap.filtered);
            count += 1;
            // 限制每帧处理量，避免单次渲染调用长时间持有会话锁。剩余块在下一渲染帧处理，
            // 无正确性代价——VT 线程按 FIFO 处理命令。
            if count >= max_chunks {
                log::trace!(
                    "poll_pty_output: hit cap of {} chunks, {} remain",
                    max_chunks,
                    self.output_rx.len(),
                );
                // 即使走到封顶路径也排空回写应答：输出洪水不得饿死 DECRPM/DSR/DA 应答，
                // 否则子应用会无限期等待。
                self.harvest_vt_side_effects();
                return true;
            }
        }
        if count > 0 {
            log::trace!("poll_pty_output: processed {count} chunks");
            self.harvest_vt_side_effects();
            true
        } else {
            // 本帧没有新输出，但 VT 线程可能仍在处理此前投递的命令：其回调事件
            // （OSC 52 写入）与回写应答此刻可能刚好就绪。仍然收割一次，否则
            // 这些副作用要一直等到下一批输出到来才上报——终端安静时（恰恰是用户
            // 复制完内容的那一刻）就永远不发生。
            self.harvest_vt_side_effects();
            false
        }
    }

    /// 收割 VT 侧的副作用：回调事件（OSC 52 写入）与 PTY 回写应答（DECRPM/DSR/DA）。
    ///
    /// 两者都是非阻塞取（`try_recv` 与加锁后 `take`），故**不**以 VT 线程的 flush ack
    /// 为条件。原实现把收割挂在 `flush_with_timeout(Duration::ZERO)` 的成功分支上，
    /// 而零期限的 `recv_timeout` 要在 ack 已被 VT 线程送出的一瞬才可能命中，实测 32 次
    /// 连续尝试 0 次成功（`zero_timeout_flush_observes_the_ack`）——于是这条分支实际是
    /// 死代码：OSC 52 写入永远不收割（用户复制内容后剪贴板不更新），
    /// DECRPM/DSR/DA 应答也永不写回 PTY（子应用无限期等待设备状态查询）。
    ///
    /// 丢弃 flush 的代价是无 Synchronization：VT 线程尚未处理完时取到空，下次取到。
    /// 收割是每帧重复的拉取式操作，事件不会因此丢失，只是可能晚一帧。
    fn harvest_vt_side_effects(&mut self) {
        self.drain_callback_events();
        self.drain_pty_write_back();
    }

    /// 把待处理的 VT 应答（DECRPM、DSR、DA 等）回写到子 PTY。VT 引擎负责缓冲，子进程在等它们。
    ///
    /// 每帧至多写 [`MAX_PTY_WRITES_PER_FRAME`] 条，写不完的（含失败的那条）留在
    /// [`Self::pty_write_back_queue`] 下帧续写。**首次写失败即停**：一次失败已说明
    /// 子进程此刻不读 stdin，继续按同一条件逐条重试只会把本帧的持锁时间乘以条数。
    /// 队列封顶丢最旧——应答是请求-响应语义，子进程不消费就永远不会回头读，
    /// 无限保留只是白占内存；丢弃时记错误，不静默。
    fn drain_pty_write_back(&mut self) {
        // 新到的应答排在既有队列之后：先前的应答先出去，顺序与引擎产生顺序一致。
        self.pty_write_back_queue
            .extend(self.terminal.drain_pty_write_responses());
        let overflow = self
            .pty_write_back_queue
            .len()
            .saturating_sub(MAX_PENDING_PTY_WRITES);
        if overflow > 0 {
            self.pty_write_back_queue.drain(..overflow);
            log::error!(
                "session: dropped {overflow} of {} VT responses, child is not reading stdin",
                self.pty_write_back_queue.len() + overflow
            );
        }
        // 整段持会话锁的时间以**墙钟**为界而非以条数计：每条各自等满
        // `WRITE_BACK_TIMEOUT` 的话，`条数 × 期限` 在主端满时仍能按住渲染与看门狗
        // 近一秒。逐帧收割且全程持会话锁，故到期即止，余下留队下帧。
        let deadline = std::time::Instant::now() + crate::terminal::pty::WRITE_BACK_TIMEOUT;
        for _ in 0..MAX_PTY_WRITES_PER_FRAME {
            // 把**剩余**预算交给这一条，而不是整份期限：否则每条都能再等满一个
            // WRITE_BACK_TIMEOUT，最坏持锁时间变成两倍，与「以墙钟为界」的说法不符。
            let Some(budget) = deadline.checked_duration_since(std::time::Instant::now()) else {
                return;
            };
            let Some(response) = self.pty_write_back_queue.pop_front() else {
                return;
            };
            log::trace!("poll_pty_output: pty write-back {} bytes", response.len());
            let Some((written, result)) = self.pty.write_with_timeout_counted(&response, budget)
            else {
                // 写入锁被等满剩余预算（用户输入正占着主端）：本条退回队首下帧续写，
                // 与墙钟闸同一语义——到期即止，不拖住会话锁。
                self.pty_write_back_queue.push_front(response);
                return;
            };
            if let Err(error) = result {
                log::error!(
                    "session: PTY write-back failed ({} bytes, {} written): {}",
                    response.len(),
                    written,
                    error
                );
                // 只退回**未写出**的部分：整条退回会让下一帧把已发出的前缀再发一遍，
                // 子应用于是收到两条 DECRPM/DSR 应答；而全部丢掉又让子应用永远等不到
                // 一个完整回答。
                if written < response.len() {
                    self.pty_write_back_queue
                        .push_front(response[written..].to_vec());
                }
                return;
            }
        }
    }

    /// 处理本帧所有可用输出与用户输入；处理过任何 VT 输出时返回 `true`
    /// （调用方应重建显示快照）。
    pub fn process_output(&mut self) -> bool {
        self.poll_pty_output(Self::MAX_CHUNKS_PER_FRAME)
    }

    /// 会话注册表持有的 `new_output` 共享标志：渲染线程直接读它，
    /// 不抢会话锁（PTY 洪流下锁可能被 VT 解析按住数十毫秒）。
    pub fn new_output_flag(&self) -> Arc<AtomicBool> {
        self.output_processor.new_output_flag()
    }

    /// 备用屏激活的无锁镜像：与 `new_output` 同批上报给宿主，
    /// 同样不经过会话锁。
    pub fn alt_screen_active_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.terminal.alt_screen_active)
    }

    /// 收割 VT 线程经上游回调上报的事件（剪贴板写入）到队列/标志位。
    /// 拉取式：每帧重复调用，VT 线程尚未处理完时取到空、下帧再取（见
    /// `harvest_vt_side_effects` 的说明——本路径不 flush）。
    fn drain_callback_events(&self) {
        let mut dropped = 0usize;
        while let Some((_, text)) = self.terminal.poll_clipboard_event() {
            // FIFO 而非单槽：渲染暂停（pollEvent 停调）期间的连续写入必须全部保留，
            // 单槽会让后一次覆盖前一次，用户丢剪贴板内容。
            let mut queue = self.clipboard_text.lock();
            queue.push_back(text);
            // 丢最旧而非拒收最新：队首才是用户已等待最久、即将被上报的那条。
            // 与 clipboard_read 的洪泛削减同口径（见 ffi 的
            // MAX_UNANSWERED_CLIPBOARD_READS_PER_SESSION），只是那边答空串、这边丢弃。
            if queue.len() > MAX_CLIPBOARD_TEXT_QUEUE {
                queue.pop_front();
                dropped += 1;
            }
        }
        // 逐条记没有任何诊断价值（每条都只是同一个「队列满」），却能让病态程序
        // 每秒刷出数十条 logcat——与渲染洪泛、事件队列溢出同款限频。
        if dropped > 0 {
            log::warn!(
                "clipboard queue overflow (>{} pending); dropped {dropped} oldest writes",
                MAX_CLIPBOARD_TEXT_QUEUE,
            );
        }
    }

    /// 轮询 OSC 52 转义序列写入的剪贴板文本：每次取走最早的一条。
    pub fn poll_clipboard(&self) -> Option<String> {
        self.clipboard_text.lock().pop_front()
    }

    /// 取走全部待处理的 OSC 52 剪贴板读取请求（selection 名，按序）。
    /// JNI 层将其转发给宿主应用，并经 [`Session::answer_clipboard_read`] 回传结果。
    pub fn take_clipboard_reads(&self) -> Vec<String> {
        let mut guard = self.clipboard_read.lock();
        guard.drain(..).collect()
    }

    /// 向 PTY 写入 `ESC ] 52 ; <selection> ; <base64> ESC \\` 应答待处理的 OSC 52
    /// 剪贴板读取请求。selection 为空串（惯例默认 `c`）时原样应答，含义由应用自行解释。
    pub fn answer_clipboard_read(
        &mut self,
        selection: &str,
        text: &str,
    ) -> Result<(), SessionError> {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
        let mut response = Vec::with_capacity(selection.len() + encoded.len() + 8);
        response.extend_from_slice(b"\x1b]52;");
        response.extend_from_slice(selection.as_bytes());
        response.push(b';');
        response.extend_from_slice(encoded.as_bytes());
        response.push(0x07); // BEL 终止符（兼容 xterm）
        if self.is_exited() {
            return Err(SessionError::Closed);
        }
        self.pty
            .write_with_timeout(&response, crate::terminal::pty::WRITE_BACK_TIMEOUT)
            .map_err(SessionError::Io)?;
        Ok(())
    }

    pub fn is_exited(&self) -> bool {
        self.exited.load(Ordering::Acquire)
    }

    pub fn exited_flag(&self) -> Arc<AtomicBool> {
        self.exited.clone()
    }

    /// 原子地标记退出事件已上报给 Kotlin 侧，仅首个调用者返回 true——pollEvent 的
    /// 逐帧后台扫描据此保证每次退出只上报一次。
    pub fn mark_exit_reported(&self) -> bool {
        !self.exit_reported.swap(true, Ordering::AcqRel)
    }

    /// 取出「已可上报的退出」：`None` 表示子进程还没退出到能上报的程度，调用方
    /// 每帧再问一次即可（不阻塞、不睡眠）。
    ///
    /// 上报条件与 `mark_exit_reported` 合成一个动作，二者因此不可能不一致。原先 FFI
    /// 层先 `mark_exit_reported()` 再去忙等退出码，等不到就上报一个伪造的 0；而标志
    /// 已锁存、退出事件又不会重发——被 OOM kill（137）之类的会话会永远显示成正常退出 0。
    /// 现在「码到手」才上报：拿到的一定是真码，且退出事件仍恰好发一次。
    pub fn take_reported_exit(&self) -> Option<ReportedExit> {
        if !self.is_exited() {
            return None;
        }
        let reported = {
            let guard = self.exit_code.lock();
            match *guard {
                ExitCodeSlot::Pending => return None,
                ExitCodeSlot::Code(code) => ReportedExit::Code(code),
                ExitCodeSlot::Unknown => ReportedExit::Unknown,
            }
        };
        if self.mark_exit_reported() {
            Some(reported)
        } else {
            None
        }
    }

    pub fn terminal(&self) -> &GhosttyTerminal {
        &self.terminal
    }

    pub fn terminal_mut(&mut self) -> &mut GhosttyTerminal {
        &mut self.terminal
    }

    /// DECSET 1004 焦点上报：返回待直写子 PTY 的序列，`None` 表示子进程未启用 1004。
    ///
    /// 序列必须**直接**写入子 PTY 而非进入 VT 引擎——引擎的输出流解析器把 `CSI I` 当作
    /// CHT（光标水平制表）、`CSI O` 当作非法 CSI，送入引擎只会把光标移到下一个制表位而非
    /// 通知应用。仅当子进程确实启用了 1004 时才发送（xterm 语义）。查询窗口须短于用户输入
    /// 的排空上限：调用方持会话锁，VT 线程卡住时不得拖满通用查询超时。
    ///
    /// 写入刻意留给调用方在**释放会话锁之后**做：主端满时排空上限是 5s，而会话锁每帧被
    /// 渲染与事件收割取得（理由同 [`PtyMaster`]）。
    pub fn focus_event_bytes(&mut self, focused: bool) -> Option<&'static [u8]> {
        if !self.terminal.mode_get_with_timeout(
            libghostty_vt::terminal::Mode::FOCUS_EVENT,
            std::time::Duration::from_millis(Self::FOCUS_MODE_QUERY_TIMEOUT_MS),
        ) {
            return None;
        }
        Some(if focused { b"\x1b[I" } else { b"\x1b[O" })
    }
}

/// 带截止超时地 join 线程句柄，最多重试 3 次。
///
/// 初次超时后以 100ms 截止再尝试 3 次，以应对线程阻塞在需要多个信号才能唤醒的 I/O 上。
/// 全部失败则分离（丢弃句柄）并记错误——该线程的资源（fd、内存）会泄漏。
pub(crate) fn join_with_timeout(
    handle: &mut Option<std::thread::JoinHandle<()>>,
    timeout: Duration,
) {
    let Some(handle) = handle.take() else {
        return;
    };
    if wait_finished(&handle, timeout) {
        join_finished(handle);
        return;
    }
    log::warn!("session: thread did not exit within {timeout:?}, retrying up to 3×");
    for _attempt in 0..3 {
        if wait_finished(&handle, Duration::from_millis(100)) {
            join_finished(handle);
            return;
        }
    }
    log::error!("session: thread failed to exit after retries — DETACHING (resource leak)");
    // 句柄在此被丢弃 → 分离
}

/// 在截止前轮询句柄完成，完成返回 true（不消费句柄，join 留给调用方）。
fn wait_finished(handle: &std::thread::JoinHandle<()>, timeout: Duration) -> bool {
    const POLL_STEP: Duration = Duration::from_millis(10);
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if handle.is_finished() {
            return true;
        }
        std::thread::sleep(POLL_STEP);
    }
    false
}

/// join 已完成的句柄并报告 panic。
fn join_finished(handle: std::thread::JoinHandle<()>) {
    if let Err(error) = handle.join() {
        log::error!("session: thread panicked: {:?}", error);
    }
}

impl Session {
    /// 立即让子进程所在进程组终止：置 exited、解除读线程的输出通道阻塞、
    /// 投递 SIGHUP/SIGCONT/SIGKILL（带宽限）。幂等 —— `Drop` 会重复调用本体，
    /// 信号重复与通道重换均无害。不 join 线程：join 由 `Drop` 统一负责，
    /// 使「立即杀掉 shell」可从任何上下文（含 `destroySession` 与已 detach 的
    /// Arc 克隆）幂等发起。
    pub fn request_exit(&mut self) {
        // 等待线程已回收子进程时不再发信号：PID 可能已被复用，直杀会误伤无关进程。
        let already_exited = self.exited.swap(true, Ordering::AcqRel);
        // 先断开输出通道：flume 旧接收端一析构，阻塞的 `output_tx.send` 即返回
        // Err 走到「output channel closed」退出分支，等待线程也随 exited 标志退出。
        // 换入的是空通道，故 `poll_pty_output` 等后续调用仍安全。自然退出后
        // 也要断开：读取线程可能正卡在满通道的 send 上。
        self.output_rx = flume::bounded::<Vec<u8>>(OUTPUT_CHANNEL_BOUND).1;
        let pid = self.pty.child_pid();
        if already_exited || pid.as_raw() <= 0 {
            return;
        }
        // 优先组杀：`kill(-pgid)` 把信号发给整个前台进程组，使 shell 的子进程
        // （管道、前台组内的后台作业）一并退出。组不可用时退化为直杀子进程，
        // 两者只差信号目标，故把目标选好后走同一条「HUP → CONT → 让步 → KILL」。
        let child_pgid = nix::unistd::getpgid(Some(pid));
        if matches!(child_pgid, Err(nix::errno::Errno::ESRCH)) {
            log::debug!("session exit: child {} already exited", pid.as_raw());
            return;
        }
        let own_pgid = nix::unistd::getpgid(None).ok();
        let group = exit_signal_target(pid, child_pgid.as_ref().ok().copied(), own_pgid);
        match (child_pgid, own_pgid) {
            (Ok(child), Some(own)) if child == own => {
                log::debug!(
                    "session exit: child {} shares our process group",
                    pid.as_raw()
                );
            }
            (Ok(_), Some(_)) => {
                log::debug!("session exit: signalling process group {}", -group.as_raw());
            }
            (Err(error), _) => {
                log::debug!(
                    "session exit: getpgid({}) failed: {error}; killing child directly",
                    pid.as_raw()
                );
            }
            (Ok(_), None) => {
                log::debug!(
                    "session exit: own process group unavailable; killing child {} directly",
                    pid.as_raw()
                );
            }
        }
        Self::terminate_target(pid, group);
    }

    /// 按 `SIGHUP → SIGCONT → 让步 → SIGKILL` 收掉目标：先给前台组一次体面退出，
    /// 让它有机会刷盘与复原终端；宽限期后仍存活即强杀。组信号失败时逐级退到子进程。
    fn terminate_target(pid: nix::unistd::Pid, group: nix::unistd::Pid) {
        // 组不可用时 `group` 就是 `pid`，标签据实切换，否则日志会把直杀子进程写成
        // 「打给了进程组」，排障时会被直接带偏。
        let (first, label) = if group == pid {
            (pid, "child")
        } else {
            (group, "pgid")
        };
        // 退路只在目标确实不同时才发：退化路径下 `first` 就是 `pid`，重发等于把
        // 同一个信号再打一遍（失败时日志会出现两条一模一样的记录）。
        let fallback = |signal: nix::sys::signal::Signal, what: &str| {
            if first != pid {
                deliver_signal(pid, signal, what);
            }
        };
        if !deliver_signal(
            first,
            nix::sys::signal::Signal::SIGHUP,
            &format!("SIGHUP to {label}"),
        ) {
            fallback(nix::sys::signal::Signal::SIGHUP, "SIGHUP to child");
        }
        deliver_signal(
            first,
            nix::sys::signal::Signal::SIGCONT,
            &format!("SIGCONT to {label}"),
        );
        std::thread::sleep(TRAILING_EXIT_GRACE);
        if !deliver_signal(
            first,
            nix::sys::signal::Signal::SIGKILL,
            &format!("SIGKILL to {label}"),
        ) {
            fallback(nix::sys::signal::Signal::SIGKILL, "SIGKILL to child");
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.request_exit();
        join_with_timeout(&mut self.reader_handle, TRAILING_EXIT_GRACE);
        join_with_timeout(&mut self.wait_handle, TRAILING_EXIT_GRACE);
    }
}

/// 向进程或进程组发信号并记录失败原因（`Drop` 无法返回错误）。
///
/// `ESRCH` 表示目标已退出，是拆卸过程的正常结局，不记警告；其余 errno 必须留痕，
/// 否则信号丢失会表现为“会话关闭后 shell 仍在后台跑”。返回信号是否送达，供组杀
/// 失败时退化为直杀子进程。
fn deliver_signal(
    target: nix::unistd::Pid,
    signal: nix::sys::signal::Signal,
    description: &str,
) -> bool {
    match nix::sys::signal::kill(target, signal) {
        Ok(()) => true,
        Err(error) => {
            if error != nix::errno::Errno::ESRCH {
                log::warn!("session drop: {description} {}: {error}", target.as_raw());
            }
            false
        }
    }
}

#[cfg(test)]
mod tests;
