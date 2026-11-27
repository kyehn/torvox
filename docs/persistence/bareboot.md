# Bareboot 上游审查

- 上游：https://github.com/rfc9849/Bareboot @ `868f685`（2026-11-26）。
- 定位：在已 root 的 Android 上直接启动未修改的 Linux 发行版；终端只是控制面，不是终端库。

## 架构与数据流

- `rust/crates/daemon/src/linux_impl.rs` 负责 mount namespace、`pivot_root`、seccomp 和进程存活；`dns.rs`、`desktop.rs`、`protocol/src/ahb_wire.rs` 分别承担 DNS、桌面 companion 与线协议。
- Kotlin 控制面在 `android/app/src/main/java/ch/ausfuhrli/bareboot/`：`BarebootViewModel.kt` 串联安装与会话，`ipc/BarebootClient.kt` 连守护进程，`terminal/TabConnection.kt` 维护单个 shell 会话。
- `TabConnection.start` 通过 `socketpair` 传 fd，读循环注册 `selector`（POLLIN），写输入与 resize 都排到同一个单线程 `io` executor；`sendInput` 同步 `Os.write`，`sendResize` 在 executor 上写 `TIOCSWINSZ`。
- `readLoop` 只在 `closing` 未置位时回报数据；EOF 时调用 `session.finish()` 并通过 `onExit` 通知 UI。UI 重建只需重新 attach 已有会话，不拥有 PTY。

## 已核实边界

- `sendInput` 在 `io` 线程阻塞写 master fd；PTY 输入缓冲满时会连带阻塞同线程的 resize，仓库没有 POLLOUT 或非阻塞写路径。
- 适用性由 `SystemChecks.kt` 约束：Linux 6.5+、`SO_PEERPIDFD`、root、SELinux enforcing；不满足即不进入安装流程。
- 没有与终端相关的 JVM 测试；大量 `unsafe` 来自系统调用与命名空间操作，镜像、模块脚本和桌面 companion 构成额外信任面。

## 对 torvox 的取舍

- 不采纳 root/容器启动链；本地 `native/src/terminal/pty.rs` 的 `PtyPair::spawn` 不依赖 root 或命名空间。
- 可借鉴“UI 与 PTY 所有者解耦”的 attach 语义；本地 `NativeBridge` 仍以会话句柄为唯一入口，不引入第二套所有权模型。
- 阻塞写输入的问题提醒本地保持 `native/src/android/ffi/input.rs` 的短临界区：JNI 只复制字节，不做阻塞 I/O。
- 跳过：未验证 Magisk/KernelSU 模块与 GPU 显示链路；风险是 root 环境下的行为无法在此复现。
