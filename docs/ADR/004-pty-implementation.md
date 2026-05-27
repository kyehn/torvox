# ADR 004: PTY 实现 — nix crate 直接 forkpty + W^X 多调用二进制

**状态**: 已接受
**日期**: 2026-05-26
**决策者**: 项目负责人

---

## 上下文

PTY (伪终端) 是终端模拟器的核心 I/O 机制。在 Android 上, PTY 实现面临独特挑战：

1. **Android bionic 与 glibc 差异**: `openpty()`/`forkpty()` 行为不完全一致
2. **Android 14+ W^X 限制**: 后台 `exec()` 被限制
3. **SELinux 策略**: 限制 PTY 从设备访问
4. **进程管理**: Android OOM killer 可能杀后台进程

## 决策

**直接使用 `nix 0.31` crate 实现 PTY, 不使用 `portable-pty`。使用多调用二进制模式绕过 W^X 限制。**

### 为什么不用 portable-pty

| 因素 | portable-pty | nix crate (选择) |
|------|-------------|------------------|
| Android 支持 | ❌ 不支持 (仅桌面 Unix/Windows) | ✅ 直接 syscall, bionic 兼容 |
| openpty/forkpty | 封装良好但不支持 Android bionic | 直接调用, 可处理 bionic 差异 |
| 交叉编译 | 未测试 Android NDK | ✅ nix crate 有 Android CI |
| 构建大小 | 较大 (多后端) | 较小 (仅 Unix 后端) |
| 维护者 | WezTerm 团队 (桌面优先) | nix crate 社区 (广泛使用) |

### PTY 实现细节

```rust
pub struct PtyPair {
    master: OwnedFd,
    child_pid: Pid,
}

impl PtyPair {
    pub fn spawn(shell: &str, rows: u16, cols: u16) -> Result<Self, PtyError> {
        let winsize = Winsize { ws_row: rows, ws_col: cols, ws_xpixel: 0, ws_ypixel: 0 };
        let pty = openpty(&winsize, None)?;

        match unsafe { fork() }? {
            ForkResult::Child => {
                setsid()?;
                close(pty.master)?;
                dup2(pty.slave, Stdio::stdin())?;
                dup2(pty.slave, Stdio::stdout())?;
                dup2(pty.slave, Stdio::stderr())?;
                close(pty.slave)?;
                execvp(shell, &[shell])?;
                unreachable!()
            }
            ForkResult::Parent { child } => {
                close(pty.slave)?;
                Ok(Self { master: pty.master, child_pid: child })
            }
        }
    }

    pub fn resize(&self, rows: u16, cols: u16) -> Result<()> {
        unsafe { ioctl(self.master.as_raw_fd(), TIOCSWINSZ, &winsize) }
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize> {
        read(self.master.as_raw_fd(), buf)
    }

    pub fn write(&self, data: &[u8]) -> Result<usize> {
        write(self.master.as_raw_fd(), data)
    }
}

impl Drop for PtyPair {
    fn drop(&mut self) {
        // 递增式终止: SIGHUP → SIGCONT → SIGKILL
        kill(self.child_pid, Signal::SIGHUP).ok();
        kill(self.child_pid, Signal::SIGCONT).ok();
        // 给进程 100ms 优雅退出
        std::thread::sleep(Duration::from_millis(100));
        kill(self.child_pid, Signal::SIGKILL).ok();
        waitpid(self.child_pid, None).ok();
    }
}
```

### W^X 变通方案

Android 10+ 限制非系统库的 `exec()`。Termux 验证了多调用二进制模式：

```
/data/data/io.torvox/bin/
├── torvox-exec          # 唯一二进制文件
├── ls -> torvox-exec    # 符号链接
├── cat -> torvox-exec
├── grep -> torvox-exec
├── vim -> torvox-exec
└── ...
```

`torvox-exec` 根据 `argv[0]` 读取调用者名称, 然后执行对应命令。这绕过了 W^X 限制，因为二进制文件由系统加载器映射为可执行。

> **实现注意**: 最初设计使用 `/proc/self/exe`，但该路径解析到实际二进制文件 (torvox-exec)，不保留符号链接名称。正确做法是检查 `argv[0]` 的 `file_name()` 部分。

```rust
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let name = std::path::Path::new(&args[0])
        .file_name().unwrap().to_str().unwrap();
    if name == "torvox-exec" {
        // 直接调用模式: torvox-exec <command> [args...]
        if args.len() < 2 {
            eprintln!("torvox-exec: 需要通过符号链接调用");
            std::process::exit(1);
        }
        execvp(&args[1], &args[2..]);
    } else {
        // 符号链接模式: 根据 argv[0] 名称执行对应命令
        execvp(name, &args[1..]);
    }
}
```

### Termios 配置

```rust
fn configure_raw_mode(fd: RawFd) -> Result<()> {
    let mut termios = tcgetattr(fd)?;
    termios.input_flags &= !(IGNBRK | BRKINT | PARMRK | ISTRIP
                              | INLCR | IGNCR | ICRNL | IXON);
    termios.output_flags &= !(OPOST);
    termios.local_flags &= !(ECHO | ECHONL | ICANON | ISIG | IEXTEN);
    termios.control_flags &= !(CSIZE | PARENB);
    termios.control_flags |= CS8;
    termios.control_chars[VMIN] = 1;
    termios.control_chars[VTIME] = 0;
    tcsetattr(fd, SetArg::TCSANOW, &termios)?;
    Ok(())
}
```

### 环境变量

```rust
fn build_env() -> Vec<(String, String)> {
    vec![
        ("TERM", "torvox-direct"),
        ("COLORTERM", "truecolor"),
        ("TERM_PROGRAM", "torvox"),
        ("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION")),
        // TERMINFO: 捆绑 terminfo 文件路径
    ]
}
```

## 后果

**正面**:
- 完全控制 Android PTY 行为, 可处理 bionic 差异
- 无第三方 PTY 库的桌面假设
- W^X 变通方案已被 Termux 在 1000万+ 安装上验证
- `kill_on_drop` 递增式终止防止僵尸进程

**负面**:
- `nix` crate fork 是 `unsafe` — 需要仔细处理信号安全
- 多调用二进制增加 APK 大小 (~1-2MB)
- W^X 方案可能在未来 Android 版本被进一步限制

**缓解措施**:
- `fork()` 调用封装在 `pty.rs` 模块中, 仅有界 `unsafe`
- `cargo geiger` CI 检查确保 `unsafe` 仅存在于 PTY 模块
- 监控 Android 版本变更, 准备替代方案 (如 proot)
