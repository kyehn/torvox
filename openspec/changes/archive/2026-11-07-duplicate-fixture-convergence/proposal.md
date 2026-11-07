# Proposal

## Why

`code-health` 规格要求「重复实现必须收敛为共享夹具」且「扫描命中必须逐条判定并留档」。
2026-11-05 的留档以当时的代码树为准（Kotlin 6 处 / 0.25%），此后新增的用例带进三处
未被覆盖的真重复：两份用例各抄了一份「构造运行期假件 + 构造 ViewModel」的夹具，另一处
两条用例各写了一份「只计数的键栏」夹具。

留档停在历史基线上，这些命中无人判定——按规格，未判定的命中即视为未完成的审查。

## What Changes

- 收敛三处真重复：
  - `FailsafeSessionRequestTest` 与 `NotificationSessionSwitchTest` 共用的运行期假件与
    ViewModel 构造夹具，收进 `TerminalViewModelFixture.kt`；
  - `ModifierBarRobolectricTest` 两条抽屉用例共用的「只计数抽屉键与粘贴键」的键栏夹具；
  - 更严阈值下另有两处：前台服务用例的「建服务 → 启命令」，通知用例的「装切换 mock +
    挂信号」。
- 新增留档 `audit.md`：本轮扫描参数、前后对比与逐条判定。
- 规约 `code-health` ADDED 一条 Requirement：审计留档必须覆盖当前代码树。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `code-health`：新增「审计留档必须覆盖当前代码树」要求（留档随代码树刷新）。

## Impact

- `android/app/src/test/java/terminal/emulator/TerminalViewModelFixture.kt`：新增共享夹具
- `android/app/src/test/java/terminal/emulator/FailsafeSessionRequestTest.kt`
- `android/app/src/test/java/terminal/emulator/NotificationSessionSwitchTest.kt`
- `android/app/src/test/java/terminal/emulator/ui/ModifierBarRobolectricTest.kt`
- `android/app/src/test/java/terminal/emulator/service/TerminalForegroundServiceTest.kt`
- `openspec/specs/code-health/spec.md`：留档覆盖当前代码树的要求
