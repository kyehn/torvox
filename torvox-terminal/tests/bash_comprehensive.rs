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
    let output = run_command("echo HELLO_BASH");
    assert!(output.contains("HELLO_BASH"), "got: {}", output);
}

#[test]
fn bash_pwd_returns_path() {
    let output = run_command("pwd");
    assert!(
        output.contains("/"),
        "pwd should contain /, got: {}",
        output
    );
}

#[test]
fn bash_pipe_grep() {
    let output = run_command("echo 'alpha beta gamma' | grep beta");
    assert!(
        output.contains("beta"),
        "pipe+grep should work, got: {}",
        output
    );
}

#[test]
fn bash_redirect_and_read() {
    let output = run_command(
        "echo TEST_REDIRECT > /tmp/_torvox_test.txt && cat /tmp/_torvox_test.txt && rm -f /tmp/_torvox_test.txt",
    );
    assert!(
        output.contains("TEST_REDIRECT"),
        "redirect+cat should work, got: {}",
        output
    );
}

#[test]
fn bash_for_loop_output() {
    let output = run_command("for i in 1 2 3; do echo LOOP_$i; done");
    assert!(
        output.contains("LOOP_1"),
        "for loop should produce LOOP_1, got: {}",
        output
    );
    assert!(
        output.contains("LOOP_2"),
        "for loop should produce LOOP_2, got: {}",
        output
    );
    assert!(
        output.contains("LOOP_3"),
        "for loop should produce LOOP_3, got: {}",
        output
    );
}

#[test]
fn bash_exit_code_success() {
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
fn bash_exit_code_nonzero() {
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
fn bash_command_substitution() {
    let output = run_command("echo $(echo NESTED)");
    assert!(
        output.contains("NESTED"),
        "command substitution should work, got: {}",
        output
    );
}

#[test]
fn bash_arithmetic_expansion() {
    let output = run_command("echo $((2 + 3))");
    assert!(
        output.contains("5"),
        "arithmetic should produce 5, got: {}",
        output
    );
}

#[test]
fn bash_variable_expansion() {
    let output = run_command("MYVAR=hello && echo $MYVAR");
    assert!(
        output.contains("hello"),
        "variable expansion should work, got: {}",
        output
    );
}

#[test]
fn bash_multiple_commands_semicolon() {
    let output = run_command("echo FIRST; echo SECOND");
    assert!(
        output.contains("FIRST"),
        "should contain FIRST, got: {}",
        output
    );
    assert!(
        output.contains("SECOND"),
        "should contain SECOND, got: {}",
        output
    );
}

#[test]
fn bash_here_string() {
    let output = run_command("cat <<< HERE_STRING");
    assert!(
        output.contains("HERE_STRING"),
        "here string should work, got: {}",
        output
    );
}

#[test]
fn bash_string_length() {
    let output = run_command("echo -n HELLO | wc -c");
    assert!(
        output.contains("5"),
        "string length of HELLO should be 5, got: {}",
        output
    );
}

#[test]
fn bash_pid_of_current_shell() {
    let output = run_command("echo $$");
    let digits: String = output.chars().filter(|c| c.is_ascii_digit()).collect();
    let pid: i32 = digits.parse().unwrap_or(0);
    assert!(
        pid > 0,
        "shell PID should be positive, got: {} (from: {:?})",
        pid,
        output
    );
}

#[test]
fn bash_wc_l_lines() {
    let output = run_command("printf 'a\nb\nc\n' | wc -l");
    assert!(
        output.contains("3"),
        "wc -l should count 3 lines, got: {}",
        output
    );
}

#[test]
fn bash_sort_unique() {
    let output = run_command("echo -e 'c\na\nb\na\nb' | sort -u");
    assert!(
        output.contains("a"),
        "sorted unique should contain a, got: {}",
        output
    );
    assert!(
        output.contains("b"),
        "sorted unique should contain b, got: {}",
        output
    );
    assert!(
        output.contains("c"),
        "sorted unique should contain c, got: {}",
        output
    );
}

#[test]
fn bash_touch_and_stat() {
    let output = run_command(
        "touch /tmp/_torvox_stat_test && test -f /tmp/_torvox_stat_test && echo EXISTS && rm -f /tmp/_torvox_stat_test",
    );
    assert!(
        output.contains("EXISTS"),
        "touch+test should work, got: {}",
        output
    );
}

#[test]
fn bash_true_false_exit_codes() {
    let output_true = run_command("true; echo $?");
    assert!(
        output_true.contains("0"),
        "true should return 0, got: {}",
        output_true
    );

    let output_false = run_command("false; echo $?");
    assert!(
        output_false.contains("1"),
        "false should return 1, got: {}",
        output_false
    );
}

#[test]
fn bash_glob_star() {
    let output = run_command("ls /tmp/*.txt 2>/dev/null || echo NO_TXT_FILES");
    assert!(!output.is_empty(), "glob should produce some output");
}

#[test]
fn bash_subshell() {
    let output = run_command("(echo IN_SUBSHELL)");
    assert!(
        output.contains("IN_SUBSHELL"),
        "subshell should work, got: {}",
        output
    );
}

#[test]
fn bash_background_and_wait() {
    let output = run_command("sleep 0.1 & wait; echo BG_DONE");
    assert!(
        output.contains("BG_DONE"),
        "background+wait should work, got: {}",
        output
    );
}
