//! PTY 主/从端创建：fork 与直接系统调用处允许 unsafe，VT/网格数据路径禁用 unsafe。
use std::cell::Cell;
use std::io;
use std::os::unix::io::{AsRawFd, OwnedFd, RawFd};
use std::path::Path;
#[cfg(test)]
use std::time::Duration;

use thiserror::Error;

use crate::terminal::shell_env::ShellEnv;

const DEFAULT_TERM: &str = "xterm-256color";
const DEFAULT_COLORTERM: &str = "truecolor";
const DEFAULT_LANG: &str = "en_US.UTF-8";
const TERMUX_VERSION: &str = "0.119.0-beta.3";

/// PTY 输入缓冲排空的等待上限。
///
/// 交互式 shell 读走一个缓冲只需微秒级，故正常粘贴永不触及该上限；它只在子进程
/// 长时间不读 stdin 时收敛，使调用方收到**错误**而不是被静默截断的输入。
/// 取值远大于任何合理的交互延迟，又短于调用线程可接受的卡顿上限。
const WRITE_DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

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

pub trait Pty: Send {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize>;
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
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
    fn master_fd(&self) -> RawFd;
    /// 返回独立所有的主端 fd 副本供专用读取线程使用。副本与 `master_fd()` 共享同一
    /// 打开文件描述（故共享 O_NONBLOCK 状态），这对 `poll` + 类阻塞读取的读取线程无碍。
    /// dup 在此处（允许 `unsafe` 的位置）完成，使调用方能经安全的 `std::fs::File`
    /// 读取而无需任何 `unsafe` 块。
    fn try_clone_reader_fd(&self) -> io::Result<OwnedFd>;
    fn set_nonblocking(&self) -> Result<(), PtyError>;
    fn spawn(
        shell: &str,
        rows: u16,
        cols: u16,
        env: &ShellEnv,
        cwd: Option<&Path>,
    ) -> Result<Box<dyn Pty>, PtyError>
    where
        Self: Sized;

    fn write_all(&mut self, mut buf: &[u8]) -> io::Result<()> {
        let mut deadline = std::time::Instant::now() + WRITE_DRAIN_TIMEOUT;
        while !buf.is_empty() {
            match self.write(buf) {
                Ok(0) => {
                    // 非阻塞主端只会以 EAGAIN 失败、绝不会返回 0，但测试替身或异常后端可能：
                    // 在 0 上死循环比一次虚假的 WouldBlock 更糟。
                    return Err(io::Error::from(io::ErrorKind::WouldBlock));
                }
                Ok(bytes_written) => {
                    buf = &buf[bytes_written..];
                    // 已推进：重新给子进程整个宽限期，慢速消费者不被累计等待误伤。
                    deadline = std::time::Instant::now() + WRITE_DRAIN_TIMEOUT;
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    // PTY 输入缓冲已满。**绝不丢弃剩余字节**：丢弃会让粘贴被静默截断
                    // （子进程执行半条命令），也会把 VT 应答截断在转义序列中间，
                    // 使子应用永久等待一个永远不完整的回答。等子进程消费即可。
                    wait_writable(self.master_fd(), deadline)?;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}

/// 等待主端变为可写直至 [deadline]。
///
/// `unsafe`：仅调用 `poll`，其输入是本方法内构造的 `pollfd` 与调用方持有的有效
/// 主端 fd（`Pty` 契约保证 `master_fd` 在对象存活期间有效），`poll` 只写 `revents`。
fn wait_writable(fd: RawFd, deadline: std::time::Instant) -> io::Result<()> {
    loop {
        let now = std::time::Instant::now();
        if now >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "PTY 输入缓冲在等待后仍未排空",
            ));
        }
        let remaining = (deadline - now).as_millis().min(i32::MAX as u128) as i32;
        let mut poll_fd = libc::pollfd {
            fd,
            events: libc::POLLOUT,
            revents: 0,
        };
        // SAFETY: 见上方文档——`poll_fd` 有效且已初始化，`fd` 由 `Pty` 保证存活。
        let result = unsafe { libc::poll(&mut poll_fd as *mut libc::pollfd, 1, remaining) };
        if result > 0 {
            return Ok(());
        }
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        // 0 = 期限内仍不可写，交给下一轮重试（由 deadline 收敛）。
    }
}

/// PTY 对：主端文件描述符 + 子进程。主端供终端模拟器读输出、写输入；
/// 子进程运行 shell 并经从端通信。
pub struct PtyPair {
    master: OwnedFd,
    child_pid: nix::unistd::Pid,
    pixel_width: Cell<u16>,
    pixel_height: Cell<u16>,
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
    /// execve 的 errno 编码进退出码（100 + errno）由等待线程解码，在 logcat 中
    /// 比直接写 stderr 更易观测。
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
        for fd in [&master_fd, &slave_fd] {
            nix::fcntl::fcntl(
                fd,
                nix::fcntl::FcntlArg::F_SETFD(nix::fcntl::FdFlag::FD_CLOEXEC),
            )
            .map_err(|errno| PtyError::Open(std::io::Error::from_raw_os_error(errno as i32)))?;
        }

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
                .map_or(interpreter.as_ptr(), |c| c.as_ptr())
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
                if let Err(e) = nix::unistd::close(slave_fd) {
                    log::warn!("failed to close PTY slave fd in parent after fork: {e}");
                }
                Ok(Self {
                    master: master_fd,
                    child_pid: child,
                    pixel_width: Cell::new(0),
                    pixel_height: Cell::new(0),
                })
            }
            nix::unistd::ForkResult::Child => {
                if let Err(e) = nix::unistd::close(master_fd) {
                    // execve 前 close(2) 失败无害，静默忽略
                    // （fork 后不允许分配/记日志）。
                    let _ = e;
                }
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
                close_stray_fds();
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
                    let mut i = prefix.len();
                    let mut e = errno as u32;
                    let mut digits = [0u8; 10];
                    let mut d = 0;
                    if e == 0 {
                        digits[d] = b'0';
                        d += 1;
                    }
                    while e > 0 {
                        digits[d] = b'0' + (e % 10) as u8;
                        e /= 10;
                        d += 1;
                    }
                    while d > 0 {
                        d -= 1;
                        buf[i] = digits[d];
                        i += 1;
                    }
                    buf[i] = b'\n';
                    i += 1;
                    let _ = libc::write(2, buf.as_ptr() as *const libc::c_void, i);
                    // 把 errno 编码进退出码（>= 100），使父进程的等待线程即使在 PTY
                    // 输出因销毁竞态丢失时也能记录确切失败原因。
                    let code = 100 + errno.min(155);
                    libc::_exit(code);
                }
            }
        }
    }

    pub fn child_pid(&self) -> nix::unistd::Pid {
        self.child_pid
    }

    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), PtyError> {
        let winsize = nix::pty::Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: self.pixel_width.get(),
            ws_ypixel: self.pixel_height.get(),
        };
        self.write_winsize(&winsize)
    }

    /// 更新缓存的像素尺寸并经 TIOCSWINSZ（ws_xpixel/ws_ypixel）推送给内核，
    /// 使全屏程序看到真实像素尺寸而非 0。TIOCSWINSZ 会替换整个结构体，
    /// 故先读回并保留当前 rows/cols。
    pub fn set_pixel_size(&self, width: u16, height: u16) -> Result<(), PtyError> {
        self.pixel_width.set(width);
        self.pixel_height.set(height);
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

    pub fn master_fd(&self) -> std::os::unix::io::RawFd {
        self.master.as_raw_fd()
    }
}

impl Pty for PtyPair {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        nix::unistd::write(&self.master, buf)
            .map_err(|errno| io::Error::from_raw_os_error(errno as i32))
    }

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
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

    fn master_fd(&self) -> RawFd {
        PtyPair::master_fd(self)
    }

    fn try_clone_reader_fd(&self) -> io::Result<OwnedFd> {
        self.master.try_clone()
    }

    fn set_nonblocking(&self) -> Result<(), PtyError> {
        PtyPair::set_nonblocking(self)
    }

    fn spawn(
        shell: &str,
        rows: u16,
        cols: u16,
        env: &ShellEnv,
        cwd: Option<&Path>,
    ) -> Result<Box<dyn Pty>, PtyError> {
        PtyPair::spawn(shell, rows, cols, env, cwd)
            .map(|pty_pair| Box::new(pty_pair) as Box<dyn Pty>)
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

/// fork 后子进程的终端模式配置，对齐 Termux 的做法：PTY 保留内核行规程
/// （ECHO+ICANON 开启），由 shell 自己的行编辑器（bash readline）负责回显，
/// 全屏程序（vim、less）启动时自行把 tty 切到 raw 模式。
///
/// 在此完整执行 cfmakeraw()（关闭 ECHO|ICANON|ISIG）会破坏 bash readline：实测输入
/// 的字符能到达 shell（命令得以执行）却完全无回显——readline 继承的 termios 中 ECHO
/// 已被清除时不会重新开启，且 ISIG 关闭时它会跳过信号设置。
///
/// 异步信号安全：不分配、不调用 `log::warn!`。失败时写 fd 2（此时 fd 2 仍是应用
/// 的 stderr，送进 logcat），与 fork 子进程的 `child_exit_with_reason` 一致——错误
/// 自述自理会静默吞掉 termios 配置失败，让"子进程拿到的不是预期模式"变成不可定位的
/// 罕见会话问题。配置失败不致命，仅告知后继续。
fn configure_raw_mode_child(fd: std::os::unix::io::RawFd) {
    let mut termios = std::mem::MaybeUninit::<libc::termios>::uninit();
    // SAFETY: `tcgetattr` 是系统调用包装，在 fork 后的单线程子进程中安全。
    if unsafe { libc::tcgetattr(fd, termios.as_mut_ptr()) } != 0 {
        child_note_fd2(b"pty: child tcgetattr failed\n");
        return; // 不致命
    }
    // SAFETY: `tcgetattr` 成功后 `assume_init()` 安全。
    let mut termios = unsafe { termios.assume_init() };
    // 对齐 termux.c：开启 UTF-8 模式，关闭流控以免 Ctrl+S 锁住显示。
    // 其余（ECHO、ICANON、ISIG、OPOST）保持内核默认。
    termios.c_iflag |= libc::IUTF8;
    termios.c_iflag &= !(libc::IXON | libc::IXOFF);
    // SAFETY: `tcsetattr` 是系统调用包装，在子进程中安全。
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &termios) } != 0 {
        child_note_fd2(b"pty: child tcsetattr failed\n");
    }
}

/// 子进程内非致命诊断：写 fd 2 而不终止，调用点负责决定是否继续。
fn child_note_fd2(message: &'static [u8]) {
    // SAFETY: `write(2)` 在 POSIX 异步信号安全列表内；调用方传 `&'static` 文本，fork
    // 后不触碰堆。
    unsafe {
        libc::write(2, message.as_ptr() as *const libc::c_void, message.len());
    }
}

/// fork 后、exec 前的子进程失败诊断：把固定文本写到 PTY 从端（用户可见）与 fd 2
/// （logcat）后按给定码退出。
///
/// 异步信号安全：只调用 `write(2)` 与 `_exit`，不分配、不格式化、不记日志——调用点
/// 都在 fork 之后的子进程里。
///
/// 这三处失败发生在 `dup2` 把 fd 2 指向 PTY 从端**之前**，故 fd 2 仍是应用自身的
/// stderr，Android 会把它送进 logcat。此前是裸 `_exit(code)`，用户只看到
/// `[Process completed (code 3)]` 这样的空会话，失败原因彻底丢失（DESIGN 的 shell
/// 崩溃保留现场要求原因对用户可见，故同时写从端）。
fn child_exit_with_reason(slave_fd: std::os::unix::io::RawFd, reason: &str, code: i32) -> ! {
    // SAFETY: `write(2)` 在 POSIX 异步信号安全列表内；`reason` 是调用点传入的
    // `&'static str` 字面量，fork 后不触碰堆。两个 fd 都是 fork 后仍有效的
    // PTY 从端与应用 stderr；写失败不致命，随后立即 `_exit`。
    unsafe {
        libc::write(
            slave_fd,
            reason.as_ptr() as *const libc::c_void,
            reason.len(),
        );
        libc::write(2, reason.as_ptr() as *const libc::c_void, reason.len());
        libc::_exit(code);
    }
}

/// `close_stray_fds` 扫描的最大 fd 号；取保守上界，避免在 `sysconf(_SC_OPEN_MAX)`
/// 报出极大值（如 100 万以上）的系统上出现病态扫描时长。
const STRAY_FD_SCAN_LIMIT: libc::c_int = 65536;

/// 关闭子进程中除标准流（0、1、2，即 `dup2` 后的 PTY 从端）外的所有已打开 fd，
/// 避免启动的 shell 继承父进程的无关 fd（它们会占用资源或泄漏能力）。
///
/// 注意：fork 后、exec 前的单线程子进程中的尽力清理，不在 {0,1,2} 的 fd 一律关闭
/// ——不支持白名单。调用进程持有的库 fd（如日志转发）也会被关闭，但子进程随即
/// exec 成 shell，故可接受。`close()` 失败一律忽略（不致命）。
fn close_stray_fds() {
    // SAFETY: `getrlimit(2)` 在 POSIX 异步信号安全列表中。只要 `RLIMIT_NOFILE` 未被
    // 并发修改（此处从不发生），在从多线程进程 fork 出的线程中调用是安全的。
    let mut rlim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let upper = if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut rlim) } == 0 {
        // `rlim_cur` 是软限制（现代 Linux 上通常 1024-4096），远比迭代到
        // `sysconf(OPEN_MAX)` 便宜（systemd 系统上后者可达约 100 万）。
        // `RLIM_INFINITY` 表示“无限制”——回退到静态上限，避免扫描到
        // `c_int::MAX`（约 20 亿）。
        if rlim.rlim_cur == libc::RLIM_INFINITY {
            STRAY_FD_SCAN_LIMIT
        } else {
            // 封顶到 `STRAY_FD_SCAN_LIMIT`，避免在 RLIMIT_NOFILE 很大的容器中
            // 迭代数百万个 fd。
            rlim.rlim_cur
                .min(STRAY_FD_SCAN_LIMIT as u64)
                .min(libc::c_int::MAX as u64) as libc::c_int
        }
    } else {
        STRAY_FD_SCAN_LIMIT
    };
    // 注意：此处不能用 log，因为它可能在 fork 后的子进程中运行（非异步信号安全）。
    // 错误按设计静默。
    for fd in 3..=upper {
        // SAFETY: 对无效 fd 执行 `close(2)` 只返回 EBADF（无害），
        // 故不存在重复关闭或无效 fd 崩溃的风险。从 3 开始即排除了标准流（0、1、2）。
        unsafe {
            libc::close(fd);
        }
    }
}

fn base_env(prefix: Option<&str>) -> Vec<(String, String)> {
    base_env_with_host(prefix, &|key| std::env::var(key).ok())
}

/// 带可注入宿主环境的 [`base_env`]：生产传真实进程环境，测试传固定映射，
/// 使断言在任何机器上都保持封闭（CI runner 会导出 ANDROID_ROOT/EXTERNAL_STORAGE，
/// 开发机通常不会——对 `base_env` 本身做精确集合断言是环境相关的，不可存在）。
fn base_env_with_host(
    prefix: Option<&str>,
    host_var: &dyn Fn(&str) -> Option<String>,
) -> Vec<(String, String)> {
    let mut result = vec![
        ("TERM".to_string(), DEFAULT_TERM.to_string()),
        ("COLORTERM".to_string(), DEFAULT_COLORTERM.to_string()),
        ("LANG".to_string(), DEFAULT_LANG.to_string()),
    ];
    // 透传 Android 系统环境变量（ANDROID_ASSETS/ANDROID_DATA/ANDROID_ROOT/ANDROID_STORAGE/
    // EXTERNAL_STORAGE/ASEC_MOUNTPOINT/LOOP_MOUNTPOINT/ANDROID_RUNTIME_ROOT/ANDROID_ART_ROOT/
    // ANDROID_I18N_ROOT/ANDROID_TZDATA_ROOT/BOOTCLASSPATH/DEX2OATBOOTCLASSPATH/
    // SYSTEMSERVERCLASSPATH）。EXTERNAL_STORAGE 是 `/system/bin/am` 在至少三星 S7 上
    // 工作的必要条件，调用 `am`/`content` 的 Termux bootstrap shell 需要这些变量。
    // 只转发宿主环境中实际存在的变量：各设备/版本取值不同，硬编码
    // ANDROID_ROOT=/system 在 API 26+ 上是错的。
    // 参考 <https://github.com/reapercanuk39/termux-kotlin-app> AndroidShellEnvironment.kt
    const ANDROID_ENV_VARS: &[&str] = &[
        "ANDROID_ASSETS",
        "ANDROID_DATA",
        "ANDROID_ROOT",
        "ANDROID_STORAGE",
        "EXTERNAL_STORAGE",
        "ASEC_MOUNTPOINT",
        "LOOP_MOUNTPOINT",
        "ANDROID_RUNTIME_ROOT",
        "ANDROID_ART_ROOT",
        "ANDROID_I18N_ROOT",
        "ANDROID_TZDATA_ROOT",
        "BOOTCLASSPATH",
        "DEX2OATBOOTCLASSPATH",
        "SYSTEMSERVERCLASSPATH",
    ];
    for key in ANDROID_ENV_VARS {
        if let Some(value) = host_var(key) {
            result.push((key.to_string(), value));
        }
    }
    if let Some(prefix) = prefix {
        result.push(("PREFIX".to_string(), prefix.to_string()));
        result.push(("TMPDIR".to_string(), format!("{prefix}/tmp")));
    }
    result
}

pub fn build_env(env: &ShellEnv) -> Vec<(String, String)> {
    let mut result = base_env(env.prefix.as_deref());
    result.push(("HOME".to_string(), env.home.clone()));
    if let Some(mkshrc_path) = env.mkshrc_path.as_deref() {
        result.push(("ENV".to_string(), mkshrc_path.to_string()));
    }
    result.push(("TERMUX_HOME_DIR_PATH".to_string(), env.home.clone()));
    if let Some(prefix) = env.prefix.as_deref() {
        result.push(("TERMUX_PREFIX_DIR_PATH".to_string(), prefix.to_string()));
        result.push((
            "TERMUX_TMP_PREFIX_DIR_PATH".to_string(),
            format!("{prefix}/tmp"),
        ));
    }
    result.push(("TERMUX_VERSION".to_string(), TERMUX_VERSION.to_string()));
    result
}

/// `path` 处的 ELF 是否为 PIE（ET_DYN，e_type == 3）。
/// Android linker 拒绝来自 app 数据的非 PIE（ET_EXEC）二进制。
fn is_pie(path: &str) -> bool {
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut header = [0u8; 20];
    if file.read_exact(&mut header).is_err() {
        return false;
    }
    if header[0] != 0x7f || header[1] != b'E' || header[2] != b'L' || header[3] != b'F' {
        return false;
    }
    let e_type = u16::from_le_bytes([header[16], header[17]]);
    const ET_DYN: u16 = 3;
    e_type == ET_DYN
}

/// 读取脚本的 `#!` 解释器：`(解释器, 可选的单个参数)`，对齐内核 `binfmt_script`
/// （至多一个可选参数，多余 token 被忽略）。非脚本或文件不可读时返回 `None`。
/// 在 fork 之前调用；子进程中不发生分配。
fn read_shebang_interpreter(path: &str) -> Option<(std::ffi::CString, Option<std::ffi::CString>)> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut header = [0u8; 256];
    let read = file.read(&mut header).ok()?;
    if read < 2 || header[0] != b'#' || header[1] != b'!' {
        return None;
    }
    let first_line = std::str::from_utf8(&header[2..read])
        .ok()?
        .lines()
        .next()
        .unwrap_or("")
        .trim();
    let mut tokens = first_line.split_whitespace();
    let interpreter = tokens.next().filter(|token| !token.is_empty())?;
    let argument = tokens.next().map(str::to_string);
    if interpreter.is_empty() || interpreter.contains('\0') {
        return None;
    }
    let interpreter_cstr = std::ffi::CString::new(interpreter).ok()?;
    let argument_cstr = argument
        .filter(|argument| !argument.contains('\0'))
        .map(std::ffi::CString::new)
        .transpose()
        .ok()?;
    Some((interpreter_cstr, argument_cstr))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Session::spawn` 中 fork 成功后的错误路径上 `Session` 从未构造，
    /// 回收只能由主端所有者（`PtyPair::drop`）完成：drop 返回后子进程必须已被
    /// waitpid 回收（再查询即 ECHILD），既不得留活子进程也不得留僵尸。
    #[test]
    fn drop_reaps_child_process() {
        let pty =
            PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None).expect("spawn failed");
        let pid = pty.child_pid();
        drop(pty);

        match nix::sys::wait::waitpid(pid, Some(nix::sys::wait::WaitPidFlag::WNOHANG)) {
            Err(nix::errno::Errno::ECHILD) => {}
            Ok(status) => panic!("drop 后子进程未被回收（僵尸残留）: {status:?}"),
            Err(error) => panic!("waitpid failed: {error}"),
        }
        assert!(
            nix::sys::signal::kill(pid, None).is_err(),
            "child {pid} 仍存活：drop 未终止子进程"
        );
    }

    /// 主端是 O_NONBLOCK：PTY 输入缓冲（~4KB）满时 `write` 失败并返回 EAGAIN。
    /// 旧实现据此返回 `WouldBlock` 并丢弃剩余字节——粘贴被静默截断、子进程执行
    /// 半条命令，VT 应答也会被截断在转义序列中间使子应用永久等待。
    ///
    /// 断言 `write_all` 对超出缓冲的载荷**等待子进程消费**而非报错丢弃。
    #[test]
    fn write_all_waits_instead_of_dropping_bytes_past_a_full_pty_buffer() {
        let capture = std::env::temp_dir().join(format!("pty-write-{}", std::process::id()));
        std::fs::remove_file(&capture).ok();
        let mut pty =
            PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None).expect("spawn failed");
        pty.set_nonblocking().expect("set_nonblocking failed");
        let script = format!("sleep 0.2; cat > {}\n", capture.display());
        Pty::write_all(&mut pty, script.as_bytes()).expect("write failed");

        // 总量远超 PTY 输入缓冲（~4KB），中途必然 EAGAIN；逐行短行避开内核
        // 规范模式 4095 字符的行长上限。
        let payload = std::iter::repeat_n(
            [PTY_TEST_PAYLOAD_BYTE; PTY_TEST_PAYLOAD_LINE_BYTES],
            PTY_TEST_PAYLOAD_LINES,
        )
        .flatten()
        .chain(std::iter::repeat_n(b'\n', PTY_TEST_PAYLOAD_LINES))
        .collect::<Vec<u8>>();
        assert!(payload.len() > PTY_INPUT_BUFFER_BYTES);

        Pty::write_all(&mut pty, &payload).expect("write_all 不得因缓冲满而丢弃字节");

        let deadline = std::time::Instant::now() + Duration::from_secs(PTY_WRITE_TEST_DRAIN_SECS);
        while std::time::Instant::now() < deadline {
            if std::fs::metadata(&capture).is_ok_and(|meta| meta.len() > 0) {
                std::fs::remove_file(&capture).ok();
                return;
            }
            std::thread::sleep(Duration::from_millis(TEST_READ_POLL_STEP_MS));
        }
        panic!("子进程在期限内未读到任何载荷：write_all 交付失败");
    }

    #[test]
    fn spawn_rejects_empty_shell_before_forking() {
        // 护栏：空 shell 曾一路走到 `execve("")`，只换来一个 ENOENT——现象是空会话
        // 加一条「查不到文件」，完全看不出是 shell 没配置。必须在 fork 前明确拒绝。
        let error = PtyPair::spawn("", 24, 80, &ShellEnv::default(), None)
            .err()
            .expect("empty shell must be rejected");
        assert!(
            matches!(error, PtyError::EmptyShell),
            "expected EmptyShell, got {error:?}"
        );
    }

    #[test]
    fn shell_entry_splits_executable_and_arguments() {
        let (executable, arguments) =
            split_shell_entry("/data/data/com.termux/files/usr/bin/bash -l");
        assert_eq!(executable, "/data/data/com.termux/files/usr/bin/bash");
        assert_eq!(arguments, vec!["-l"]);
        let (single, empty) = split_shell_entry("/system/bin/sh");
        assert_eq!(single, "/system/bin/sh");
        assert!(empty.is_empty());
    }

    fn test_env() -> ShellEnv {
        ShellEnv {
            home: "/tmp/test_home".to_string(),
            working_directory: "/tmp/test_home".to_string(),
            prefix: None,
            mkshrc_path: Some("/tmp/test_app_data/.mkshrc".to_string()),
        }
    }

    /// 测试读取缓冲（单次 `read` 上限）与轮询节拍：`read_until` 的截止、
    /// 步长与 `double_write` 的尝试次数/最小输出。
    const TEST_READ_BUF_SIZE: usize = 4096;
    const TEST_READ_DEADLINE_SECS: u64 = 2;
    const TEST_READ_POLL_STEP_MS: u64 = 10;
    const TEST_READ_ATTEMPTS: usize = 50;
    const TEST_MIN_OUTPUT_LEN: usize = 200;

    /// 写入排空测试的读取上限：远大于实测耗时，只为在异常时给出确定的失败而非挂起。
    const PTY_WRITE_TEST_DRAIN_SECS: u64 = 10;

    /// 每行远小于内核规范模式行长上限（4095），行数足够多使总量远超
    /// PTY 输入缓冲（~4KB）——那才是 EAGAIN 的真实触发条件。
    const PTY_TEST_PAYLOAD_LINE_BYTES: usize = 1000;
    const PTY_TEST_PAYLOAD_LINES: usize = 16;

    /// 载荷字节取值，命令文本中不含该字节。
    const PTY_TEST_PAYLOAD_BYTE: u8 = b'z';

    /// Linux tty 线路缓冲上限（`N_TTY_BUF_SIZE`）：超过即触发 EAGAIN。
    const PTY_INPUT_BUFFER_BYTES: usize = 4096;

    /// DESIGN 声明的规范路径常量（测试断言专用）：生产代码经调用方
    ///（Kotlin `filesDir`）传入，绝不硬编码；此处断言规范值本身。
    const TEST_PREFIX: &str = "/data/data/com.termux/files/usr";
    const TEST_TMPDIR: &str = "/data/data/com.termux/files/usr/tmp";

    /// 读取 PTY 输出直到出现 `needle`（截止见 `TEST_READ_DEADLINE_SECS`）；返回已读内容，调用方自行断言。
    fn read_until(pty: &mut PtyPair, needle: &[u8]) -> Vec<u8> {
        use crate::terminal::pty::Pty;

        let mut buf = [0u8; TEST_READ_BUF_SIZE];
        let mut output = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(TEST_READ_DEADLINE_SECS);
        while std::time::Instant::now() < deadline {
            match Pty::read(pty, &mut buf) {
                Ok(byte_count) => output.extend_from_slice(&buf[..byte_count]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(TEST_READ_POLL_STEP_MS));
                }
                Err(_) => break,
            }
            if output.windows(needle.len()).any(|window| window == needle) {
                return output;
            }
        }
        output
    }

    #[test]
    fn shebang_plain_interpreter_parses() {
        let path = std::env::temp_dir().join("terminal-shebang-plain.sh");
        std::fs::write(&path, "#!/system/bin/sh\nset -eu\n").unwrap();
        let (interpreter, argument) = read_shebang_interpreter(path.to_str().unwrap()).unwrap();
        assert_eq!(interpreter.to_str().unwrap(), "/system/bin/sh");
        assert!(argument.is_none());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn shebang_single_argument_parses() {
        let path = std::env::temp_dir().join("terminal-shebang-arg.sh");
        std::fs::write(&path, "#!/system/bin/sh -eu\n").unwrap();
        let (interpreter, argument) = read_shebang_interpreter(path.to_str().unwrap()).unwrap();
        assert_eq!(interpreter.to_str().unwrap(), "/system/bin/sh");
        assert_eq!(argument.unwrap().to_str().unwrap(), "-eu");
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn shebang_missing_returns_none() {
        let path = std::env::temp_dir().join("terminal-shebang-plain-bin");
        std::fs::write(&path, [0x7fu8, b'E', b'L', b'F', 2]).unwrap();
        assert!(read_shebang_interpreter(path.to_str().unwrap()).is_none());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn base_env_includes_xterm_256color() {
        let env = base_env(None);
        assert!(
            env.iter()
                .any(|(key, value)| key == "TERM" && value == "xterm-256color")
        );
    }

    #[test]
    fn base_env_is_minimal_set() {
        // 固定契约：宿主环境为空时 `base_env` 只产出 TERM/COLORTERM/LANG；
        // 无 PREFIX 时不产出 PREFIX/TMPDIR（规范未声明无引导程序会话的取值）。
        // 不能直接断言 `base_env`：CI 会导出 ANDROID_ROOT/EXTERNAL_STORAGE。
        let keys: std::collections::BTreeSet<String> = base_env_with_host(None, &|_| None)
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        let expected: std::collections::BTreeSet<String> = ["COLORTERM", "LANG", "TERM"]
            .into_iter()
            .map(str::to_string)
            .collect();
        assert_eq!(keys, expected);
    }

    #[test]
    fn base_env_forwards_only_present_host_vars() {
        // 规范透传（封闭）：宿主中存在的变量按原值转发，不存在的省略。
        let host = std::collections::HashMap::from([
            ("ANDROID_ROOT".to_string(), "/system".to_string()),
            ("EXTERNAL_STORAGE".to_string(), "/sdcard".to_string()),
        ]);
        let env = base_env_with_host(None, &|key| host.get(key).cloned());
        assert!(
            env.iter()
                .any(|(key, value)| key == "ANDROID_ROOT" && value == "/system"),
        );
        assert!(
            env.iter()
                .any(|(key, value)| key == "EXTERNAL_STORAGE" && value == "/sdcard"),
        );
        assert!(env.iter().all(|(key, _)| key != "ANDROID_DATA"));
    }

    #[test]
    fn base_env_includes_lang() {
        let env = base_env(None);
        assert!(
            env.iter()
                .any(|(key, value)| key == "LANG" && value == "en_US.UTF-8")
        );
    }

    #[test]
    fn base_env_omits_tmpdir_without_prefix() {
        // 规范只声明 TMPDIR = files/usr/tmp；无 PREFIX 时不存在合法的 TMPDIR 取值，
        // 不得回退到未声明路径（DESIGN Bootstrap 节白名单 + 禁止未声明回退）。
        let env = base_env(None);
        assert!(!env.iter().any(|(key, _)| key == "TMPDIR"));
    }

    #[test]
    fn base_env_includes_prefix_and_tmpdir_when_set() {
        let env = base_env(Some(TEST_PREFIX));
        assert!(
            env.iter()
                .any(|(key, value)| key == "PREFIX" && value == TEST_PREFIX)
        );
        assert!(
            env.iter()
                .any(|(key, value)| key == "TMPDIR" && value == TEST_TMPDIR)
        );
    }

    #[test]
    fn build_env_includes_term() {
        let env = test_env();
        let result = build_env(&env);
        assert!(
            result
                .iter()
                .any(|(key, value)| key == "TERM" && value == "xterm-256color")
        );
    }

    #[test]
    fn build_env_includes_mksh_startup_pointer() {
        let env = test_env();
        let result = build_env(&env);
        assert!(
            result
                .iter()
                .any(|(key, value)| key == "ENV" && value == "/tmp/test_app_data/.mkshrc")
        );
    }

    #[test]
    fn build_env_omits_env_without_mkshrc_path() {
        let env = ShellEnv {
            mkshrc_path: None,
            ..test_env()
        };
        let result = build_env(&env);
        assert!(!result.iter().any(|(key, _)| key == "ENV"));
    }

    #[test]
    fn build_env_includes_colorterm() {
        let env = test_env();
        let result = build_env(&env);
        assert!(
            result
                .iter()
                .any(|(key, value)| key == "COLORTERM" && value == "truecolor")
        );
    }

    #[test]
    fn build_env_includes_home_from_env() {
        let env = test_env();
        let result = build_env(&env);
        assert!(
            result
                .iter()
                .any(|(key, value)| key == "HOME" && value == "/tmp/test_home")
        );
    }

    #[test]
    fn build_env_emits_prefix_pairs() {
        let mut env = test_env();
        env.prefix = Some(TEST_PREFIX.to_string());
        let result = build_env(&env);
        for (key, value) in [
            ("PREFIX", TEST_PREFIX),
            ("TERMUX_PREFIX_DIR_PATH", TEST_PREFIX),
            ("TMPDIR", TEST_TMPDIR),
            ("TERMUX_TMP_PREFIX_DIR_PATH", TEST_TMPDIR),
            ("TERMUX_VERSION", "0.119.0-beta.3"),
            ("TERMUX_HOME_DIR_PATH", "/tmp/test_home"),
        ] {
            assert!(
                result
                    .iter()
                    .any(|(entry_key, entry_value)| entry_key == key && entry_value == value),
                "missing {key}={value}"
            );
        }
    }

    #[test]
    fn build_env_rejects_unlisted_variables() {
        let mut env = test_env();
        env.prefix = Some(TEST_PREFIX.to_string());
        let result = build_env(&env);
        for key in [
            "LD_PRELOAD",
            "LD_LIBRARY_PATH",
            "PWD",
            "USER",
            "SHELL",
            "PATH",
            "LINES",
            "COLUMNS",
            "SSL_CERT_FILE",
            "CURL_CA_BUNDLE",
        ] {
            assert!(
                !result.iter().any(|(entry_key, _)| entry_key == key),
                "unlisted variable {key} must not be set"
            );
        }
        for (key, _) in &result {
            assert!(
                !key.starts_with("TERMUX__") && !key.starts_with("TERMUX_APP__"),
                "undeclared variable {key} must not be set"
            );
        }
    }

    #[test]
    fn build_env_without_prefix_omits_prefix_vars() {
        let result = build_env(&test_env());
        for key in [
            "PREFIX",
            "TERMUX_PREFIX_DIR_PATH",
            "TMPDIR",
            "TERMUX_TMP_PREFIX_DIR_PATH",
        ] {
            assert!(
                !result.iter().any(|(name, _)| name == key),
                "{key} must be absent without a prefix, got {result:?}"
            );
        }
    }

    #[test]
    fn spawn_and_read_shell() {
        use crate::terminal::pty::Pty;

        let mut pty =
            PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None).expect("spawn failed");
        pty.set_nonblocking().expect("set_nonblocking failed");

        Pty::write_all(&mut pty, b"echo hello_vt\n").expect("write failed");

        let output = read_until(&mut pty, b"hello_vt");
        assert!(
            output.windows(8).any(|window| window == b"hello_vt"),
            "did not see 'hello_vt' in output: {}",
            String::from_utf8_lossy(&output)
        );
    }

    #[test]
    fn spawn_seeds_24x80_winsize() {
        // 在任何 UI 驱动的 resize 之前，TIOCGWINSZ 必须返回种子值 24x80，
        // 使 shell（行编辑器等）不会看到 0x0。
        let pty =
            PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None).expect("spawn failed");
        let (rows, cols) = pty.get_winsize().expect("TIOCGWINSZ failed");
        assert_eq!(
            rows, 24,
            "seeded rows must be 24 before any resize, got {rows}"
        );
        assert_eq!(
            cols, 80,
            "seeded cols must be 80 before any resize, got {cols}"
        );
    }

    #[test]
    fn resize_reflected_in_get_winsize() {
        let pty =
            PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None).expect("spawn failed");
        pty.resize(40, 120).expect("resize failed");
        let (rows, cols) = pty.get_winsize().expect("TIOCGWINSZ failed");
        assert_eq!(
            (rows, cols),
            (40, 120),
            "resize must be reflected in TIOCGWINSZ"
        );
    }

    #[test]
    fn set_pixel_size_reflected_in_winsize() {
        // `ws_xpixel`/`ws_ypixel` 必须送达内核，像素感知的程序（icat、全屏 TUI）
        // 才能自行定尺寸；像素字段为 0 会导致错渲。TIOCSWINSZ 会整体替换结构体，
        // 故 `set_pixel_size` 必须保留 rows/cols。
        let pty =
            PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None).expect("spawn failed");
        pty.set_pixel_size(640, 480).expect("set_pixel_size failed");
        let mut winsize = nix::pty::Winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: TIOCGWINSZ 把格式良好的 Winsize 结构写入 `winsize`；
        // fd 自有且有效。返回值已做检查。
        let result = unsafe { libc::ioctl(pty.master_fd(), libc::TIOCGWINSZ, &mut winsize) };
        assert_eq!(result, 0, "TIOCGWINSZ ioctl must succeed");
        assert_eq!(
            (winsize.ws_row, winsize.ws_col),
            (24, 80),
            "set_pixel_size must preserve rows/cols"
        );
        assert_eq!(
            (winsize.ws_xpixel, winsize.ws_ypixel),
            (640, 480),
            "set_pixel_size must write pixel fields to the kernel"
        );
    }

    #[test]
    fn child_pid_is_positive() {
        let pty =
            PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None).expect("spawn failed");
        let pid = pty.child_pid();
        assert!(
            pid.as_raw() > 0,
            "child PID should be positive, got {pid:?}"
        );
    }

    #[test]
    fn drop_closes_master_fd() {
        // `PtyPair` 的析构必须能无阻塞完成；信号投递与子进程回收由
        // `Session::drop()` 负责。
        let _pty =
            PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None).expect("spawn failed");
        // `PtyPair` 在作用域末尾于此析构——不得阻塞。
    }

    #[test]
    fn child_survives_without_session_kill() {
        // `PtyPair::drop()` 关闭主端 fd，内核据此向从端的前台进程组发送 SIGHUP
        // （见 pts(4)）。子进程**可能**因 SIGHUP 死亡——这是预期的内核行为。
        // 本测试验证的是：析构不阻塞、不 panic，也不调用 waitpid（回收是
        // `Session::drop()` 的职责）。
        let child = {
            let pty = PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None)
                .expect("spawn failed");
            pty.child_pid()
        };
        std::thread::sleep(Duration::from_millis(100));
        // 子进程可能存活（SIGTERM 成功）或已死（ESRCH），两者皆可。
        // 关键不变式：不 panic、不挂起。
        let result = nix::sys::signal::kill(child, nix::sys::signal::Signal::SIGTERM);
        assert!(
            matches!(result, Ok(()) | Err(nix::errno::Errno::ESRCH)),
            "SIGTERM to child after PtyPair drop must be Ok or ESRCH, got {result:?}"
        );
        // 仍存活则清理。
        let _ = nix::sys::signal::kill(child, nix::sys::signal::Signal::SIGKILL);
        let _ = nix::sys::wait::waitpid(child, Some(nix::sys::wait::WaitPidFlag::WNOHANG));
    }

    #[test]
    fn pty_error_display_works() {
        let display = format!("{}", PtyError::Open(nix::errno::Errno::EINVAL.into()));
        assert!(
            display.contains("failed to open pseudoterminal"),
            "PtyError::Open Display should describe the failure, got: {display}"
        );
        let fork_display = format!("{}", PtyError::Fork(nix::errno::Errno::EINVAL));
        assert!(
            fork_display.contains("fork failed"),
            "PtyError::Fork Display should describe the failure, got: {fork_display}"
        );
        let resize_display = format!("{}", PtyError::Resize(nix::errno::Errno::EINVAL));
        assert!(
            resize_display.contains("TIOCSWINSZ"),
            "PtyError::Resize Display should describe the failure, got: {resize_display}"
        );
    }

    #[test]
    fn chdir_changes_working_directory() {
        use crate::terminal::pty::Pty;

        let temp = std::env::temp_dir().join("vt_test_chdir");
        std::fs::create_dir_all(&temp).expect("create test dir failed");

        let env = ShellEnv {
            working_directory: temp.to_string_lossy().to_string(),
            ..ShellEnv::default()
        };

        let mut pty = PtyPair::spawn("/bin/sh", 24, 80, &env, None).expect("spawn failed");
        pty.set_nonblocking().expect("set_nonblocking failed");
        Pty::write_all(&mut pty, b"pwd\n").expect("write failed");

        let output = read_until(&mut pty, temp.to_string_lossy().as_bytes());
        std::fs::remove_dir_all(&temp).ok();
        assert!(
            output
                .windows(temp.to_string_lossy().len())
                .any(|window| window == temp.to_string_lossy().as_bytes()),
            "did not see working directory '{}' in pwd output: {}",
            temp.display(),
            String::from_utf8_lossy(&output)
        );
    }

    // ── mkshrc 路径端到端：build_env 注入的 ENV 必须真实到达子进程环境。
    // 子进程 `echo $ENV` 回显传入的 mkshrc_path，证明 ShellEnv → execve 整链透传。
    #[test]
    fn spawned_child_sees_env_mkshrc_path() {
        use crate::terminal::pty::Pty;

        let marker = "/tmp/vt_test_mkshrc/.mkshrc";
        let env = ShellEnv {
            mkshrc_path: Some(marker.to_string()),
            ..ShellEnv::default()
        };

        let mut pty = PtyPair::spawn("/bin/sh", 24, 80, &env, None).expect("spawn failed");
        pty.set_nonblocking().expect("set_nonblocking failed");
        Pty::write_all(&mut pty, b"echo \"ENV-is:$ENV\"\n").expect("write failed");

        let needle = format!("ENV-is:{marker}");
        let output = read_until(&mut pty, needle.as_bytes());
        assert!(
            output
                .windows(needle.len())
                .any(|window| window == needle.as_bytes()),
            "did not see ENV '{}' in child output: {}",
            needle,
            String::from_utf8_lossy(&output)
        );
    }

    #[test]
    fn configure_raw_mode_sets_iutf8_and_clears_ixon_ixoff() {
        let pty =
            PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None).expect("spawn failed");
        let fd = pty.master_fd();

        // 施加与子进程在从端相同的 raw 模式配置。
        configure_raw_mode(fd).expect("configure_raw_mode failed");

        // SAFETY: `tcgetattr` 是简单系统调用包装；`fd` 是有效且自有的 PTY 主端描述符，
        // 读取其 termios 安全。
        let mut termios = std::mem::MaybeUninit::<libc::termios>::uninit();
        let termios = unsafe {
            assert_eq!(
                libc::tcgetattr(fd, termios.as_mut_ptr()),
                0,
                "tcgetattr failed: {}",
                std::io::Error::last_os_error()
            );
            termios.assume_init()
        };

        let iutf8_set = (termios.c_iflag & libc::IUTF8) != 0;
        let ixon_cleared = (termios.c_iflag & libc::IXON) == 0;
        let ixoff_cleared = (termios.c_iflag & libc::IXOFF) == 0;

        assert!(iutf8_set, "IUTF8 must be set on the PTY line discipline");
        assert!(ixon_cleared, "IXON (software flow control) must be cleared");
        assert!(
            ixoff_cleared,
            "IXOFF (software flow control) must be cleared"
        );
    }

    #[test]
    fn double_write_then_read_does_not_panic() {
        use crate::terminal::pty::Pty;

        let mut pty = PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None)
            .expect("spawn must succeed in test env");
        pty.set_nonblocking().expect("set_nonblocking failed");

        Pty::write_all(&mut pty, b"echo a\n").expect("first write must succeed");
        Pty::write_all(&mut pty, b"echo b\n").expect("second write must succeed");

        let mut buf = [0u8; TEST_READ_BUF_SIZE];
        let mut output = Vec::new();
        for _ in 0..TEST_READ_ATTEMPTS {
            match Pty::read(&mut pty, &mut buf) {
                Ok(byte_count) => output.extend_from_slice(&buf[..byte_count]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(TEST_READ_POLL_STEP_MS));
                }
                Err(_) => break,
            }
            if output.len() > TEST_MIN_OUTPUT_LEN {
                break;
            }
        }
        assert!(
            !output.is_empty(),
            "must read at least some output after two writes"
        );
        let text = String::from_utf8_lossy(&output);
        assert!(
            text.contains('a') || text.contains('b'),
            "output must contain echoed text"
        );
    }
}
