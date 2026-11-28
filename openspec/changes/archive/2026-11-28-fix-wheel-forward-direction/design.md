# Design

## 上下文

同一手势由两条路径消费：

- 本地回滚：`applyScrollDistance(accumulatorPx, distanceY, …)`。`distanceY` 为
  `GestureDetector` 的「前一帧减当前帧」（手指上移为正），累加换算为
  `accumulator - distanceY`，正 = 更旧的待消化位移。
- 转发滚轮：`wheelDragStep(accumulatorPx, distanceY, …)`，此前为 `accumulatorPx + distanceY`，
  且把正值映射为滚轮上。

两处共用 `surface.scrollAccumulatorPx`，故累加器的符号约定必须一致：约定不同则同一
手势内路径切换时余量被反向解释。

## 决策

转发路径改用与本地回滚相同的换算：

- `olderPx = accumulatorPx - distanceY`（正 = 更旧的待消化位移）
- `olderNotches = (olderPx / cellHeight).toInt()`，截断趋向零，正负两向对称
- 按钮按符号取：正 → 滚轮上（更旧），负 → 滚轮下（更新）

取整沿用截断趋向零而非 floor：floor 在负方向过激（-0.9 行 → -1 行，而 +0.9 行 → 0 行），
连续拖动会累积 1 行级漂移。

## 对标

termux `TerminalView`：`onScroll` 的 `deltaRows = (int)(distanceY / mFontLineSpacing)`
经 `doScroll(rowsDown)`，`up = rowsDown < 0` 时鼠标上报路径发滚轮上；手指上移时
`deltaRows > 0` → `rowsDown > 0` → 滚轮下，与该路径的本地回滚（`mTopRow + 1`，更新）
同向。即 termux 保持「同一手势在两条路径上看到同一段内容」。

## 风险与兜底

方向反转的判据是「转发路径与本地回滚同向」，由纯函数单测同时比对两条路径的结论；
变异验证（恢复旧符号、改 floor 取整）确认用例可判红。仪器用例改判 SGR 65
（ghostty `Button::Five`），与原生 `mouse_button` 的 4 → `Button::Five` 映射一致。
