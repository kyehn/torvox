# Proposal

## Why

一轮 logcat 审计（3 分 25 秒会话）暴露出一条真实的应用侧缺陷链：平台回收 Surface
后废弃其 BufferQueue，而宿主在**新窗口挂上原生之前**就恢复了渲染，那一帧正落在
已废弃的队列上——`dequeueBuffer failed: No such device (-19)`、
`ERROR_SURFACE_LOST_KHR`，随后被迫重建整条交换链。

时序实测：恢复渲染（`setRenderPaused: paused=false`）比新窗口挂载
（`FFI: attachWindow`）早 27ms。同一会话内第二次重建只差 15ms 且无在途帧落在切换
点，故零报错——这说明它是竞态，不是必现故障，也正因如此此前未被察觉。

同一份日志里其余 W/E 条目（厂商分配器探测、平台合成器、上游 kitty 介质、
输入法会话切换竞态等）经逐条归因后确认不是应用缺陷，本次一并留档。

## What Changes

- 「Surface 销毁」那次渲染持有改由**挂载门闸**持有：只在新窗口挂上原生时解除，
  恢复渲染的路径（切前台、延迟解除暂停、强制出帧）一律无权解除。
- 恢复渲染前先判「新窗口是否已挂上」：未挂上时保持暂停，也不重启渲染线程
  （避免逐帧记「render paused」诊断）。
- 删除 `clearLifecycleRenderPause` 与 `clearSurfaceDestroyedRenderPause` 两个入口：
  前者与 `resumeRenderingFromBackground` 等价，后者与门闸冲突。
- 新增 `SurfaceAttachGate`（纯状态，可 JVM 单测）与配套单测。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `render-stability`: 新增「渲染恢复 MUST 晚于新窗口挂载」要求；原「强制出帧只作废
  生命周期持有」一条的 Surface 销毁持有改由挂载门闸解除。

## Impact

- 运行期：`TerminalRuntime.kt`（挂载门闸、恢复路径）、`TerminalSurface.kt`
  （延迟解除暂停、强制出帧）、`TerminalScreen.kt`（切前台恢复）。
- 新增：`android/app/src/main/java/terminal/emulator/ui/SurfaceAttachGate.kt` 与
  `android/app/src/test/java/terminal/emulator/ui/SurfaceAttachGateTest.kt`。
- 规范：`openspec/specs/render-stability/spec.md`；审计留档：本变更的 `evidence.md`。
- 不涉及原生代码、依赖与对外 API。
