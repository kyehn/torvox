//! 会话编排器：串接 PTY 读取、VT 解析与进程等待。
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::fs::File;
use std::io::Read;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use flume::{Receiver, bounded};
use thiserror::Error;

use crate::terminal::ghostty_terminal::GhosttyTerminal;
use crate::terminal::output_processor::OutputProcessor;
use crate::terminal::pty::{Pty, PtyError, PtyPair};
use crate::terminal::shell_env::ShellEnv;

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
    #[error("invalid dimensions (out of u16 range)")]
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
/// `pty::WRITE_DRAIN_TIMEOUT`（5s），而会话锁每帧都被渲染与事件收割取得——
/// 持锁写入会把整台终端（渲染、事件、输入）冻住同样长的时间。
/// 写入只需主端 fd，故句柄单独克隆、锁外完成；写入之间仍由本互斥量串行
/// （同一 PTY 的字节序要求）。
#[derive(Clone)]
pub struct PtyMaster {
    inner: Arc<Mutex<Box<dyn Pty>>>,
    exited: Arc<AtomicBool>,
}

impl PtyMaster {
    fn new(pty: Box<dyn Pty>, exited: Arc<AtomicBool>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(pty)),
            exited,
        }
    }

    fn with<R>(&self, action: impl FnOnce(&mut dyn Pty) -> R) -> R {
        action(&mut **self.inner.lock())
    }

    /// 写入子进程 stdin。会话已退出即拒绝（`SessionError::Closed`）——
    /// 退出后写入只会得到 EIO，读起来像「写失败」而非「会话已关」。
    pub fn write(&self, data: &[u8]) -> Result<(), SessionError> {
        if self.exited.load(Ordering::Acquire) {
            return Err(SessionError::Closed);
        }
        self.with(|pty| pty.write_all(data))
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
    /// 上游回调通道有界（`EVENT_CHANNEL_CAPACITY`）且满时记日志丢弃，
    /// 故队列深度天然受其约束。
    clipboard_text: Arc<Mutex<VecDeque<String>>>,
    /// 待处理的 OSC 52 剪贴板读取请求：所请求的 selection 名。
    /// 待上报的 OSC 52 剪贴板读取（FIFO）：`poll_pty_output` 收割，
    /// JNI 层逐帧取走全部并转发给宿主应用，经 [`Session::answer_clipboard_read`]
    /// 写回应答。队列而非单槽——同帧/块内多个读请求必须全部作答，
    /// 单槽 last-wins 会让被挤掉的请求永不作答；
    /// 上游 flood 由 JNI 侧单会话上限显式作答空串，故深度天然有界。
    clipboard_read: Arc<Mutex<VecDeque<String>>>,

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
}

pub struct ThemeConfig {
    pub background: [u8; 3],
    pub foreground: [u8; 3],
    pub ansi: [[u8; 3]; 16],
}

impl Default for ThemeConfig {
    fn default() -> Self {
        let (ansi, background, foreground) = GhosttyTerminal::catppuccin_mocha_palette();
        Self {
            background,
            foreground,
            ansi,
        }
    }
}

impl Session {
    /// 以已构造的 PTY 创建会话，不启动读取/等待线程（PTY I/O 由调用方驱动），
    /// 主要用于配合 `MockPty` 的测试。
    pub fn with_pty(pty: Box<dyn Pty>, rows: u32, cols: u32) -> Result<Self, SessionError> {
        Self::spawn_with_theme_inner(pty, rows, cols, ThemeConfig::default())
    }

    /// 以默认 Catppuccin Mocha 主题创建会话。`cwd` 可选，覆盖子进程初始工作目录；
    /// `None` 则 chdir 到 `env.working_directory`。cwd 不存在或非目录时由子进程记 stderr
    /// 但不使启动失败（与 shell 行为一致）。
    pub fn spawn(
        shell: &str,
        rows: u32,
        cols: u32,
        env: &ShellEnv,
        cwd: Option<&Path>,
    ) -> Result<Self, SessionError> {
        Self::spawn_with_theme(shell, rows, cols, env, cwd, ThemeConfig::default())
    }

    pub fn spawn_with_theme(
        shell: &str,
        rows: u32,
        cols: u32,
        env: &ShellEnv,
        cwd: Option<&Path>,
        theme: ThemeConfig,
    ) -> Result<Self, SessionError> {
        log::info!("Session::spawn: shell='{shell}', rows={rows}, cols={cols}, cwd={cwd:?}");
        // 预先拒绝越界尺寸（对齐 `resize` 的 `InvalidDimensions` 检查）：下方的
        // `as u16` 会静默截断，导致缓存的 grid_size 与 PTY 不一致。
        if !(u16::try_from(rows).is_ok() && u16::try_from(cols).is_ok()) {
            return Err(SessionError::InvalidDimensions);
        }
        let pty = match PtyPair::spawn(shell, rows as u16, cols as u16, env, cwd) {
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

        let mut session =
            match Self::spawn_with_theme_inner(Box::new(pty) as Box<dyn Pty>, rows, cols, theme) {
                Ok(session) => session,
                Err(inner_error) => {
                    // `read_file` 在此被丢弃，其 fd 随之安全关闭。
                    return Err(inner_error);
                }
            };

        let exited = session.exited.clone();
        let output_tx = session.output_tx.clone();

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
            if let Ok(nix::sys::wait::WaitStatus::Exited(_, code)) = result
                && code >= 100
            {
                // 退出码 >= 100 可能编码了 execve 的 errno + 100（本仓子进程会先向 PTY
                // 写 “execve failed: errno=”，该标记才权威）。Shell 也按惯例退出
                // 126/127（如启动器脚本内部 exec 被拒），故无 PTY 标记的 126 并非 execve 失败。
                log::error!(
                    "wait thread: child exited with code {code} (possible execve errno={}; see PTY output for the authoritative marker)",
                    code - 100
                );
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

    fn spawn_with_theme_inner(
        pty: Box<dyn Pty>,
        rows: u32,
        cols: u32,
        theme: ThemeConfig,
    ) -> Result<Self, SessionError> {
        log::info!("Session::spawn_with_theme_inner: creating Arc/Channel");
        let exited = Arc::new(AtomicBool::new(false));
        let exit_reported = Arc::new(AtomicBool::new(false));
        let clipboard_text = Arc::new(Mutex::new(VecDeque::new()));
        let clipboard_read = Arc::new(Mutex::new(VecDeque::new()));
        let (output_tx, output_rx) = bounded::<Vec<u8>>(OUTPUT_CHANNEL_BOUND);
        // 管道分支：原始 PTY 输出的次要消费者（日志、追踪）。

        let terminal = GhosttyTerminal::new_with_theme(
            rows,
            cols,
            DEFAULT_SCROLLBACK_LINES,
            theme.background,
            theme.foreground,
            theme.ansi,
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
            reader_handle: None,
            wait_handle: None,
            exit_code: Arc::new(Mutex::new(ExitCodeSlot::Pending)),
            terminal_rows: AtomicU32::new(rows),
            terminal_cols: AtomicU32::new(cols),
            grid_dirty: AtomicBool::new(false),
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
        let (Ok(rows), Ok(cols)) = (u16::try_from(rows), u16::try_from(cols)) else {
            return Err(SessionError::InvalidDimensions);
        };
        // 相同尺寸直接短路，**除非**上一次网格 resize 被丢弃——此时缓存尺寸已与网格
        // 不符，必须重发相同尺寸以修复分歧。
        let dirty = self.grid_dirty.load(Ordering::Acquire);
        if !dirty && (rows as u32, cols as u32) == self.grid_size() {
            return Ok(ResizeOutcome::Applied);
        }
        self.pty.with(|pty| pty.resize(rows, cols))?;
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
        self.pty.with(|pty| pty.set_pixel_size(width, height))?;
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
    pub fn reset_terminal(&self) {
        self.terminal.reset();
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

    /// [`Self::focus_event`] 中 DECSET 1004 模式查询的超时。`focus_event` 运行在 UI
    /// 线程（窗口焦点变化），故 VT 线程卡住时必须快速失败而非按完整查询超时拖住 UI。
    const FOCUS_MODE_QUERY_TIMEOUT_MS: u64 = 50;

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
            // （OSC 52 写入、BEL）与回写应答此刻可能刚好就绪。仍然收割一次，否则
            // 这些副作用要一直等到下一批输出到来才上报——终端安静时（恰恰是用户
            // 复制完内容的那一刻）就永远不发生。
            self.harvest_vt_side_effects();
            false
        }
    }

    /// 收割 VT 侧的副作用：回调事件（OSC 52 写入、BEL）与 PTY 回写应答（DECRPM/DSR/DA）。
    ///
    /// 两者都是非阻塞取（`try_recv` 与加锁后 `take`），故**不**以 VT 线程的 flush ack
    /// 为条件。原实现把收割挂在 `flush_with_timeout(Duration::ZERO)` 的成功分支上，
    /// 而零期限的 `recv_timeout` 要在 ack 已被 VT 线程送出的一瞬才可能命中，实测 32 次
    /// 连续尝试 0 次成功（`zero_timeout_flush_observes_the_ack`）——于是这条分支实际是
    /// 死代码：OSC 52 写入与 BEL 永远不收割（用户复制内容后剪贴板不更新、振铃不响），
    /// DECRPM/DSR/DA 应答也永不写回 PTY（子应用无限期等待设备状态查询）。
    ///
    /// 丢弃 flush 的代价是无 Synchronization：VT 线程尚未处理完时取到空，下次取到。
    /// 收割是每帧重复的拉取式操作，事件不会因此丢失，只是可能晚一帧。
    fn harvest_vt_side_effects(&mut self) {
        self.drain_callback_events();
        self.drain_pty_write_back();
    }

    /// 把待处理的 VT 应答（DECRPM、DSR、DA 等）回写到子 PTY。VT 引擎负责缓冲，子进程在等它们。
    fn drain_pty_write_back(&mut self) {
        for response in self.terminal.drain_pty_write_responses() {
            log::trace!("poll_pty_output: pty write-back {} bytes", response.len());
            if let Err(error) = self.pty.with(|pty| pty.write_all(&response)) {
                log::error!(
                    "session: PTY write-back failed ({} bytes): {}",
                    response.len(),
                    error
                );
            }
        }
    }

    /// 处理本帧所有可用输出与用户输入；处理过任何 VT 输出时返回 `true`
    /// （调用方应重建显示快照）。
    pub fn process_output(&mut self) -> bool {
        self.poll_pty_output(Self::MAX_CHUNKS_PER_FRAME)
    }

    /// 读取并清除 `new_output` 标志（见 docs/specification/REFERENCE.md）。由 PTY 摄入
    /// 路径置位，渲染线程是唯一的读清消费者；与 `dirty` 标志相互独立。
    pub fn take_new_output(&self) -> bool {
        self.output_processor.take_new_output()
    }

    /// 收割 VT 线程经上游回调上报的事件（剪贴板写入 / BEL 振铃）到队列/标志位。
    /// 拉取式：每帧重复调用，VT 线程尚未处理完时取到空、下帧再取（见
    /// `harvest_vt_side_effects` 的说明——本路径不 flush）。
    fn drain_callback_events(&self) {
        while let Some((_, text)) = self.terminal.poll_clipboard_event() {
            // FIFO 而非单槽：渲染暂停（pollEvent 停调）期间的连续写入必须全部保留，
            // 单槽会让后一次覆盖前一次，用户丢剪贴板内容。
            self.clipboard_text.lock().push_back(text);
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
            .with(|pty| pty.write_all(&response))
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

    pub fn title(&self) -> String {
        self.terminal.title()
    }

    pub fn mode_get(&self, mode_num: u16, kind: u8) -> bool {
        self.terminal.mode_get(mode_num, kind)
    }

    pub fn focus_event(&mut self, focused: bool) {
        // DECSET 1004 焦点上报：序列必须**直接**写入子 PTY 而非进入 VT 引擎——引擎的
        // 输出流解析器把 `CSI I` 当作 CHT（光标水平制表）、`CSI O` 当作非法 CSI，送入
        // 引擎只会把光标移到下一个制表位而非通知应用。仅当子进程确实启用了 1004 时才发送
        // （xterm 语义）。超时很短：本调用在 UI 线程运行，VT 线程卡住时不得拖满查询超时。
        if !self.terminal.mode_get_with_timeout(
            1004,
            0,
            std::time::Duration::from_millis(Self::FOCUS_MODE_QUERY_TIMEOUT_MS),
        ) {
            return;
        }
        let data = if focused { b"\x1b[I" } else { b"\x1b[O" };
        if let Err(error) = self.pty.with(|pty| pty.write_all(data)) {
            log::warn!("session: focus_event write failed: {error}");
        }
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
    if let Err(e) = handle.join() {
        log::error!("session: thread panicked: {:?}", e);
    }
}

impl Session {
    /// 立即让子进程所在进程组终止：置 exited、解除读线程的输出通道阻塞、
    /// 投递 SIGHUP/SIGCONT/SIGKILL（带宽限）。幂等 —— `Drop` 会重复调用本体，
    /// 信号重复与通道重换均无害。不 join 线程：join 由 `Drop` 统一负责，
    /// 使「立即杀掉 shell」可从任何上下文（含 `destroySession` 与已 detach 的
    /// Arc 克隆）幂等发起。
    pub fn request_exit(&mut self) {
        self.exited.store(true, Ordering::Release);
        // 先断开输出通道：flume 旧接收端一析构，阻塞的 `output_tx.send` 即返回
        // Err 走到「output channel closed」退出分支，等待线程也随 exited 标志退出。
        // 换入的是空通道，故 `poll_pty_output` 等后续调用仍安全。
        self.output_rx = flume::bounded::<Vec<u8>>(OUTPUT_CHANNEL_BOUND).1;
        let pid = self.pty.with(|pty| pty.child_pid());
        if pid.as_raw() > 0 {
            // 优先组杀：`kill(-pgid)` 把信号发给整个前台进程组，使 shell 的子进程
            // （管道、前台组内的后台作业）一并退出。
            if let Ok(pgid) = nix::unistd::getpgid(Some(pid)) {
                let pgid_raw = pgid.as_raw();
                // 自杀 guard：子进程 fork 后在 setsid 前与本进程同组，组杀会连带杀死
                // 本进程（设备实证 SIGKILL 自杀），故退化为直杀子进程。
                let own_pgid = nix::unistd::getpgid(None)
                    .map(|group| group.as_raw())
                    .unwrap_or(-1);
                if pgid_raw == own_pgid {
                    log::warn!("session drop: child shares our process group, direct kill only");
                }
                if pgid_raw > 0 && pgid_raw != own_pgid {
                    let group = nix::unistd::Pid::from_raw(-pgid_raw);
                    if !deliver_signal(group, nix::sys::signal::Signal::SIGHUP, "SIGHUP to pgid") {
                        deliver_signal(pid, nix::sys::signal::Signal::SIGHUP, "SIGHUP to child");
                    }
                    deliver_signal(group, nix::sys::signal::Signal::SIGCONT, "SIGCONT to pgid");
                    std::thread::sleep(TRAILING_EXIT_GRACE);
                    if !deliver_signal(group, nix::sys::signal::Signal::SIGKILL, "SIGKILL to pgid")
                    {
                        deliver_signal(pid, nix::sys::signal::Signal::SIGKILL, "SIGKILL to child");
                    }
                    return;
                }
            }
            // 退化路径：`getpgid` 失败或 pgid <= 0 时直杀子进程。
            deliver_signal(pid, nix::sys::signal::Signal::SIGHUP, "SIGHUP to child");
            deliver_signal(pid, nix::sys::signal::Signal::SIGCONT, "SIGCONT to child");
            std::thread::sleep(TRAILING_EXIT_GRACE);
            deliver_signal(pid, nix::sys::signal::Signal::SIGKILL, "SIGKILL to child");
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
mod tests {
    use super::*;

    fn drain_output(session: &mut Session, deadline: std::time::Instant) {
        while std::time::Instant::now() < deadline {
            session.process_output();
            if session.is_exited() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// 为测试创建 24x80 的 `/bin/sh` 会话。
    fn spawn_test_session() -> Session {
        Session::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None).expect("spawn failed")
    }

    /// 启动 shell、发送 `exit` 并等待会话报告退出。
    fn spawn_and_exit() -> Session {
        let mut session = spawn_test_session();
        session.write(b"exit\n").expect("write failed");
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        drain_output(&mut session, deadline);
        assert!(session.is_exited());
        session
    }

    #[test]
    fn session_spawn_and_exit() {
        spawn_and_exit();
    }

    #[test]
    fn session_echo_hello() {
        let mut session = spawn_test_session();
        session.write(b"echo hello_p12\n").expect("write failed");
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut found = false;
        while std::time::Instant::now() < deadline {
            session.process_output();
            let rows = session.terminal().rows();
            for row in 0..rows {
                if let Some(line) = session.terminal().read_line_text(row)
                    && line.contains("hello_p12")
                {
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(found, "did not find 'hello_p12' in terminal");
    }

    #[test]
    fn session_resize() {
        let mut session = spawn_test_session();
        session.resize(40, 120).expect("resize failed");
        assert_eq!(session.terminal().rows(), 40);
        assert_eq!(session.terminal().cols(), 120);
    }

    /// 回归护栏：PTY 写入不得在会话锁内阻塞。
    ///
    /// 子进程长时间不读 stdin 时 `write_all` 最多等 `WRITE_DRAIN_TIMEOUT`（5s），
    /// 而会话锁每帧都被 `render_inner` 与 `poll_event` 取得——写入一旦持锁，
    /// 整台终端（渲染、事件收割、其它会话）就一起停摆。本测试用只到第一个字节就
    /// 让写入线程挂起的替身（真实 `write_all` 到此返回 `Ok(1)`，不进入等待分支），
    /// 断言会话锁在这段时间内仍可取得。
    #[test]
    fn pty_write_does_not_hold_the_session_lock() {
        use std::io;
        use std::os::unix::io::{OwnedFd, RawFd};
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::time::Instant;

        /// 写入挂起直到本测试放行：模拟真实 `write_all` 在主端缓冲满时的等待分支。
        struct BlockingPty {
            entered: Arc<AtomicBool>,
            released: Arc<AtomicBool>,
        }

        impl Pty for BlockingPty {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.entered.store(true, Ordering::Release);
                while !self.released.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(1));
                }
                Ok(buf.len())
            }
            fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::WouldBlock, "no data"))
            }
            fn resize(&self, _rows: u16, _cols: u16) -> Result<(), PtyError> {
                Ok(())
            }
            fn get_winsize(&self) -> Result<(u16, u16), PtyError> {
                Ok((24, 80))
            }
            fn child_pid(&self) -> nix::unistd::Pid {
                nix::unistd::Pid::from_raw(4_194_305)
            }
            fn master_fd(&self) -> RawFd {
                -1
            }
            fn try_clone_reader_fd(&self) -> io::Result<OwnedFd> {
                std::fs::File::open("/dev/null").map(OwnedFd::from)
            }
            fn set_nonblocking(&self) -> Result<(), PtyError> {
                Ok(())
            }
            fn spawn(
                _shell: &str,
                _rows: u16,
                _cols: u16,
                _env: &ShellEnv,
                _cwd: Option<&Path>,
            ) -> Result<Box<dyn Pty>, PtyError> {
                unreachable!("本替身只经 with_pty 构造")
            }
        }

        let entered = Arc::new(AtomicBool::new(false));
        let released = Arc::new(AtomicBool::new(false));
        let session = Arc::new(std::sync::Mutex::new(
            Session::with_pty(
                Box::new(BlockingPty {
                    entered: entered.clone(),
                    released: released.clone(),
                }) as Box<dyn Pty>,
                24,
                80,
            )
            .expect("with_pty must succeed"),
        ));
        let writer = session.lock().expect("session lock").pty_master();

        let writer_thread = std::thread::spawn(move || writer.write(b"x"));
        // 写入线程已进入 `Pty::write` 并挂起在等待分支。
        let entered_deadline = Instant::now() + Duration::from_secs(5);
        while !entered.load(Ordering::Acquire) && Instant::now() < entered_deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(
            entered.load(Ordering::Acquire),
            "写入线程必须已挂起在 PTY 写入内，否则本测试无判别力"
        );

        // 关键断言：写入挂起期间会话锁仍可取得。锁被占用即代表写入持锁——
        // 渲染与事件收割每帧都取这把锁，阻塞写入会把整台终端冻住。
        {
            let locked = session
                .try_lock()
                .expect("PTY 写入不得持有会话锁：渲染与事件收割每帧都取这把锁");
            assert_eq!(locked.terminal().rows(), 24, "取得的必须是本会话");
        }

        // 放行后写入照常完成（锁外写入的字节序与结果语义不变）。
        released.store(true, Ordering::Release);
        writer_thread
            .join()
            .expect("writer thread panicked")
            .expect("放行后写入必须成功");
        assert_eq!(session.lock().expect("session lock").terminal().rows(), 24);
    }

    #[test]
    fn session_resize_same_size_is_applied_noop() {
        let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let mut session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        let before = handle.resize_count();
        // 干净状态：同尺寸 resize 短路并报告 Applied。
        let outcome = session.resize(24, 80).expect("resize failed");
        assert!(matches!(outcome, ResizeOutcome::Applied));
        assert!(!session.grid_dirty.load(Ordering::Acquire));
        // 短路不得改动 PTY。
        assert_eq!(
            handle.resize_count(),
            before,
            "pty.resize must not be called"
        );
        // 网格未变（仍是 spawn 时的尺寸）。
        assert_eq!(session.grid_size(), (24, 80));
        assert_eq!(session.terminal().rows(), 24);
    }

    /// OSC 52 读取应答以 `ESC ] 52 ; <selection> ; <base64> BEL` 写回 PTY。
    #[test]
    fn answer_clipboard_read_writes_esc52_reply() {
        let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let mut session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        session
            .answer_clipboard_read("c", "Hello, 世界")
            .expect("answer must succeed");
        let written = handle.written();
        assert_eq!(
            written, b"\x1b]52;c;SGVsbG8sIOS4lueVjA==\x07",
            "answer must be base64-encoded OSC 52 with BEL terminator"
        );
    }

    /// 剪贴板应答为空时仍产生合法（空载荷）的 OSC 52 回复，即兼容 xterm 的“空剪贴板”响应。
    #[test]
    fn answer_clipboard_read_empty_text() {
        let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let mut session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        session
            .answer_clipboard_read("c", "")
            .expect("answer must succeed");
        assert_eq!(handle.written(), b"\x1b]52;c;\x07");
    }

    /// 会话退出后写应答必须干净失败。
    #[test]
    fn answer_clipboard_read_after_exit_fails() {
        let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let mut session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        session.exited_flag().store(true, Ordering::Release);
        let result = session.answer_clipboard_read("c", "text");
        assert!(result.is_err(), "exited session must reject writes");
        assert!(handle.written().is_empty(), "nothing may reach the PTY");
    }

    #[test]
    fn session_resize_dirty_same_size_still_retries() {
        let mut session = spawn_test_session();
        // 模拟网格命令被丢弃：置脏标志，缓存保持旧尺寸。
        session.grid_dirty.store(true, Ordering::Release);
        // 同尺寸 resize 不得短路：须重发 ioctl 与网格命令并清除脏标志（修复分歧）。
        let outcome = session.resize(24, 80).expect("resize failed");
        assert!(matches!(outcome, ResizeOutcome::Applied));
        assert!(!session.grid_dirty.load(Ordering::Acquire));
        assert_eq!(session.grid_size(), (24, 80));
    }

    #[test]
    fn session_resize_dirty_cleared_on_success() {
        let mut session = spawn_test_session();
        session.grid_dirty.store(true, Ordering::Release);
        // 真正不同的尺寸在成功后清除脏标志。
        let outcome = session.resize(40, 120).expect("resize failed");
        assert!(matches!(outcome, ResizeOutcome::Applied));
        assert!(!session.grid_dirty.load(Ordering::Acquire));
        assert_eq!(session.grid_size(), (40, 120));
    }

    #[test]
    fn session_after_exit_returns_error() {
        let mut session = spawn_and_exit();
        let result = session.write(b"echo after-exit\n");
        assert!(
            matches!(result, Err(SessionError::Closed)),
            "write after exit must report Closed, got: {result:?}"
        );
    }

    #[test]
    fn session_new_creates_pty() {
        let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        assert_eq!(
            session.terminal().rows(),
            24,
            "terminal rows must be 24 after creation"
        );
        assert_eq!(
            session.terminal().cols(),
            80,
            "terminal cols must be 80 after creation"
        );
        assert!(
            !session.is_exited(),
            "new session must not be in exited state"
        );
    }

    #[test]
    fn session_resize_sends_signal() {
        let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let mut session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        session.resize(40, 120).expect("resize must succeed");
        assert_eq!(
            session.terminal().rows(),
            40,
            "terminal rows must update after resize"
        );
        assert_eq!(
            session.terminal().cols(),
            120,
            "terminal cols must update after resize"
        );
        assert_eq!(handle.rows(), 40, "PTY rows must update after resize");
        assert_eq!(handle.cols(), 120, "PTY cols must update after resize");
    }

    #[test]
    fn session_write_input() {
        let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let mut session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        session.write(b"hello world").expect("write must succeed");
        let written = handle.written();
        assert_eq!(
            written, b"hello world",
            "input written to session must reach PTY master"
        );
    }

    #[test]
    fn take_reported_exit_waits_for_the_real_code_then_reports_once() {
        // 这是本次改动的核心契约：真实 `exit 3` 的会话必须报出 3 而不是 0。
        // 旧实现先锁存上报标志、再忙等退出码，等不到就报 0，且退出事件不会重发——
        // 于是一次 3 退出被永久谎报成 0。
        let mut session = spawn_test_session();
        // 用非零码：裸 `exit` 的惯例码是 0，断言 0 等于没断言——正是本缺陷的形态。
        session.write(b"exit 3\n").expect("write failed");
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        drain_output(&mut session, deadline);
        assert!(session.is_exited());
        // 每帧轮询直到可上报；真实退出码必然在等待线程里被写入，无需忙等。
        let reported = loop {
            if let Some(reported) = session.take_reported_exit() {
                break reported;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "exit code must become reportable well within 3s"
            );
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(reported, ReportedExit::Code(3));
        // 恰好一次：此后不再有可上报的退出。
        assert_eq!(session.take_reported_exit(), None);
    }

    /// 回归护栏：OSC 52 写入必须被收割，用户复制的内容才能进系统剪贴板。
    ///
    /// 原实现把收割挂在 `flush_with_timeout(Duration::ZERO)` 成功分支上，而零期限
    /// `recv_timeout` 实测 32 次 0 次命中 ack，故 OSC 52 与 BEL 事件永不上报
    /// （`NativeBridgeSmokeTest > feedTerminal OSC52 write surfaces clipboard poll event`
    /// 长期失败即由此而来）。本测试只依赖「投递 + 收割」，不依赖任何 flush 时序。
    #[test]
    fn osc52_write_is_harvested_without_relying_on_flush_ack() {
        let mut session = spawn_test_session();
        let marker = "harvest-probe";
        use base64::Engine;
        let payload = base64::engine::general_purpose::STANDARD.encode(marker.as_bytes());
        // 与 `feedTerminal` 同路径：直接投进 VT 解析器（不经 PTY，故不依赖回显）。
        session
            .terminal_mut()
            .vt_write(format!("\u{1b}]52;c;{payload}\u{7}").as_bytes());
        // 单次收割即须命中：VT 线程处理完 `vt_write` 后回调事件已在通道里，
        // `harvest_vt_side_effects` 是非阻塞取。这里**不轮询**——轮询会把「收割被
        // 条件门控住、每帧才有机会捡到」的实现也判为通过，护栏就形同虚设。
        // 与生产渲染帧完全同口径：只 `process_output` + 收割，**不 flush、不轮询
        // 额外同步**。VT 线程是异步的，事件在它处理完 `vt_write` 之后的某一帧被收割；
        // 这正是「每帧重复的拉取式收割」的设计前提，也让本护栏对「收割被条件门控住」
        // 的旧实现为红（门控一旦失配，事件在所有帧里都取不到）。
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut harvested = session.poll_clipboard();
        while harvested.is_none() && std::time::Instant::now() < deadline {
            session.process_output();
            std::thread::sleep(Duration::from_millis(5));
            harvested = session.poll_clipboard();
        }
        assert_eq!(
            harvested.as_deref(),
            Some(marker),
            "OSC 52 payload must reach the clipboard slot within a few render frames"
        );
    }

    /// 渲染暂停（`pollEvent` 停调）期间的连续 OSC 52 写入必须全部保留：
    /// 单槽锁存会让后一次覆盖前一次，用户丢剪贴板内容。
    #[test]
    fn osc52_writes_queue_in_order_when_not_polled() {
        let mut session = spawn_test_session();
        use base64::Engine;
        let mut written = Vec::new();
        for marker in ["first-copy", "second-copy"] {
            let payload = base64::engine::general_purpose::STANDARD.encode(marker.as_bytes());
            session
                .terminal_mut()
                .vt_write(format!("\u{1b}]52;c;{payload}\u{7}").as_bytes());
            written.push(marker);
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut harvested = Vec::new();
        while harvested.len() < written.len() && std::time::Instant::now() < deadline {
            session.process_output();
            if let Some(text) = session.poll_clipboard() {
                harvested.push(text);
            } else {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        assert_eq!(
            harvested, written,
            "渲染暂停期间的 OSC 52 写入必须按序全部上报，不得互相覆盖"
        );
    }

    #[test]
    fn take_reported_exit_is_silent_before_exit() {
        // 运行中的会话不得产生退出上报——否则会误报一个还在跑的 shell。
        let session = spawn_test_session();
        assert!(!session.is_exited());
        assert_eq!(session.take_reported_exit(), None);
    }

    #[test]
    fn unknown_exit_code_is_reported_rather_than_suppressed() {
        // `waitpid` 失败时代码槽为 Unknown。必须照常上报（`ReportedExit::Unknown`），
        // 否则退出事件永不到达、会话表泄漏——比报出错误的码更糟。
        let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        *session.exit_code.lock() = ExitCodeSlot::Unknown;
        session.exited.store(true, Ordering::Release);
        assert_eq!(session.take_reported_exit(), Some(ReportedExit::Unknown));
    }

    #[test]
    fn mark_exit_reported_is_idempotent_under_concurrency() {
        let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        // 上报竞态中只能有一个调用者胜出；其余（并发的 pollEvent 线程）都须看到 false，
        // 使 Exit 事件永不重复。`Arc<Mutex<..>>` 镜像生产 SESSION_REGISTRY 的形状
        // （`Box<dyn Pty>` 非 Sync，故会话须经互斥锁共享）。
        let session = std::sync::Arc::new(parking_lot::Mutex::new(session));
        let mut handles = Vec::new();
        for _ in 0..8 {
            let session = session.clone();
            handles.push(std::thread::spawn(move || {
                session.lock().mark_exit_reported()
            }));
        }
        let winners = handles
            .into_iter()
            .filter_map(|join_handle| join_handle.join().ok())
            .filter(|won| *won)
            .count();
        assert_eq!(winners, 1, "exactly one caller must report the exit");
        // 后续调用保持 false。
        assert!(!session.lock().mark_exit_reported());
    }

    #[test]
    fn session_title_default_is_empty() {
        let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        assert_eq!(session.title(), "");
    }

    #[test]
    fn session_mode_get_default_false() {
        let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        // 模式 2004（bracketed paste）默认应关闭。
        assert!(!session.mode_get(2004, 0));
    }

    #[test]
    fn session_focus_event_writes_to_terminal() {
        let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let mut session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        // `focus_event` 向终端写 CSI 序列，不应 panic。
        session.focus_event(true);
        session.focus_event(false);
    }

    #[test]
    fn session_exited_flag() {
        let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        assert!(!session.is_exited(), "fresh session must not be exited");
        let flag = session.exited_flag();
        assert!(!flag.load(std::sync::atomic::Ordering::Acquire));
        // 标记已退出并验证。
        handle.set_exited();
        assert!(handle.is_exited());
    }

    /// 护栏：满输出通道下的会话拆卸必须及时完成。
    ///
    /// 读取线程会持续生产直到通道满（128 块）然后阻塞在 `send`。`Drop` 体运行时
    /// `output_rx` 仍存活，故若不先换掉接收端，读取线程永远等不到解除——四个
    /// `wait_finished` 窗口全部超时，每次关闭会话都要付满约 400ms 并泄漏线程句柄。
    /// 这里用真实 PTY 产生远超通道容量的输出，再断言拆卸在预算内返回。
    #[test]
    fn drop_does_not_stall_on_a_full_output_channel() {
        // 预算：正常拆卸路径含 SIGHUP/SIGCONT 与两个 50ms grace 窗口，理论下界远小于
        // 1s。旧实现在此稳定超过 1s（四个窗口全超时 + 重试）。
        let start = std::time::Instant::now();
        {
            let mut session = spawn_test_session();
            session.write(b"seq 1 4000000\n").expect("write failed");
            // 等到通道真的填满：此时读取线程必然阻塞在 `send` 上。
            //
            // 不等填满就 drop 是测不出来的——子进程可能还没产出 128 块，读取线程于是
            // 从 EOF 正常退出，断言有没有修复都会通过。这正是我第一版测试无效的原因。
            let fill_deadline = std::time::Instant::now() + Duration::from_secs(10);
            while session.output_rx.len() < OUTPUT_CHANNEL_BOUND {
                assert!(
                    std::time::Instant::now() < fill_deadline,
                    "output channel never filled; this test cannot discriminate"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_secs(1),
            "dropping a session with a full output channel took {elapsed:?},              the reader thread is blocked on send()"
        );
    }

    #[test]
    fn request_exit_kills_child_while_session_still_alive() {
        // destroySession 从注册表摘下条目后即向宿主返回，此时剪贴板应答等异步
        // 克隆仍持有 Arc<Mutex<Session>>，Drop 尚未执行——shell 必须在此时就死，
        // 否则宿主已经认为会话销毁、shell 却在后台继续跑（生命周期契约破裂）。
        let mut session = spawn_test_session();
        session.write(b"echo alive\n").expect("write failed");
        let pid = session.pty.with(|pty| pty.child_pid());
        assert!(pid.as_raw() > 0, "spawned session must have a child pid");

        session.request_exit();

        // 子进程必须在 request_exit 返回后即已消失（信号已投递）。
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut still_running = true;
        while std::time::Instant::now() < deadline {
            match nix::sys::signal::kill(pid, None) {
                Err(nix::errno::Errno::ESRCH) => {
                    still_running = false;
                    break;
                }
                Ok(()) => std::thread::sleep(Duration::from_millis(10)),
                Err(error) => panic!("unexpected kill probe error: {error}"),
            }
        }
        assert!(
            !still_running,
            "request_exit returned but child {} is still alive — destroySession would \
             report success while the shell keeps running",
            pid.as_raw()
        );
        // 会话对象本身仍在（模拟异步克隆持有），清理交给 Drop。
        assert!(session.is_exited());
    }

    #[test]
    fn session_write_after_exit_returns_error() {
        let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let mut session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        // 置会话的退出标志，使 `write()` 在调用 PTY 前先检查它。
        session.exited_flag().store(true, Ordering::Release);
        let result = session.write(b"test");
        assert!(
            result.is_err(),
            "write after exit must return error, got Ok"
        );
    }

    #[test]
    fn read_error_action_classifies_errno() {
        // EINTR/EAGAIN 是瞬时错误，必须保持读取循环存活。
        assert_eq!(
            read_error_action(Some(libc::EINTR)),
            ReaderErrorAction::Retry
        );
        assert_eq!(
            read_error_action(Some(libc::EAGAIN)),
            ReaderErrorAction::Retry
        );
        // EIO 表示 PTY 从端已关闭（Linux PTY 上的 EOF）——停止。
        assert_eq!(read_error_action(Some(libc::EIO)), ReaderErrorAction::Stop);
        // 其他 errno（或无 errno）同样停止读取。
        assert_eq!(
            read_error_action(Some(libc::EPIPE)),
            ReaderErrorAction::Stop
        );
        assert_eq!(read_error_action(None), ReaderErrorAction::Stop);
    }
}
