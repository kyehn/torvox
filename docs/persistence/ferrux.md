# ferrux 研究留档

## 定位

“Rust 核心 + 薄 Kotlin 外壳”的 Termux 风格终端。Rust 工作区按职责拆成 `ferrux-term`、`ferrux-pty`、`ferrux-session`、`ferrux-env`、`ferrux-jni-core`、`ferrux-android` 等 crate；`ferrux-term`、`ferrux-pty`、`ferrux-session` 不依赖 Android，可在 Linux 上测试。另一个应用 Ferride 复用同一套 terminal crate 并通过 Binder 访问 Ferrux。

## 终端核心

`ferrux-term` 是 `alacritty_terminal` 的薄封装，只用它的 grid 与 parser，不用 `tty` 模块（后者会安装进程级 `SIGCHLD` 处理器）。`Terminal::feed` 解析程序输出，`snapshot` 返回给渲染器，`take_responses` 返回要写回程序的设备应答，`drain_events` 返回标题、响铃、剪贴板事件，`keys` 负责键与粘贴编码。

`Snapshot` 是纯数据：颜色已解析成 RGB、反显已应用、宽字符已标记、软换行位供复制与无障碍重连。`CellFlags` 的 `u32` 位布局是 Kotlin `SnapshotLayout` 的 FFI 契约。`TermConfig` 默认回滚 10000 行，`bold_is_bright` 为真，词分隔符去掉冒号以便长按选中整条 URL 或 `src/main.rs:10:5`；OSC 52 写剪贴板上限 1 MiB。脏信息用 `Damage::Full` 或 `Damage::Rows(Vec<usize>)` 表达。

## 会话与输入

`ferrux-session` 用一个 `poll(2)` I/O 线程读所有会话的 PTY、喂终端并冲刷排队输入；每个子进程有一个只等自己 pid 的 waiter 线程，避免 `waitpid(-1)` 与全局信号处理器。`ChildHandle` 在 `reaped` 锁下发信号，防止 pid 复用误杀。输入 outbox 上限 8 MiB，超限直接丢弃并告警；写输入会把视图带回实时底部。关闭会话先 `SIGHUP`，`CLOSE_GRACE` 为 3 秒。`ferrux-pty` 支持 Direct 与经 `/system/bin/linker64` 的 Linker 两种 exec 模式。`ferrux-env` 负责 bootstrap 解压、校验与原子重命名。

Kotlin `SessionManager` 起一条 `ferrux-events` 线程轮询 `TerminalNative.pollEvents`，解析标题、响铃、退出与剪贴板请求并更新 `StateFlow`；OSC 52 写入会先询问“Session N wants to copy …”，问题出现在终端下方而不是遮住正在输入的行。终端视图是 Kotlin Canvas（wgpu 的 `ferrux-render` 只是 M0 试验）：回滚 10000 行、fling、细滚动条、回到底部按钮；备用屏按 DECSET 1007 把滑动转成方向键；鼠标报告 1000/1002/1003 支持 X10、UTF-8、SGR 编码；软键盘弹出时顶部行进入回滚，关闭后回来。

## 已核实局限

- 剪贴板写入、系统操作等权限边界比本仓规范复杂得多；本仓明确不考虑安全与隐私，不引入同意流程与权限策略。
- 渲染器主体仍是 Kotlin Canvas，README 中描述的 wgpu 渲染处于试验阶段，不能把它当成已落地的 GPU 路径。
- 输入 outbox 超限采用“丢字节”策略，适合其远程/多会话场景；本仓的输入链路有有界快照缓冲，不应照搬丢弃语义。

## 与本仓对照

- `Snapshot` 纯数据加固定 `CellFlags` 位布局，与本仓 JNI 侧“只传轻量事件和查询结果、不送逐帧网格”的方向一致，值得核对本仓打包结构是否有明确契约与范围检查。
- 单 `poll(2)` 线程读所有会话、per-pid waiter、有界输入队列，都是本仓多会话 PTY 线程设计的参照；本仓的会话状态归 Rust，Kotlin 只持有会话表，这点与 ferrux 相同。
- 回滚 10000 行、软键盘顶起时把顶部行移入回滚、备用屏滑动转方向键，可作为本仓终端页行为测试的用例来源。
