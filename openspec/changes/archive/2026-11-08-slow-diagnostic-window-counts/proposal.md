# Proposal

## Why

`logcat-hygiene` 要求逐帧诊断以窗口汇总输出，且该行 MUST 带上窗口内的事件计数。

原生侧的四类逐帧诊断（帧内分段、渲染分段、单元构建、采样段）与宿主侧的两条
（`SLOW_FRAME`、`SLOW_LOOP`）都只做了「限频」：窗口内命中的次数被直接丢弃，
行内也没有次数字段。

后果是排障时看不出形状：慢帧诊断只说「这一帧慢了 58ms」，不说「这 500ms 里慢
了几帧」——后者才是区分「持续慢」与「偶发慢」的关键。命令入队失败、剪贴板队列
溢出、VT 应答队列溢出这几处早已改用带计数的闸门并带上次数，逐帧诊断却仍停在
旧口径：同一份规范下出现两种做法。

## What Changes

- 原生侧四类逐帧诊断改用带计数的限频闸门，行内新增 `slow_frames` 字段给出窗口
  内的慢帧数。
- 宿主侧 `SLOW_FRAME` 与 `SLOW_LOOP` 各持一把带计数的闸门，行内新增
  `slow_frames` / `slow_loops` 字段给出窗口内次数。
- 宿主侧新增 `CountedLogThrottle`：与原生侧同语义（单调时钟判间隔、窗口内的
  计数延后交出而不丢弃）。
- `logcat-hygiene` 的「高频诊断日志必须限频」补一条场景：窗口次数与行内既有的
  渲染实例数各自以独立字段给出。

## Capabilities

### New Capabilities

无（不新增能力）。

### Modified Capabilities

- `logcat-hygiene`：给「高频诊断日志必须限频」补一条场景（窗口次数的字段名）。

## Impact

- `native/src/android/ffi.rs`、`native/src/android/ffi/render/frame.rs`、
  `native/src/android/ffi/render/diagnostics.rs`、`native/src/render/pass/frame.rs`：
  四类逐帧诊断改用带计数的闸门。
- `android/app/src/main/java/terminal/emulator/runtime/CountedLogThrottle.kt`：新增。
- `android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt`：
  `SLOW_FRAME` 与 `SLOW_LOOP` 改用带计数的闸门。
- `android/app/src/test/java/terminal/emulator/runtime/CountedLogThrottleTest.kt`：
  新增回归用例。
- `openspec/specs/logcat-hygiene/spec.md`：归档时同步实现说明。
