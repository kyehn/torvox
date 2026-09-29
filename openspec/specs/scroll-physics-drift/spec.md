# scroll-physics-drift Specification

## Purpose

消除拖动与惯性滚动中的 1 行级漂移和虚浮感。像素到行的换算若用向零取整之外的
语义，跨零点的正负亚行余量阈值就不对称：向上拖过零点时无位移、向下同样余量却
跳一行，视觉上是内容「虚浮」。fling 被触摸打断时若不清零亚行余量，render-loop
的 `shouldResetScroll` 会被旧 fling 残留状态挡住，新输出无法自动回底。

## Requirements

### Requirement: 亚行余量取整对称

`applyScrollDistance` 中像素→行转换 MUST 使用截断趋向零语义（`toInt()`），正/负方向的亚行阈值 MUST 对称。正值余量不触发亚行移动时，同量级负值余量也不得触发；反之亦然。

#### Scenario: 拖动经过零点

- **WHEN** 用户拖动使累加器从正（向上）越过零变为负（向下），且绝对值均 < 1 行
- **THEN** 双方向均不触发行偏移变化，不产生 1 行漂移

#### Scenario: 往返等量拖动

- **WHEN** 用户向上拖动 1.5 行再向下拖动 1.5 行
- **THEN** 最终偏移与起始偏移一致（±1 行容差内）

### Requirement: fling 中断须收尾滚动状态

`stopFlingAnimation()` 在中断进行中的 fling 时 MUST 清零亚行余量（`scrollAccumulatorPx` 与 `setScrollRemainderPx`），并发出 `onScrollingStateChanged(false)` 回调，使 render-loop 的 `shouldResetScroll` 不被旧 fling 残留状态阻止。

#### Scenario: 惯性滚动中触屏打断

- **WHEN** 用户在 fling 动画进行中触摸屏幕
- **THEN** fling 立即停止，`isScrolling` 为假，像素余量为零，后续新输出可自动回底

#### Scenario: 惯性自然结束

- **WHEN** fling 动画自然减速至停止
- **THEN** `finishFlingAnimation` 正常发出 `onScrollingStateChanged(false)`（现有行为，不回归）
