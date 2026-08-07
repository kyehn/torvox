//! PTY master/slave creation — only allowed fork unsafe.
//!
//! # Requirements
//! - [FR-026](crate) — PTY: master/slave pair creation
use std::io;
use std::os::unix::io::{AsRawFd, OwnedFd, RawFd};
use std::time::Duration;

use thiserror::Error;

use crate::terminal::shell_env::ShellEnv;

const DEFAULT_TERM: &str = "xterm-256color";
const DEFAULT_COLORTERM: &str = "truecolor";
const DEFAULT_TERM_PROGRAM: &str = "terminal";
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
    /// Query the current terminal window size (rows x cols) via
    /// TIOCGWINSZ. Round-224: used to verify the 24x80 spawn seed.
    fn get_winsize(&self) -> Result<(u16, u16), PtyError>;
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
    fn spawn(shell: &str, rows: u16, cols: u16, env: &ShellEnv) -> Result<Box<dyn Pty>, PtyError>
    where
        Self: Sized;

    fn write_all(&mut self, mut buf: &[u8]) -> io::Result<()> {
        while !buf.is_empty() {
            let bytes_written = self.write(buf)?;
            if bytes_written == 0 {
                // Non-blocking master returns EAGAIN via write(), never a
                // 0 count, but a test double or unusual backend could: an
                // infinite loop on 0 is worse than a spurious WouldBlock.
                return Err(io::Error::from(io::ErrorKind::WouldBlock));
            }
            buf = &buf[bytes_written..];
        }
        Ok(())
    }
}

/// A PTY pair consisting of a master file descriptor and child process.
///
/// The master side is used by the terminal emulator to read output and write input.
/// The child process runs the shell and communicates via the slave side.
pub struct PtyPair {
    master: OwnedFd,
    child_pid: nix::unistd::Pid,
    pixel_width: u16,
    pixel_height: u16,
}

impl PtyPair {
    /// Spawn a child process in a new PTY.
    ///
    /// # async-signal-safety
    ///
    /// After `fork()`, the child process must only call async-signal-safe
    /// functions. All heap allocation (`CString::new`, `format!`, `log!`,
    /// `println!`, etc.) happens **before** the fork. The child path uses
    /// only `execvp`, `dup2`, `close`, `write(2)`, and `_exit(2)` which
    /// are all async-signal-safe in a single-threaded child.
    ///
    /// **DO NOT add `log::debug!`, `format!`, or any allocation in the
    /// child branch after `fork()`.**
    ///
    /// Reference: warp-mobile-android crates/android-host/src/pty.rs:106-160
    /// — identical AS-safe discipline (pre-built CStrings, setsid +
    /// TIOCSCTTY, 24x80 TIOCSWINSZ seed, errno via write(2) on execve
    /// failure). torvox encodes the execve errno in the exit code
    /// (100 + errno) decoded by the wait thread; warp writes it directly
    /// to stderr. Both are correct; the write(2) variant is more
    /// immediately visible in logcat.
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
        // Round-215: Android 15+ SELinux denies `execute_no_trans` on
        // app_data_file for untrusted_app, so execve() of Termux binaries
        // under $PREFIX fails with EACCES. The Termux solution: exec the
        // system linker with the ELF path as its argument
        // (`/system/bin/linker64 $PREFIX/bin/bash`) — the linker runs in
        // system_linker_exec domain and loads app-data ELFs fine. The
        // child also gets LD_PRELOAD=$PREFIX/lib/libtermux-exec.so so
        // *its own* execve() calls (running `ls`, `apt`, ...) go through
        // the same linker indirection.
        let prefix = env.prefix.as_deref().unwrap_or("");
        let use_linker =
            !prefix.is_empty() && shell.starts_with(&format!("{prefix}/")) && !shell.contains('\0');
        if use_linker {
            log::info!("SPAWN_LINKER: shell={shell} prefix={prefix}");
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
        let shell_ptr = shell_cstr.as_ptr();
        // execve()'s FIRST argument is the executable PATH; argv[0] is
        // passed separately in args_ptrs. With the linker, the path is
        // /system/bin/linker64 and argv = [linker64, bash].
        let exec_path_ptr = linker_cstr.as_ref().map_or(shell_ptr, |c| c.as_ptr());
        let working_directory_ptr = working_directory_cstr.as_ptr();
        let args_ptrs: Vec<*const libc::c_char> = if let Some(linker_cstr) = &linker_cstr {
            vec![linker_cstr.as_ptr(), shell_ptr, std::ptr::null()]
        } else {
            vec![shell_ptr, std::ptr::null()]
        };
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
                    // close(2) failure before execve is harmless; ignore
                    // silently (no allocation/logging allowed post-fork).
                    let _ = e;
                }
                // Manually set controlling terminal using only syscalls.
                // Avoid login_tty because it may call malloc() internally,
                // which is unsafe after fork in a multithreaded process.
                // Create a new session/process group so the shell is detached
                // from the parent's controlling terminal (termux.c:54-96). Only
                // call setsid() if we are not already a session leader, since
                // calling it again would fail with EPERM.
                // SAFETY: getsid(0) is a POSIX function that returns the calling
                // process's session ID. Passing 0 is always valid and safe in
                // the single-threaded child after fork.
                let is_session_leader =
                    unsafe { libc::getsid(0) } == nix::unistd::getpid().as_raw();
                if !is_session_leader && nix::unistd::setsid().is_err() {
                    // nosemgrep: semgrep.no-process-exit — child process after fork, must not longjmp
                    unsafe {
                        libc::_exit(2);
                    }
                }
                // Detect a fork-time orphan: if the app process died between
                // fork() and this check, the child was reparented to init
                // (PPid == 1). Exit instead of leaking a permanent orphan.
                // NOTE (round-213): PR_SET_PDEATHSIG is deliberately NOT
                // used. It binds to the fork thread (a Kotlin
                // Dispatchers.IO worker), and when that thread times out
                // and exits after ~60s of idle (kotlinx KEEP_ALIVE), the
                // shell is killed by the parent-death signal — observed on
                // the emulator (Android 15 GKI 6.6): every session died
                // ~60s after startup, and disabling PDEATHSIG made shells
                // survive indefinitely. (Whether this is the standard
                // thread-group semantics or an Android kernel nuance is
                // not asserted here; the empirical result is what matters.)
                // App-death cleanup is covered by Android's cgroup.kill
                // (AMS/lmkd kill the app's process group on force-stop,
                // crash and OOM — verified on-device: force-stop reaps the
                // shell), so PDEATHSIG is redundant as well as harmful.
                // SAFETY: getppid is a plain syscall; safe after fork.
                let orphaned = unsafe { libc::getppid() } == 1;
                if orphaned {
                    // nosemgrep: semgrep.no-process-exit — child after fork
                    unsafe {
                        libc::_exit(1);
                    }
                }
                let slave_raw = slave_fd.as_raw_fd();
                // SAFETY: All these libc calls are lightweight syscall wrappers that do not allocate.
                // The child process is single-threaded. No signal handlers run between fork and exec
                // (all operations are async-signal-safe syscalls).
                let result = unsafe { libc::ioctl(slave_raw, libc::TIOCSCTTY, 0) };
                if result < 0 {
                    // nosemgrep: semgrep.no-process-exit — child process after fork, must not longjmp
                    unsafe {
                        libc::_exit(3);
                    }
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
                // SAFETY: tcgetattr/tcsetattr are lightweight syscall wrappers, safe in
                // single-threaded child after fork.
                configure_raw_mode_child(libc::STDIN_FILENO);
                // SAFETY: chdir is safe with a valid, null-terminated path string.
                // working_directory_ptr was allocated via CString::new() which guarantees
                // null termination. Failure is non-fatal (defaults to /).
                if unsafe { libc::chdir(working_directory_ptr) } != 0 {
                    const MSG: &[u8] = b"chdir to working directory failed, using /\n";
                    // SAFETY: write(2) is async-signal-safe in single-threaded child after fork.
                    // MSG is a static byte slice (no allocation).
                    let _ =
                        unsafe { libc::write(2, MSG.as_ptr() as *const libc::c_void, MSG.len()) };
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
                // SAFETY: execve with pre-allocated arrays (see above).
                // exec_path_ptr is the linker when use_linker, else shell.
                unsafe {
                    libc::execve(exec_path_ptr, args_ptrs.as_ptr(), env_ptrs.as_ptr());
                }
                // execve only returns on failure. Write the errno to the PTY
                // (async-signal-safe: static buffer + manual itoa) so a
                // failing shell path is diagnosable on-device, then _exit
                // (not exit) to avoid running atexit handlers from the
                // parent process (round-215: bash exited 4 with no output).
                unsafe {
                    // nix::errno::errno() reads the thread-local errno
                    // (no allocation); platform-specific underneath.
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
                    // Encode errno in the exit code (>= 100) so the parent's
                    // wait thread can log the exact failure cause even when
                    // the PTY output is lost to a destroy race (round-215).
                    let code = 100 + (errno as i32).min(155);
                    libc::_exit(code);
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

    /// Query the current terminal window size via TIOCGWINSZ. Round-224:
    /// verifies the 24x80 spawn seed is in place before any resize.
    pub fn get_winsize(&self) -> Result<(u16, u16), PtyError> {
        let mut winsize = nix::pty::Winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: ioctl TIOCGWINSZ fills a well-formed Winsize struct from
        // the kernel; the fd is owned and valid. The struct is fully
        // initialized before use.
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
        Ok((winsize.ws_row, winsize.ws_col))
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

    fn wait(&self) -> nix::Result<nix::sys::wait::WaitStatus> {
        PtyPair::wait(self)
    }

    fn set_nonblocking(&self) -> Result<(), PtyError> {
        PtyPair::set_nonblocking(self)
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
        // Use WNOHANG + retry loop to avoid blocking Drop.
        // SIGKILL was sent above, so reaping should complete quickly.
        // ECHILD means already reaped by another thread.
        for _attempt in 0..10 {
            match nix::sys::wait::waitpid(
                self.child_pid,
                Some(nix::sys::wait::WaitPidFlag::WNOHANG),
            ) {
                Ok(
                    nix::sys::wait::WaitStatus::Exited(_, _)
                    | nix::sys::wait::WaitStatus::Signaled(_, _, _),
                ) => break,
                Ok(_) => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(nix::errno::Errno::ECHILD) => break,
                Err(e) => {
                    log::warn!("waitpid for child {} failed: {e}", self.child_pid);
                    break;
                }
            }
        }
    }
}

#[cfg(any(test, feature = "test-util"))]
#[allow(dead_code)]
fn configure_raw_mode(fd: std::os::unix::io::RawFd) -> Result<(), PtyError> {
    let mut termios = std::mem::MaybeUninit::<libc::termios>::uninit();
    // SAFETY: tcgetattr is safe with a valid fd. The caller must pass a
    // terminal fd it holds (tests pass the PTY master; production paths
    // pass the slave after login).
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
    //   * IUTF8: tell the kernel the input is UTF-8 so it correctly handles
    //     erase/word-erase and character width on Android.
    // NOTE (round-217): we deliberately do NOT clear ECHO/ICANON/ISIG here.
    // Termux leaves the line discipline canonical with echo on; bash readline
    // (which re-configures the tty itself on startup) then echoes typed
    // characters. A full raw mask breaks readline echo (see
    // configure_raw_mode_child for the full analysis).
    termios.c_iflag &= !(libc::IXON | libc::IXOFF);
    termios.c_iflag |= libc::IUTF8;
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

/// Terminal-mode setup for the child after fork, matching Termux's
/// termux.c (terminal-emulator/src/main/jni/termux.c): the PTY keeps the
/// kernel line discipline (ECHO+ICANON on) so the shell's own line editor
/// (bash readline) performs echo, and interactive full-screen programs
/// (vim, less) put the tty into raw mode themselves when they start.
///
/// A full cfmakeraw() here (ECHO|ICANON|ISIG off) breaks bash readline:
/// round-217 observed typed characters reaching the shell (commands
/// executed) with zero echo — readline does not re-enable echo when the
/// termios it inherits already has ECHO cleared, and with ISIG off it
/// skips its signal setup. Termux intentionally avoids this.
///
/// Async-signal-safe: does NOT allocate, does NOT call log::warn!.
/// Silently ignores errors (non-fatal).
fn configure_raw_mode_child(fd: std::os::unix::io::RawFd) {
    let mut termios = std::mem::MaybeUninit::<libc::termios>::uninit();
    // SAFETY: tcgetattr is a syscall wrapper, safe in single-threaded child after fork.
    if unsafe { libc::tcgetattr(fd, termios.as_mut_ptr()) } != 0 {
        return; // Non-fatal
    }
    // SAFETY: assume_init() after successful tcgetattr.
    let mut termios = unsafe { termios.assume_init() };
    // Match termux.c: enable UTF-8 mode, disable flow control so Ctrl+S
    // cannot lock the display. Everything else (ECHO, ICANON, ISIG, OPOST)
    // stays at the kernel default.
    termios.c_iflag |= libc::IUTF8;
    termios.c_iflag &= !(libc::IXON | libc::IXOFF);
    // SAFETY: tcsetattr is a syscall wrapper, safe in child.
    let _ = unsafe { libc::tcsetattr(fd, libc::TCSANOW, &termios) };
}

/// Conservative upper bound (in fd numbers) used when scanning for stray
/// file descriptors to close in the child, if `sysconf(_SC_OPEN_MAX)` is
/// unavailable. Kept small enough to bound syscall volume on any platform.
/// Maximum fd number to scan in `close_stray_fds`. Chosen as a safe
/// upper bound that prevents pathological scan times on systems where
/// `sysconf(_SC_OPEN_MAX)` reports a very large value (e.g. 1 M+).
const STRAY_FD_SCAN_LIMIT: libc::c_int = 65536;

/// Close every open file descriptor in the child except the standard streams
/// (0,1,2), which are the PTY slave after `dup2`. This mirrors termux-app's
/// termux.c:54-96 cleanup so the spawned shell does not inherit unrelated open
/// fds from the parent (which could keep resources alive or leak capabilities).
///
/// NOTE: This is a best-effort cleanup in the single-threaded child after fork
/// but before exec. Any fd not in {0,1,2} is closed unconditionally — no
/// white-list (e.g., for fd-passing socket pairs) is supported. If the calling
/// process holds an fd from a library (e.g., log forwarding), it will be closed.
/// This is acceptable because the child immediately exec()s into the shell.
///
/// Non-fatal: a failed `close()` (e.g. already closed / invalid) is ignored.
fn close_stray_fds() {
    // SAFETY: getrlimit(2) is in the POSIX async-signal-safe list. It is
    // safe to call from a thread forked from a multi-threaded process if
    // RLIMIT_NOFILE has not been modified concurrently (it never is here).
    let mut rlim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let upper = if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut rlim) } == 0 {
        // rlim_cur is the soft limit (typically 1024-4096 on modern Linux).
        // This is vastly cheaper than iterating to sysconf(OPEN_MAX) which can
        // return ~1M on systemd-based systems.
        // RLIM_INFINITY means "unlimited" — fall back to the static limit
        // to avoid scanning to c_int::MAX (~2 billion).
        if rlim.rlim_cur == libc::RLIM_INFINITY {
            STRAY_FD_SCAN_LIMIT
        } else {
            // Cap at STRAY_FD_SCAN_LIMIT to avoid iterating millions of fds
            // in containers with large RLIMIT_NOFILE.
            rlim.rlim_cur
                .min(STRAY_FD_SCAN_LIMIT as u64)
                .min(libc::c_int::MAX as u64) as libc::c_int
        }
    } else {
        STRAY_FD_SCAN_LIMIT
    };
    // NOTE: cannot use log here because this may run in the child after fork
    // (not async-signal-safe). Errors are silent by design.
    for fd in 3..=upper {
        // SAFETY: close(2) on an invalid fd returns EBADF (harmless),
        // so there is no risk of double-close or invalid-fd crash.
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
    // Round-217 (recheck round-3): passthrough of Android system env vars.
    // Reference (termux-kotlin AndroidShellEnvironment.kt:19-66,
    // https://github.com/reapercanuk39/termux-kotlin-app):
    //   ANDROID_ASSETS/ANDROID_DATA/ANDROID_ROOT/ANDROID_STORAGE/
    //   EXTERNAL_STORAGE/ASEC_MOUNTPOINT/LOOP_MOUNTPOINT/
    //   ANDROID_RUNTIME_ROOT/ANDROID_ART_ROOT/ANDROID_I18N_ROOT/
    //   ANDROID_TZDATA_ROOT/BOOTCLASSPATH/DEX2OATBOOTCLASSPATH/
    //   SYSTEMSERVERCLASSPATH
    // EXTERNAL_STORAGE is required for /system/bin/am to work on at
    // least Samsung S7 (termux-kotlin comment); a Termux-bootstrap shell
    // that invokes `am`/`content` needs these. Only vars present in the
    // host process env are forwarded (values differ per device/Android
    // version — hardcoding like the old ANDROID_ROOT=/system would be
    // wrong on API 26+ where ANDROID_ROOT may be /system or /system_ext).
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
        if let Ok(value) = std::env::var(key) {
            result.push((key.to_string(), value));
        }
    }
    if let Some(p) = prefix {
        result.push(("PREFIX".to_string(), p.to_string()));
        result.push(("TMPDIR".to_string(), format!("{p}/tmp")));
    } else {
        result.push(("TMPDIR".to_string(), ANDROID_TMPDIR.to_string()));
    }
    result
}

/// Build the environment variables for the child process.
///
/// Combines the base system environment with user-specified overrides from
/// `ShellEnv`, including HOME, USER, TERM, and terminal size variables.
pub fn build_env(env: &ShellEnv, shell_path: &str, rows: u16, cols: u16) -> Vec<(String, String)> {
    let prefix_str = env.prefix.as_deref();
    let mut result = base_env(prefix_str);
    // Round-215: LD_PRELOAD libtermux-exec for $PREFIX shells — it
    // wraps execve() so child processes of the shell (ls, apt, ...) are
    // executed via the system linker, which Android 15+ SELinux allows
    // (direct execute_no_trans of app_data_file is denied).
    //
    // The direct-ld-preload variant (`libtermux-exec-ld-preload.so`) is
    // the one that intercepts execve from an LD_PRELOAD context; the bare
    // `libtermux-exec.so` is the runtime library the preload variant
    // dlopens (naming from termux-exec's lib/ld-preload build). Falling
    // back to the bare name keeps older bootstraps working.
    if let Some(p) = prefix_str {
        let exec_lib =
            if std::path::Path::new(&format!("{p}/lib/libtermux-exec-ld-preload.so")).exists() {
                format!("{p}/lib/libtermux-exec-ld-preload.so")
            } else {
                format!("{p}/lib/libtermux-exec.so")
            };
        result.push(("LD_PRELOAD".to_string(), exec_lib));
    }
    result.push(("HOME".to_string(), env.home.clone()));
    result.push(("USER".to_string(), env.user.clone()));
    result.push(("SHELL".to_string(), shell_path.to_string()));
    result.push(("PATH".to_string(), env.path.clone()));
    result.push(("PWD".to_string(), env.working_directory.clone()));
    result.push(("LINES".to_string(), rows.to_string()));
    result.push(("COLUMNS".to_string(), cols.to_string()));
    // Reference (zed-android-port adapters/bootstrap.rs env_for_terminal
    // :386-434, https://github.com/GeneralKaos666/zed-android-port):
    // a Termux-bootstrap PTY also needs TERMUX__ROOTFS / TERMUX__PREFIX /
    // TERMUX__HOME / TERMUX_APP__PACKAGE_NAME / HOME=$termux_home and,
    // when $PREFIX/etc/tls/cert.pem exists, SSL_CERT_FILE + CURL_CA_BUNDLE
    // so cargo/npm/curl don't fail with "unable to get local issuer
    // certificate". torvox currently sets only LD_PRELOAD + HOME/USER/... ;
    // the Termux-var block + cert vars are a P0 bootstrap-env gap.
    // Reference (std::env overlay): terminal.rs insert_zed_terminal_env
    // :123-161 copies HOME/PATH/SHELL/TMPDIR/LANG then applies the overlay.
    // Kill-chain reference: terminal.rs kill_active_task (:2276-2288)
    // calls pty_info.rs kill_current_process (:144-149, tcgetpgrp ->
    // killpg SIGKILL on the foreground process GROUP) then
    // kill_child_process (:156-158, single-pid SIGKILL); torvox
    // session.rs send_signal() only kills the direct child pid.
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
                .any(|(k, v)| k == "TERM_PROGRAM" && v == "terminal")
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
    fn base_env_passthrough_android_vars_from_host() {
        // Round-217: Android system env vars present in the host process
        // env must be forwarded (termux-kotlin AndroidShellEnvironment
        // pattern). Set one and verify it appears; unset others stay absent.
        unsafe {
            std::env::set_var("ANDROID_ROOT", "/system_ext");
        }
        unsafe {
            std::env::set_var("EXTERNAL_STORAGE", "/sdcard");
        }
        let result = base_env(None);
        assert!(
            result
                .iter()
                .any(|(k, v)| k == "ANDROID_ROOT" && v == "/system_ext"),
            "host ANDROID_ROOT must be forwarded as-is, not hardcoded"
        );
        assert!(
            result
                .iter()
                .any(|(k, v)| k == "EXTERNAL_STORAGE" && v == "/sdcard")
        );
        // Cleanup: unset so other tests are not polluted.
        unsafe {
            std::env::remove_var("ANDROID_ROOT");
            std::env::remove_var("EXTERNAL_STORAGE");
        }
    }

    #[test]
    fn base_env_does_not_invent_android_vars() {
        // Vars absent from the host env must NOT be fabricated (no
        // hardcoded /system default — values differ per Android version).
        unsafe {
            std::env::remove_var("ANDROID_ROOT");
            std::env::remove_var("BOOTCLASSPATH");
        }
        let result = base_env(None);
        assert!(
            !result.iter().any(|(k, _)| k == "ANDROID_ROOT"),
            "absent host var must stay absent"
        );
        assert!(!result.iter().any(|(k, _)| k == "BOOTCLASSPATH"));
    }

    #[test]
    fn spawn_and_read_shell() {
        use crate::terminal::pty::Pty;

        let mut pty =
            PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default()).expect("spawn failed");
        pty.set_nonblocking().expect("set_nonblocking failed");

        Pty::write_all(&mut pty, b"echo hello_vt\n").expect("write failed");

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
            if output.windows("hello_vt".len()).any(|w| w == b"hello_vt") {
                return;
            }
        }
        panic!(
            "did not see 'hello_vt' in output: {}",
            String::from_utf8_lossy(&output)
        );
    }

    #[test]
    fn resize_succeeds() {
        let pty = PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default()).expect("spawn failed");
        pty.resize(40, 120).expect("resize failed");
    }

    #[test]
    fn spawn_seeds_24x80_winsize() {
        // Round-224 (warp WarpTerminalService.kt:797-808): a TIOCGWINSZ
        // before any UI-driven resize must return the seeded 24x80, so
        // shells (zsh ZLE etc.) never see 0x0.
        let pty = PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default()).expect("spawn failed");
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
        let pty = PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default()).expect("spawn failed");
        pty.resize(40, 120).expect("resize failed");
        let (rows, cols) = pty.get_winsize().expect("TIOCGWINSZ failed");
        assert_eq!(
            (rows, cols),
            (40, 120),
            "resize must be reflected in TIOCGWINSZ"
        );
    }

    #[test]
    fn child_pid_is_positive() {
        let pty = PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default()).expect("spawn failed");
        let pid = pty.child_pid();
        assert!(
            pid.as_raw() > 0,
            "child PID should be positive, got {pid:?}"
        );
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
        use crate::terminal::pty::Pty;

        let temp = std::env::temp_dir().join("vt_test_chdir");
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
        use crate::terminal::pty::Pty;

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

#[cfg(test)]
mod round215_tests {
    use super::*;

    #[test]
    fn build_env_adds_ld_preload_for_prefixed_shell() {
        let env = ShellEnv {
            home: "/data/user/0/com.termux/files/home".to_string(),
            user: "shell".to_string(),
            path: "/data/user/0/com.termux/files/usr/bin:/system/bin".to_string(),
            working_directory: "/data/user/0/com.termux/files/home".to_string(),
            prefix: Some("/data/user/0/com.termux/files/usr".to_string()),
            extra: vec![],
        };
        let result = build_env(&env, "/data/user/0/com.termux/files/usr/bin/bash", 24, 80);
        assert!(
            result.iter().any(|(k, v)| {
                k == "LD_PRELOAD" && v == "/data/user/0/com.termux/files/usr/lib/libtermux-exec.so"
            }),
            "LD_PRELOAD must point at libtermux-exec.so for prefixed shells"
        );
    }

    #[test]
    fn build_env_no_ld_preload_without_prefix() {
        let env = ShellEnv {
            home: "/tmp/test_home".to_string(),
            user: "testuser".to_string(),
            path: "/usr/bin:/bin".to_string(),
            working_directory: "/tmp/test_home".to_string(),
            prefix: None,
            extra: vec![],
        };
        let result = build_env(&env, "/system/bin/sh", 24, 80);
        assert!(
            !result.iter().any(|(k, _)| k == "LD_PRELOAD"),
            "no LD_PRELOAD for system shells"
        );
    }
}

#[test]
fn build_env_uses_ld_preload_variant_when_present() {
    // The direct-ld-preload variant is preferred (SELinux execve
    // interception); fall back to the bare name for old bootstraps.
    let env = ShellEnv {
        home: "/tmp/test_home".to_string(),
        user: "testuser".to_string(),
        path: "/usr/bin:/bin".to_string(),
        working_directory: "/tmp/test_home".to_string(),
        prefix: Some("/data/data/com.termux/files/usr".to_string()),
        extra: vec![],
    };
    let result = build_env(&env, "/data/data/com.termux/files/usr/bin/bash", 24, 80);
    let preload = result
        .iter()
        .find(|(k, _)| k == "LD_PRELOAD")
        .map(|(_, v)| v.clone());
    // On the build host the preload variant does not exist, so the
    // fallback bare name is used — the important contract is that an
    // LD_PRELOAD pointing at the prefix lib dir is always set.
    assert!(
        preload
            .as_deref()
            .is_some_and(|v| v.starts_with("/data/data/com.termux/files/usr/lib/libtermux-exec")),
        "expected LD_PRELOAD=.../libtermux-exec*.so, got {preload:?}",
    );
}
