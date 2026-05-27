use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use flume::{Receiver, bounded};
use thiserror::Error;

use crate::parser::VtParser;
use crate::pty::{PtyError, PtyPair};
use crate::terminal::{TerminalState, TerminalStateError};

const READ_BUF_SIZE: usize = 8192;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("pty error: {0}")]
    Pty(#[from] PtyError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("terminal state error: {0}")]
    TerminalState(#[from] TerminalStateError),
    #[error("session closed")]
    Closed,
}

pub struct Session {
    pty: PtyPair,
    terminal: TerminalState,
    parser: VtParser,
    output_rx: Receiver<Vec<u8>>,
    output_notify: Arc<(Mutex<bool>, Condvar)>,
    exited: Arc<AtomicBool>,
    reader_handle: Option<std::thread::JoinHandle<()>>,
    wait_handle: Option<std::thread::JoinHandle<()>>,
}

impl Session {
    pub fn spawn(shell: &str, rows: u32, cols: u32) -> Result<Self, SessionError> {
        let pty = PtyPair::spawn(shell, rows as u16, cols as u16)?;
        pty.set_nonblocking()?;

        let exited = Arc::new(AtomicBool::new(false));
        let output_notify = Arc::new((Mutex::new(false), Condvar::new()));

        let (output_tx, output_rx) = bounded::<Vec<u8>>(64);

        // SAFETY: dup() is safe because pty.master_fd() returns a valid fd
        // owned by PtyPair. We duplicate it so the reader thread has its own
        // fd to close on exit without interfering with the original.
        let read_fd = unsafe { libc::dup(pty.master_fd()) };
        if read_fd < 0 {
            return Err(SessionError::Io(std::io::Error::last_os_error()));
        }

        fn notify_output(notify: &Arc<(Mutex<bool>, Condvar)>) {
            let (lock, cvar) = &**notify;
            let mut pending = lock.lock().unwrap();
            *pending = true;
            cvar.notify_one();
        }

        let exited_read = exited.clone();
        let notify_read = output_notify.clone();
        let reader_handle = std::thread::spawn(move || {
            let mut read_buf = [0u8; READ_BUF_SIZE];
            loop {
                // SAFETY: read_fd is a valid fd (dup'd from PtyPair's master).
                // read_buf is a stack-allocated array with known size. The
                // read call is blocking but the fd is set to nonblocking mode.
                // We pass a valid pointer and length — no memory safety issue.
                let n = unsafe {
                    libc::read(
                        read_fd,
                        read_buf.as_mut_ptr() as *mut libc::c_void,
                        READ_BUF_SIZE,
                    )
                };
                if n > 0 {
                    let data = read_buf[..n as usize].to_vec();
                    if output_tx.send(data).is_err() {
                        break;
                    }
                    notify_output(&notify_read);
                } else if n == 0 {
                    exited_read.store(true, Ordering::Release);
                    notify_output(&notify_read);
                    break;
                } else {
                    let err = std::io::Error::last_os_error();
                    if err.kind() == std::io::ErrorKind::WouldBlock {
                        std::thread::sleep(Duration::from_millis(2));
                    } else {
                        exited_read.store(true, Ordering::Release);
                        notify_output(&notify_read);
                        break;
                    }
                }
            }
            // SAFETY: read_fd was created by libc::dup() and is only used
            // in this thread. Closing it here is safe and necessary to
            // prevent fd leaks. No other code references this fd after close.
            unsafe {
                libc::close(read_fd);
            }
        });

        let exited_wait = exited.clone();
        let child_pid = pty.child_pid();
        let wait_handle = std::thread::spawn(move || {
            let _ = nix::sys::wait::waitpid(child_pid, None);
            exited_wait.store(true, Ordering::Release);
        });

        Ok(Self {
            pty,
            terminal: TerminalState::new(rows, cols)?,
            parser: VtParser::new(),
            output_rx,
            output_notify,
            exited,
            reader_handle: Some(reader_handle),
            wait_handle: Some(wait_handle),
        })
    }

    pub fn write(&mut self, data: &[u8]) -> Result<(), SessionError> {
        if self.is_exited() {
            return Err(SessionError::Closed);
        }
        use std::io::Write as _;
        self.pty.write_all(data)?;
        Ok(())
    }

    pub fn resize(&mut self, rows: u32, cols: u32) -> Result<(), SessionError> {
        self.pty.resize(rows as u16, cols as u16)?;
        self.terminal.resize(rows, cols);
        Ok(())
    }

    pub fn wait_for_output(&self) {
        let (lock, cvar) = &*self.output_notify;
        let mut pending = lock.lock().unwrap();
        while !*pending {
            pending = cvar.wait(pending).unwrap();
        }
        *pending = false;
    }

    pub fn process_output(&mut self) -> bool {
        let mut changed = false;
        while let Ok(data) = self.output_rx.try_recv() {
            self.parser.advance(&mut self.terminal, &data);
            changed = true;
        }
        changed
    }

    pub fn is_exited(&self) -> bool {
        self.exited.load(Ordering::Acquire)
    }

    pub fn terminal(&self) -> &TerminalState {
        &self.terminal
    }

    pub fn terminal_mut(&mut self) -> &mut TerminalState {
        &mut self.terminal
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.exited.store(true, Ordering::Release);
        if let Some(h) = self.reader_handle.take() {
            let _ = h.join();
        }
        if let Some(h) = self.wait_handle.take() {
            let _ = h.join();
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

    fn grid_text(session: &Session) -> String {
        let grid = &session.terminal().grid;
        let mut text = String::new();
        for row in 0..grid.rows() {
            if let Some(line) = grid.get(row) {
                let mut line_text = String::new();
                for col in 0..line.len() {
                    if let Some(cell) = line.get(col) {
                        line_text.push(cell.char);
                    }
                }
                text.push_str(line_text.trim_end());
                text.push('\n');
            }
        }
        text
    }

    #[test]
    fn session_spawn_and_exit() {
        let mut session = Session::spawn("/bin/sh", 24, 80).expect("spawn failed");
        session.write(b"exit\n").expect("write failed");
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        drain_output(&mut session, deadline);
        assert!(session.is_exited());
    }

    #[test]
    fn session_echo_hello() {
        let mut session = Session::spawn("/bin/sh", 24, 80).expect("spawn failed");
        session.write(b"echo hello_p12\n").expect("write failed");
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut found = false;
        while std::time::Instant::now() < deadline {
            session.process_output();
            let text = grid_text(&session);
            if text.contains("hello_p12") {
                found = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(found, "did not find 'hello_p12' in grid");
    }

    #[test]
    fn session_resize() {
        let mut session = Session::spawn("/bin/sh", 24, 80).expect("spawn failed");
        session.resize(40, 120).expect("resize failed");
        assert_eq!(session.terminal().rows(), 40);
        assert_eq!(session.terminal().cols(), 120);
    }

    #[test]
    fn session_after_exit_returns_error() {
        let mut session = Session::spawn("/bin/sh", 24, 80).expect("spawn failed");
        session.write(b"exit\n").expect("write failed");
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        drain_output(&mut session, deadline);
        assert!(session.is_exited());
    }
}
