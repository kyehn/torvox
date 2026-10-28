//! fork 之后、exec 之前的子进程侧逻辑：终端模式、失败自述与 fd 清理。
//!
//! 单列一处的理由：这些函数只在 fork 出的子进程里运行，受**异步信号安全**
//! 约束（不分配、不记日志、不格式化），与父进程侧的 spawn 流程混在一起时，
//! 改动很容易越过这条约束。

use std::os::unix::io::RawFd;

/// fork 后子进程的终端模式配置，对齐 Termux 的做法：PTY 保留内核行规程
/// （ECHO+ICANON 开启），由 shell 自己的行编辑器（bash readline）负责回显，
/// 全屏程序（vim、less）启动时自行把 tty 切到 raw 模式。
///
/// 在此完整执行 cfmakeraw()（关闭 ECHO|ICANON|ISIG）会破坏 bash readline：实测输入
/// 的字符能到达 shell（命令得以执行）却完全无回显——readline 继承的 termios 中 ECHO
/// 已被清除时不会重新开启，且 ISIG 关闭时它会跳过信号设置。
///
/// 异步信号安全：不分配、不调用 `log::warn!`。失败时写 fd 2（此时 fd 2 仍是应用
/// 的 stderr，送进 logcat），与 fork 子进程的 [`child_exit_with_reason`] 一致——错误
/// 自述自理会静默吞掉 termios 配置失败，让"子进程拿到的不是预期模式"变成不可定位的
/// 罕见会话问题。配置失败不致命，仅告知后继续。
pub(super) fn configure_raw_mode_child(fd: RawFd) {
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
pub(super) fn child_exit_with_reason(slave_fd: RawFd, reason: &str, code: i32) -> ! {
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

/// 关闭子进程中除标准流（0、1、2，即 `dup2` 后的 PTY 从端）与 [keep_fd] 外的
/// 所有已打开 fd，避免启动的 shell 继承父进程的无关 fd（它们会占用资源或泄漏能力）。
///
/// [keep_fd] 是 execve 失败回报管道的写端：它必须活到 execve，否则失败时父进程
/// 读不到 errno，只能看到一次没有原因的退出。
///
/// 注意：fork 后、exec 前的单线程子进程中的尽力清理，不在 {0,1,2} 的 fd 一律关闭
/// ——不支持白名单。调用进程持有的库 fd（如日志转发）也会被关闭，但子进程随即
/// exec 成 shell，故可接受。`close()` 失败一律忽略（不致命）。
pub(super) fn close_stray_fds(keep_fd: RawFd) {
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
        if fd == keep_fd {
            continue;
        }
        // SAFETY: 对无效 fd 执行 `close(2)` 只返回 EBADF（无害），
        // 故不存在重复关闭或无效 fd 崩溃的风险。从 3 开始即排除了标准流（0、1、2）。
        unsafe {
            libc::close(fd);
        }
    }
}

/// `path` 处的 ELF 是否为 PIE（ET_DYN，e_type == 3）。
/// Android linker 拒绝来自 app 数据的非 PIE（ET_EXEC）二进制。
pub(super) fn is_pie(path: &str) -> bool {
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
pub(super) fn read_shebang_interpreter(
    path: &str,
) -> Option<(std::ffi::CString, Option<std::ffi::CString>)> {
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
        .next()?
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
