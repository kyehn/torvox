# Proposal

## Why

触摸拖动转发为滚轮时，方向与本地回滚相反：转发路径把「手指上移」映射为滚轮上
（更早的历史），而本地回滚路径是「内容跟随手指」（手指下移进入更早的历史）。同一个
手势因此会因前台程序是否启用鼠标上报而朝相反方向滚动——用户的手感在应用切换那一刻
被反转；两条路径共用的亚行累加器符号约定也随之相反，手势中途在两路径间切换时余量
被反向解释，滚动跳变。

对标 termux `TerminalView.onScroll`：`deltaRows = (int)(distanceY / lineSpacing)` 经
`doScroll(rowsDown)` 落到 `rowsDown < 0 → 滚轮上`，即手指上移发滚轮下，与本地回滚
同向。本仓此前的实现与其自述的对标对象不一致。

## What Changes

- 转发滚轮的方向改为与本地回滚同一语义：手指下移（更旧）编码为滚轮上，手指上移
  （更新）编码为滚轮下
- 亚行累加器统一为「正 = 更旧的待消化位移」，两条路径共用同一符号约定
- 规范补上「转发滚轮与本地回滚同向」的要求与场景

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `key-input-encoding`：转发滚轮的方向 MUST 与本地回滚同向，累加器符号约定统一。

## Impact

- `android/app/src/main/java/terminal/emulator/ui/TerminalSurfaceMath.kt`：`wheelDragStep`
  的累加符号与按钮映射
- `android/app/src/main/java/terminal/emulator/ui/SurfaceInput.kt`：转发分支的注释口径
- `android/app/src/test/java/terminal/emulator/ui/WheelDragStepTest.kt`：方向、截断对称性
  与「转发与本地同向」回归钉
- `android/app/src/androidTest/java/terminal/emulator/ui/MouseReportingInstrumentedTest.kt`：
  拖动转发的按钮号判据（SGR 65）
- 受影响的行为：应用启用鼠标上报时，触摸拖动的滚动方向与未启用时一致
