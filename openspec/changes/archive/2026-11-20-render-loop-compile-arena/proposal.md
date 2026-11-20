# Proposal

## Why

2026-11-20 12-45-04 的真机日志里，ART 仍在为渲染循环记 6 条
`I/com.termux: Compiler allocated ...`：

```text
Compiler allocated 4232KB to compile java.lang.Object terminal.emulator.runtime.RenderSupervisor$startRenderThread$renderThread$1$1.invokeSuspend(java.lang.Object)
Compiler allocated 4972KB to compile java.lang.Object terminal.emulator.runtime.RenderSupervisor$startRenderThread$renderThread$1$1.invokeSuspend(java.lang.Object)
```

2026-11-15 的审计已把这一条列为当次唯一的新增可修项，并按职责抽出了退出回收
（`reapExitedSessions`）、慢循环诊断（`logSlowLoop`）与计时窗口（`logLoopTiming`）
三块。**但那次处置没有生效**：同一方法在 2026-11-20 的日志里仍以 4232KB／4972KB
出现，模拟器上复现的数值是 release 4520KB／4739KB、debug 5787KB／6019KB。

两个根因：

1. **方法体量仍在阈值之上**。ART 只在编译期 arena 工作内存超过 4MiB 时才打这一行；
   抽出三块并没有把剩余的循环体降到阈值以下，循环体仍是「唤醒门控 + 视口推送 +
   渲染发布 + 输出消费 + 失败处置 + 事件泵送 + 诊断发布 + 循环计时」一整块。
2. **口径写错了**。规范里把它写成「单方法被 ART 编译成约 4.9MB／5.8MB」，即编译
   产物的字节数。实际上这个数字是 `Jit::AddMemoryUsage` 上报的
   `ArenaAllocator::BytesUsed()`，即编译期的工作内存；AOSP 的编译产物字节数由
   `JitCodeCache::Reserve` 的另一条日志（阈值 50KB）上报。口径错了会让后续判据失焦。

## What Changes

- `RenderSupervisor.startRenderThread` 的渲染循环按职责拆成独立方法：唤醒门控、
  选区推送、视口滚动推送、渲染发布、表面失效处置、帧计时、新输出消费、渲染结果
  处置、输出泵送、诊断发布、循环计时；循环体只留流程编排。
- 保留渲染循环外层的 `runBlocking`：协程包装不是 arena 越线的原因（拆分后同一判据
  实测 0 条），去掉它只会额外改动关闭路径的中断语义而没有实测收益。
- `logcat-hygiene` 规范：校正「`Compiler allocated` 的数字」口径；新增
  「渲染循环的待编译方法必须保持小体量」要求；登记 2026-11-20 12-45-04 日志的
  聚合归因（应用进程 W/E 70 条逐条处置 + 首帧与设备丢失的时序）。

## 不做什么

- 不改 `R8` 等平台探测条目的处置：它们由 AOSP `swapchain.cpp` 的格式探测决定，
  应用侧无 API 可改（见同规范既有的分配器场景）。
- 不以过滤日志、降低日志等级或调高 ART 阈值来消除该行。
- 不改 `Cargo.toml`、`build.gradle.kts`、`settings.gradle.kts` 等受保护文件。
- 不动 `docs/STATUS.md`：该文件当前无未解决问题条目，且首行声明禁止修改。

## Impact

- Affected specs: `logcat-hygiene`（新增「渲染循环的待编译方法必须保持小体量」要求，
  校正编译 arena 口径，登记 2026-11-20 日志归因场景）。
- Affected code: `android/app/src/main/java/terminal/emulator/runtime/RenderSupervisor.kt`。
- 行为变化：无（渲染循环的判据、顺序与日志内容逐条保持不变；仅方法边界变化）。
- 验证方式：模拟器上以 release 与 debug 两种构建各跑一次，`logcat` 里 MUST NOT 再
  出现本应用的 `Compiler allocated` 行。
