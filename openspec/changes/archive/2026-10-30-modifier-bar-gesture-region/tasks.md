# Tasks

## 1. 键栏与终端避开系统手势区

- [x] 终端承载 Box 的底部内边距取导航栏 inset 与 `safeGestures` 的较大者
- [x] 键栏随之抬出手势区（网格随 Surface 高度重算）
- [x] `ModifierBarTest#modifier_bar_bottom_position_above_gesture_zone` 改用手势区判定

## 2. 按下反馈的复位不得有位移

- [x] 缩放与底色的插值规格按方向分开：按下 `tween`，抬起/取消 `snap`
- [x] 按下延迟窗口保持不变（覆盖系统认领窗口）

## 3. 验证

- [x] `:app:connectedDebugAndroidTest -Pclass=terminal.emulator.ui.ModifierBarTest` 全绿
- [x] `ModifierBar.kt` / `TerminalScreen.kt` 通过 detekt 与 spotless
