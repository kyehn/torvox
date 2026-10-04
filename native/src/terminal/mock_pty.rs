use std::collections::VecDeque;
use std::io;
use std::os::unix::io::{OwnedFd, RawFd};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::terminal::pty::{Pty, PtyError};
use crate::terminal::shell_env::ShellEnv;

struct MockPtyInner {
    input_buffer: VecDeque<Vec<u8>>,
    child_exited: bool,
    rows: u16,
    cols: u16,
    /// Number of resize() calls, for tests asserting the short-circuit does
    /// not touch the PTY.
    resize_count: usize,
}

/// Mock PTY for testing — simulates a pseudo-terminal without forking a real process.
pub struct MockPty {
    inner: Arc<Mutex<MockPtyInner>>,
}

/// Handle to a mock PTY — allows reading output and checking state.
pub struct MockPtyHandle {
    inner: Arc<Mutex<MockPtyInner>>,
}

impl MockPty {
    pub fn new(rows: u16, cols: u16) -> (Self, MockPtyHandle) {
        let inner = Arc::new(Mutex::new(MockPtyInner {
            input_buffer: VecDeque::new(),
            resize_count: 0,
            child_exited: false,
            rows,
            cols,
        }));
        (
            MockPty {
                inner: inner.clone(),
            },
            MockPtyHandle { inner },
        )
    }
}

impl MockPtyHandle {
    pub fn set_exited(&self) {
        self.inner.lock().expect("mock mutex poisoned").child_exited = true;
    }

    pub fn is_exited(&self) -> bool {
        self.inner.lock().expect("mock mutex poisoned").child_exited
    }

    pub fn written(&self) -> Vec<u8> {
        let inner = self.inner.lock().expect("mock mutex poisoned");
        let mut result = Vec::new();
        for chunk in &inner.input_buffer {
            result.extend_from_slice(chunk);
        }
        result
    }

    pub fn rows(&self) -> u16 {
        self.inner.lock().expect("mock mutex poisoned").rows
    }

    /// Number of resize calls made to this mock.
    pub fn resize_count(&self) -> usize {
        self.inner.lock().expect("mock mutex poisoned").resize_count
    }

    pub fn cols(&self) -> u16 {
        self.inner.lock().expect("mock mutex poisoned").cols
    }
}

impl Pty for MockPty {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut inner = self.inner.lock().expect("mock mutex poisoned");
        if inner.child_exited {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "child process exited",
            ));
        }
        inner.input_buffer.push_back(buf.to_vec());
        Ok(buf.len())
    }

    /// 替身不承载输出侧：没有注入口，故 PTY 输出经 `try_clone_reader_fd`
    /// 返回的 `/dev/null` 读取（见该方法），这里恒报「无数据」。
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        let inner = self.inner.lock().expect("mock mutex poisoned");
        if inner.child_exited {
            return Ok(0);
        }
        Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "no data available",
        ))
    }

    fn resize(&self, rows: u16, cols: u16) -> Result<(), PtyError> {
        let mut inner = self.inner.lock().expect("mock mutex poisoned");
        inner.rows = rows;
        inner.cols = cols;
        inner.resize_count += 1;
        Ok(())
    }

    fn get_winsize(&self) -> Result<(u16, u16), PtyError> {
        let inner = self.inner.lock().expect("mock mutex poisoned");
        Ok((inner.rows, inner.cols))
    }

    fn child_pid(&self) -> nix::unistd::Pid {
        // Return a pid above PID_MAX_LIMIT (2²² = 4,194,304 on Linux) so that
        // kill(pid, signal) returns ESRCH ("no such process") instead of
        // broadcasting to all processes. Using -1 would send the signal to
        // every process the caller can signal, which is catastrophic in tests.
        nix::unistd::Pid::from_raw(4_194_305)
    }

    fn master_fd(&self) -> RawFd {
        -1
    }

    fn try_clone_reader_fd(&self) -> io::Result<OwnedFd> {
        // Mock PTY output is delivered through the in-memory buffer, never a
        // real fd; return a throwaway read fd so the reader thread exits cleanly.
        std::fs::File::open("/dev/null").map(OwnedFd::from)
    }

    fn set_nonblocking(&self) -> Result<(), PtyError> {
        Ok(())
    }

    fn spawn(
        _shell: &str,
        rows: u16,
        cols: u16,
        _env: &ShellEnv,
        _cwd: Option<&Path>,
    ) -> Result<Box<dyn Pty>, PtyError> {
        let (mock, _handle) = MockPty::new(rows, cols);
        Ok(Box::new(mock))
    }
}
