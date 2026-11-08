# Proposal

## Why

`logcat-hygiene` 要求逐帧诊断以窗口汇总输出，且该行 MUST 带上窗口内的事件计数。
OSC 52 写回路径上的两类告警——负载被截断、写入因通道满被丢弃——此前逐条记一行。

两条都在可被程序洪泛的路径上：负载截断随每次超限的 OSC 52 写入发生，写入丢弃随
每次通道满（`bounded(16)`）发生。程序持续发送时，逐条告警会把 logcat 刷满，
真正的错误反而看不见。

即便限频，窗口内发生了多少次也不可见：看到一行「channel full」既不知道丢了
一条还是一百条，也不知道截断是偶发还是持续。

## What Changes

- OSC 52 负载截断告警改用带计数的闸门，行内给出窗口内累计截断的次数。
- OSC 52 写入丢弃告警改用带计数的闸门，行内给出窗口内累计丢弃的次数。
- 两类诊断各持一把闸门：它们是不同的事，共用一把会让先到的诊断吞掉后到的次数。
- `logcat-hygiene` 的「高频诊断日志必须限频」补一条场景：OSC 52 写回诊断。

## Capabilities

### New Capabilities

无（不新增能力）。

### Modified Capabilities

- `logcat-hygiene`：给「高频诊断日志必须限频」补一条场景（OSC 52 写回诊断）。

## Impact

- `native/src/terminal/ghostty_terminal/internal/run.rs`：OSC 52 写回回调的两类
  告警改用带计数的闸门。
- `native/src/terminal/ghostty_terminal/internal/tests.rs`：补回归用例。
- `openspec/specs/logcat-hygiene/spec.md`：归档时同步场景。
