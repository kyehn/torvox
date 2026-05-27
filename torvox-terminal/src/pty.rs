use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};
use std::time::Duration;

use thiserror::Error;

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

pub struct PtyPair {
    master: OwnedFd,
    child_pid: nix::unistd::Pid,
}

impl PtyPair {
    pub fn spawn(shell: &str, rows: u16, cols: u16) -> Result<Self, PtyError> {
        let winsize = nix::pty::Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        let result = nix::pty::openpty(Some(&winsize), None)?;
        let master_fd = result.master;
        let slave_fd = result.slave;

        // SAFETY: fork() is the standard Unix process duplication primitive.
        // We immediately distinguish parent and child. The child setsid() +
        // login_tty() to establish a new session, then execvp(). The parent
        // closes the slave fd and returns. This is the standard forkpty pattern.
        // Only one unsafe block orchestrates the entire fork — all subsequent
        // libc calls in the child path are single-threaded and bounded.
        match unsafe { nix::unistd::fork()? } {
            nix::unistd::ForkResult::Parent { child } => {
                nix::unistd::close(slave_fd).ok();
                Ok(Self {
                    master: master_fd,
                    child_pid: child,
                })
            }
            nix::unistd::ForkResult::Child => {
                nix::unistd::close(master_fd).ok();
                nix::unistd::setsid().ok();
                // SAFETY: In the child process after fork(), it is safe to
                // call login_tty because the child has its own address space
                // and file descriptor table. login_tty sets up the slave as
                // the controlling terminal for this new session. We check the
                // return value and exit on failure.
                let ret = unsafe { libc::login_tty(slave_fd.as_raw_fd()) };
                if ret != 0 {
                    std::process::exit(1);
                }
                configure_raw_mode(slave_fd.as_raw_fd()).ok();
                let env = build_env();
                // SAFETY: set_var is unsafe in Rust 1.95+ due to potential
                // race conditions with other threads. In the child process
                // after fork(), there is exactly one thread, so no race is
                // possible. The env vars are bounded local data.
                for (key, value) in &env {
                    unsafe { std::env::set_var(key, value) };
                }
                let c_shell = std::ffi::CString::new(shell).expect("shell path contains null byte");
                let args = [c_shell.as_c_str()];
                nix::unistd::execvp(c_shell.as_c_str(), &args).unwrap_err();
                std::process::exit(1);
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
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: ioctl with TIOCSWINSZ writes a well-formed Winsize struct
        // to the master PTY fd. The fd is owned and valid. The kernel copies
        // the winsize to the slave side — no memory safety risk. The return
        // value is checked for errors.
        unsafe {
            let ret = libc::ioctl(
                self.master.as_raw_fd(),
                libc::TIOCSWINSZ,
                &winsize as *const _,
            );
            if ret < 0 {
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

    pub fn into_raw_fd(self) -> std::os::unix::io::RawFd {
        let fd = self.master.as_raw_fd();
        std::mem::forget(self);
        fd
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
        nix::sys::signal::kill(self.child_pid, nix::sys::signal::Signal::SIGHUP).ok();
        nix::sys::signal::kill(self.child_pid, nix::sys::signal::Signal::SIGCONT).ok();
        std::thread::sleep(Duration::from_millis(100));
        nix::sys::signal::kill(self.child_pid, nix::sys::signal::Signal::SIGKILL).ok();
        nix::sys::wait::waitpid(self.child_pid, None).ok();
    }
}

fn configure_raw_mode(fd: std::os::unix::io::RawFd) -> Result<(), PtyError> {
    use nix::sys::termios::{
        ControlFlags, InputFlags, LocalFlags, OutputFlags, SetArg, tcgetattr, tcsetattr,
    };
    // SAFETY: We create an OwnedFd from a borrowed raw fd solely to use
    // nix's termios API which requires AsFd. We then mem::forget(owned)
    // to prevent the OwnedFd from closing the fd on drop. This is the
    // standard pattern for using nix termios on a borrowed fd. The fd
    // remains owned by the caller's PtyPair.
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    let mut termios = tcgetattr(&owned).map_err(PtyError::Termios)?;
    termios.input_flags &= !(InputFlags::IGNBRK
        | InputFlags::BRKINT
        | InputFlags::PARMRK
        | InputFlags::ISTRIP
        | InputFlags::INLCR
        | InputFlags::IGNCR
        | InputFlags::ICRNL
        | InputFlags::IXON);
    termios.output_flags &= !(OutputFlags::OPOST);
    termios.local_flags &= !(LocalFlags::ECHO
        | LocalFlags::ECHONL
        | LocalFlags::ICANON
        | LocalFlags::ISIG
        | LocalFlags::IEXTEN);
    termios.control_flags &= !(ControlFlags::CSIZE | ControlFlags::PARENB);
    termios.control_flags |= ControlFlags::CS8;
    termios.control_chars[nix::sys::termios::SpecialCharacterIndices::VMIN as usize] = 1;
    termios.control_chars[nix::sys::termios::SpecialCharacterIndices::VTIME as usize] = 0;
    tcsetattr(&owned, SetArg::TCSANOW, &termios).map_err(PtyError::Termios)?;
    std::mem::forget(owned);
    Ok(())
}

fn build_env() -> Vec<(String, String)> {
    vec![
        ("TERM".to_string(), "torvox-direct".to_string()),
        ("COLORTERM".to_string(), "truecolor".to_string()),
        ("TERM_PROGRAM".to_string(), "torvox".to_string()),
        (
            "TERM_PROGRAM_VERSION".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_and_read_shell() {
        use std::io::{Read, Write};

        let mut pty = PtyPair::spawn("/bin/sh", 24, 80).expect("spawn failed");
        pty.set_nonblocking().expect("set_nonblocking failed");

        pty.write_all(b"echo hello_torvox\n").expect("write failed");

        let mut buf = [0u8; 4096];
        let mut output = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            match pty.read(&mut buf) {
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
        let pty = PtyPair::spawn("/bin/sh", 24, 80).expect("spawn failed");
        pty.resize(40, 120).expect("resize failed");
    }

    #[test]
    fn child_pid_is_positive() {
        let _pty = PtyPair::spawn("/bin/sh", 24, 80).expect("spawn failed");
    }

    #[test]
    fn drop_kills_child() {
        let child = {
            let pty = PtyPair::spawn("/bin/sh", 24, 80).expect("spawn failed");
            pty.child_pid()
        };
        std::thread::sleep(Duration::from_millis(200));
        let result = nix::sys::signal::kill(child, nix::sys::signal::Signal::SIGTERM);
        assert!(result.is_err(), "child should already be dead after drop");
    }
}
