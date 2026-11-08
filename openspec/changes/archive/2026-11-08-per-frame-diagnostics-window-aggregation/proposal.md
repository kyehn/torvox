# Proposal

## Why

渲染循环里有三处按帧判定的告警仍逐条记一行：滚动指令投递失败、渲染遇到未知会话、
VT 应答写回失败。三处都在逐帧路径上——滚动期间每帧投递一次滚动指令，宿主在会话
被销毁后仍可能逐帧渲染它，子进程不读 stdin 时 VT 应答每帧都写回失败。

后果是 logcat 被刷成洪水：60fps 下每处每秒可出数十行，真正的异常被淹没。即便
限频，窗口内失败了几次也不可见。

VT 应答写回失败还有更糟的一面：子进程已退出后，待写的应答永远无人接收，而旧实现
不判退出态，失败的应答退回队首、下帧再写，于是每帧记一条 error。「子进程已退出」
是正常退化，本就该记 debug。

## What Changes

- 三处逐帧告警改用带计数的闸门，行内分别给出窗口内累计失败的次数、命中帧数。
- VT 应答写回在子进程已退出时不再重试：丢弃待写应答并记 debug。
- `logcat-hygiene` 的「高频诊断日志必须限频」补三条场景（滚动投递失败、未知会话
  渲染、VT 应答写回失败）。
- `logcat-hygiene` 的「应用自身的正常路径不得产生警告与错误」补一条场景（子进程
  已退出后的 VT 应答写回）。

## Capabilities

### New Capabilities

无（不新增能力）。

### Modified Capabilities

- `logcat-hygiene`：给「高频诊断日志必须限频」补三条场景；给「应用自身的正常路径
  不得产生警告与错误」补一条场景。

## Impact

- `native/src/android/ffi/state.rs`：滚动指令投递失败告警改按窗口汇总。
- `native/src/android/ffi/render/frame.rs`：未知会话告警改按窗口汇总。
- `native/src/terminal/session/output.rs`：VT 应答写回失败改按窗口汇总；子进程已
  退出时丢弃待写应答并记 debug。
- `native/src/terminal/session/mod.rs`、`spawn.rs`：会话新增写回失败告警闸门。
- `native/src/terminal/session/tests/write_backpressure.rs`：补回归用例。
- `openspec/specs/logcat-hygiene/spec.md`：归档时同步场景。
