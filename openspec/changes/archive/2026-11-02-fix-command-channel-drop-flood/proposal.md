# Proposal

## Why

命令通道（`GhosttyTerminal::cmd_tx`，容量 1024）满时，每一次 `try_send` 失败都记一
行 `log::warn!`。实测一次输出高峰（PTY 持续输出、VT 线程处理不过来）在 2 秒内刷出
**22778 行**同文案告警：

```text
W/native::terminal::ghostty_terminal::public_api: ghostty_terminal: cmd_tx full/dropped failed: sending on a full channel
```

该告警来自 `logcat_2026-11-02_17-33-06.txt`，应用 pid 5660，集中在 `17:31:25`–`17:31:27`，
占该文件应用侧条目的 96.7%。后果有三：

1. **日志分级失效**：其余诊断（慢帧、慢循环、字体回退、GPU 错误）被同文案告警淹没，
   而 `logcat-hygiene` 规范把「日志分级是排障时区分应用缺陷与环境噪声的第一道闸门」
   写成了硬要求。
2. **日志通道成为瓶颈**：两万行同文案写入把 logcat 的吞吐吃满，排障时反而找不到
   真正的首条告警。
3. **同一状况被逐条记录**：命令通道满是**一种**状况（VT 线程繁忙），却按失败次数
   记成两万条，违反 `logcat-hygiene` 的「同一类诊断 MUST 只有一处限频判定实现」。

根因不在日志侧而在调用侧：命令入队失败共有 12 处调用点，各写各的 `log::warn!`，
没有一处经过限频闸门。仓库已有 `native/src/slow_log.rs` 的 `CountedLogThrottle`
（带计数的限频闸门），渲染侧慢帧诊断、剪贴板队列溢出告警、事件队列溢出告警都已
接入，唯独命令入队失败这一类没有。

## What Changes

- `native/src/slow_log.rs`：`CountedLogThrottle` 的计数由普通整数改为原子量，方法
  改收 `&self`。此前它只能被独占 `&mut` 的调用点使用，而命令入队失败来自渲染线程
  与输入线程，共享同一把闸门，必须能以 `&self` 共享。
- `native/src/terminal/ghostty_terminal/mod.rs`：`GhosttyTerminal` 增加
  `cmd_drop_log` 闸门，每终端一把。
- `native/src/terminal/ghostty_terminal/public_api.rs`：12 处命令入队失败告警统一
  走 `warn_command_dropped`——按 500ms 窗口放行一次，行内带上窗口内累计丢弃次数，
  并指明最近一次失败的操作名。

## Capabilities

### New Capabilities

无。本次不引入任何新的能力面。

### Modified Capabilities

- `logcat-hygiene`：把「高频诊断必须限频」这条既有纪律的适用面由「逐帧诊断」扩到
  「命令入队失败告警」——此前该纪律只落在逐帧诊断与队列溢出告警上，命令入队失败
  这一类高频告警是漏网的。

## 不做什么

- 不改命令通道的容量、批处理预算与丢弃语义：被丢弃的命令不会重放这一点由单独的
  变更处理（命令通道满时的输出留存），本次只治理日志。
- 不把命令入队失败降级为 info：通道满是真实异常（命令不会重放），按
  `logcat-hygiene` 的判据仍属「可记为 warn」的一类。
- 不改限频窗口长度：沿用 `logcat-hygiene` 既定的 500ms 下限。
