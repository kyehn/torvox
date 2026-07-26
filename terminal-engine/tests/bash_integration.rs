use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};
use std::time::Duration;

struct TestPty {
    master: OwnedFd,
    child_pid: libc::pid_t,
}

fn open_pty() -> TestPty {
    unsafe {
        let mut master_fd: libc::c_int = 0;
        let mut slave_fd: libc::c_int = 0;
        let ret = libc::openpty(
            &mut master_fd,
            &mut slave_fd,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        assert_eq!(
            ret,
            0,
            "openpty failed: {}",
            std::io::Error::last_os_error()
        );

        let pid = libc::fork();
        assert!(pid >= 0, "fork failed");

        if pid == 0 {
            libc::setsid();
            libc::ioctl(slave_fd, libc::TIOCSCTTY, 0);
            libc::dup2(slave_fd, 0);
            libc::dup2(slave_fd, 1);
            libc::dup2(slave_fd, 2);
            if slave_fd > 2 {
                libc::close(slave_fd);
            }
            libc::close(master_fd);
            let shell = std::ffi::CString::new("/bin/bash").unwrap();
            let args: [*const libc::c_char; 2] = [shell.as_ptr(), std::ptr::null()];
            libc::execvp(shell.as_ptr(), args.as_ptr());
            libc::_exit(127);
        }

        libc::close(slave_fd);
        let flags = libc::fcntl(master_fd, libc::F_GETFL);
        libc::fcntl(master_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);

        let winsize = libc::winsize {
            ws_row: 24,
            ws_col: 80,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        libc::ioctl(master_fd, libc::TIOCSWINSZ, &winsize);

        TestPty {
            master: OwnedFd::from_raw_fd(master_fd),
            child_pid: pid,
        }
    }
}

fn nonblocking_read(fd: &OwnedFd, buf: &mut [u8]) -> isize {
    unsafe {
        libc::read(
            fd.as_raw_fd() as libc::c_int,
            buf.as_mut_ptr() as *mut libc::c_void,
            buf.len(),
        )
    }
}

fn write_all(fd: &OwnedFd, data: &[u8]) {
    let mut written = 0;
    while written < data.len() {
        let n = unsafe {
            libc::write(
                fd.as_raw_fd() as libc::c_int,
                data[written..].as_ptr() as *const libc::c_void,
                data.len() - written,
            )
        };
        if n <= 0 {
            break;
        }
        written += n as usize;
    }
}

fn write_str(fd: &OwnedFd, data: &str) {
    write_all(fd, data.as_bytes());
}

fn read_available(fd: &OwnedFd) -> String {
    let mut buf = [0u8; 8192];
    let mut total = String::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        let n = nonblocking_read(fd, &mut buf);
        if n > 0 {
            total.push_str(&String::from_utf8_lossy(&buf[..n as usize]));
        } else {
            std::thread::sleep(Duration::from_millis(50));
        }
        if std::time::Instant::now() > deadline {
            break;
        }
    }
    total
}

fn wait_for_prompt(fd: &OwnedFd) -> String {
    let mut buf = [0u8; 8192];
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut output = String::new();
    loop {
        let n = nonblocking_read(fd, &mut buf);
        if n > 0 {
            output.push_str(&String::from_utf8_lossy(&buf[..n as usize]));
            if output.contains("$ ") || output.contains("# ") {
                break;
            }
        } else {
            std::thread::sleep(Duration::from_millis(50));
        }
        if std::time::Instant::now() > deadline {
            break;
        }
    }
    output
}

impl Drop for TestPty {
    fn drop(&mut self) {
        unsafe {
            libc::waitpid(self.child_pid, std::ptr::null_mut(), libc::WNOHANG);
        }
    }
}

fn run_command(cmd: &str) -> String {
    let pty = open_pty();
    let _prompt = wait_for_prompt(&pty.master);
    write_str(&pty.master, &format!("{}\n", cmd));
    std::thread::sleep(Duration::from_millis(300));
    read_available(&pty.master)
}

#[test]
fn bash_echo() {
    let output = run_command("echo HELLO_TEST");
    assert!(output.contains("HELLO_TEST"), "got: {}", output);
}

#[test]
fn bash_pwd() {
    let output = run_command("pwd");
    assert!(output.contains('/'), "got: {}", output);
}

#[test]
fn bash_ls_root() {
    let output = run_command("ls /");
    assert!(
        output.contains("tmp") || output.contains("usr") || output.contains("etc"),
        "got: {}",
        output
    );
}

#[test]
fn bash_exit_code_zero() {
    let pty = open_pty();
    let _prompt = wait_for_prompt(&pty.master);
    write_all(&pty.master, b"exit 0\n");
    std::thread::sleep(Duration::from_millis(500));
    let mut status: i32 = 0;
    unsafe {
        libc::waitpid(pty.child_pid, &mut status, 0);
    }
    assert!(libc::WIFEXITED(status), "bash should exit normally");
    assert_eq!(libc::WEXITSTATUS(status), 0);
}

#[test]
fn bash_exit_code_42() {
    let pty = open_pty();
    let _prompt = wait_for_prompt(&pty.master);
    write_all(&pty.master, b"exit 42\n");
    std::thread::sleep(Duration::from_millis(500));
    let mut status: i32 = 0;
    unsafe {
        libc::waitpid(pty.child_pid, &mut status, 0);
    }
    assert!(libc::WIFEXITED(status));
    assert_eq!(libc::WEXITSTATUS(status), 42);
}

#[test]
fn bash_redirect_and_cat() {
    let output = run_command("echo REDIRECT_OK > /tmp/test_123.txt && cat /tmp/test_123.txt");
    assert!(output.contains("REDIRECT_OK"), "got: {}", output);
}

#[test]
fn bash_for_loop() {
    let output = run_command("for i in 1 2 3; do echo ITEM_$i; done");
    assert!(output.contains("ITEM_1"), "got: {}", output);
    assert!(output.contains("ITEM_2"), "got: {}", output);
    assert!(output.contains("ITEM_3"), "got: {}", output);
}

#[test]
fn bash_cjk_text() {
    let output = run_command("echo '\\xe4\\xbd\\xa0\\xe5\\xa5\\xbd\\xe4\\xb8\\x96\\xe7\\x95\\x8c'");
    assert!(!output.is_empty(), "CJK echo should produce output");
}

#[test]
fn bash_seq_200() {
    let output = run_command("seq 1 200 | tail -1");
    assert!(output.contains("200"), "got: {}", output);
}

#[test]
fn bash_pipe_grep() {
    let output = run_command("echo 'hello world test' | grep world");
    assert!(output.contains("hello world test"), "got: {}", output);
}

#[test]
fn bash_env_variable() {
    let output = run_command("MY_VAR=42 && echo $MY_VAR");
    assert!(output.contains("42"), "got: {}", output);
}

#[test]
fn bash_command_not_found() {
    let output = run_command("nonexistent_command_xyz_12345");
    let lower = output.to_lowercase();
    assert!(
        lower.contains("not found") || lower.contains("no such"),
        "got: {}",
        output
    );
}
