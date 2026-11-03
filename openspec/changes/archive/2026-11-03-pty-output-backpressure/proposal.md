# Proposal

## Why

`logcat_2026-11-02_17-33-06.txt` 里 **22778 行**是同一条告警：

```text
W/native::terminal::ghostty_terminal::public_api
ghostty_terminal: cmd_tx full/dropped failed: sending on a full channel
```

全部来自 PTY 读取线程（同一 pid/tid）。它不是日志格式问题，而是**丢数据**：
`Session::poll_pty_output` 对每块 PTY 输出调用 `pty_write`（内部 `try_send`），
VT 线程忙于长命令时命令通道（容量 1024）满，块被**逐块丢弃**并逐块告警。

后果有两层：

1. 日志：一次输出高峰刷出两万余行，把真正的锚点淹没（`logcat-hygiene` 的要求失效）。
2. 正确性：丢掉的字节永不重放，终端状态与 PTY 流从此**永久分叉**——屏幕少了
   内容，程序与用户看到的不是同一个终端。

限频告警只能压住第 1 层。用户明确要求「修复问题，而不是掩盖或限制日志」，故必须
修掉「丢」本身。

## What Changes

- **改成真背压**：命令通道满时把**已成形**的载荷交还调用方，块存入会话的单槽
  `pending_pty_output`，本帧停止从 PTY 通道取新数据。
- 压力由此沿链路向上游传递：`pending_pty_output` → 有界 `output_rx`（容量 128）→
  读取线程的阻塞 `send` → 内核 PTY 缓冲 → 子进程 `write` 阻塞。全链路不丢字节。
- 拆出 `resend_pty_write`（只入队、不再成形）：重投必须复用同一份成形结果，二次
  成形会把「上块以 `\r` 结尾、本块以 `\n` 开头」的合法 CRLF 重新算一遍，补出多余
  回车。
- `pty_write` 保留为「无重试槽调用方」的兼容入口（测试与一次性馈入），通道满时按
  窗口累计告警后丢弃；会话路径不再走它。

## Capabilities

### New Capabilities

- `pty-output-backpressure`：PTY 输出在 VT 命令通道拥塞时逐帧重试而非丢弃。

### Modified Capabilities

- `logcat-hygiene`：丢弃类告警不再出现在正常路径上，因为正常路径不再丢数据。

## Impact

- `native/src/terminal/ghostty_terminal/public_api.rs`：`pty_write` 拆为
  `build_pty_write_buffer` / `resend_pty_write` / `try_pty_write` / `pty_write`。
- `native/src/terminal/session/{mod,spawn,output}.rs`：新增 `pending_pty_output` 单槽。
- 回归用例：`full_command_channel_backpressures_output_instead_of_dropping`、
  `try_pty_write_hands_back_the_shaped_payload_when_full`、
  `resend_pty_write_does_not_reshape_payload`。
- 代价：VT 线程长时间卡住时，终端输入会随上游阻塞而变慢——这是背压的**定义**，
  而不是回归；旧实现的「不卡」是用丢字节换来的。
