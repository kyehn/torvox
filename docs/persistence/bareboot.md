# bareboot 研究留档

## 定位

Bareboot 不是普通的本地终端 App：它在 root module 里运行 `bareboot daemon`，把发行版作为 ext4 镜像挂在 loop 设备上，holder 进程建立 mount/UTS/PID/IPC namespace 后 `pivot_root` 启动 PID 1；Android App 只是一个通过 Unix socket 连接的附着客户端。因此关闭 App 后 shell 仍存活。终端 UI 使用 `io.github.sagernet.libghostty.GhosttyTerminalSession`。

## 关键实现与文件锚点

- `android/app/src/main/java/ch/ausfuhrli/bareboot/terminal/TabConnection.kt`：`GhosttyTerminalSession.Transport` 实现、`readLoop`、`Os.poll`、wake pipe、`ByteArray(16 * 1024)`。
- `android/app/src/main/java/ch/ausfuhrli/bareboot/ipc/BarebootClient.kt`：`DEFAULT_SOCKET_PATH = "/dev/socket/bareboot/barebootd.sock"`。
- `android/app/src/main/java/ch/ausfuhrli/bareboot/BarebootViewModel.kt`：标签页、会话标题与设置状态。
- `rust/crates/protocol/src/fdpass.rs`：PTY master 的 FD 传递。
- `rust/crates/runtime/src/auth.rs`：`SO_PEERPIDFD` 校验。
- `rust/crates/runtime/src/mount.rs`：`pivot_root`。
- `rust/crates/{daemon,display,cli}/`：daemon、Wayland 合成器与命令行入口。

## 附着链路

`BarebootClient` 经 `/dev/socket/bareboot/barebootd.sock` 与 daemon 通信，daemon 传回 PTY master 的 FileDescriptor。`TabConnection` 实现 `GhosttyTerminalSession.Transport`：

- `start` 先把 `attached.replay` 喂给会话，再启动名为 `bareboot-tab-<id>-read` 的守护读线程。
- 读循环用 `Os.poll` 同时等 master 与一根 wake pipe，超时 -1；`EINTR` 继续，`EAGAIN` 重试，读到 `n <= 0` 退出；每次读 16 KiB 后 `feedOutput`。
- `sendInput` 与 `sendResize` 投递到单线程 Executor，写 master 时循环处理部分写；resize 经 `BarebootClient.resizeTab` 走 socket。
- `close` 写 wake pipe 让读循环退出，`release` 关闭 master 与两根 pipe 以及 socket；`session.finish()` 与 `onExit` 只触发一次。

`BarebootViewModel` 持有标签页、会话标题、主题与字体设置；搜索用 `tab.session.transcriptText()` 的纯文本行，不是 VT 回滚搜索。

## daemon 侧

Android App 通过 `unix socket` 与 daemon 交换协议消息；daemon 管理镜像、holder、Wayland 合成器 `bareboot-display`、GPU 渲染 `Kumquat`（gfxstream）与视频编码 `bareboot-media`。Linux 图形程序走 Mesa Vulkan 到 Kumquat，OpenGL 经 Zink。桌面 companion 通过 adb 开端口转发、开窗与推文件。README 给出的基准显示其进程创建明显快于 Termux，而计算与 IPC 接近。

## 已核实局限

- 文本搜索走 transcript 纯文本，拿不到 VT 语义（如宽字符、软换行、OSC 8），与本仓规范“搜索使用 libghostty 提供能力”不同。
- 依赖 root、GKI 6.6+ 与 `SO_PEERPIDFD`，systemd/Docker 还需要 PID namespace；这是本仓明确不要求的环境。
- 会话标题与标签在 ViewModel 侧用 `mutableStateListOf`/`mutableStateMapOf` 管理，属于应用状态；本仓也由 Kotlin 持有会话表，但渲染调度与 VT 状态必须由 Rust 负责。

## 与本仓对照

- FD 传递 + poll 读循环 + wake pipe 的优雅关闭，是“传输适配器与本地 PTY 解耦、控制流与字节流分离”的一个完整实例，可用来核对本仓会话关闭与读线程退出是否存在竞态。
- 本仓不做 daemon、不做 namespace、不做桌面 GUI，也不允许 transcript 式搜索；只吸收“会话在 App 之外存活”和“附着时回放已有输出”的用法。
- 读循环的 `EINTR`/`EAGAIN` 处理与部分写循环是可靠的工程细节，但本仓 PTY 读写已封装在 Rust 侧，不应在 Kotlin 里复制一份。
