use super::super::*;
use super::{spawn_and_exit, spawn_test_session};

use std::time::Duration;

#[test]
fn session_echo_hello() {
    let mut session = spawn_test_session();
    session.write(b"echo hello_p12\n").expect("write failed");
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut found = false;
    while std::time::Instant::now() < deadline {
        session.process_output();
        let rows = session.terminal().rows();
        for row in 0..rows {
            if let Some(line) = session.terminal().read_line_text(row)
                && line.contains("hello_p12")
            {
                found = true;
                break;
            }
        }
        if found {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(found, "did not find 'hello_p12' in terminal");
}

#[test]
fn session_after_exit_returns_error() {
    let mut session = spawn_and_exit();
    let result = session.write(b"echo after-exit\n");
    assert!(
        matches!(result, Err(SessionError::Closed)),
        "write after exit must report Closed, got: {result:?}"
    );
}

#[test]
fn session_new_creates_pty() {
    let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
    assert_eq!(
        session.terminal().rows(),
        24,
        "terminal rows must be 24 after creation"
    );
    assert_eq!(
        session.terminal().cols(),
        80,
        "terminal cols must be 80 after creation"
    );
    assert!(
        !session.is_exited(),
        "new session must not be in exited state"
    );
}

#[test]
fn session_write_input() {
    let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let mut session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
    session.write(b"hello world").expect("write must succeed");
    let written = handle.written();
    assert_eq!(
        written, b"hello world",
        "input written to session must reach PTY master"
    );
}

#[test]
fn session_title_default_is_empty() {
    let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
    assert_eq!(session.terminal().title(), "");
}

#[test]
fn session_mode_get_default_false() {
    let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
    // 模式 2004（bracketed paste）默认应关闭。
    assert!(!session.terminal().mode_get_with_timeout(
        libghostty_vt::terminal::Mode::BRACKETED_PASTE,
        std::time::Duration::from_millis(crate::terminal::ghostty_terminal::QUERY_TIMEOUT_MS),
    ));
}

#[test]
fn session_write_after_exit_returns_error() {
    let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    let mut session = Session::with_pty_for_test(Box::new(pty) as Box<dyn Pty>, 24, 80)
        .expect("with_pty must succeed");
    // 置会话的退出标志，使 `write()` 在调用 PTY 前先检查它。
    session.exited_flag().store(true, Ordering::Release);
    let result = session.write(b"test");
    assert!(
        result.is_err(),
        "write after exit must return error, got Ok"
    );
}
