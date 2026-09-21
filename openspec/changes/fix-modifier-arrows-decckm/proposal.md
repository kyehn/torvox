## Why

修饰键栏方向键在 `nix --help` 等分页器（DECCKM 应用光标模式）中无作用：生产路径恒走可配置键栏，其方向键写死 CSI 序列；DECCKM 感知的 `dispatchArrow` 只在不可达的默认分支。输入法/硬件箭头走 `processKeyEvent` 感知模式所以可用。

## What Changes

- 可配置键栏方向键（含长按重复）在点击时刻经 `isAppCursorMode` 查询 DECCKM，按 `TerminalInputEncoder.arrowSequence` 编码（CSI/SS3）。
- 可配置路径两种模式单测覆盖。

## Capabilities

### New Capabilities

- `modifier-bar-arrow-encoding`: 可配置修饰键栏方向键跟随 DECCKM 编码。

### Modified Capabilities

无。

## Impact

- `ModifierBar.kt` 可配置路径接线；`TerminalScreen.kt` 接线不变（已传 `isAppCursorMode`）。
- `ModifierBarRobolectricTest` 新增可配置路径用例。
