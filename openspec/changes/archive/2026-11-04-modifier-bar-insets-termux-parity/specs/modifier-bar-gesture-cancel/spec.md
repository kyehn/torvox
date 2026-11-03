## REMOVED Requirements

### Requirement: 键栏位于系统底部手势区之上

**Reason**：该判定建立在「键位落入手势区会重现按下动画不复位回归」的前提上。
前提已失效——`ExtraKeyButton` 已实现 `ACTION_CANCEL` 合成（`processCancel` +
`isGestureCancelled`）与按下反馈延迟（`PRESS_FEEDBACK_DELAY_MS` 覆盖取消窗口），
被认领的手势不再产生输出或残留动画；且同能力第一个 Requirement（触摸取消不触发
按键）已把该行为钉为规约。布局层为手势区留白遂失去必要性：ZTE 折叠屏外屏等
导航栏 inset=0 的纯手势设备上，附加内缩只在键栏下留出一条背景色空带
（`logcat_2026-11-03_17-29-27.txt` 取证）。

**Migration**：触摸安全改由取消链路承担（「触摸取消不触发按键」Requirement 不变），
布局层只按导航栏 inset 内缩——见本变更 ADDED 的「底部内缩只按导航栏 inset」。

## ADDED Requirements

### Requirement: 底部内缩只按导航栏 inset

内容盒底部 MUST 只按导航栏 inset（`navigationBarsPadding`）内缩，MUST NOT 为
全面屏手势区（`safeGestures` 超出导航栏的部分）附加内缩：导航栏 inset 为 0 的
纯手势设备（折叠屏外屏等）上，附加内缩会在键栏下留出一条只有背景色的空带。

键栏底边因此允许落在系统手势区内。手势认领的后果由「触摸取消不触发按键」
Requirement 与按下反馈延迟承担：静止轻点不被系统认领、必须正常触发；起滑点命中
按键的上滑被系统认领时，取消链路必须无输出、无残留动画。

终端 Surface 与键栏覆盖层 MUST 贴内容盒底边（键栏与内容盒之间不得出现任何内缩带），
内容盒底边 MUST 恰在导航栏上沿。

#### Scenario: 导航栏 inset 为 0 的设备无键栏下空带

- **WHEN** 设备使用全面屏手势导航且导航栏 inset 报 0（如 ZTE 折叠屏外屏）
- **THEN** 键栏底边与终端 Surface 底边都落在屏幕底边，键栏下不再出现背景色空带

#### Scenario: 带胶囊条的设备键栏贴导航栏上沿

- **WHEN** 设备导航栏 inset 大于 0
- **THEN** 内容盒底边恰在导航栏上沿，键栏覆盖层底边与内容盒底边齐平

#### Scenario: 手势区内的按键轻点仍触发

- **WHEN** 键位落在系统手势区内，用户在按键上静止按下并抬手
- **THEN** 按键正常触发一次（底部上滑需要位移才会被系统认领，轻点不受影响）

#### Scenario: 起滑点命中按键的上滑被认领

- **WHEN** 用户从某个按键上起滑、向上滑动被系统手势认领并向应用补发 `ACTION_CANCEL`
- **THEN** 按键不触发、无残留动画（由「触摸取消不触发按键」与按下反馈延迟承担）
