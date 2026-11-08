# Proposal

## Why

平台对显示适配的约束是明确的：刷新率不恒定，系统可能因省电或热节流把它限制到
60Hz，应用必须仍能正确工作；未声明的配置限定符变化会重建 Activity。逐文件审查发现，
本项目的适配实现有四处与这些约束不符：

- 显示密度变化会重建 Activity，而重建路径不重推渲染契约，已有会话的文字光栅系数
  停在旧密度；同类故障已在字体缩放上修复并有注释留档，密度这一半没有覆盖。
- 首次建会话用整块显示尺寸推导 Surface 下界，分屏、自由窗口与折叠半屏的合法小窗口
  被判为过小，会话建不出来；只有窗口随后变大才会重试。
- 帧回调链在首个会话后无条件自投，没有停止路径；无会话、切后台、长时间空闲仍按
  显示帧唤醒主线程。链常驻的理由引用了一个已经不存在的字段。
- 配置变更声明缺少 `smallestScreenSize`、`keyboard` 与 `navigation`，参考项目均完整
  声明；硬件键盘接入、自由窗口形态变化会做可避免的重建。

这四条都不需要更换渲染栈或改动架构，属于「按平台口径补齐生命周期与判据」的落差。

## What Changes

- 新增显示适配专项审查留档：平台口径、实现现状、参考项目对照、逐条判定、复现方法。
- 新增 `display-adaptation` 能力，四条要求：配置变化就地生效、任意合法窗口可启动、
  帧回调链随活跃度启停、刷新率与省电只影响节拍不影响正确性。
- 把需要改源码、Manifest 或补测试的事项登记为未完成任务，注明授权前置条件。
- 不改状态文档、受保护文件、源码、测试与脚本。

## Capabilities

### New Capabilities

- `display-adaptation`：分辨率与显示密度变化、刷新率节拍、省电与空闲期帧回调的
  行为判据与可观察结果。

### Modified Capabilities

无（不改变现有能力的需求语义）。

## Impact

- `docs/audit/display-adaptation-review.md`：专项审查留档与复现方法。
- `openspec/changes/2026-11-08-display-adaptation/`：提案、设计、任务与规约增量。
- `openspec/specs/display-adaptation/spec.md`：归档时新建并同步四条要求。
- `android/app/src/main/AndroidManifest.xml`、`MainActivity.kt`、`TerminalRuntime.kt`、
  `TerminalSurface.kt`：本次不修改，修复步骤登记在任务中。
