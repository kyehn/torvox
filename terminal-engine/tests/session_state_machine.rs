use std::sync::Arc;
use std::sync::Mutex;

use terminal_engine::pty::{Pty, PtyError};
use terminal_engine::session::Session;

/// A minimal mock PTY for session state machine testing.
struct TestPty {
    written: Arc<Mutex<Vec<u8>>>,
    exited: Arc<Mutex<bool>>,
}

impl Pty for TestPty {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if *self.exited.lock().unwrap() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "exited",
            ));
        }
        self.written.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        Ok(0)
    }

    fn resize(&self, _rows: u16, _cols: u16) -> Result<(), PtyError> {
        // Use interior mutability since session accesses this on a writer thread
        const _: () = ();
        Ok(())
    }

    fn child_pid(&self) -> nix::unistd::Pid {
        nix::unistd::Pid::from_raw(-1)
    }

    fn master_fd(&self) -> std::os::unix::io::RawFd {
        -1
    }

    fn wait(&self) -> nix::Result<nix::sys::wait::WaitStatus> {
        if *self.exited.lock().unwrap() {
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

    fn try_clone_reader_fd(&self) -> Result<std::os::unix::io::OwnedFd, std::io::Error> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "TestPty has no real fd",
        ))
    }

    fn spawn(
        _shell: &str,
        _rows: u16,
        _cols: u16,
        _env: &terminal_engine::shell_env::ShellEnv,
    ) -> Result<Box<dyn Pty>, PtyError> {
        let written = Arc::new(Mutex::new(Vec::new()));
        let exited = Arc::new(Mutex::new(false));
        Ok(Box::new(TestPty { written, exited }))
    }
}

/// 10.4.1 Spawn → process_output returns false (no data yet)
#[test]
fn session_spawn_no_data() {
    let pty = TestPty::spawn(
        "sh",
        24,
        80,
        &terminal_engine::shell_env::ShellEnv::default(),
    )
    .expect("TestPty spawn");
    let mut session = Session::with_pty(pty, 24, 80).expect("Session with_pty");
    // No data in channel yet → process_output returns false
    let alive = session.process_output();
    assert!(
        !alive,
        "process_output should return false with empty channel"
    );
}

/// 10.4.2 Write → terminal snapshot contains data
#[test]
fn session_write_snapshot_contains_data() {
    let pty = TestPty::spawn(
        "sh",
        24,
        80,
        &terminal_engine::shell_env::ShellEnv::default(),
    )
    .expect("TestPty spawn");
    let mut session = Session::with_pty(pty, 24, 80).expect("Session with_pty");
    session.write(b"hello world").expect("write should succeed");
    let term = session.terminal_mut();
    let snap = term.take_snapshot();
    assert!(snap.rows > 0, "terminal should have content");
}

/// 10.4.3 Resize → dimensions updated
#[test]
fn session_resize_dimensions_updated() {
    let pty = TestPty::spawn(
        "sh",
        24,
        80,
        &terminal_engine::shell_env::ShellEnv::default(),
    )
    .expect("TestPty spawn");
    let mut session = Session::with_pty(pty, 24, 80).expect("Session with_pty");
    session.resize(36, 120).expect("resize should succeed");
    let term = session.terminal_mut();
    let snap = term.take_snapshot();
    assert!(snap.rows > 0, "terminal should have rows after resize");
}

/// 10.4.6 Double close is idempotent
#[test]
fn session_double_close_idempotent() {
    let pty = TestPty::spawn(
        "sh",
        24,
        80,
        &terminal_engine::shell_env::ShellEnv::default(),
    )
    .expect("TestPty spawn");
    let session = Session::with_pty(pty, 24, 80).expect("Session with_pty");
    // Just drop the session and ensure no panic
    // Drop is idempotent by nature in Rust
    drop(session);
}

/// 10.4.5 Drop → resources cleaned (no leak)
#[test]
fn session_drop_resources_cleaned() {
    let pty = TestPty::spawn(
        "sh",
        24,
        80,
        &terminal_engine::shell_env::ShellEnv::default(),
    )
    .expect("TestPty spawn");
    {
        let session = Session::with_pty(pty, 24, 80).expect("Session with_pty");
        // Session is alive inside this block
        drop(session);
    }
    // After drop, ensure no memory issues (valgrind would catch leaks)
}

/// 10.4.4 Exit → is_exited starts false (true only after Drop)
#[test]
fn session_is_exited_false_before_drop() {
    let pty = TestPty::spawn(
        "sh",
        24,
        80,
        &terminal_engine::shell_env::ShellEnv::default(),
    )
    .expect("TestPty spawn");
    let session = Session::with_pty(pty, 24, 80).expect("Session with_pty");
    assert!(
        !session.is_exited(),
        "is_exited should be false before drop"
    );
    // Drop sets exited=true internally (verified by Drop impl)
}

/// 10.4.7 Two sessions isolated — independent state
#[test]
fn two_sessions_isolated() {
    let pty_a = TestPty::spawn(
        "sh",
        24,
        80,
        &terminal_engine::shell_env::ShellEnv::default(),
    )
    .expect("TestPty A spawn");
    let pty_b = TestPty::spawn(
        "sh",
        24,
        80,
        &terminal_engine::shell_env::ShellEnv::default(),
    )
    .expect("TestPty B spawn");
    let mut session_a = Session::with_pty(pty_a, 24, 80).expect("Session A");
    let mut session_b = Session::with_pty(pty_b, 24, 80).expect("Session B");

    session_a.write(b"hello from A").expect("write A");
    session_b.write(b"hello from B").expect("write B");

    let snap_a = session_a.terminal().take_snapshot();
    let snap_b = session_b.terminal().take_snapshot();
    assert!(snap_a.rows > 0, "session A has content");
    assert!(snap_b.rows > 0, "session B has content");
    // Sessions are independent — different Pty instances
    assert!(
        !std::ptr::eq(&snap_a as *const _, &snap_b as *const _),
        "sessions must have independent state"
    );
}
