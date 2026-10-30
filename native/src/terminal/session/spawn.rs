//! 会话启动：PTY 建立、读取线程与等待线程。
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::fs::File;
use std::io::Read;
use std::os::unix::io::AsFd;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};

use flume::bounded;
use nix::errno::Errno;
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};

use crate::terminal::ghostty_terminal::GhosttyTerminal;
use crate::terminal::output_processor::OutputProcessor;
use crate::terminal::pty::{Pty, PtyPair};
use crate::terminal::shell_env::ShellEnv;

use super::pty_master::PtyMaster;
use super::{
    ExitCodeSlot, OUTPUT_CHANNEL_BOUND, OutputWake, Session, SessionError, validated_grid,
};

const READ_BUF_SIZE: usize = 8192;
/// 信号致死退出码基数（shell 惯例：128 + 信号码）。
const SIGNAL_EXIT_BASE: i32 = 128;
/// `read(2)` 失败后读取线程应采取的动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReaderErrorAction {
    /// 重试读取——瞬时条件（EINTR），保持循环存活。
    Retry,
    /// 停止读取——PTY 已消失（EIO = 从端关闭）或错误致命。
    Stop,
}

/// 把 `read(2)` 错误归类为重试/停止决策。纯函数，使读取循环分支无需真实 PTY 即可测试。
pub(super) fn read_error_action(errno: Option<Errno>) -> ReaderErrorAction {
    match errno {
        Some(Errno::EINTR) | Some(Errno::EAGAIN) => ReaderErrorAction::Retry,
        _ => ReaderErrorAction::Stop,
    }
}
/// 读取线程在 `poll` 中停驻多久后重新检查退出标志。改用 poll 后输出延迟依旧很低，
/// 而 PTY 空闲时不再空转 CPU。
const READ_POLL_TIMEOUT_MS: u16 = 100;

impl Session {
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
        scrollback_lines: u32,
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
            scrollback_lines,
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
            loop {
                if exited_read.load(Ordering::Acquire) {
                    log::info!("reader thread: exiting due to exited flag");
                    break;
                }
                // 借用只活在这一小段里：`read_file` 随后还要以 `&mut` 读取。
                let poll_result = {
                    let mut poll_fds = [PollFd::new(read_file.as_fd(), PollFlags::POLLIN)];
                    poll(&mut poll_fds, PollTimeout::from(READ_POLL_TIMEOUT_MS))
                };
                match poll_result {
                    // 0 = 本轮无数据，回到循环顶部重查退出标志。
                    Ok(0) => continue,
                    Ok(_) => {}
                    // EINTR 不是失败：信号打断了 poll，等下一轮即可。
                    Err(Errno::EINTR) => continue,
                    Err(errno) => {
                        log::info!("reader thread: poll error: {errno}");
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
                    Err(read_error) => {
                        let errno = read_error.raw_os_error().map(Errno::from_raw);
                        match read_error_action(errno) {
                            ReaderErrorAction::Retry => {}
                            ReaderErrorAction::Stop => {
                                if errno == Some(Errno::EIO) {
                                    log::info!("reader thread: PTY EOF (slave closed, EIO)");
                                } else {
                                    log::info!("reader thread: read error: {read_error}");
                                }
                                exited_read.store(true, Ordering::Release);
                                break;
                            }
                        }
                    }
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

    pub(super) fn spawn_with_pty(
        pty: Box<dyn Pty>,
        rows: u32,
        cols: u32,
        frame_wake: Option<OutputWake>,
        kitty_temp_dir: Option<std::path::PathBuf>,
        scrollback_lines: u32,
    ) -> Result<Self, SessionError> {
        // 与 `resize` 同一条判据：上游 `Terminal::new` 只收 `u16`，在此放行越界值等于
        // 用一次静默截断替调用方决定网格尺寸。取回的 `u16` 只用于确认已校验，本函数
        // 的缓存与下游仍按 `u32` 记账。
        let (grid_rows, grid_cols) = validated_grid(rows, cols)?;
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
            u32::from(grid_rows),
            u32::from(grid_cols),
            scrollback_lines,
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
            terminal_rows: AtomicU32::new(u32::from(grid_rows)),
            terminal_cols: AtomicU32::new(u32::from(grid_cols)),
            grid_dirty: AtomicBool::new(false),
            last_scroll_offset: AtomicI64::new(0),
        })
    }
}
