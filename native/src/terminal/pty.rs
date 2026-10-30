//! PTY 主/从端创建：fork 与直接系统调用处允许 unsafe，VT/网格数据路径禁用 unsafe。
use std::io;
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::path::Path;
use std::sync::atomic::{AtomicU16, Ordering};
#[cfg(test)]
use std::time::Duration;

use nix::errno::Errno;
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use thiserror::Error;

use crate::terminal::shell_env::ShellEnv;

mod child;
mod env;

use child::{
    child_exit_with_reason, close_stray_fds, configure_raw_mode_child, is_pie,
    read_shebang_interpreter,
};
use env::{build_env, title_report_hook};

/// 对外声明的 Termux 版本：子进程据此判断自身是 Termux 环境。
pub(crate) const TERMUX_VERSION: &str = "0.119.0-beta.3";

/// PTY 输入缓冲排空的等待上限。
///
/// 交互式 shell 读走一个缓冲只需微秒级，故正常粘贴永不触及该上限；它只在子进程
/// 长时间不读 stdin 时收敛，使调用方收到**错误**而不是被静默截断的输入。
/// 取值远大于任何合理的交互延迟，又短于调用线程可接受的卡顿上限。
pub(crate) const WRITE_DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// VT 应答回写（DECRPM/DSR/DA、OSC 52 应答）的排空期限。
///
/// 与 [`WRITE_DRAIN_TIMEOUT`] 分开：这两条在**渲染线程**逐帧收割，且全程持有会话锁
/// ——按帧取同一把锁的渲染与看门狗都会被拖住整段等待。应答只有几十字节，正常情况下
/// 微秒级写完，故取一个帧级上限：写不完就当作子进程此刻不读，返回错误并如实记日志，
/// 下一次收割重试（VT 引擎的应答缓冲仍在，不丢）。
pub(crate) const WRITE_BACK_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(50);

/// 切分 Shell 启动入口为可执行路径与附加参数（DESIGN Shell 节支持
/// `/data/.../bash -l` 形态）。以 ASCII 空白切分，无引号转义语义。
fn split_shell_entry(shell: &str) -> (&str, Vec<&str>) {
    let mut words = shell.split_ascii_whitespace();
    let executable = words.next().unwrap_or(shell);
    (executable, words.collect())
}

#[derive(Debug, Error)]
pub enum PtyError {
    #[error("fork failed: {0}")]
    Fork(nix::errno::Errno),
    #[error("failed to open pseudoterminal: {0}")]
    Open(std::io::Error),
    #[error("ioctl TIOCSWINSZ failed: {0}")]
    Resize(nix::errno::Errno),
    #[error("shell path is empty")]
    EmptyShell,
    #[error("fcntl failed: {0}")]
    Fcntl(nix::errno::Errno),
    #[error("termios configuration failed: {0}")]
    Termios(nix::errno::Errno),
}

/// 仅供 [`nix::unistd::fork`] 使用：整条 `?` 链上唯一的裸 `Errno` 源就是 fork。
/// 其余调用点（`openpty`、TIOCSWINSZ、fcntl、termios）各自显式 `map_err`
/// 到对应变体，不经此转换——通配的 `From` 会把它们统统说成「fork failed」。
impl From<nix::errno::Errno> for PtyError {
    fn from(err: nix::errno::Errno) -> Self {
        PtyError::Fork(err)
    }
}

pub trait Pty: Send + Sync {
    /// 写入主端。取 `&self` 而非 `&mut`：实现内部只对 fd 做系统调用，
    /// 并不需要独占借用——写成 `&mut` 会迫使共享路径（resize / set_pixel_size /
    /// child_pid）去取互斥量，而互斥量此刻可能被无上界的 `write_all` 持住。
    /// 写入之间的字节序由 [crate::terminal::session::PtyMaster] 的写入锁保证。
    fn write(&self, buf: &[u8]) -> io::Result<usize>;
    /// 读主端。与 [Pty::write] 同理只做系统调用，取 `&self`。
    fn read(&self, buf: &mut [u8]) -> io::Result<usize>;
    fn resize(&self, rows: u16, cols: u16) -> Result<(), PtyError>;
    /// 更新缓存的像素尺寸并经 TIOCSWINSZ（ws_xpixel/ws_ypixel）写入内核，
    /// 使全屏程序看到真实像素尺寸而非 0。
    ///
    /// 默认空操作——无内核 winsize 的测试替身（如 `MockPty`）继承此实现，真实 PTY 覆写。
    fn set_pixel_size(&self, _width: u16, _height: u16) -> Result<(), PtyError> {
        Ok(())
    }
    /// 经 TIOCGWINSZ 查询当前终端窗口尺寸（rows x cols），用于验证 24x80 spawn 种子。
    fn get_winsize(&self) -> Result<(u16, u16), PtyError>;
    fn child_pid(&self) -> nix::unistd::Pid;
    /// 返回主端 fd 的借用视图，供 `poll` 等待主端可写。
    ///
    /// 默认 `None`：没有真实主端的测试替身（`MockPty` 等）继承此实现——
    /// `write_all` 遇 EAGAIN 时退化为「等满期限即报 WouldBlock」，与真实主端
    /// 被填满时的观感一致。
    fn master_borrowed_fd(&self) -> Option<BorrowedFd<'_>> {
        None
    }
    /// 返回独立所有的主端 fd 副本供专用读取线程使用。副本与 [`Pty::master_borrowed_fd`]
    /// 共享同一打开文件描述（故共享 O_NONBLOCK 状态），这对 `poll` + 类阻塞读取的读取线程无碍。
    /// dup 在此处（允许 `unsafe` 的位置）完成，使调用方能经安全的 `std::fs::File`
    /// 读取而无需任何 `unsafe` 块。
    fn try_clone_reader_fd(&self) -> io::Result<OwnedFd>;
    fn set_nonblocking(&self) -> Result<(), PtyError>;
    /// 写入全部字节，主端满时最多等 `drain_timeout`。
    ///
    /// 返回 `(已写出的字节数, 结果)`：出错时也带上已写出的长度，调用方据此只把
    /// **未写出**的部分退回队列——否则「部分写出后超时」会让已发出的前缀在下次重试时
    /// 再发一遍，子应用于是收到两条应答。
    ///
    /// [`crate::terminal::session::PtyMaster::write_with_timeout`] 丢掉该计数（用户输入
    /// 不容忍重发、也不重试），而 VT 应答的续写队列要用它。
    ///
    /// `refresh_budget_on_progress` 为假时 `drain_timeout` 是**总**预算而非每次部分写
    /// 都重新计的续额：逐帧回写 VT 应答全程持会话锁，给它续命等于取消封顶。
    ///
    /// 超时时长由调用方给：用户输入经单线程发送器写出（可以等满用户输入的排空上限
    /// 5s，等的是子进程消费），而渲染线程收割的 VT 应答（DECRPM/DSR/DA）只有几十
    /// 字节，却会连带冻结按帧取会话锁的渲染与看门狗——故那条路径给一个帧级短期限
    /// （见 `WRITE_BACK_TIMEOUT`）。
    fn write_all(
        &self,
        mut buf: &[u8],
        drain_timeout: std::time::Duration,
        refresh_budget_on_progress: bool,
    ) -> (usize, io::Result<()>) {
        let total = buf.len();
        let mut written = 0usize;
        let mut deadline = std::time::Instant::now() + drain_timeout;
        while !buf.is_empty() {
            match self.write(buf) {
                Ok(0) => {
                    // 非阻塞主端只会以 EAGAIN 失败、绝不会返回 0，但测试替身或异常后端可能：
                    // 在 0 上死循环比一次虚假的 WouldBlock 更糟。
                    return (written, Err(io::Error::from(io::ErrorKind::WouldBlock)));
                }
                Ok(bytes_written) => {
                    buf = &buf[bytes_written..];
                    written += bytes_written;
                    // 已推进：默认重新给子进程整个宽限期，慢速消费者不被累计等待
                    // 误伤。逐帧回写 VT 应答的那条路 MUST NOT 续命——调用方给的是
                    // 一个绝对墙钟预算（每帧至多 WRITE_BACK_TIMEOUT），在每次部分写
                    // 后重置等于给「每个期限前吃一口」的消费者开了无上限的窗口，
                    // 渲染与看门狗仍会被按住。
                    if refresh_budget_on_progress {
                        deadline = std::time::Instant::now() + drain_timeout;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    // PTY 输入缓冲已满。**绝不丢弃剩余字节**：丢弃会让粘贴被静默截断
                    // （子进程执行半条命令），也会把 VT 应答截断在转义序列中间，
                    // 使子应用永久等待一个永远不完整的回答。等子进程消费即可。
                    let Some(master) = self.master_borrowed_fd() else {
                        // 没有可轮询的真实主端（测试替身）：等满期限即报未排空，
                        // 与真实主端排不空时的结果同形。
                        std::thread::sleep(
                            deadline.saturating_duration_since(std::time::Instant::now()),
                        );
                        return (written, Err(not_drained()));
                    };
                    if let Err(error) = wait_writable(master, deadline) {
                        return (written, Err(error));
                    }
                }
                // EINTR 不是失败：信号打断了 write。resize 的 SIGWINCH 与子进程退出
                // 的 SIGCHLD 都走这条，于是一次击键会被报成「写入失败、已丢弃」，
                // 而字节其实从未送出。直接重试，且**不动**期限——信号不算等待。
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return (written, Err(error)),
            }
        }
        (total, Ok(()))
    }
}

/// 给一组 fd 置 `FD_CLOEXEC`。
///
/// `exec` 后仍存活的 fd 会泄漏给子进程：pty 主端泄漏会让 shell 子进程继续持有终端，
/// 父进程退出的 EOF 永远到不了 shell；回报管道写端泄漏则让父进程永远等不到 EOF，
/// 把「exec 成功」误判成「exec 失败」。
fn set_cloexec(fds: &[&OwnedFd]) -> Result<(), PtyError> {
    for fd in fds {
        nix::fcntl::fcntl(
            *fd,
            nix::fcntl::FcntlArg::F_SETFD(nix::fcntl::FdFlag::FD_CLOEXEC),
        )
        .map_err(|errno| PtyError::Open(std::io::Error::from_raw_os_error(errno as i32)))?;
    }
    Ok(())
}

/// 「等满期限仍未排空」的错误。真实主端与测试替身两条路径共用同一措辞。
fn not_drained() -> io::Error {
    io::Error::new(io::ErrorKind::WouldBlock, "PTY 输入缓冲在等待后仍未排空")
}

/// 等待主端变为可写直至 `deadline`。
fn wait_writable(fd: BorrowedFd<'_>, deadline: std::time::Instant) -> io::Result<()> {
    loop {
        let now = std::time::Instant::now();
        if now >= deadline {
            return Err(not_drained());
        }
        let remaining = (deadline - now).as_millis().min(i32::MAX as u128) as i32;
        // 夹到 `0..=i32::MAX` 之后转换不会失败。此前用 `unwrap_or(PollTimeout::MAX)`
        // 兜底：真走到那里就会把一次「已到期」翻译成**无限等待**，正是最坏的掩盖方向。
        let timeout =
            PollTimeout::try_from(remaining).expect("remaining is clamped to 0..=i32::MAX");
        let mut poll_fds = [PollFd::new(fd, PollFlags::POLLOUT)];
        match poll(&mut poll_fds, timeout) {
            // 0 = 期限内仍不可写，交给下一轮重试（由 deadline 收敛）。
            Ok(0) => continue,
            Ok(_) => {
                // 必须查 revents：主端挂断（子进程退出）时 poll 立刻返回 > 0，但置的是
                // POLLHUP/POLLERR 而非 POLLOUT。无条件 Ok(()) 会让调用方重入 write 拿到
                // EAGAIN → 再次 wait_writable → 立刻返回，整段排空期限变成满速空转，
                // 既烧 CPU 又拖住持锁的调用方。
                let revents = poll_fds[0].revents().unwrap_or_else(PollFlags::empty);
                let fatal =
                    revents & (PollFlags::POLLERR | PollFlags::POLLNVAL | PollFlags::POLLHUP);
                if fatal != PollFlags::empty() {
                    return Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "PTY 主端已挂断，无法继续写入",
                    ));
                }
                return Ok(());
            }
            // EINTR 不是失败：信号打断了 poll，等下去即可。
            Err(Errno::EINTR) => continue,
            Err(errno) => return Err(io::Error::from_raw_os_error(errno as i32)),
        }
    }
}

/// PTY 对：主端文件描述符 + 子进程。主端供终端模拟器读输出、写输入；
/// 子进程运行 shell 并经从端通信。
pub struct PtyPair {
    master: OwnedFd,
    child_pid: nix::unistd::Pid,
    /// 原子而非 `Cell`：后者 `Send` 但不 `Sync`，会让整个 `PtyPair`（进而整个
    /// `Pty` 替身）无法跨线程共享——`resize`/`set_pixel_size`/`child_pid` 都是
    /// `&self` 方法，本可完全无锁，却被迫与无上界的写入争同一把互斥量。
    pixel_width: AtomicU16,
    pixel_height: AtomicU16,
    /// execve 失败回报管道的读端：写端带 FD_CLOEXEC，execve 成功即随 exec 关闭，
    /// 父进程读到 EOF；失败时子进程写入 errno 后退出，父进程读到确切 errno。
    ///
    /// 取走（[`PtyPair::take_exec_failure_reader`]）后由等待线程在子进程退出后读取。
    exec_failure_reader: Option<OwnedFd>,
}

impl PtyPair {
    /// 在新 PTY 中启动子进程。
    ///
    /// 异步信号安全：`fork()` 后子进程只能调用异步信号安全函数，所有堆分配都在
    /// fork **之前**完成，子进程路径只用 `execvp`/`dup2`/`close`/`write(2)`/`_exit(2)`。
    /// **切勿在 `fork()` 之后的子进程分支中添加 `log::debug!`、`format!` 或任何分配。**
    ///
    /// TIOCSWINSZ 种子取调用方给的 rows/cols（测试与默认会话均以 24x80 调用，
    /// 由 `spawn_seeds_24x80_winsize` 验证），使 shell 在首次 resize 前不会看到 0x0。
    /// execve 失败时子进程把确切 errno 写进回报管道，等待线程经本模块的
    /// `read_exec_failure_errno` 取走——退出码不作判定依据：正常收尾时 shell 的
    /// 退出码与旧编码撞车。
    pub fn spawn(
        shell: &str,
        rows: u16,
        cols: u16,
        env: &ShellEnv,
        cwd: Option<&Path>,
    ) -> Result<Self, PtyError> {
        let winsize = nix::pty::Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        let result = nix::pty::openpty(Some(&winsize), None)
            .map_err(|pty_error| PtyError::Open(std::io::Error::other(pty_error)))?;
        let master_fd = result.master;
        let slave_fd = result.slave;
        // openpty 不保证 FD_CLOEXEC：exec 后泄漏的 pty 主 fd 会让 shell 子进程
        // 继续持有终端，父进程退出的 EOF 永远到不了 shell。
        set_cloexec(&[&master_fd, &slave_fd])?;

        // execve 失败回报管道：写端带 FD_CLOEXEC，execve 成功时随 exec 关闭，
        // 父进程读到 EOF 即「exec 成功」；失败时子进程把 errno 写进去。
        //
        // 退出码承担不了这件事：shell 被 SIGHUP 杀掉时退出码是 129，与「errno=29」
        // 的编码撞车，会把正常收尾误报成 execve 失败。管道给的是确切 errno。
        let (exec_failure_reader, exec_failure_writer) = nix::unistd::pipe()
            .map_err(|errno| PtyError::Open(std::io::Error::from_raw_os_error(errno as i32)))?;
        set_cloexec(&[&exec_failure_reader, &exec_failure_writer])?;

        // fork 之前构建好子进程的全部数据，避免子进程中分配
        // （多线程进程中 fork 可能破坏 malloc 堆）。
        // 空 shell 早于 fork 就拒绝：`split_shell_entry("")` 会产出 `("", [])`，
        // 随后 `execve("")` 失败，errno 是 ENOENT——用户看到的是「空会话 + 一个查不到
        // 文件的错误」，既不像「没配置 shell」，也不指向真正的配置项。明确报错才可定位。
        if shell.trim_ascii().is_empty() {
            return Err(PtyError::EmptyShell);
        }
        let (shell_executable, shell_argument_texts) = split_shell_entry(shell);
        let shell_cstr = std::ffi::CString::new(shell_executable).map_err(|null_error| {
            let message = format!("shell path contains null byte: {null_error}");
            log::error!("{message}");
            PtyError::Fork(nix::errno::Errno::EINVAL)
        })?;
        let shell_argument_cstrs: Vec<std::ffi::CString> = shell_argument_texts
            .into_iter()
            .map(|argument| {
                std::ffi::CString::new(argument).map_err(|_| {
                    log::error!("shell argument contains null byte");
                    PtyError::Fork(nix::errno::Errno::EINVAL)
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let env_cstrings: Vec<std::ffi::CString> = build_env(env)
            .into_iter()
            .chain(
                title_report_hook(shell_executable)
                    .map(|(name, value)| (name.to_string(), value.to_string())),
            )
            .map(|(key, value)| {
                std::ffi::CString::new(format!("{key}={value}")).map_err(|null_error| {
                    let message = format!("env var contains null byte: {null_error}");
                    log::error!("{message}");
                    PtyError::Fork(nix::errno::Errno::EINVAL)
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let chdir_target: &str = cwd.map_or(env.working_directory.as_str(), |path| {
            path.to_str().unwrap_or(env.working_directory.as_str())
        });
        let working_directory_cstr =
            std::ffi::CString::new(chdir_target).map_err(|null_error| {
                let message = format!("working directory contains null byte: {null_error}");
                log::error!("{message}");
                PtyError::Fork(nix::errno::Errno::EINVAL)
            })?;

        // fork 之前预分配参数与环境数组；fork 之后子进程不得调用任何分配函数。
        // Android 15+ 的 SELinux 对 untrusted_app 拒绝 app_data_file 上的
        // `execute_no_trans`，故直接 execve $PREFIX 下的二进制会 EACCES。解法是 exec
        // 系统 linker 并把 ELF 路径作为其参数（`/system/bin/linker64 $PREFIX/bin/bash`）
        // ——linker 运行于 system_linker_exec 域，能正常加载 app-data 中的 ELF。
        // 子进程不设置 `LD_PRELOAD`，仅走 linker 间接。
        let prefix = env.prefix.as_deref().unwrap_or("");
        let use_linker = !prefix.is_empty()
            && shell_executable.starts_with(&format!("{prefix}/"))
            && !shell_executable.contains('\0')
            && is_pie(shell_executable);
        if use_linker {
            log::info!("SPAWN_LINKER: shell={shell} prefix={prefix}");
        } else if !prefix.is_empty() && shell_executable.starts_with(&format!("{prefix}/")) {
            log::info!("SPAWN_DIRECT_ET_EXEC: shell={shell} (non-PIE, skip linker)");
        } else {
            log::info!("SPAWN_DIRECT: shell={shell} prefix={prefix:?}");
        }
        let linker_cstr = if use_linker {
            let linker = if cfg!(any(target_arch = "aarch64", target_arch = "x86_64")) {
                "/system/bin/linker64"
            } else {
                "/system/bin/linker"
            };
            Some(
                std::ffi::CString::new(linker)
                    .map_err(|_| PtyError::Fork(nix::errno::Errno::EINVAL))?,
            )
        } else {
            None
        };
        let script_dispatch: Option<(std::ffi::CString, Option<std::ffi::CString>)> =
            if linker_cstr.is_none() {
                read_shebang_interpreter(shell_executable)
            } else {
                None
            };
        if let Some((interpreter, _)) = &script_dispatch {
            log::info!(
                "SPAWN_SCRIPT: shell={shell} interpreter={}",
                interpreter.to_string_lossy()
            );
        }
        let shell_ptr = shell_cstr.as_ptr();
        let script_linker_cstr: Option<std::ffi::CString> = match &script_dispatch {
            Some((interpreter, _)) => {
                let interpreter_text = interpreter.to_string_lossy();
                let interpreter_under_prefix =
                    !prefix.is_empty() && interpreter_text.starts_with(&format!("{prefix}/"));
                if interpreter_under_prefix && is_pie(&interpreter_text) {
                    let linker = if cfg!(any(target_arch = "aarch64", target_arch = "x86_64")) {
                        "/system/bin/linker64"
                    } else {
                        "/system/bin/linker"
                    };
                    log::info!("SPAWN_SCRIPT_LINKER: shell={shell} interpreter={interpreter_text}");
                    Some(
                        std::ffi::CString::new(linker)
                            .map_err(|_| PtyError::Fork(nix::errno::Errno::EINVAL))?,
                    )
                } else {
                    None
                }
            }
            None => None,
        };
        let exec_path_ptr = if let Some(linker_cstr) = linker_cstr.as_ref() {
            linker_cstr.as_ptr()
        } else if let Some((interpreter, _)) = script_dispatch.as_ref() {
            script_linker_cstr
                .as_ref()
                .map_or(interpreter.as_ptr(), |candidate| candidate.as_ptr())
        } else {
            shell_ptr
        };
        let working_directory_ptr = working_directory_cstr.as_ptr();
        let args_ptrs: Vec<*const libc::c_char> = if let Some(linker_cstr) = &linker_cstr {
            let mut args = Vec::with_capacity(4 + shell_argument_cstrs.len());
            args.push(linker_cstr.as_ptr());
            args.push(shell_ptr);
            for argument in &shell_argument_cstrs {
                args.push(argument.as_ptr());
            }
            args.push(std::ptr::null());
            args
        } else if let Some((interpreter, interpreter_argument)) = &script_dispatch {
            let mut args = Vec::with_capacity(6 + shell_argument_cstrs.len());
            if let Some(script_linker) = &script_linker_cstr {
                args.push(script_linker.as_ptr());
            }
            args.push(interpreter.as_ptr());
            if let Some(argument) = interpreter_argument {
                args.push(argument.as_ptr());
            }
            args.push(shell_ptr);
            for argument in &shell_argument_cstrs {
                args.push(argument.as_ptr());
            }
            args.push(std::ptr::null());
            args
        } else {
            let mut args = Vec::with_capacity(3 + shell_argument_cstrs.len());
            args.push(shell_ptr);
            for argument in &shell_argument_cstrs {
                args.push(argument.as_ptr());
            }
            args.push(std::ptr::null());
            args
        };
        let env_ptrs: Vec<*const libc::c_char> = env_cstrings
            .iter()
            .map(|cstring| cstring.as_ptr())
            .chain(std::iter::once(std::ptr::null()))
            .collect();

        // SAFETY: `fork()` 之所以不安全在于它创建了新进程。子进程调用 `execve()` 替换
        // 进程映像（fork 之后不使用任何堆数据——数据全部预先分配，且在 execve 前重置
        // 信号处理器）。fork 与 exec 之间不运行任何信号处理器（所有操作均为异步信号
        // 安全系统调用）。父进程检查 `ForkResult` 返回值并经 `?` 处理错误。
        match unsafe { nix::unistd::fork()? } {
            nix::unistd::ForkResult::Parent { child } => {
                if let Err(error) = nix::unistd::close(slave_fd) {
                    log::warn!("failed to close PTY slave fd in parent after fork: {error}");
                }
                // 父进程只留读端：写端留着则子进程 exec 成功后管道也不 EOF，
                // 等待线程会一直阻塞在读上。
                drop(exec_failure_writer);
                Ok(Self {
                    master: master_fd,
                    child_pid: child,
                    pixel_width: AtomicU16::new(0),
                    pixel_height: AtomicU16::new(0),
                    exec_failure_reader: Some(exec_failure_reader),
                })
            }
            nix::unistd::ForkResult::Child => {
                if let Err(error) = nix::unistd::close(master_fd) {
                    // execve 前 close(2) 失败无害，静默忽略
                    // （fork 后不允许分配/记日志）。
                    let _ = error;
                }
                // 子进程只留写端：读端留着则父进程永远拿不到 EOF。
                drop(exec_failure_reader);
                let slave_raw = slave_fd.as_raw_fd();
                // SAFETY: `getsid(0)` 是返回调用进程会话 ID 的 POSIX 函数。传 0 始终
                // 有效，在 fork 后的单线程子进程中安全。
                let is_session_leader =
                    unsafe { libc::getsid(0) } == nix::unistd::getpid().as_raw();
                if !is_session_leader && nix::unistd::setsid().is_err() {
                    child_exit_with_reason(slave_raw, "terminal: setsid() failed\n", 2);
                }
                // 检测 fork 时的孤儿：若应用进程在 fork() 与本检查之间死亡，
                // 子进程已被重新托管给 init（PPid == 1），此时退出而非泄漏永久孤儿。
                // 注意：**故意不用** PR_SET_PDEATHSIG——它绑定的是 fork 线程
                // （Kotlin Dispatchers.IO 的 worker），该线程在空闲约 60s 后超时退出
                // （kotlinx KEEP_ALIVE），shell 会被父进程死亡信号杀死（模拟器
                // Android 15 GKI 6.6 实测：每个会话启动约 60s 后死亡，禁用 PDEATHSIG
                // 后 shell 无限存活）。应用死亡清理由 Android 的 cgroup.kill 负责
                // （AMS/lmkd 在强制停止、崩溃与 OOM 时杀掉应用的进程组，真机实测
                // 强制停止能回收 shell），故 PDEATHSIG 既多余又有害。
                // SAFETY: `getppid` 是普通系统调用，fork 后安全。
                let orphaned = unsafe { libc::getppid() } == 1;
                if orphaned {
                    child_exit_with_reason(
                        slave_raw,
                        "terminal: app died during fork; child is orphaned\n",
                        1,
                    );
                }
                // SAFETY: 这些 libc 调用都是不分配的轻量系统调用包装。子进程是单线程的；
                // fork 与 exec 之间不运行信号处理器（所有操作均为异步信号安全系统调用）。
                let result = unsafe { libc::ioctl(slave_raw, libc::TIOCSCTTY, 0) };
                if result < 0 {
                    child_exit_with_reason(
                        slave_raw,
                        "terminal: TIOCSCTTY failed; no controlling terminal\n",
                        3,
                    );
                }
                // SAFETY: 在标准 fd（0、1、2）上执行 dup2 在 fork 后安全且异步信号安全。
                // 从端 fd 有效，因为上方的 `setsid()` + `ioctl(TIOCSCTTY)` 已把它指定为
                // 控制终端（`login_tty` 的手工替代方案）。
                unsafe {
                    libc::dup2(slave_raw, 0);
                    libc::dup2(slave_raw, 1);
                    libc::dup2(slave_raw, 2);
                }
                if slave_raw > 2 {
                    // SAFETY: 仅当 `slave_raw` 不是标准 fd（0、1、2）时才关闭，
                    // 确保不会误关关键 fd。
                    unsafe {
                        libc::close(slave_raw);
                    }
                }
                // SAFETY: `tcgetattr`/`tcsetattr` 是轻量系统调用包装，在 fork 后的
                // 单线程子进程中安全。
                configure_raw_mode_child(libc::STDIN_FILENO);
                // SAFETY: 传入有效且 NUL 结尾的路径字符串时 `chdir` 安全。
                // `working_directory_ptr` 由 `CString::new()` 构造，保证 NUL 结尾。
                // 失败不致命（默认回退到 `/`）。
                if unsafe { libc::chdir(working_directory_ptr) } != 0 {
                    const MSG: &[u8] = b"chdir to working directory failed, using /\n";
                    // SAFETY: `write(2)` 在 fork 后的单线程子进程中异步信号安全。
                    // `MSG` 是静态字节切片（不分配）。
                    let _ =
                        unsafe { libc::write(2, MSG.as_ptr() as *const libc::c_void, MSG.len()) };
                }
                // SAFETY: `signal()` 在 fork 后的单线程子进程中安全。
                // 复位为 `SIG_DFL` 可防止父进程自定义信号处理器泄漏。
                unsafe {
                    libc::signal(libc::SIGCHLD, libc::SIG_DFL);
                    libc::signal(libc::SIGHUP, libc::SIG_DFL);
                    libc::signal(libc::SIGINT, libc::SIG_DFL);
                    libc::signal(libc::SIGQUIT, libc::SIG_DFL);
                    libc::signal(libc::SIGTERM, libc::SIG_DFL);
                    libc::signal(libc::SIGPIPE, libc::SIG_DFL);
                    libc::signal(libc::SIGALRM, libc::SIG_DFL);
                }
                close_stray_fds(exec_failure_writer.as_raw_fd());
                // SAFETY: 传入预先分配且 NUL 结尾的数组时 `execve` 安全。
                // `shell_ptr`、`args_ptrs`、`env_ptrs` 均在 fork() 之前经
                // `CString::new()` 与 `CString::as_ptr()` 构造，保证指针有效。
                // `nix::unistd::execve` 内部经 `collect()` 分配，在多线程进程 fork 后
                // 不安全——故直接调用 libc。
                // SAFETY: 使用预分配数组的 `execve`（见上）。
                // `use_linker` 时 `exec_path_ptr` 是 linker，否则是 shell。
                unsafe {
                    libc::execve(exec_path_ptr, args_ptrs.as_ptr(), env_ptrs.as_ptr());
                }
                unsafe {
                    // `nix::errno::errno()` 读取线程局部 errno（不分配），底层依平台而定。
                    let errno = nix::errno::Errno::last_raw();
                    let mut buf = [0u8; 64];
                    let prefix = b"execve failed: errno=";
                    buf[..prefix.len()].copy_from_slice(prefix);
                    let mut write_index = prefix.len();
                    let mut errno_value = errno as u32;
                    let mut digits = [0u8; 10];
                    let mut digit_index = 0;
                    if errno_value == 0 {
                        digits[digit_index] = b'0';
                        digit_index += 1;
                    }
                    while errno_value > 0 {
                        digits[digit_index] = b'0' + (errno_value % 10) as u8;
                        errno_value /= 10;
                        digit_index += 1;
                    }
                    while digit_index > 0 {
                        digit_index -= 1;
                        buf[write_index] = digits[digit_index];
                        write_index += 1;
                    }
                    buf[write_index] = b'\n';
                    write_index += 1;
                    let _ = libc::write(2, buf.as_ptr() as *const libc::c_void, write_index);
                    // 把确切 errno 写回父进程：等待线程据此判定 execve 真失败。
                    // 退出码承担不了这件事——shell 被 SIGHUP 杀掉时退出码 129 与
                    // 「errno=29」的编码撞车，会把正常收尾误报成 execve 失败。
                    let errno_bytes = errno.to_ne_bytes();
                    // SAFETY: `write(2)` 在 fork 后的单线程子进程中异步信号安全；
                    // `errno_bytes` 是栈上定长数组，不触碰堆。
                    let _ = libc::write(
                        exec_failure_writer.as_raw_fd(),
                        errno_bytes.as_ptr() as *const libc::c_void,
                        errno_bytes.len(),
                    );
                    // 退出码仍按 100 + errno 编码（errno 上界 133，即 EHWPOISON），
                    // 供 UI 显示；它不再是判定 execve 失败的依据。
                    let code = 100 + errno.min(133);
                    libc::_exit(code);
                }
            }
        }
    }

    pub fn child_pid(&self) -> nix::unistd::Pid {
        self.child_pid
    }

    /// 取走 execve 失败回报管道的读端。
    ///
    /// 由等待线程在子进程退出后读取：读到的 errno 即 execve 的确切失败原因，
    /// 读不到（EOF）即 execve 成功。
    pub fn take_exec_failure_reader(&mut self) -> Option<OwnedFd> {
        self.exec_failure_reader.take()
    }

    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), PtyError> {
        let winsize = nix::pty::Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: self.pixel_width.load(Ordering::Acquire),
            ws_ypixel: self.pixel_height.load(Ordering::Acquire),
        };
        self.write_winsize(&winsize)
    }

    /// 更新缓存的像素尺寸并经 TIOCSWINSZ（ws_xpixel/ws_ypixel）推送给内核，
    /// 使全屏程序看到真实像素尺寸而非 0。TIOCSWINSZ 会替换整个结构体，
    /// 故先读回并保留当前 rows/cols。
    pub fn set_pixel_size(&self, width: u16, height: u16) -> Result<(), PtyError> {
        self.pixel_width.store(width, Ordering::Release);
        self.pixel_height.store(height, Ordering::Release);
        let mut winsize = self.read_winsize()?;
        winsize.ws_xpixel = width;
        winsize.ws_ypixel = height;
        self.write_winsize(&winsize)
    }

    /// 经 TIOCGWINSZ 查询当前终端窗口尺寸，用于在任何 resize 之前验证 24x80 spawn 种子。
    pub fn get_winsize(&self) -> Result<(u16, u16), PtyError> {
        let winsize = self.read_winsize()?;
        Ok((winsize.ws_row, winsize.ws_col))
    }

    /// 经 TIOCGWINSZ 从内核读取当前终端窗口尺寸。ioctl 会填充格式良好的 Winsize 结构；
    /// fd 为自有且有效，结构在使用前已完全初始化。
    fn read_winsize(&self) -> Result<nix::pty::Winsize, PtyError> {
        let mut winsize = nix::pty::Winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: TIOCGWINSZ 的 ioctl 从内核填充格式良好的 Winsize 结构；
        // fd 自有且有效，结构在使用前已完全初始化。
        unsafe {
            let result = libc::ioctl(
                self.master.as_raw_fd(),
                libc::TIOCGWINSZ,
                std::ptr::from_mut(&mut winsize),
            );
            if result < 0 {
                return Err(PtyError::Resize(nix::errno::Errno::last()));
            }
        }
        Ok(winsize)
    }

    /// 经 TIOCSWINSZ 把 Winsize 结构推送给内核。ioctl 会把 winsize 复制到从端；
    /// fd 自有且有效，返回值已做错误检查。
    fn write_winsize(&self, winsize: &nix::pty::Winsize) -> Result<(), PtyError> {
        // SAFETY: TIOCSWINSZ 的 ioctl 向 PTY 主端 fd 写入格式良好的 Winsize 结构。
        // fd 自有且有效。内核把 winsize 复制到从端——无内存安全风险。返回值已做错误检查。
        unsafe {
            let result = libc::ioctl(
                self.master.as_raw_fd(),
                libc::TIOCSWINSZ,
                std::ptr::from_ref(winsize),
            );
            if result < 0 {
                return Err(PtyError::Resize(nix::errno::Errno::last()));
            }
        }
        Ok(())
    }

    pub fn set_nonblocking(&self) -> Result<(), PtyError> {
        let flags = nix::fcntl::fcntl(&self.master, nix::fcntl::FcntlArg::F_GETFL)
            .map_err(PtyError::Fcntl)?;
        let new_flags =
            nix::fcntl::OFlag::from_bits_truncate(flags) | nix::fcntl::OFlag::O_NONBLOCK;
        nix::fcntl::fcntl(&self.master, nix::fcntl::FcntlArg::F_SETFL(new_flags))
            .map_err(PtyError::Fcntl)?;
        Ok(())
    }
}

impl Pty for PtyPair {
    fn write(&self, buf: &[u8]) -> io::Result<usize> {
        nix::unistd::write(&self.master, buf)
            .map_err(|errno| io::Error::from_raw_os_error(errno as i32))
    }

    fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        nix::unistd::read(&self.master, buf)
            .map_err(|errno| io::Error::from_raw_os_error(errno as i32))
    }

    fn resize(&self, rows: u16, cols: u16) -> Result<(), PtyError> {
        PtyPair::resize(self, rows, cols)
    }

    fn set_pixel_size(&self, width: u16, height: u16) -> Result<(), PtyError> {
        PtyPair::set_pixel_size(self, width, height)
    }

    fn get_winsize(&self) -> Result<(u16, u16), PtyError> {
        PtyPair::get_winsize(self)
    }

    fn child_pid(&self) -> nix::unistd::Pid {
        PtyPair::child_pid(self)
    }

    fn master_borrowed_fd(&self) -> Option<BorrowedFd<'_>> {
        Some(self.master.as_fd())
    }

    fn try_clone_reader_fd(&self) -> io::Result<OwnedFd> {
        self.master.try_clone()
    }

    fn set_nonblocking(&self) -> Result<(), PtyError> {
        PtyPair::set_nonblocking(self)
    }
}

impl Drop for PtyPair {
    fn drop(&mut self) {
        // `Session::drop` 先投递信号再由 wait 线程回收；此处只兜住
        // `Session::spawn` 中 fork 成功后的错误路径——那些路径上 `Session`
        // 从未构造，没有 wait 线程，子进程会永久滞留。
        //
        // 先 WNOHANG 查询而非直接 kill：`Session::drop` 的正常路径已由 wait
        // 线程回收，此时 pid 可能已被系统复用，直接 kill 会误杀无关进程。
        // 只有仍处于「未回收且存活」状态才动手。
        let pid = self.child_pid;
        if !matches!(
            nix::sys::wait::waitpid(pid, Some(nix::sys::wait::WaitPidFlag::WNOHANG)),
            Ok(nix::sys::wait::WaitStatus::StillAlive)
        ) {
            return;
        }
        // 直杀子进程：错误路径上 shell 尚未 fork 任何子进程，且组杀会因
        // setsid 前的同组窗口连带杀死本进程（见 `Session::drop` 的自杀 guard）。
        let _ = nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGKILL);
        if let Err(error) = nix::sys::wait::waitpid(pid, None)
            && error != nix::errno::Errno::ECHILD
        {
            log::warn!("pty drop: waitpid({}) failed: {error}", pid.as_raw());
        }
    }
}

#[cfg(test)]
fn configure_raw_mode(fd: std::os::unix::io::RawFd) -> Result<(), PtyError> {
    let mut termios = std::mem::MaybeUninit::<libc::termios>::uninit();
    // SAFETY: 传入有效 fd 时 `tcgetattr` 安全。调用方必须传入自己持有的终端 fd
    // （测试传 PTY 主端；生产路径在 login 后传从端）。
    let result = unsafe { libc::tcgetattr(fd, termios.as_mut_ptr()) };
    if result != 0 {
        return Err(PtyError::Termios(nix::errno::Errno::last()));
    }
    // SAFETY: 上方已检查 `tcgetattr` 返回 0，保证 termios 已被内核初始化，
    // 故 `assume_init()` 安全。
    let mut termios = unsafe { termios.assume_init() };
    // 沿用 termux-app 的既定做法：关闭软件流控（IXON/IXOFF，置位时内核会把
    // Ctrl+S/Ctrl+Q 当作冻结/恢复输出，使终端看似卡死），并开启 IUTF8 让内核
    // 正确处理 UTF-8 擦除/词擦除与字符宽度。
    // 注意：**故意不**清除 ECHO/ICANON/ISIG——全量 raw 掩码会破坏 bash readline
    // 的回显与信号设置（完整分析见 `configure_raw_mode_child`）。
    termios.c_iflag &= !(libc::IXON | libc::IXOFF);
    termios.c_iflag |= libc::IUTF8;
    log::debug!(
        "configuring PTY termios: IUTF8 set={}, IXON disabled={}, IXOFF disabled={}",
        (termios.c_iflag & libc::IUTF8) != 0,
        (termios.c_iflag & libc::IXON) == 0,
        (termios.c_iflag & libc::IXOFF) == 0,
    );
    // SAFETY: fd 有效且 termios 结构有效时 `tcsetattr` 安全。
    let result = unsafe { libc::tcsetattr(fd, libc::TCSANOW, &termios) };
    if result != 0 {
        return Err(PtyError::Termios(nix::errno::Errno::last()));
    }
    Ok(())
}

/// 读回子进程报告的 execve 失败 errno。
///
/// 必须在子进程退出后读：execve 成功时写端随 exec 关闭，读到 EOF（无失败）；
/// 失败时子进程先写入 errno 再退出，故此时管道里必有那 4 个字节。
pub(crate) fn read_exec_failure_errno(reader: &OwnedFd) -> Option<i32> {
    let mut buf = [0u8; 4];
    match nix::unistd::read(reader, &mut buf) {
        Ok(4) => Some(i32::from_ne_bytes(buf)),
        Ok(_) => None,
        Err(errno) => {
            log::debug!("read exec failure pipe failed: {errno}");
            None
        }
    }
}

#[cfg(test)]
mod tests;
