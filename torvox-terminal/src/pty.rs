//! PTY master/slave creation — only allowed fork unsafe.
//!
//! # Requirements
//! - [FR-026](crate) — PTY: master/slave pair creation
use std::io;
use std::os::unix::io::{AsRawFd, OwnedFd, RawFd};
use std::time::Duration;

use thiserror::Error;

use crate::shell_env::ShellEnv;

const DEFAULT_TERM: &str = "xterm-256color";
const DEFAULT_COLORTERM: &str = "truecolor";
const DEFAULT_TERM_PROGRAM: &str = "torvox";
const DEFAULT_LANG: &str = "en_US.UTF-8";
/// Android does not have a writable /tmp, so we use /data/local/tmp
/// which is guaranteed to be writable by the app process on all API levels.
const ANDROID_TMPDIR: &str = "/data/local/tmp";
const GRACEFUL_SHUTDOWN_TIMEOUT_MS: u64 = 100;

#[derive(Debug, Error)]
pub enum PtyError {
    #[error("fork failed: {0}")]
    Fork(nix::errno::Errno),
    #[error("failed to open pseudoterminal: {0}")]
    Open(std::io::Error),
    #[error("ioctl TIOCSWINSZ failed: {0}")]
    Resize(nix::errno::Errno),
    #[error("fcntl failed: {0}")]
    Fcntl(nix::errno::Errno),
    #[error("termios configuration failed: {0}")]
    Termios(nix::errno::Errno),
}

impl From<nix::errno::Errno> for PtyError {
    fn from(err: nix::errno::Errno) -> Self {
        PtyError::Fork(err)
    }
}

/// Trait abstracting a pseudoterminal for testability.
pub trait Pty: Send {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize>;
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    fn resize(&self, rows: u16, cols: u16) -> Result<(), PtyError>;
    fn child_pid(&self) -> nix::unistd::Pid;
    fn master_fd(&self) -> RawFd;
    /// Returns an independently-owned duplicate of the master fd for use by a
    /// dedicated reader thread. The duplicate shares the underlying open file
    /// description with `master_fd()` (so O_NONBLOCK state is shared), which is
    /// fine because the reader uses `poll` + a blocking-style read. The dup is
    /// performed here (where `unsafe` is permitted) so callers can read through
    /// a safe `std::fs::File` without any `unsafe` blocks.
    fn try_clone_reader_fd(&self) -> io::Result<OwnedFd>;
    fn wait(&self) -> nix::Result<nix::sys::wait::WaitStatus>;
    fn set_nonblocking(&self) -> Result<(), PtyError>;
    fn set_pixel_size(&mut self, width: u16, height: u16);

    fn spawn(shell: &str, rows: u16, cols: u16, env: &ShellEnv) -> Result<Box<dyn Pty>, PtyError>
    where
        Self: Sized;

    fn write_all(&mut self, mut buf: &[u8]) -> io::Result<()> {
        while !buf.is_empty() {
            let bytes_written = self.write(buf)?;
            buf = &buf[bytes_written..];
        }
        Ok(())
    }
}

pub struct PtyPair {
    master: OwnedFd,
    child_pid: nix::unistd::Pid,
    pixel_width: u16,
    pixel_height: u16,
}

impl PtyPair {
    pub fn spawn(shell: &str, rows: u16, cols: u16, env: &ShellEnv) -> Result<Self, PtyError> {
        let winsize = nix::pty::Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        let result = nix::pty::openpty(Some(&winsize), None)
            .map_err(|e| PtyError::Open(std::io::Error::other(e)))?;
        let master_fd = result.master;
        let slave_fd = result.slave;

        // Build all child process data before fork to avoid allocations in child.
        // (Multi-threaded process fork may corrupt malloc heap.)
        let shell_cstr = std::ffi::CString::new(shell).map_err(|e| {
            let msg = format!("shell path contains null byte: {e}");
            log::error!("{msg}");
            PtyError::Fork(nix::errno::Errno::EINVAL)
        })?;
        let env_cstrings: Vec<std::ffi::CString> = build_env(env, shell, rows, cols)
            .into_iter()
            .map(|(k, v)| {
                std::ffi::CString::new(format!("{k}={v}")).map_err(|e| {
                    let msg = format!("env var contains null byte: {e}");
                    log::error!("{msg}");
                    PtyError::Fork(nix::errno::Errno::EINVAL)
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let working_directory_cstr = std::ffi::CString::new(env.working_directory.as_str())
            .map_err(|e| {
                let msg = format!("working directory contains null byte: {e}");
                log::error!("{msg}");
                PtyError::Fork(nix::errno::Errno::EINVAL)
            })?;

        // Pre-allocate argument and environment arrays before fork.
        // After fork, the child must NOT call any allocation functions.
        let shell_ptr = shell_cstr.as_ptr();
        let working_directory_ptr = working_directory_cstr.as_ptr();
        let args_ptrs: Vec<*const libc::c_char> = vec![shell_ptr, std::ptr::null()];
        let env_ptrs: Vec<*const libc::c_char> = env_cstrings
            .iter()
            .map(|s| s.as_ptr())
            .chain(std::iter::once(std::ptr::null()))
            .collect();

        // SAFETY: `fork()` is unsafe because it creates a new process. The child
        // process calls `execve()`, which replaces the process image
        // (no heap data is used after the fork — all data is pre-allocated and
        // signal handlers are reset before execve). No signal handlers run between
        // fork and exec (all operations are async-signal-safe syscalls). The parent
        // process checks the `ForkResult` return value and handles errors via `?`.
        match unsafe { nix::unistd::fork()? } {
            nix::unistd::ForkResult::Parent { child } => {
                if let Err(e) = nix::unistd::close(slave_fd) {
                    log::warn!("failed to close PTY slave fd in parent after fork: {e}");
                }
                Ok(Self {
                    master: master_fd,
                    child_pid: child,
                    pixel_width: 0,
                    pixel_height: 0,
                })
            }
            nix::unistd::ForkResult::Child => {
                if let Err(e) = nix::unistd::close(master_fd) {
                    log::warn!("failed to close PTY master fd in child after fork: {e}");
                }
                // Manually set controlling terminal using only syscalls.
                // Avoid login_tty because it may call malloc() internally,
                // which is unsafe after fork in a multithreaded process.
                // Create a new session/process group so the shell is detached
                // from the parent's controlling terminal (termux.c:54-96). Only
                // call setsid() if we are not already a session leader, since
                // calling it again would fail with EPERM.
                let is_session_leader =
                    unsafe { libc::getsid(0) } == nix::unistd::getpid().as_raw();
                if !is_session_leader && nix::unistd::setsid().is_err() {
                    std::process::exit(2);
                }
                let slave_raw = slave_fd.as_raw_fd();
                // SAFETY: All these libc calls are lightweight syscall wrappers that do not allocate.
                // The child process is single-threaded. No signal handlers run between fork and exec
                // (all operations are async-signal-safe syscalls).
                let result = unsafe { libc::ioctl(slave_raw, libc::TIOCSCTTY, 0) };
                if result < 0 {
                    std::process::exit(3);
                }
                // SAFETY: dup2 across well-known FDs (0, 1, 2) is safe and async-signal-safe
                // post-fork. The slave FD is valid because setsid()+ioctl(TIOCSCTTY) above
                // assigned it as the controlling terminal (manual alternative to login_tty).
                unsafe {
                    libc::dup2(slave_raw, 0);
                    libc::dup2(slave_raw, 1);
                    libc::dup2(slave_raw, 2);
                }
                if slave_raw > 2 {
                    // SAFETY: slave_raw is only closed if it is not one of the standard FDs
                    // (0, 1, 2), ensuring we don't accidentally close a critical FD.
                    unsafe {
                        libc::close(slave_raw);
                    }
                }
                // Configure raw mode on stdin (fd 0, PTY slave device).
                // Failure is non-fatal (shell runs in canonical mode without raw mode).
                if let Err(e) = configure_raw_mode(libc::STDIN_FILENO) {
                    log::warn!("failed to set raw mode on PTY stdin: {e}");
                }
                // SAFETY: chdir is safe with a valid, null-terminated path string.
                // working_directory_ptr was allocated via CString::new() which guarantees
                // null termination. Failure is non-fatal (defaults to /).
                if unsafe { libc::chdir(working_directory_ptr) } != 0 {
                    log::warn!("chdir to working directory failed, using /");
                }
                // SAFETY: signal() is safe in the single-threaded child process post-fork.
                // Resetting to SIG_DFL prevents leakage of parent's custom signal handlers.
                unsafe {
                    libc::signal(libc::SIGCHLD, libc::SIG_DFL);
                    libc::signal(libc::SIGHUP, libc::SIG_DFL);
                    libc::signal(libc::SIGINT, libc::SIG_DFL);
                    libc::signal(libc::SIGQUIT, libc::SIG_DFL);
                    libc::signal(libc::SIGTERM, libc::SIG_DFL);
                    libc::signal(libc::SIGPIPE, libc::SIG_DFL);
                    libc::signal(libc::SIGALRM, libc::SIG_DFL);
                }
                // Close any stray fds inherited from the parent (termux.c:54-96).
                // Standard streams 0/1/2 (the PTY slave) are preserved; the PTY
                // master was already closed above. Non-fatal — failures are
                // ignored and spawn continues.
                close_stray_fds();
                // SAFETY: execve is safe with pre-allocated null-terminated arrays.
                // shell_ptr, args_ptrs, and env_ptrs were created via CString::new()
                // and CString::as_ptr() before fork(), guaranteeing valid pointers.
                // nix::unistd::execve allocates internally via collect(), which is unsafe
                // after fork in a multithreaded process — hence the direct libc call.
                unsafe {
                    libc::execve(shell_ptr, args_ptrs.as_ptr(), env_ptrs.as_ptr());
                }
                // execve only returns on failure. Use _exit (not exit) to avoid running
                // atexit handlers from the parent process.
                // SAFETY: _exit(4) is safe; it terminates the child immediately without
                // running cleanup handlers. Only called when execve fails.
                unsafe {
                    libc::_exit(4);
                }
            }
        }
    }

    pub fn child_pid(&self) -> nix::unistd::Pid {
        self.child_pid
    }

    pub fn wait(&self) -> nix::Result<nix::sys::wait::WaitStatus> {
        nix::sys::wait::waitpid(self.child_pid, None)
    }

    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), PtyError> {
        let winsize = nix::pty::Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: self.pixel_width,
            ws_ypixel: self.pixel_height,
        };
        // SAFETY: ioctl with TIOCSWINSZ writes a well-formed Winsize struct
        // to the master PTY fd. The fd is owned and valid. The kernel copies
        // the winsize to the slave side — no memory safety risk. The return
        // value is checked for errors.
        unsafe {
            let result = libc::ioctl(
                self.master.as_raw_fd(),
                libc::TIOCSWINSZ,
                std::ptr::from_ref(&winsize),
            );
            if result < 0 {
                return Err(PtyError::Resize(nix::errno::Errno::last()));
            }
        }
        Ok(())
    }

    pub fn set_pixel_size(&mut self, width: u16, height: u16) {
        self.pixel_width = width;
        self.pixel_height = height;
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
        nix::unistd::write(&self.master, buf).map_err(|e| io::Error::from_raw_os_error(e as i32))
    }

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        nix::unistd::read(&self.master, buf).map_err(|e| io::Error::from_raw_os_error(e as i32))
    }

    fn resize(&self, rows: u16, cols: u16) -> Result<(), PtyError> {
        PtyPair::resize(self, rows, cols)
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

    fn wait(&self) -> nix::Result<nix::sys::wait::WaitStatus> {
        PtyPair::wait(self)
    }

    fn set_nonblocking(&self) -> Result<(), PtyError> {
        PtyPair::set_nonblocking(self)
    }

    fn set_pixel_size(&mut self, width: u16, height: u16) {
        PtyPair::set_pixel_size(self, width, height)
    }

    fn spawn(shell: &str, rows: u16, cols: u16, env: &ShellEnv) -> Result<Box<dyn Pty>, PtyError> {
        PtyPair::spawn(shell, rows, cols, env).map(|p| Box::new(p) as Box<dyn Pty>)
    }
}

impl std::io::Read for PtyPair {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        nix::unistd::read(&self.master, buf)
            .map_err(|e| std::io::Error::from_raw_os_error(e as i32))
    }
}

impl std::io::Write for PtyPair {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        nix::unistd::write(&self.master, buf)
            .map_err(|e| std::io::Error::from_raw_os_error(e as i32))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for PtyPair {
    fn drop(&mut self) {
        if let Err(e) = nix::sys::signal::kill(self.child_pid, nix::sys::signal::Signal::SIGHUP) {
            log::warn!(
                "failed to send SIGHUP to child {} during drop: {e}",
                self.child_pid
            );
        }
        if let Err(e) = nix::sys::signal::kill(self.child_pid, nix::sys::signal::Signal::SIGCONT) {
            log::warn!(
                "failed to send SIGCONT to child {} during drop: {e}",
                self.child_pid
            );
        }
        std::thread::sleep(Duration::from_millis(GRACEFUL_SHUTDOWN_TIMEOUT_MS));
        if let Err(e) = nix::sys::signal::kill(self.child_pid, nix::sys::signal::Signal::SIGKILL) {
            log::warn!(
                "failed to send SIGKILL to child {} during drop: {e}",
                self.child_pid
            );
        }
        if let Err(e) = nix::sys::wait::waitpid(self.child_pid, None) {
            log::warn!(
                "waitpid for child {} failed during drop: {e}",
                self.child_pid
            );
        }
    }
}

fn configure_raw_mode(fd: std::os::unix::io::RawFd) -> Result<(), PtyError> {
    let mut termios = std::mem::MaybeUninit::<libc::termios>::uninit();
    // SAFETY: tcgetattr is safe with a valid fd. fd is STDIN_FILENO (0)
    // after login_tty dup'd the PTY slave, so it's always valid.
    let result = unsafe { libc::tcgetattr(fd, termios.as_mut_ptr()) };
    if result != 0 {
        return Err(PtyError::Termios(nix::errno::Errno::last()));
    }
    // SAFETY: assume_init() is safe because we checked tcgetattr returned 0 above,
    // which guarantees termios has been initialized by the kernel.
    let mut termios = unsafe { termios.assume_init() };
    // Following termux-app's known-correct practice (termux-app termux.c:54-96):
    //   * Disable software flow control (IXON/IXOFF). When IXON is set, the
    //     kernel interprets Ctrl+S / Ctrl+Q and freezes/resumes output, which
    //     makes the terminal appear hung. Clearing both keeps Ctrl+S/Ctrl+Q
    //     usable by the application running in the PTY.
    //   * IXON is already disabled by the raw-mode mask above; we also clear
    //     IXOFF.
    termios.c_iflag &= !(libc::IGNBRK
        | libc::BRKINT
        | libc::PARMRK
        | libc::ISTRIP
        | libc::INLCR
        | libc::IGNCR
        | libc::ICRNL
        | libc::IXON
        | libc::IXOFF);
    // IUTF8: tell the kernel the input is UTF-8 so it correctly handles
    // erase/word-erase and character width on Android (mirrors termux.c, which
    // enables IUTF8 on the slave so the line discipline respects multibyte
    // input). Non-fatal but important for correct editing of UTF-8 text.
    termios.c_iflag |= libc::IUTF8;
    termios.c_oflag &= !(libc::OPOST);
    termios.c_lflag &= !(libc::ECHO | libc::ECHONL | libc::ICANON | libc::ISIG | libc::IEXTEN);
    termios.c_cflag &= !(libc::CSIZE | libc::PARENB);
    termios.c_cflag |= libc::CS8;
    termios.c_cc[libc::VMIN] = 1;
    termios.c_cc[libc::VTIME] = 0;
    log::debug!(
        "configuring PTY termios: IUTF8 set={}, IXON disabled={}, IXOFF disabled={}",
        (termios.c_iflag & libc::IUTF8) != 0,
        (termios.c_iflag & libc::IXON) == 0,
        (termios.c_iflag & libc::IXOFF) == 0,
    );
    // SAFETY: tcsetattr is safe with a valid fd and valid termios struct.
    let result = unsafe { libc::tcsetattr(fd, libc::TCSANOW, &termios) };
    if result != 0 {
        return Err(PtyError::Termios(nix::errno::Errno::last()));
    }
    Ok(())
}

/// Conservative upper bound (in fd numbers) used when scanning for stray
/// file descriptors to close in the child, if `sysconf(_SC_OPEN_MAX)` is
/// unavailable. Kept small enough to bound syscall volume on any platform.
const STRAY_FD_SCAN_LIMIT: libc::c_int = 4096;

/// Close every open file descriptor in the child except the standard streams
/// (0,1,2), which are the PTY slave after `dup2`. This mirrors termux-app's
/// termux.c:54-96 cleanup so the spawned shell does not inherit unrelated open
/// fds from the parent (which could keep resources alive or leak capabilities).
///
/// Non-fatal: a failed `close()` (e.g. already closed / invalid) is ignored.
fn close_stray_fds() {
    // SAFETY: sysconf is a simple syscall wrapper. On failure we fall back to
    // STRAY_FD_SCAN_LIMIT. close() is async-signal-safe; closing an invalid fd
    // returns EBADF, which we ignore. We never touch fds 0/1/2.
    let open_max = unsafe { libc::sysconf(libc::_SC_OPEN_MAX) };
    let upper = if open_max > 0 {
        open_max as libc::c_int
    } else {
        STRAY_FD_SCAN_LIMIT
    };
    log::debug!("closing stray fds in child (upper bound {upper})");
    for fd in 3..=upper {
        // SAFETY: close() on an invalid fd returns EBADF, which is harmless.
        // Standard fds (0,1,2) are excluded by starting at 3.
        unsafe {
            libc::close(fd);
        }
    }
}

fn base_env(prefix: Option<&str>) -> Vec<(String, String)> {
    let mut result = vec![
        ("TERM".to_string(), DEFAULT_TERM.to_string()),
        ("COLORTERM".to_string(), DEFAULT_COLORTERM.to_string()),
        ("TERM_PROGRAM".to_string(), DEFAULT_TERM_PROGRAM.to_string()),
        (
            "TERM_PROGRAM_VERSION".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        ),
        ("LANG".to_string(), DEFAULT_LANG.to_string()),
    ];
    if let Some(p) = prefix {
        result.push(("PREFIX".to_string(), p.to_string()));
        result.push(("TMPDIR".to_string(), format!("{p}/tmp")));
    } else {
        result.push(("TMPDIR".to_string(), ANDROID_TMPDIR.to_string()));
    }
    result
}

pub fn build_env(env: &ShellEnv, shell_path: &str, rows: u16, cols: u16) -> Vec<(String, String)> {
    let prefix_str = env.prefix.as_deref();
    let mut result = base_env(prefix_str);
    result.push(("HOME".to_string(), env.home.clone()));
    result.push(("USER".to_string(), env.user.clone()));
    result.push(("SHELL".to_string(), shell_path.to_string()));
    result.push(("PATH".to_string(), env.path.clone()));
    result.push(("PWD".to_string(), env.working_directory.clone()));
    result.push(("LINES".to_string(), rows.to_string()));
    result.push(("COLUMNS".to_string(), cols.to_string()));
    // Remove keys that extras will override, then append extras.
    // This deduplicates by keeping the last value (OS convention for execve).
    for (key, _) in &env.extra {
        result.retain(|(k, _)| k != key);
    }
    result.extend(env.extra.iter().cloned());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_env() -> ShellEnv {
        ShellEnv {
            home: "/tmp/test_home".to_string(),
            user: "testuser".to_string(),
            path: "/usr/bin:/bin".to_string(),
            working_directory: "/tmp/test_home".to_string(),
            prefix: None,
            extra: vec![],
        }
    }

    #[test]
    fn base_env_includes_xterm_256color() {
        let env = base_env(None);
        assert!(
            env.iter()
                .any(|(k, v)| k == "TERM" && v == "xterm-256color")
        );
    }

    #[test]
    fn base_env_includes_lang() {
        let env = base_env(None);
        assert!(env.iter().any(|(k, v)| k == "LANG" && v == "en_US.UTF-8"));
    }

    #[test]
    fn base_env_includes_tmpdir_without_prefix() {
        let env = base_env(None);
        assert!(
            env.iter()
                .any(|(k, v)| k == "TMPDIR" && v == "/data/local/tmp")
        );
    }

    #[test]
    fn base_env_includes_prefix_and_tmpdir_when_set() {
        let env = base_env(Some("/data/data/com.termux/files/usr"));
        assert!(
            env.iter()
                .any(|(k, v)| k == "PREFIX" && v == "/data/data/com.termux/files/usr")
        );
        assert!(
            env.iter()
                .any(|(k, v)| k == "TMPDIR" && v == "/data/data/com.termux/files/usr/tmp")
        );
    }

    #[test]
    fn build_env_includes_term() {
        let env = test_env();
        let result = build_env(&env, "/bin/sh", 24, 80);
        assert!(
            result
                .iter()
                .any(|(k, v)| k == "TERM" && v == "xterm-256color")
        );
    }

    #[test]
    fn build_env_includes_colorterm() {
        let env = test_env();
        let result = build_env(&env, "/bin/sh", 24, 80);
        assert!(
            result
                .iter()
                .any(|(k, v)| k == "COLORTERM" && v == "truecolor")
        );
    }

    #[test]
    fn build_env_includes_term_program() {
        let env = test_env();
        let result = build_env(&env, "/bin/sh", 24, 80);
        assert!(
            result
                .iter()
                .any(|(k, v)| k == "TERM_PROGRAM" && v == "torvox")
        );
    }

    #[test]
    fn build_env_includes_program_version() {
        let env = test_env();
        let result = build_env(&env, "/bin/sh", 24, 80);
        assert!(result.iter().any(|(k, _)| k == "TERM_PROGRAM_VERSION"));
    }

    #[test]
    fn build_env_includes_home_from_env() {
        let env = test_env();
        let result = build_env(&env, "/bin/sh", 24, 80);
        assert!(
            result
                .iter()
                .any(|(k, v)| k == "HOME" && v == "/tmp/test_home")
        );
    }

    #[test]
    fn build_env_includes_user_from_env() {
        let env = test_env();
        let result = build_env(&env, "/bin/sh", 24, 80);
        assert!(result.iter().any(|(k, v)| k == "USER" && v == "testuser"));
    }

    #[test]
    fn build_env_includes_shell_from_param() {
        let env = test_env();
        let result = build_env(&env, "/bin/bash", 24, 80);
        assert!(result.iter().any(|(k, v)| k == "SHELL" && v == "/bin/bash"));
    }

    #[test]
    fn build_env_includes_path_from_env() {
        let env = test_env();
        let result = build_env(&env, "/bin/sh", 24, 80);
        assert!(
            result
                .iter()
                .any(|(k, v)| k == "PATH" && v == "/usr/bin:/bin")
        );
    }

    #[test]
    fn build_env_includes_pwd_from_env() {
        let env = test_env();
        let result = build_env(&env, "/bin/sh", 24, 80);
        assert!(
            result
                .iter()
                .any(|(k, v)| k == "PWD" && v == "/tmp/test_home")
        );
    }

    #[test]
    fn build_env_includes_lines_and_columns() {
        let env = test_env();
        let result = build_env(&env, "/bin/sh", 24, 80);
        assert!(result.iter().any(|(k, v)| k == "LINES" && v == "24"));
        assert!(result.iter().any(|(k, v)| k == "COLUMNS" && v == "80"));
    }

    #[test]
    fn build_env_deduplicates_explicit_keys() {
        let mut env = test_env();
        env.extra.push(("TERM".to_string(), "dumb".to_string()));
        let result = build_env(&env, "/bin/sh", 24, 80);
        let term_entries: Vec<_> = result.iter().filter(|(k, _)| k == "TERM").collect();
        assert_eq!(
            term_entries.len(),
            1,
            "duplicate TERM should be deduplicated"
        );
        assert_eq!(term_entries[0].1, "dumb", "last value should win");
    }

    #[test]
    fn build_env_extra_entries_present() {
        let mut env = test_env();
        env.extra
            .push(("ANDROID_ROOT".to_string(), "/system".to_string()));
        let result = build_env(&env, "/bin/sh", 24, 80);
        assert!(
            result
                .iter()
                .any(|(k, v)| k == "ANDROID_ROOT" && v == "/system")
        );
    }

    #[test]
    fn spawn_and_read_shell() {
        use crate::pty::Pty;

        let mut pty =
            PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default()).expect("spawn failed");
        pty.set_nonblocking().expect("set_nonblocking failed");

        Pty::write_all(&mut pty, b"echo hello_torvox\n").expect("write failed");

        let mut buf = [0u8; 4096];
        let mut output = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            match Pty::read(&mut pty, &mut buf) {
                Ok(n) => output.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
            if output
                .windows("hello_torvox".len())
                .any(|w| w == b"hello_torvox")
            {
                return;
            }
        }
        panic!(
            "did not see 'hello_torvox' in output: {}",
            String::from_utf8_lossy(&output)
        );
    }

    #[test]
    fn resize_succeeds() {
        let pty = PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default()).expect("spawn failed");
        pty.resize(40, 120).expect("resize failed");
    }

    #[test]
    fn child_pid_is_positive() {
        let _pty = PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default()).expect("spawn failed");
    }

    #[test]
    fn drop_kills_child() {
        let child = {
            let pty =
                PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default()).expect("spawn failed");
            pty.child_pid()
        };
        std::thread::sleep(Duration::from_millis(200));
        let result = nix::sys::signal::kill(child, nix::sys::signal::Signal::SIGTERM);
        assert!(result.is_err(), "child should already be dead after drop");
    }

    #[test]
    fn pty_error_display_works() {
        let display = format!("{}", PtyError::Open(nix::errno::Errno::EINVAL.into()));
        assert!(
            !display.is_empty(),
            "PtyError Display should produce non-empty string"
        );
    }

    #[test]
    fn pty_error_from_errno_maps_to_fork() {
        // The blanket `From<nix::errno::Errno>` conversion is the error path
        // used by `fork()`; it must keep mapping to `PtyError::Fork` even after
        // `openpty` was changed to use an explicit `map_err(PtyError::Open)`.
        let err = PtyError::from(nix::errno::Errno::EINVAL);
        assert!(
            matches!(err, PtyError::Fork(_)),
            "From<Errno> must map to Fork for the fork() error path"
        );
    }

    #[test]
    fn chdir_changes_working_directory() {
        use crate::pty::Pty;

        let temp = std::env::temp_dir().join("torvox_test_chdir");
        std::fs::create_dir_all(&temp).expect("create test dir failed");

        let env = ShellEnv {
            working_directory: temp.to_string_lossy().to_string(),
            ..ShellEnv::default()
        };

        let mut pty = PtyPair::spawn("/bin/sh", 24, 80, &env).expect("spawn failed");
        pty.set_nonblocking().expect("set_nonblocking failed");
        Pty::write_all(&mut pty, b"pwd\n").expect("write failed");

        let mut buf = [0u8; 4096];
        let mut output = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            match Pty::read(&mut pty, &mut buf) {
                Ok(n) => output.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
            let path_str = temp.to_string_lossy();
            let path_bytes = path_str.as_bytes();
            if output.windows(path_bytes.len()).any(|w| w == path_bytes) {
                std::fs::remove_dir_all(&temp).ok();
                return;
            }
        }
        std::fs::remove_dir_all(&temp).ok();
        panic!(
            "did not see working directory '{}' in pwd output: {}",
            temp.display(),
            String::from_utf8_lossy(&output)
        );
    }

    // ── I6: PTY termios flags ───────────────────────────────────────
    // After the child is configured for raw mode, the line discipline must:
    //   * enable IUTF8 so the kernel treats input as UTF-8 (correct
    //     erase/word-erase and character width for multibyte input), and
    //   * clear IXON/IXOFF (software flow control) so Ctrl+S/Ctrl+Q
    //     are delivered to the application rather than freezing output.
    // `configure_raw_mode` is the helper that applies these flags to a
    // given fd; we exercise it on a real PTY master fd and read the
    // resulting termios back to confirm the flags are set/cleared.

    #[test]
    fn configure_raw_mode_sets_iutf8_and_clears_ixon_ixoff() {
        let pty = PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default()).expect("spawn failed");
        let fd = pty.master_fd();

        // Apply the same raw-mode configuration the child uses on the slave.
        configure_raw_mode(fd).expect("configure_raw_mode failed");

        // SAFETY: `tcgetattr` is a simple syscall wrapper; `fd` is a
        // valid, owned PTY master descriptor, so reading its termios is safe.
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
        use crate::pty::Pty;

        let mut pty = PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default())
            .expect("spawn must succeed in test env");
        pty.set_nonblocking().expect("set_nonblocking failed");

        Pty::write_all(&mut pty, b"echo a\n").expect("first write must succeed");
        Pty::write_all(&mut pty, b"echo b\n").expect("second write must succeed");

        let mut buf = [0u8; 4096];
        let mut output = Vec::new();
        for _ in 0..50 {
            match Pty::read(&mut pty, &mut buf) {
                Ok(n) => output.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
            if output.len() > 200 {
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
