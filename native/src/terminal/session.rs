//! 会话编排器：串接 PTY 读取、VT 解析与进程等待。
use parking_lot::Mutex;
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
        Some(libc::EINTR) => ReaderErrorAction::Retry,
        _ => ReaderErrorAction::Stop,
    }
}

/// 读取线程在 `poll` 中停驻多久后重新检查退出标志。改用 poll 后输出延迟依旧很低，
/// 而 PTY 空闲时不再空转 CPU。
const READ_POLL_TIMEOUT_MS: i32 = 100;

/// 回滚行数固定值（与 Termux 默认 transcript-rows=2000 一致）。
/// PROHIBITED 禁止「终端回滚行数」设置，因此既无产品入口也无 FFI 入参通道。
pub(crate) const DEFAULT_SCROLLBACK_LINES: u32 = 2000;

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
/// （通道满 / VT 线程卡住），此时调用方不得把新尺寸作为权威值发布。
pub enum ResizeOutcome {
    Applied,
    Dropped,
}

impl SessionError {
    /// 底层写入以 EAGAIN/EWOULDBLOCK 失败时为真，即非阻塞主端的 PTY 缓冲区已满
    /// （子进程未读取）。此时调用方按 xterm 语义丢弃该输入而非当作错误上报。
    pub fn is_would_block(&self) -> bool {
        matches!(self, SessionError::Io(e) if e.kind() == std::io::ErrorKind::WouldBlock)
    }
}

/// 终端会话：串接 PTY 读取、VT 解析与进程等待。
///
/// 生命周期：`Spawned`（PTY 建立、线程启动）→ `Running` ⇄ `Idle`/`Paused`（有输出即回到
/// `Running`）→ `Exited`（EOF/退出）→ `Cleaned`（`cleanup_resources`）。
pub struct Session {
    pty: Box<dyn Pty>,
    terminal: GhosttyTerminal,
    output_processor: OutputProcessor,
    output_tx: flume::Sender<Vec<u8>>,
    output_rx: Receiver<Vec<u8>>,
    // ── 事件状态（Kotlin 经 push_event 轮询） ────────────────────────
    exited: Arc<AtomicBool>,
    /// 后台（非活跃）会话的 Exit 事件推入事件队列后置位，使 pollEvent 的逐帧扫描
    /// 只上报一次。
    exit_reported: Arc<AtomicBool>,
    clipboard_text: Arc<Mutex<Option<String>>>,
    /// 待上报的 BEL 振铃（上游 on_bell 回调经通道推送，drain_callback_events 收割）。
    /// 瞬时提示：单帧多响合并为一，poll_bell 取走并清零（get-and-clear）。
    bell_pending: Mutex<bool>,
    /// 待处理的 OSC 52 剪贴板读取请求：所请求的 selection 名。
    /// 由 JNI 层（`poll_clipboard_read`）消费，转发给宿主应用后经
    /// [`Session::answer_clipboard_read`] 写回应答。
    clipboard_read: Arc<Mutex<Option<String>>>,

    // ── 线程生命周期 ─────────────────────────────────────────────────
    reader_handle: Option<std::thread::JoinHandle<()>>,
    wait_handle: Option<std::thread::JoinHandle<()>>,

    // ── 运行态 ───────────────────────────────────────────────────────
    /// 来自 waitpid 的退出码；进程运行中为 `None`。
    pub(crate) exit_code: Arc<Mutex<Option<i32>>>,
    /// 子进程存活时长（毫秒，fork → waitpid），由等待线程在退出时写入，
    /// 随 Exit 事件载荷作诊断用。
    pub(crate) exit_alive_ms: Arc<Mutex<Option<u64>>>,
    /// fork 时间戳，即 [Self::exit_alive_ms] 的起点。
    spawned_at: std::time::Instant,

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
            Ok(p) => {
                log::info!("Session::spawn: PtyPair::spawn OK");
                p
            }
            Err(e) => {
                log::info!("Session::spawn: PtyPair::spawn error: {e}");
                return Err(e.into());
            }
        };
        match pty.set_nonblocking() {
            Ok(()) => log::info!("Session::spawn: set_nonblocking OK"),
            Err(e) => {
                log::info!("Session::spawn: set_nonblocking error: {e}");
                return Err(e.into());
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
                Err(e) => {
                    // `read_file` 在此被丢弃，其 fd 随之安全关闭。
                    return Err(e);
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
                        // NUL 剥离：VT 解析前剔除 0x00 字节，避免 APC-NUL 渲染伪影。
                        let mut data = read_buf[..bytes_read].to_vec();
                        if data.contains(&0) {
                            data.retain(|&b| b != 0);
                        }
                        if output_tx.send(data).is_err() {
                            log::info!("reader thread: output channel closed");
                            break;
                        }
                    }
                    Err(e) => match read_error_action(e.raw_os_error()) {
                        ReaderErrorAction::Retry => {}
                        ReaderErrorAction::Stop => {
                            if e.raw_os_error() == Some(libc::EIO) {
                                log::info!("reader thread: PTY EOF (slave closed, EIO)");
                            } else {
                                log::info!("reader thread: read error: {e}");
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
        let exit_alive_ms = session.exit_alive_ms.clone();
        let spawned_at = session.spawned_at;
        let exited_wait = exited.clone();
        let wait_handle = std::thread::spawn(move || {
            log::info!("wait thread: waiting for child pid={child_pid}");
            let result = nix::sys::wait::waitpid(child_pid, None);
            // 记录子进程真实存活时长（fork → waitpid）供 Exit 事件诊断载荷使用。
            *exit_alive_ms.lock() = Some(spawned_at.elapsed().as_millis() as u64);
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
                    *exit_code.lock() = Some(code);
                }
                // Shell 被信号杀死（Ctrl+\、kill -9）：上报惯例的 128 + 信号码，
                // 避免 UI 把信号死亡当成干净的退出码 0。
                Ok(nix::sys::wait::WaitStatus::Signaled(_, signal, _)) => {
                    *exit_code.lock() = Some(128 + signal as i32);
                }
                _ => {}
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
        let clipboard_text = Arc::new(Mutex::new(None));
        let clipboard_read = Arc::new(Mutex::new(None));
        let (output_tx, output_rx) = bounded::<Vec<u8>>(128);
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
            pty,
            terminal,
            output_processor: OutputProcessor::new(),
            output_tx,
            output_rx,
            exited,
            exit_reported,
            clipboard_text,
            clipboard_read,
            bell_pending: Mutex::new(false),
            reader_handle: None,
            wait_handle: None,
            exit_code: Arc::new(Mutex::new(None)),
            exit_alive_ms: Arc::new(Mutex::new(None)),
            spawned_at: std::time::Instant::now(),
            terminal_rows: AtomicU32::new(rows),
            terminal_cols: AtomicU32::new(cols),
            grid_dirty: AtomicBool::new(false),
        })
    }

    pub fn write(&mut self, data: &[u8]) -> Result<(), SessionError> {
        if self.is_exited() {
            return Err(SessionError::Closed);
        }
        self.pty.write_all(data).map_err(SessionError::Io)?;
        Ok(())
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

    /// 向本会话子进程发送 POSIX 信号（按编号），供外部控制器向存活 Shell 发送中断/终止信号。
    pub fn send_signal(&self, signum: i32) -> Result<(), SessionError> {
        let signal = nix::sys::signal::Signal::try_from(signum)
            .map_err(|error| SessionError::Ghostty(format!("invalid signal {signum}: {error}")))?;
        let child = self.pty.child_pid();
        // 先杀前台进程组。
        if let Some(foreground_pid) = self.pty.foreground_pid() {
            let foreground_raw = foreground_pid.as_raw() as libc::pid_t;
            if foreground_raw > 1 {
                let pgid = -foreground_raw;
                // SAFETY: `killpg` 向进程组发送信号；`pgid` 取自存活的前台进程组。
                let result = unsafe { libc::kill(pgid, signal as i32) };
                if result == 0 {
                    return Ok(());
                }
                log::warn!(
                    "send_signal: group kill(-{foreground_raw}, {signal:?}) failed: {}, falling back to child",
                    nix::errno::Errno::last()
                );
            }
        }
        // 回退为直接杀子进程
        // SAFETY: `child` 为存活子进程 pid，仅发送已校验的 `Signal`。
        let result = unsafe { libc::kill(child.as_raw() as libc::pid_t, signal as i32) };
        if result == 0 {
            Ok(())
        } else {
            Err(SessionError::Ghostty(format!(
                "kill({}, {signal:?}) failed: {}",
                child,
                nix::errno::Errno::last()
            )))
        }
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

            if let Some(selection) = snap.clipboard_read {
                *self.clipboard_read.lock() = Some(selection);
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
                self.terminal.flush();
                self.drain_callback_events();
                // 即使走到封顶路径也排空回写应答：输出洪水不得饿死 DECRPM/DSR/DA 应答，
                // 否则子应用会无限期等待。
                self.drain_pty_write_back();
                return true;
            }
        }
        if count > 0 {
            log::trace!("poll_pty_output: processed {count} chunks");
            self.terminal.flush();
            self.drain_callback_events();
            self.drain_pty_write_back();
            true
        } else {
            false
        }
    }

    /// 把待处理的 VT 应答（DECRPM、DSR、DA 等）回写到子 PTY。VT 引擎负责缓冲，子进程在等它们。
    fn drain_pty_write_back(&mut self) {
        for response in self.terminal.drain_pty_write_responses() {
            log::trace!("poll_pty_output: pty write-back {} bytes", response.len());
            if let Err(error) = self.pty.write_all(&response) {
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

    /// 收割 VT 线程经上游回调上报的事件（剪贴板写入 / BEL 振铃）到锁存槽。
    /// 紧跟 flush 调用：flush 返回时 VT 线程已处理完本批输出，回调已触发。
    fn drain_callback_events(&self) {
        while let Some((_, text)) = self.terminal.poll_clipboard_event() {
            *self.clipboard_text.lock() = Some(text);
        }
        while self.terminal.poll_bell_event().is_some() {
            *self.bell_pending.lock() = true;
        }
    }

    /// 取走待上报的 BEL 振铃并清零（get-and-clear，无振铃为 false）。
    pub fn poll_bell(&self) -> bool {
        let mut guard = self.bell_pending.lock();
        std::mem::replace(&mut *guard, false)
    }

    /// 轮询 OSC 52 转义序列写入的剪贴板文本。
    pub fn poll_clipboard(&self) -> Option<String> {
        let mut guard = self.clipboard_text.lock();
        guard.take()
    }

    /// 取走待处理的 OSC 52 剪贴板读取请求（selection 名）。
    /// JNI 层将其转发给宿主应用，并经 [`Session::answer_clipboard_read`] 回传结果。
    pub fn poll_clipboard_read(&self) -> Option<String> {
        let mut guard = self.clipboard_read.lock();
        guard.take()
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
        response.push(0x07); // BEL terminator (xterm-compatible)
        if self.is_exited() {
            return Err(SessionError::Closed);
        }
        self.pty.write_all(&response).map_err(SessionError::Io)?;
        Ok(())
    }

    pub fn is_exited(&self) -> bool {
        self.exited.load(Ordering::Acquire)
    }

    /// 读取子进程退出码（若等待线程已写入）。
    /// 非阻塞：需要等待退出码的调用方在**不**持有会话锁的前提下轮询（见 `ffi::wait_exit_code`）。
    pub fn exit_code_now(&self) -> Option<i32> {
        let guard = self.exit_code.lock();
        *guard
    }

    pub fn exited_flag(&self) -> Arc<AtomicBool> {
        self.exited.clone()
    }

    /// 原子地标记退出事件已上报给 Kotlin 侧，仅首个调用者返回 true——pollEvent 的
    /// 逐帧后台扫描据此保证每次退出只上报一次。
    pub fn mark_exit_reported(&self) -> bool {
        !self.exit_reported.swap(true, Ordering::AcqRel)
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
        if let Err(error) = self.pty.write_all(data) {
            log::warn!("session: focus_event write failed: {error}");
        }
    }
}

/// 带截止超时地 join 线程句柄，最多重试 3 次。
///
/// 初次超时后以 100ms 截止再尝试 3 次，以应对线程阻塞在需要多个信号才能唤醒的 I/O 上。
/// 全部失败则分离（丢弃句柄）并记错误——该线程的资源（fd、内存）会泄漏。
fn join_with_timeout(handle: &mut Option<std::thread::JoinHandle<()>>, timeout: Duration) {
    let Some(handle) = handle.take() else {
        return;
    };
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if handle.is_finished() {
            if let Err(e) = handle.join() {
                log::error!("session: thread panicked: {:?}", e);
            }
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    log::warn!("session: thread did not exit within {timeout:?}, retrying up to 3×");
    for _attempt in 0..3 {
        let retry_deadline = std::time::Instant::now() + Duration::from_millis(100);
        while std::time::Instant::now() < retry_deadline {
            if handle.is_finished() {
                if let Err(e) = handle.join() {
                    log::error!("session: thread panicked: {:?}", e);
                }
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    log::error!("session: thread failed to exit after retries — DETACHING (resource leak)");
    // 句柄在此被丢弃 → 分离
}

impl Drop for Session {
    fn drop(&mut self) {
        self.exited.store(true, Ordering::Release);
        let pid = self.pty.child_pid();
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
                        // 组杀失败，退化为直杀子进程。
                        deliver_signal(pid, nix::sys::signal::Signal::SIGHUP, "SIGHUP to child");
                    }
                    deliver_signal(group, nix::sys::signal::Signal::SIGCONT, "SIGCONT to pgid");
                    std::thread::sleep(TRAILING_EXIT_GRACE);
                    if !deliver_signal(group, nix::sys::signal::Signal::SIGKILL, "SIGKILL to pgid")
                    {
                        deliver_signal(pid, nix::sys::signal::Signal::SIGKILL, "SIGKILL to child");
                    }
                    // 等待组内进程退出。
                    join_with_timeout(&mut self.reader_handle, TRAILING_EXIT_GRACE);
                    join_with_timeout(&mut self.wait_handle, TRAILING_EXIT_GRACE);
                    return;
                }
            }
            // 退化路径：`getpgid` 失败或 pgid <= 0 时直杀子进程。
            deliver_signal(pid, nix::sys::signal::Signal::SIGHUP, "SIGHUP to child");
            deliver_signal(pid, nix::sys::signal::Signal::SIGCONT, "SIGCONT to child");
            std::thread::sleep(TRAILING_EXIT_GRACE);
            deliver_signal(pid, nix::sys::signal::Signal::SIGKILL, "SIGKILL to child");
        }
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
            .filter_map(|h| h.join().ok())
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
        // EINTR 是瞬时错误，必须保持读取循环存活。
        assert_eq!(
            read_error_action(Some(libc::EINTR)),
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
