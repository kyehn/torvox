use std::collections::VecDeque;
use std::io;
use std::os::unix::io::{OwnedFd, RawFd};
use std::sync::{Arc, Mutex};

use crate::pty::{Pty, PtyError};
use crate::shell_env::ShellEnv;

struct MockPtyInner {
    input_buffer: VecDeque<Vec<u8>>,
    output_buffer: VecDeque<Vec<u8>>,
    child_exited: bool,
    rows: u16,
    cols: u16,
}

pub struct MockPty {
    inner: Arc<Mutex<MockPtyInner>>,
}

pub struct MockPtyHandle {
    inner: Arc<Mutex<MockPtyInner>>,
}

impl MockPty {
    pub fn new(rows: u16, cols: u16) -> (Self, MockPtyHandle) {
        let inner = Arc::new(Mutex::new(MockPtyInner {
            input_buffer: VecDeque::new(),
            output_buffer: VecDeque::new(),
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
    pub fn inject_output(&self, data: &[u8]) {
        self.inner
            .lock()
            .unwrap()
            .output_buffer
            .push_back(data.to_vec());
    }

    pub fn drain_written(&self) -> Vec<Vec<u8>> {
        let mut inner = self.inner.lock().unwrap();
        std::mem::take(&mut inner.input_buffer)
            .into_iter()
            .collect()
    }

    pub fn set_exited(&self) {
        self.inner.lock().unwrap().child_exited = true;
    }

    pub fn is_exited(&self) -> bool {
        self.inner.lock().unwrap().child_exited
    }

    pub fn written(&self) -> Vec<u8> {
        let inner = self.inner.lock().unwrap();
        let mut result = Vec::new();
        for chunk in &inner.input_buffer {
            result.extend_from_slice(chunk);
        }
        result
    }

    pub fn resize(&self, rows: u16, cols: u16) {
        let mut inner = self.inner.lock().unwrap();
        inner.rows = rows;
        inner.cols = cols;
    }

    pub fn rows(&self) -> u16 {
        self.inner.lock().unwrap().rows
    }

    pub fn cols(&self) -> u16 {
        self.inner.lock().unwrap().cols
    }
}

impl Pty for MockPty {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut inner = self.inner.lock().unwrap();
        if inner.child_exited {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "child process exited",
            ));
        }
        inner.input_buffer.push_back(buf.to_vec());
        Ok(buf.len())
    }

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut inner = self.inner.lock().unwrap();
        if inner.child_exited && inner.output_buffer.is_empty() {
            return Ok(0);
        }
        if let Some(data) = inner.output_buffer.pop_front() {
            let n = data.len().min(buf.len());
            buf[..n].copy_from_slice(&data[..n]);
            if n < data.len() {
                inner.output_buffer.push_front(data[n..].to_vec());
            }
            Ok(n)
        } else {
            Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "no data available",
            ))
        }
    }

    fn resize(&self, rows: u16, cols: u16) -> Result<(), PtyError> {
        let mut inner = self.inner.lock().unwrap();
        inner.rows = rows;
        inner.cols = cols;
        Ok(())
    }

    fn child_pid(&self) -> nix::unistd::Pid {
        nix::unistd::Pid::from_raw(-1)
    }

    fn master_fd(&self) -> RawFd {
        -1
    }

    fn try_clone_reader_fd(&self) -> io::Result<OwnedFd> {
        // Mock PTY output is delivered through the in-memory buffer, never a
        // real fd; return a throwaway read fd so the reader thread exits cleanly.
        std::fs::File::open("/dev/null").map(OwnedFd::from)
    }

    fn wait(&self) -> nix::Result<nix::sys::wait::WaitStatus> {
        let inner = self.inner.lock().unwrap();
        if inner.child_exited {
            Ok(nix::sys::wait::WaitStatus::Exited(
                nix::unistd::Pid::from_raw(-1),
                0,
            ))
        } else {
            Ok(nix::sys::wait::WaitStatus::StillAlive)
        }
    }

    fn set_nonblocking(&self) -> Result<(), PtyError> {
        Ok(())
    }

    fn set_pixel_size(&mut self, _width: u16, _height: u16) {}

    fn spawn(
        _shell: &str,
        rows: u16,
        cols: u16,
        _env: &ShellEnv,
    ) -> Result<Box<dyn Pty>, PtyError> {
        let (mock, _handle) = MockPty::new(rows, cols);
        Ok(Box::new(mock))
    }
}
