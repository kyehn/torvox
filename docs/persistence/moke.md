# moke 上游审查

- 上游：https://github.com/briqt/moke @ `0b70b2c`（2026-11-27）；产品 GPL-3.0-or-later，vendored Termux 模块 Apache-2.0。
- 定位：Android SSH/mosh 终端，内置 Tailscale、代理、tmux、SFTP 与加密备份；终端核心来自 Termux 生态。

## 架构与数据流

- `terminal-emulator/`（`TerminalEmulator`、`TerminalBuffer`、`TerminalRow`、`TerminalSession`、`TerminalOutput`）是 vendored Termux VT 与缓冲实现；`terminal-view/`（`TerminalView`、`TerminalRenderer`、`TerminalInputConnection`）负责绘制与输入。
- `app/src/main/java/com/briqt/moke/terminal/` 把认证、剪贴板、滚动与会话串起来：`SshTransport.kt`、`MoshTransport.kt`、`TmuxCoordinator.kt`、`TmuxRemote.kt`、`TerminalSession.kt`、`TerminalInputQueue.kt`。
- 传输线程把远端字节写入 64 KiB `ByteQueue`，队列满时阻塞读线程形成背压；`MainThreadHandler` 收到 `MSG_NEW_INPUT` 后在 UI 线程调用 `TerminalEmulator.append`，因此解析与绘制同线程，无跨线程屏幕竞争。
- `TerminalRenderer` 两遍绘制：先所有背景（含光标/选择），再所有文字；行内按 run 绘制，框线/图形由 `TerminalGraphics` 处理。
- `TmuxCoordinator` 管理远端面板与窗口；mosh 要求远端 `mosh-server` 与 UDP 可达，HTTP/HTTPS/SOCKS4 代理不承载 mosh。凭据用 Android Keystore 加密，端口转发只听设备 loopback。

## 已核实边界

- 双许可与 vendored 修改使再分发义务复杂；产品层 GPL-3.0 限制直接复用。
- 队列满时阻塞传输读线程是有意背压，但连接失败路径也会调用 `processToEmulator`，需注意失败时队列状态。
- 大量 androidTest 覆盖输入连接/剪贴板/滚动，但仓库内仍有 14 处 TODO/FIXME；Tailscale、代理、mosh、备份各自有状态机，复杂度远超终端核心。

## 对 torvox 的取舍

- 借鉴输入连接、剪贴板与滚动手势的仪器测试组织方式；不引入 SSH/Tailscale/备份。
- 对照本地 `android/app/src/main/java/terminal/emulator/ui/ImeConnection.kt`：保持批量编辑与 IME 提交的确定性，但本地输入编码在 `TerminalInputEncoder`，无需 Termux 队列。
- 凭据加密与“转发只听 loopback”是远程能力的安全边界示例；本地当前无远程能力，不移植。
- 跳过：未运行 androidTest 与 SSH/mosh 集成（需 SDK、网络与远端服务）；风险是网络状态机只按源码核对。
