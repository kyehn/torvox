# Proposal

## Why

`logcat-hygiene` 要求逐帧诊断以窗口汇总输出，且该行 MUST 带上窗口内的事件计数。
命令入队失败、剪贴板队列溢出、VT 应答队列溢出三处早已改用带计数的闸门并带上
次数，事件队列与桌面通知队列的溢出告警却仍停在旧口径。

旧口径是「只限频、不计数」：闸门放行时输出一次，窗口内的命中直接丢弃，行内也
没有次数字段。后果是排障时看不出形状——看到「队列满」既不知道丢了多少条，也
不知道丢的是持续在丢还是只丢了一次。

两处都处在可被程序洪泛的路径上：事件队列由 PTY 输出驱动（远端可无界地循环
OSC 52 读请求），桌面通知队列由 OSC 9 / OSC 777 通知驱动。洪泛时逐条告警会把
logcat 刷满，真正的错误反而看不见；只限频不计数则让「刷了多少」不可见。

## What Changes

- 事件队列溢出告警改用带计数的闸门，行内新增 `dropped_events` 字段给出窗口内
  累计丢弃的事件数。
- 桌面通知队列溢出告警改用带计数的闸门，行内新增 `dropped_notifications` 字段
  给出窗口内累计丢弃的通知数。
- `slow_log::CountedLogThrottle` 补回 `with_interval`：两个队列的静默期是 1s，
  而默认窗口是 500ms，差异 MUST 体现为闸门的配置而非另一份私有实现。
- `logcat-hygiene` 的「高频诊断日志必须限频」补两条场景：事件队列与桌面通知
  队列的溢出告警。

## Capabilities

### New Capabilities

无（不新增能力）。

### Modified Capabilities

- `logcat-hygiene`：给「高频诊断日志必须限频」补两条场景（事件队列与桌面通知
  队列的溢出告警）。

## Impact

- `native/src/event.rs`：事件队列溢出告警改用带计数的闸门。
- `native/src/terminal/ghostty_terminal/notification_queue.rs`：桌面通知队列溢出
  告警改用带计数的闸门。
- `native/src/slow_log.rs`：`CountedLogThrottle` 补 `with_interval`。
- `openspec/specs/logcat-hygiene/spec.md`：归档时同步场景。
