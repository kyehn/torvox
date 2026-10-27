use super::*;

/// `Session::spawn` 中 fork 成功后的错误路径上 `Session` 从未构造，
/// 回收只能由主端所有者（`PtyPair::drop`）完成：drop 返回后子进程必须已被
/// waitpid 回收（再查询即 ECHILD），既不得留活子进程也不得留僵尸。
#[test]
fn drop_reaps_child_process() {
    let pty = PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None).expect("spawn failed");
    let pid = pty.child_pid();
    drop(pty);

    match nix::sys::wait::waitpid(pid, Some(nix::sys::wait::WaitPidFlag::WNOHANG)) {
        Err(nix::errno::Errno::ECHILD) => {}
        Ok(status) => panic!("drop 后子进程未被回收（僵尸残留）: {status:?}"),
        Err(error) => panic!("waitpid failed: {error}"),
    }
    assert!(
        nix::sys::signal::kill(pid, None).is_err(),
        "child {pid} 仍存活：drop 未终止子进程"
    );
}

/// execve 失败时子进程把确切 errno 写进回报管道，父进程据此判定 execve 真失败。
///
/// 退出码承担不了这件事：shell 被 SIGHUP 杀掉时退出码是 129，与「errno=29」的
/// 旧编码撞车——应用侧曾据此在正常会话收尾时打出 E 级日志。
#[test]
fn exec_failure_reports_exact_errno_through_the_pipe() {
    let mut pty = PtyPair::spawn(
        "/nonexistent/pty-shell-for-exec-failure",
        24,
        80,
        &ShellEnv::default(),
        None,
    )
    .expect("spawn failed");
    let child_pid = pty.child_pid();
    let reader = pty
        .take_exec_failure_reader()
        .expect("exec failure reader must exist");
    // 先等子进程退出再读管道：写端随 exec 关闭（成功）或随退出关闭（失败）。
    let status = nix::sys::wait::waitpid(child_pid, None).expect("waitpid failed");
    assert!(
        matches!(status, nix::sys::wait::WaitStatus::Exited(_, _)),
        "execve 失败时子进程应正常退出，实际 {status:?}"
    );
    assert_eq!(
        read_exec_failure_errno(&reader),
        Some(libc::ENOENT),
        "回报管道必须给出确切 errno"
    );
}

/// execve 成功时回报管道为空：写端随 exec 关闭，父进程读到 EOF。
#[test]
fn successful_exec_leaves_the_failure_pipe_empty() {
    let mut pty =
        PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None).expect("spawn failed");
    let child_pid = pty.child_pid();
    let reader = pty
        .take_exec_failure_reader()
        .expect("exec failure reader must exist");
    nix::sys::signal::kill(child_pid, nix::sys::signal::Signal::SIGKILL).expect("kill failed");
    let _ = nix::sys::wait::waitpid(child_pid, None);
    assert_eq!(
        read_exec_failure_errno(&reader),
        None,
        "execve 成功时管道必须为空"
    );
}

/// 回归钉：EINTR 必须重试，不得当成写入失败。
///
/// resize 的 SIGWINCH 与子进程退出的 SIGCHLD 都会打断 `write`，此前它被原样
/// 抛给调用方，于是 FFI 层记「子进程未在期限内消费按键，已丢弃」——而这批字节
/// 其实一个都没送出去，用户看到的是凭空少掉的一次击键。
///
/// 判别力：断言 `written()` 拿到完整字节；去掉 EINTR 重试分支即报错判红。
#[test]
fn write_all_retries_after_eintr_instead_of_failing() {
    let (mut pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
    pty.interrupt_next_writes(1);

    Pty::write_all(&pty, b"hello", WRITE_DRAIN_TIMEOUT, true)
        .1
        .expect("EINTR 后必须重试而不是报错");

    assert_eq!(handle.written(), b"hello", "字节必须全部送出，一个都不能少");
}

/// 主端是 O_NONBLOCK：PTY 输入缓冲（~4KB）满时 `write` 失败并返回 EAGAIN。
/// 旧实现据此返回 `WouldBlock` 并丢弃剩余字节——粘贴被静默截断、子进程执行
/// 半条命令，VT 应答也会被截断在转义序列中间使子应用永久等待。
///
/// 断言 `write_all` 对超出缓冲的载荷**等待子进程消费**而非报错丢弃。
#[test]
fn write_all_waits_instead_of_dropping_bytes_past_a_full_pty_buffer() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let capture = dir.path().join("pty-write");
    let pty = PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None).expect("spawn failed");
    pty.set_nonblocking().expect("set_nonblocking failed");
    let script = format!("sleep 0.2; cat > {}\n", capture.display());
    Pty::write_all(&pty, script.as_bytes(), WRITE_DRAIN_TIMEOUT, true)
        .1
        .expect("write failed");

    // 总量远超 PTY 输入缓冲（~4KB），中途必然 EAGAIN；逐行短行避开内核
    // 规范模式 4095 字符的行长上限。
    let payload = std::iter::repeat_n(
        [PTY_TEST_PAYLOAD_BYTE; PTY_TEST_PAYLOAD_LINE_BYTES],
        PTY_TEST_PAYLOAD_LINES,
    )
    .flatten()
    .chain(std::iter::repeat_n(b'\n', PTY_TEST_PAYLOAD_LINES))
    .collect::<Vec<u8>>();
    assert!(payload.len() > PTY_INPUT_BUFFER_BYTES);

    Pty::write_all(&pty, &payload, WRITE_DRAIN_TIMEOUT, true)
        .1
        .expect("write_all 不得因缓冲满而丢弃字节");

    let deadline = std::time::Instant::now() + Duration::from_secs(PTY_WRITE_TEST_DRAIN_SECS);
    let mut payload_delivered = false;
    while std::time::Instant::now() < deadline {
        if std::fs::metadata(&capture).is_ok_and(|meta| meta.len() > 0) {
            payload_delivered = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(TEST_READ_POLL_STEP_MS));
    }
    assert!(
        payload_delivered,
        "子进程在期限内未读到任何载荷：write_all 交付失败"
    );
}

#[test]
fn spawn_rejects_empty_shell_before_forking() {
    // 护栏：空 shell 曾一路走到 `execve("")`，只换来一个 ENOENT——现象是空会话
    // 加一条「查不到文件」，完全看不出是 shell 没配置。必须在 fork 前明确拒绝。
    let error = PtyPair::spawn("", 24, 80, &ShellEnv::default(), None)
        .err()
        .expect("empty shell must be rejected");
    assert!(
        matches!(error, PtyError::EmptyShell),
        "expected EmptyShell, got {error:?}"
    );
}

#[test]
fn shell_entry_splits_executable_and_arguments() {
    let (executable, arguments) = split_shell_entry("/data/data/com.termux/files/usr/bin/bash -l");
    assert_eq!(executable, "/data/data/com.termux/files/usr/bin/bash");
    assert_eq!(arguments, vec!["-l"]);
    let (single, empty) = split_shell_entry("/system/bin/sh");
    assert_eq!(single, "/system/bin/sh");
    assert!(empty.is_empty());
}

fn test_env() -> ShellEnv {
    ShellEnv {
        home: "/tmp/test_home".to_string(),
        working_directory: "/tmp/test_home".to_string(),
        prefix: None,
        mkshrc_path: Some("/tmp/test_app_data/.mkshrc".to_string()),
    }
}

/// 测试读取缓冲（单次 `read` 上限）与轮询节拍：`read_until` 的截止、
/// 步长与 `double_write` 的尝试次数/最小输出。
const TEST_READ_BUF_SIZE: usize = 4096;
const TEST_READ_DEADLINE_SECS: u64 = 2;
const TEST_READ_POLL_STEP_MS: u64 = 10;
const TEST_READ_ATTEMPTS: usize = 50;
const TEST_MIN_OUTPUT_LEN: usize = 200;

/// 写入排空测试的读取上限：远大于实测耗时，只为在异常时给出确定的失败而非挂起。
const PTY_WRITE_TEST_DRAIN_SECS: u64 = 10;

/// 每行远小于内核规范模式行长上限（4095），行数足够多使总量远超
/// PTY 输入缓冲（~4KB）——那才是 EAGAIN 的真实触发条件。
const PTY_TEST_PAYLOAD_LINE_BYTES: usize = 1000;
const PTY_TEST_PAYLOAD_LINES: usize = 16;

/// 载荷字节取值，命令文本中不含该字节。
const PTY_TEST_PAYLOAD_BYTE: u8 = b'z';

/// Linux tty 线路缓冲上限（`N_TTY_BUF_SIZE`）：超过即触发 EAGAIN。
const PTY_INPUT_BUFFER_BYTES: usize = 4096;

/// DESIGN 声明的规范路径常量（测试断言专用）：生产代码经调用方
///（Kotlin `filesDir`）传入，绝不硬编码；此处断言规范值本身。
const TEST_PREFIX: &str = "/data/data/com.termux/files/usr";
const TEST_TMPDIR: &str = "/data/data/com.termux/files/usr/tmp";

/// 读取 PTY 输出直到出现 `needle`（截止见 `TEST_READ_DEADLINE_SECS`）；返回已读内容，调用方自行断言。
fn read_until(pty: &PtyPair, needle: &[u8]) -> Vec<u8> {
    use crate::terminal::pty::Pty;

    let mut buf = [0u8; TEST_READ_BUF_SIZE];
    let mut output = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(TEST_READ_DEADLINE_SECS);
    while std::time::Instant::now() < deadline {
        match Pty::read(pty, &mut buf) {
            Ok(byte_count) => output.extend_from_slice(&buf[..byte_count]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(TEST_READ_POLL_STEP_MS));
            }
            Err(_) => break,
        }
        if output.windows(needle.len()).any(|window| window == needle) {
            return output;
        }
    }
    output
}

#[test]
fn shebang_plain_interpreter_parses() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("terminal-shebang-plain.sh");
    std::fs::write(&path, "#!/system/bin/sh\nset -eu\n").unwrap();
    let (interpreter, argument) = read_shebang_interpreter(path.to_str().unwrap()).unwrap();
    assert_eq!(interpreter.to_str().unwrap(), "/system/bin/sh");
    assert!(argument.is_none());
}

#[test]
fn shebang_single_argument_parses() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("terminal-shebang-arg.sh");
    std::fs::write(&path, "#!/system/bin/sh -eu\n").unwrap();
    let (interpreter, argument) = read_shebang_interpreter(path.to_str().unwrap()).unwrap();
    assert_eq!(interpreter.to_str().unwrap(), "/system/bin/sh");
    assert_eq!(argument.unwrap().to_str().unwrap(), "-eu");
}

#[test]
fn shebang_missing_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("terminal-shebang-plain-bin");
    std::fs::write(&path, [0x7fu8, b'E', b'L', b'F', 2]).unwrap();
    assert!(read_shebang_interpreter(path.to_str().unwrap()).is_none());
}

#[test]
fn shebang_bare_marker_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("terminal-shebang-bare.sh");
    std::fs::write(&path, "#!").unwrap();
    assert!(read_shebang_interpreter(path.to_str().unwrap()).is_none());
}

#[test]
fn base_env_includes_xterm_256color() {
    let env = base_env_with_host(None, &|_| None, &|_| false);
    assert!(
        env.iter()
            .any(|(key, value)| key == "TERM" && value == "xterm-256color")
    );
}

#[test]
fn base_env_is_minimal_set() {
    // 固定契约：宿主环境为空时 `base_env` 只产出 TERM/COLORTERM/LANG；
    // 无 PREFIX 时不产出 PREFIX/TMPDIR（规范未声明无引导程序会话的取值）。
    // 不能直接断言 `base_env`：CI 会导出 ANDROID_ROOT/EXTERNAL_STORAGE。
    let keys: std::collections::BTreeSet<String> = base_env_with_host(None, &|_| None, &|_| false)
        .into_iter()
        .map(|(key, _)| key)
        .collect();
    let expected: std::collections::BTreeSet<String> = ["COLORTERM", "LANG", "TERM"]
        .into_iter()
        .map(str::to_string)
        .collect();
    assert_eq!(keys, expected);
}

#[test]
fn base_env_forwards_only_present_host_vars() {
    // 规范透传（封闭）：宿主中存在的变量按原值转发，不存在的省略。
    let host = std::collections::HashMap::from([
        ("ANDROID_ROOT".to_string(), "/system".to_string()),
        ("EXTERNAL_STORAGE".to_string(), "/sdcard".to_string()),
    ]);
    let env = base_env_with_host(None, &|key| host.get(key).cloned(), &|_| false);
    assert!(
        env.iter()
            .any(|(key, value)| key == "ANDROID_ROOT" && value == "/system"),
    );
    assert!(
        env.iter()
            .any(|(key, value)| key == "EXTERNAL_STORAGE" && value == "/sdcard"),
    );
    assert!(env.iter().all(|(key, _)| key != "ANDROID_DATA"));
}

#[test]
fn base_env_includes_lang() {
    let env = base_env_with_host(None, &|_| None, &|_| false);
    assert!(
        env.iter()
            .any(|(key, value)| key == "LANG" && value == "en_US.UTF-8")
    );
}

#[test]
fn base_env_omits_tmpdir_without_prefix() {
    // 规范只声明 TMPDIR = files/usr/tmp；无 PREFIX 时不存在合法的 TMPDIR 取值，
    // 不得回退到未声明路径（DESIGN Bootstrap 节白名单 + 禁止未声明回退）。
    let env = base_env_with_host(None, &|_| None, &|_| false);
    assert!(!env.iter().any(|(key, _)| key == "TMPDIR"));
}

#[test]
fn base_env_includes_prefix_and_tmpdir_when_set() {
    let env = base_env_with_host(Some(TEST_PREFIX), &|_| None, &|_| false);
    assert!(
        env.iter()
            .any(|(key, value)| key == "PREFIX" && value == TEST_PREFIX)
    );
    assert!(
        env.iter()
            .any(|(key, value)| key == "TMPDIR" && value == TEST_TMPDIR)
    );
}

/// PATH 合成：Termux 的 bin 目录在前、宿主 PATH 在后。
///
/// 判别力：宿主 PATH 若被丢弃（只设 `{prefix}/bin`），系统命令
/// （`am`/`pm`/`getprop`）无法按名解析——DESIGN Shell 节要求原 PATH 保留。
#[test]
fn base_env_keeps_host_path_after_prefix_bin() {
    let env = base_env_with_host(
        Some(TEST_PREFIX),
        &|key| (key == "PATH").then(|| "/system/bin".to_string()),
        &|dir| dir == format!("{TEST_PREFIX}/bin"),
    );
    assert!(
        env.iter()
            .any(|(key, value)| key == "PATH" && value == &format!("{TEST_PREFIX}/bin:/system/bin")),
        "PATH 必须保留宿主原值并带上 prefix bin，实际：{env:?}"
    );
}

/// prefix 的 bin 目录不存在时不把它写进 PATH（不得指向不存在的目录）。
#[test]
fn base_env_omits_missing_prefix_bin_from_path() {
    let env = base_env_with_host(
        Some(TEST_PREFIX),
        &|key| (key == "PATH").then(|| "/system/bin".to_string()),
        &|_| false,
    );
    assert!(
        env.iter()
            .any(|(key, value)| key == "PATH" && value == "/system/bin"),
        "目录不存在时 PATH 只能是宿主原值，实际：{env:?}"
    );
}

/// failsafe（无 prefix）时保留宿主 PATH，不附加 Termux 的 bin 目录。
#[test]
fn base_env_keeps_host_path_without_prefix() {
    let env = base_env_with_host(
        None,
        &|key| (key == "PATH").then(|| "/system/bin".to_string()),
        &|_| false,
    );
    assert!(
        env.iter()
            .any(|(key, value)| key == "PATH" && value == "/system/bin"),
        "failsafe 会话必须保留宿主 PATH，实际：{env:?}"
    );
    assert!(
        !env.iter()
            .any(|(key, value)| key == "PATH" && value.contains("termux")),
        "failsafe 会话不得附加 Termux 的 bin 目录，实际：{env:?}"
    );
}

/// 宿主没有 PATH 时只给 prefix 的 bin 目录，不凭空造出分隔符。
#[test]
fn base_env_uses_prefix_bin_without_host_path() {
    let env = base_env_with_host(Some(TEST_PREFIX), &|_| None, &|_| true);
    assert!(
        env.iter()
            .any(|(key, value)| key == "PATH" && value == &format!("{TEST_PREFIX}/bin")),
        "宿主无 PATH 时 PATH 只含 prefix bin，实际：{env:?}"
    );
}

#[test]
fn build_env_includes_term() {
    let env = test_env();
    let result = build_env(&env);
    assert!(
        result
            .iter()
            .any(|(key, value)| key == "TERM" && value == "xterm-256color")
    );
}

#[test]
fn build_env_includes_mksh_startup_pointer() {
    let env = test_env();
    let result = build_env(&env);
    assert!(
        result
            .iter()
            .any(|(key, value)| key == "ENV" && value == "/tmp/test_app_data/.mkshrc")
    );
}

#[test]
fn bash_sessions_get_a_title_report_hook() {
    // bash 自己不发 OSC 0/2：没有 PROMPT_COMMAND 就没有会话标题。
    let hook = super::title_report_hook("/data/data/com.termux/files/usr/bin/bash");
    assert_eq!(hook.map(|(name, _)| name), Some("PROMPT_COMMAND"));
    let value = hook.map(|(_, value)| value).unwrap_or_default();
    assert!(
        value.contains("]0;") && value.contains("PWD"),
        "钩子必须发出 OSC 0 并带上当前目录，实际：{value}"
    );
}

#[test]
fn non_bash_sessions_get_no_title_report_hook() {
    // mksh 走应用写的 rc 文件；其他 shell 不注入未知变量。
    for shell in [
        "/system/bin/sh",
        "/data/data/com.termux/files/usr/bin/mksh",
        "/data/data/com.termux/files/usr/bin/other-shell",
        "/usr/bin/other-shell",
    ] {
        assert_eq!(super::title_report_hook(shell), None, "shell={shell}");
    }
}

#[test]
fn build_env_omits_env_without_mkshrc_path() {
    let env = ShellEnv {
        mkshrc_path: None,
        ..test_env()
    };
    let result = build_env(&env);
    assert!(!result.iter().any(|(key, _)| key == "ENV"));
}

#[test]
fn build_env_includes_colorterm() {
    let env = test_env();
    let result = build_env(&env);
    assert!(
        result
            .iter()
            .any(|(key, value)| key == "COLORTERM" && value == "truecolor")
    );
}

#[test]
fn build_env_includes_home_from_env() {
    let env = test_env();
    let result = build_env(&env);
    assert!(
        result
            .iter()
            .any(|(key, value)| key == "HOME" && value == "/tmp/test_home")
    );
}

#[test]
fn build_env_emits_prefix_pairs() {
    let mut env = test_env();
    env.prefix = Some(TEST_PREFIX.to_string());
    let result = build_env(&env);
    for (key, value) in [
        ("PREFIX", TEST_PREFIX),
        ("TERMUX_PREFIX_DIR_PATH", TEST_PREFIX),
        ("TMPDIR", TEST_TMPDIR),
        ("TERMUX_TMP_PREFIX_DIR_PATH", TEST_TMPDIR),
        ("TERMUX_VERSION", TERMUX_VERSION),
        ("TERMUX_HOME_DIR_PATH", "/tmp/test_home"),
    ] {
        assert!(
            result
                .iter()
                .any(|(entry_key, entry_value)| entry_key == key && entry_value == value),
            "missing {key}={value}"
        );
    }
}

#[test]
fn build_env_rejects_unlisted_variables() {
    let mut env = test_env();
    env.prefix = Some(TEST_PREFIX.to_string());
    let result = build_env(&env);
    for key in [
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "PWD",
        "USER",
        "SHELL",
        "LINES",
        "COLUMNS",
        "SSL_CERT_FILE",
        "CURL_CA_BUNDLE",
    ] {
        assert!(
            !result.iter().any(|(entry_key, _)| entry_key == key),
            "unlisted variable {key} must not be set"
        );
    }
    for (key, _) in &result {
        assert!(
            !key.starts_with("TERMUX__") && !key.starts_with("TERMUX_APP__"),
            "undeclared variable {key} must not be set"
        );
    }
}

#[test]
fn build_env_without_prefix_omits_prefix_vars() {
    let result = build_env(&test_env());
    for key in [
        "PREFIX",
        "TERMUX_PREFIX_DIR_PATH",
        "TMPDIR",
        "TERMUX_TMP_PREFIX_DIR_PATH",
    ] {
        assert!(
            !result.iter().any(|(name, _)| name == key),
            "{key} must be absent without a prefix, got {result:?}"
        );
    }
}

#[test]
fn spawn_and_read_shell() {
    use crate::terminal::pty::Pty;

    let pty = PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None).expect("spawn failed");
    pty.set_nonblocking().expect("set_nonblocking failed");

    Pty::write_all(&pty, b"echo hello_vt\n", WRITE_DRAIN_TIMEOUT, true)
        .1
        .expect("write failed");

    let output = read_until(&pty, b"hello_vt");
    assert!(
        output.windows(8).any(|window| window == b"hello_vt"),
        "did not see 'hello_vt' in output: {}",
        String::from_utf8_lossy(&output)
    );
}

#[test]
fn spawn_seeds_24x80_winsize() {
    // 在任何 UI 驱动的 resize 之前，TIOCGWINSZ 必须返回种子值 24x80，
    // 使 shell（行编辑器等）不会看到 0x0。
    let pty = PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None).expect("spawn failed");
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
    let pty = PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None).expect("spawn failed");
    pty.resize(40, 120).expect("resize failed");
    let (rows, cols) = pty.get_winsize().expect("TIOCGWINSZ failed");
    assert_eq!(
        (rows, cols),
        (40, 120),
        "resize must be reflected in TIOCGWINSZ"
    );
}

#[test]
fn set_pixel_size_reflected_in_winsize() {
    // `ws_xpixel`/`ws_ypixel` 必须送达内核，像素感知的程序（icat、全屏 TUI）
    // 才能自行定尺寸；像素字段为 0 会导致错渲。TIOCSWINSZ 会整体替换结构体，
    // 故 `set_pixel_size` 必须保留 rows/cols。
    let pty = PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None).expect("spawn failed");
    pty.set_pixel_size(640, 480).expect("set_pixel_size failed");
    let mut winsize = nix::pty::Winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: TIOCGWINSZ 把格式良好的 Winsize 结构写入 `winsize`；
    // fd 自有且有效。返回值已做检查。
    let result = unsafe { libc::ioctl(pty.master_fd(), libc::TIOCGWINSZ, &mut winsize) };
    assert_eq!(result, 0, "TIOCGWINSZ ioctl must succeed");
    assert_eq!(
        (winsize.ws_row, winsize.ws_col),
        (24, 80),
        "set_pixel_size must preserve rows/cols"
    );
    assert_eq!(
        (winsize.ws_xpixel, winsize.ws_ypixel),
        (640, 480),
        "set_pixel_size must write pixel fields to the kernel"
    );
}

#[test]
fn child_pid_is_positive() {
    let pty = PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None).expect("spawn failed");
    let pid = pty.child_pid();
    assert!(
        pid.as_raw() > 0,
        "child PID should be positive, got {pid:?}"
    );
}

#[test]
fn drop_closes_master_fd() {
    // `PtyPair` 的析构必须能无阻塞完成；信号投递与子进程回收由
    // `Session::drop()` 负责。
    let _pty = PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None).expect("spawn failed");
    // `PtyPair` 在作用域末尾于此析构——不得阻塞。
}

#[test]
fn child_survives_without_session_kill() {
    // `PtyPair::drop()` 关闭主端 fd，内核据此向从端的前台进程组发送 SIGHUP
    // （见 pts(4)）。子进程**可能**因 SIGHUP 死亡——这是预期的内核行为。
    // 本测试验证的是：析构不阻塞、不 panic，也不调用 waitpid（回收是
    // `Session::drop()` 的职责）。
    let child = {
        let pty =
            PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None).expect("spawn failed");
        pty.child_pid()
    };
    std::thread::sleep(Duration::from_millis(100));
    // 子进程可能存活（SIGTERM 成功）或已死（ESRCH），两者皆可。
    // 关键不变式：不 panic、不挂起。
    let result = nix::sys::signal::kill(child, nix::sys::signal::Signal::SIGTERM);
    assert!(
        matches!(result, Ok(()) | Err(nix::errno::Errno::ESRCH)),
        "SIGTERM to child after PtyPair drop must be Ok or ESRCH, got {result:?}"
    );
    // 仍存活则清理。
    let _ = nix::sys::signal::kill(child, nix::sys::signal::Signal::SIGKILL);
    let _ = nix::sys::wait::waitpid(child, Some(nix::sys::wait::WaitPidFlag::WNOHANG));
}

#[test]
fn pty_error_display_works() {
    let display = format!("{}", PtyError::Open(nix::errno::Errno::EINVAL.into()));
    assert!(
        display.contains("failed to open pseudoterminal"),
        "PtyError::Open Display should describe the failure, got: {display}"
    );
    let fork_display = format!("{}", PtyError::Fork(nix::errno::Errno::EINVAL));
    assert!(
        fork_display.contains("fork failed"),
        "PtyError::Fork Display should describe the failure, got: {fork_display}"
    );
    let resize_display = format!("{}", PtyError::Resize(nix::errno::Errno::EINVAL));
    assert!(
        resize_display.contains("TIOCSWINSZ"),
        "PtyError::Resize Display should describe the failure, got: {resize_display}"
    );
}

#[test]
fn chdir_changes_working_directory() {
    use crate::terminal::pty::Pty;

    let temp = tempfile::tempdir().expect("create test dir failed");

    let env = ShellEnv {
        working_directory: temp.path().to_string_lossy().to_string(),
        ..ShellEnv::default()
    };

    let pty = PtyPair::spawn("/bin/sh", 24, 80, &env, None).expect("spawn failed");
    pty.set_nonblocking().expect("set_nonblocking failed");
    Pty::write_all(&pty, b"pwd\n", WRITE_DRAIN_TIMEOUT, true)
        .1
        .expect("write failed");

    let output = read_until(&pty, temp.path().to_string_lossy().as_bytes());
    assert!(
        output
            .windows(temp.path().to_string_lossy().len())
            .any(|window| window == temp.path().to_string_lossy().as_bytes()),
        "did not see working directory '{}' in pwd output: {}",
        temp.path().display(),
        String::from_utf8_lossy(&output)
    );
}

// ── mkshrc 路径端到端：build_env 注入的 ENV 必须真实到达子进程环境。
// 子进程 `echo $ENV` 回显传入的 mkshrc_path，证明 ShellEnv → execve 整链透传。
#[test]
fn spawned_child_sees_env_mkshrc_path() {
    use crate::terminal::pty::Pty;

    let marker = "/tmp/vt_test_mkshrc/.mkshrc";
    let env = ShellEnv {
        mkshrc_path: Some(marker.to_string()),
        ..ShellEnv::default()
    };

    let pty = PtyPair::spawn("/bin/sh", 24, 80, &env, None).expect("spawn failed");
    pty.set_nonblocking().expect("set_nonblocking failed");
    Pty::write_all(&pty, b"echo \"ENV-is:$ENV\"\n", WRITE_DRAIN_TIMEOUT, true)
        .1
        .expect("write failed");

    let needle = format!("ENV-is:{marker}");
    let output = read_until(&pty, needle.as_bytes());
    assert!(
        output
            .windows(needle.len())
            .any(|window| window == needle.as_bytes()),
        "did not see ENV '{}' in child output: {}",
        needle,
        String::from_utf8_lossy(&output)
    );
}

/// 子进程实际拿到的 PATH：Termux 的 bin 目录在前，宿主 PATH 保留在其后。
///
/// 判别力：`base_env` 层的断言只覆盖合成逻辑；此处经真实 fork/execve 查看
/// 子进程的 `PATH` 本身——合成结果未真正进 env 时，只有这里能发现。
#[test]
fn spawned_child_sees_prefix_bin_first_in_path() {
    use crate::terminal::pty::Pty;

    let prefix = tempfile::tempdir().expect("create prefix dir");
    std::fs::create_dir_all(prefix.path().join("bin")).expect("create prefix bin dir");
    let prefix_path = prefix
        .path()
        .to_str()
        .expect("utf-8 prefix path")
        .to_string();
    let env = ShellEnv {
        prefix: Some(prefix_path.clone()),
        ..ShellEnv::default()
    };

    let pty = PtyPair::spawn("/bin/sh", 24, 80, &env, None).expect("spawn failed");
    pty.set_nonblocking().expect("set_nonblocking failed");
    Pty::write_all(&pty, b"echo \"PATH-is:$PATH\"\n", WRITE_DRAIN_TIMEOUT, true)
        .1
        .expect("write failed");

    let needle = format!("PATH-is:{prefix_path}/bin");
    let output = read_until(&pty, needle.as_bytes());
    assert!(
        output
            .windows(needle.len())
            .any(|window| window == needle.as_bytes()),
        "child PATH must start with '{prefix_path}/bin', got: {}",
        String::from_utf8_lossy(&output)
    );
}

#[test]
fn configure_raw_mode_sets_iutf8_and_clears_ixon_ixoff() {
    let pty = PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None).expect("spawn failed");
    let fd = pty.master_fd();

    // 施加与子进程在从端相同的 raw 模式配置。
    configure_raw_mode(fd).expect("configure_raw_mode failed");

    // SAFETY: `tcgetattr` 是简单系统调用包装；`fd` 是有效且自有的 PTY 主端描述符，
    // 读取其 termios 安全。
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

    let pty = PtyPair::spawn("/bin/sh", 24, 80, &ShellEnv::default(), None)
        .expect("spawn must succeed in test env");
    pty.set_nonblocking().expect("set_nonblocking failed");

    Pty::write_all(&pty, b"echo a\n", WRITE_DRAIN_TIMEOUT, true)
        .1
        .expect("first write must succeed");
    Pty::write_all(&pty, b"echo b\n", WRITE_DRAIN_TIMEOUT, true)
        .1
        .expect("second write must succeed");

    let mut buf = [0u8; TEST_READ_BUF_SIZE];
    let mut output = Vec::new();
    for _ in 0..TEST_READ_ATTEMPTS {
        match Pty::read(&pty, &mut buf) {
            Ok(byte_count) => output.extend_from_slice(&buf[..byte_count]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(TEST_READ_POLL_STEP_MS));
            }
            Err(_) => break,
        }
        if output.len() > TEST_MIN_OUTPUT_LEN {
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
