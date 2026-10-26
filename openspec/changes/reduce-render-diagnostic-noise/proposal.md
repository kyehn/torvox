# Proposal

## Why

真机 logcat（2026-10-25 采集，84s 窗口）里，应用自身的日志有 1770 条，其中
**逐帧诊断占 85%**：

| 条目 | 条数 | 相邻间隔中位 | 级别 |
| --- | --- | --- | --- |
| `render_frame: presented …` | 691 | 15ms | debug |
| `setScrollOffset: target=… delta=…` | 222 | 24ms | debug |
| `FONT_XML: skip fallbackFor=…` | 373 | — | debug |
| `loop timing window` / `frame timing window` | 178 | 0.93s | info |

`logcat-hygiene` 规范要求逐帧诊断两次输出间隔不小于 500ms、不得以逐帧频率刷屏；
原生侧的「已呈现」诊断正是逐帧输出（15ms 一行），与该要求相抵。同一份采集里，
宿主侧帧节奏窗口的相邻间隔出现 492ms——60 帧的统计窗口在 120Hz 以上的设备上
不足 500ms，同样触线。

另有两处措辞与分级问题：

- `render_frame: presented N instances (partial=false, bands=1)` 里的 `bands` 是
  **脏带数**，全量帧也会打印（空闲路径只标光标行），却紧挨着 `partial=false`，
  极易被读成「部分帧指示」；上一次会话的判读也正是在这一点上绕了远路。
- 滚动偏移落点与同路径的 `setScrollYPx` 都是每次手势 MOVE 一条，前者留在 debug、
  后者已是 trace，分级互相矛盾。

（`FONT_XML` 的重复行已由「每进程只解析一次 fonts.xml」与「跳过项汇总成一行」
修掉，本次不再处理。）

## What Changes

- 原生侧的逐帧呈现诊断改为**按 500ms 窗口汇总**：每行带上窗口内的呈现帧数与
  末帧形态（实例数、是否部分帧、脏带数）。帧数留在汇总行里，限频不吞掉计数。
- 汇总行里的 `bands` 改称 `dirty_bands`，与 `partial` 明确区分。
- 滚动偏移落点日志降为 trace，与同路径的 `setScrollYPx` 一致。
- 宿主侧帧节奏窗口加 500ms 输出间隔下限：窗口攒满而间隔未到时，报告延后到
  间隔满足时给出，新帧覆盖最旧样本，统计不丢。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `logcat-hygiene`：补充「逐帧诊断以窗口汇总输出」「报告延后而不丢弃」
  「热路径落点日志只留 trace」的实现要求。

## Impact

- `native/src/render/slow_log.rs`：新增带计数的限频闸门（`CountedLogThrottle`）。
- `native/src/render/context.rs`：`Renderer` 持有呈现诊断的汇总闸门。
- `native/src/render/pass.rs`：逐帧呈现诊断改为汇总输出，`bands` 改称 `dirty_bands`。
- `native/src/android/ffi.rs`：滚动偏移落点日志降为 trace。
- `android/app/src/main/java/terminal/emulator/runtime/FrameTimingStats.kt`：
  输出间隔下限、样本环形覆盖、报告延后给出。
- `android/app/src/test/java/terminal/emulator/runtime/FrameTimingStatsTest.kt`：
  间隔下限与覆盖行为的用例。
- `android/app/src/androidTest/java/terminal/emulator/ScrollBehaviorQuantifiedTest.kt`：
  汇总行的语义措辞。
- `openspec/specs/logcat-hygiene/spec.md`：归档时同步新要求。
