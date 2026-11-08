# Proposal

## Why

剪贴板应答回写失败（OSC 52 的答复写回 PTY）当前是逐次记 `warn`：每次失败一行。

逐次记不是理论情形。洪泛削减路径按帧封顶——每帧最多写 16 次应答，这些回写都
发生在渲染线程上，且每条都可能因 PTY 写超时而失败。子进程不读 stdin 时，一帧
16 条失败就是 16 行 warn，60fps 下即每秒近千行。真正的缺陷（编码失败、会话通道
失效）会被这层洪水挤到不可读。

同一处的洪泛削减告警早已限频（`FLOOD_SHED_WARN_THROTTLE`），回写失败这条却逐次
输出；两者描述的是同一次洪泛事件的前后两半，口径却分成两种。

## What Changes

- 剪贴板应答回写失败的告警改走带计数的限频闸门：一个窗口一行，行内带窗口内
  累计的失败次数，窗口内的计数延后交出而不丢弃。
- 分级维持 warn：失败是真实异常，限频只把「逐条」改为「按窗口」，不压住信号。
- 会话已结束导致的回写失败仍是正常退化路径：记 debug，且不计入失败窗口。
- 补单测：首条立即交出窗口计数；窗口内的失败攒着下次交出；会话已结束不计数。
- `logcat-hygiene` 增一条 Requirement：剪贴板应答回写失败的告警按窗口输出。

## Capabilities

### New Capabilities

无（不新增能力）。

### Modified Capabilities

- `logcat-hygiene`：增一条 Requirement，规定剪贴板应答回写失败告警的限频口径。

## Impact

- `native/src/android/ffi/render/clipboard.rs`：回写失败告警改走限频闸门。
- `openspec/specs/logcat-hygiene/spec.md`：归档时同步该条 Requirement。
