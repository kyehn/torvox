# Proposal

## Why

`docs/STATUS.md` 仍把“终端会话概率卡死”列为未决问题。现有 `RenderWatchDog` 只能判定
渲染线程长时间未完成一帧，无法区分卡死发生在 VT 线程、渲染线程还是 Kotlin 生命周期
路径，也无法把“挂起”与“恢复”关联到具体会话。缺少分层诊断与可恢复策略时，问题只能
靠真机 logcat 事后猜，且恢复手段只有整进程重启或关闭会话。

## What Changes

- 为渲染线程、VT 线程与输出投递定义统一的“忙标记（自忙起时刻）”口径，使看门狗能
  定位挂起层级，而不是只报告“渲染挂起”。
- 判据以“有工作在途”为前提：空闲 shell 的读取线程阻塞在 `read()`、VT 线程阻塞在
  通道等待上都不推进任何计数，按“计数未推进”判定会把空闲会话判死，故不采用。
- 把现有渲染看门狗从“标记线程死亡”扩展为“记录挂起层级并交由会话管理器决定恢复”；
  恢复策略按层级分级：渲染线程可重建，VT 层挂起时关闭会话（不静默重建）。
- 在 `TerminalRuntime` 的会话表中保留挂起诊断信息（层级、挂起起点），供日志与后续
  自动化判据使用，不改变用户可见的会话列表语义。
- 不改变 PTY 字节一致性、背压与退出语义；不新增权限、通知或后台服务。
- 不在本变更中实现代码，仅落设计与任务拆分；实现另起变更。

## Capabilities

### New Capabilities

无（本变更为设计与诊断契约整理，不新增用户可观察能力）。

### Modified Capabilities

无（不改变现有 spec 的需求语义；若后续实现落地，再以独立变更修改对应 spec）。

## Impact

- 设计文档：`openspec/changes/session-liveness-watchdog/design.md`
- 任务拆分：`openspec/changes/session-liveness-watchdog/tasks.md`
- 参考代码：`android/app/src/main/java/terminal/emulator/monitor/RenderWatchDog.kt`、
  `android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt`、
  `native/src/terminal/session/*`、`native/src/android/ffi/session.rs`
- 参考项目：termux-app 的会话线程分工与 `TerminalSession` 退出上报；Ghostty 的
  渲染线程邮箱与 mailbox 模型仅作设计参考，不直接移植
