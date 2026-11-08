# Proposal

## Why

第二轮全量复核（规范七篇、1292 个文本文件、五个参考项目）确认：上一轮
`display-adaptation` 变更的四条发现至今未修复，且本轮新增三条事实：

- `MainActivity` 已为 `previousNightMode`、`previousFontScale` 保存两个字段，
  但没有密度字段；`AndroidManifest.xml` 的 `configChanges` 仍缺 `density`、
  `smallestScreenSize`、`keyboard`、`navigation`，与 termux-app 的八项声明
  （含 `fontScale` 在内本项目六项）相比少四项。
- `vsyncFrameCallback` 无条件自投；空闲时主线程仍按显示帧率被唤醒。参考的
  Drosh 用 `start()`/`stop()` 成对启停，本项目没有停止路径；链常驻理由在
  注释里的 `pending_flash_phase` 字段全仓已不存在（上一轮留档已证）。
- `TerminalRuntime.start()` 用整块显示尺寸推导 Surface 下界，分屏与自由窗口
  冷启动仍会被误判过小；仪器化测试用 `test.minSurface` 绕过同一守卫的
  事实没有改变。

前后台、电源、诊断、并发四份 `docs/audit` 留档中，未收口的「终端会话
概率卡死」仍无实现（`session-liveness` 主规约已存在），但不属于显示
适配范围，不在本变更处置。

## What Changes

- 新增显示适配第二轮审查留档：独立复核五个参考项目 Manifest 与帧驱动，
  给出与上一轮逐条对照的复核结论。
- 新增 `display-adaptation` 第二轮增量规约：密度变化就地生效（补
  `densityDpi` 比较与重推）、帧回调链随活跃度启停、窗口尺寸以窗口为准、
  配置声明补齐四项。
- 把实现登记为未完成任务并注明授权前置条件（Manifest 与源码属受保护
  需授权修改面），不修改源码、测试、脚本与受保护文件。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `display-adaptation`：补第二轮复核的密度重推、帧回调启停、窗口下界、
  配置声明四条要求。

## Impact

- `docs/audit/display-adaptation-round2-review.md`：第二轮审查留档。
- `openspec/changes/2026-11-08-display-adaptation-round2/`：提案、设计、
  任务与规约增量。
- 归档时同步 `openspec/specs/display-adaptation/spec.md`。
- 源码、测试、AndroidManifest 与受保护脚本均不在本次改动范围。
