use std::collections::VecDeque;
use std::io;
use std::os::unix::io::{OwnedFd, RawFd};
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};

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
    /// 前 N 次 write 返回 EINTR，用于验证调用方把信号打断当作「重试」而非失败
    /// （SIGWINCH / SIGCHLD 都会走到这条）。
    interrupt_writes: usize,
    /// 置位后 write 先进 gate 的「已进入」闸并等待「放行」，用于确定性地复现
    /// 「持有互斥量的写入进行中」这一并发状态（靠 sleep 判定必然带竞态）。
    gate: Option<Arc<WriteGate>>,
}

/// 让 [Pty::write] 可被外部确定性地停在临界区内。
pub struct WriteGate {
    entered: (Mutex<bool>, Condvar),
    release: (Mutex<bool>, Condvar),
}

impl WriteGate {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            entered: (Mutex::new(false), Condvar::new()),
            release: (Mutex::new(false), Condvar::new()),
        })
    }

    /// 等待 write 真正进入临界区（而不是「大概已经开始了」）。
    pub fn wait_until_entered(&self) {
        let (lock, cvar) = &self.entered;
        let mut entered = lock.lock().expect("gate mutex poisoned");
        while !*entered {
            entered = cvar.wait(entered).expect("gate mutex poisoned");
        }
    }

    pub fn release(&self) {
        let (lock, cvar) = &self.release;
        *lock.lock().expect("gate mutex poisoned") = true;
        cvar.notify_all();
    }
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
    /// 把接下来的 write 卡在 gate 上，直到 [WriteGate::release]。
    pub fn gate_writes(&mut self, gate: Arc<WriteGate>) {
        self.inner.lock().expect("mock mutex poisoned").gate = Some(gate);
    }

    /// 让接下来的 `count` 次 [Pty::write] 以 EINTR 失败。
    pub fn interrupt_next_writes(&mut self, count: usize) {
        self.inner
            .lock()
            .expect("mock mutex poisoned")
            .interrupt_writes = count;
    }

    pub fn new(rows: u16, cols: u16) -> (Self, MockPtyHandle) {
        let inner = Arc::new(Mutex::new(MockPtyInner {
            input_buffer: VecDeque::new(),
            resize_count: 0,
            interrupt_writes: 0,
            gate: None,
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
    fn write(&self, buf: &[u8]) -> io::Result<usize> {
        let gate = {
            let inner = self.inner.lock().expect("mock mutex poisoned");
            inner.gate.clone()
        };
        if let Some(gate) = gate {
            let (lock, cvar) = &gate.entered;
            *lock.lock().expect("gate mutex poisoned") = true;
            cvar.notify_all();
            let (lock, cvar) = &gate.release;
            let mut released = lock.lock().expect("gate mutex poisoned");
            while !*released {
                released = cvar.wait(released).expect("gate mutex poisoned");
            }
        }
        let mut inner = self.inner.lock().expect("mock mutex poisoned");
        if inner.interrupt_writes > 0 {
            inner.interrupt_writes -= 1;
            return Err(io::Error::from(io::ErrorKind::Interrupted));
        }
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
    fn read(&self, _buf: &mut [u8]) -> io::Result<usize> {
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
        // 输出侧没有注入口（见 `read`），故这里给读取线程一个立即 EOF 的一次性
        // fd，使它能干净退出；需要「程序输出了什么」的用例走
        // `Session::inject_output_for_test`。
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
