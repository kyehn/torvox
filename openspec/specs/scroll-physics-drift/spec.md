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

### Requirement: 边界外位移就地丢弃

已在边界（偏移 0 或回滚长度）时继续向边界外拖动的位移 MUST 就地丢弃
（`ScrollStep.overflowPx`），MUST NOT 作为亚行余量下发渲染器。下发会让整幅网格被投影
亚行平移、边缘露出一条背景空带，等余量攒够一整行才被清零、内容整体弹回一行
——即「上下滑动到边界跳动、闪烁」。边界**内**的亚行位移是合法的行内跟随，
MUST NOT 被丢弃；边界判定 MUST 排在亚行早退之前。

#### Scenario: 底部边界继续上滑

- **WHEN** 偏移为 0（实时视图）且用户继续上滑
- **THEN** 返回偏移 0、亚行余量 0，越界位移落在 `overflowPx`，不进入渲染器

#### Scenario: 顶部边界继续下滑

- **WHEN** 偏移等于回滚长度且用户继续下滑
- **THEN** 返回偏移不变、亚行余量 0，越界位移落在 `overflowPx`；有键盘位移可显出时
      先转为显出量（见 `ime-animation-smoothness`）

#### Scenario: 边界内的亚行跟随不受影响

- **WHEN** 偏移等于回滚长度且用户向边界内拖不足一行
- **THEN** 偏移不变，亚行余量照常保留并在渲染器上生效

### Requirement: 惯性滚动按像素推进

惯性滚动 MUST 以像素为行程单位交给 `OverScroller`（速度原样按像素/秒传入），逐帧
增量 MUST 与拖动共用同一换算与边界处理，MUST NOT 按行高把速度换算成行/秒——那样
每帧落点被截成整行，内容只能逐行跳到目的地（「上下滑动未实现按像素滚动」）。亚行
余量 MUST 在惯性期间下发渲染器，使内容行内连续跟随；行程起点 MUST 含当前亚行
余量（否则起步先弹回整行），上界 MUST 含键盘位移的显出量容量，使猛甩也能到达
「显出被位移裁掉的网格顶部」的行程末端（见 `ime-animation-smoothness`）。

#### Scenario: 慢速惯性不足一行

- **WHEN** 惯性速度为每秒不足一行的像素量，逐帧增量累加后仍不足一行
- **THEN** 偏移不变、亚行余量按像素累积并可下发渲染器，MUST NOT 逐帧跳整行；
      余量满一行时偏移恰好进位一行，不丢像素也不多走

#### Scenario: 惯性方向与拖动一致

- **WHEN** 同一物理方向先拖动再甩动
- **THEN** 惯性速度符号与拖动位移符号一致，偏移同向变化

#### Scenario: 猛甩到回滚区顶端

- **WHEN** 有键盘位移、惯性冲到回滚区顶端后仍有剩余行程
- **THEN** 剩余行程按像素转为位移显出量，可一直走到显出量容量，最早一行可见

### Requirement: fling 中断须收尾滚动状态

`stopFlingAnimation()` 在中断进行中的 fling 时 MUST 清零亚行余量（`scrollAccumulatorPx` 与 `setScrollRemainderPx`），并发出 `onScrollingStateChanged(false)` 回调，使 render-loop 的 `shouldResetScroll` 不被旧 fling 残留状态阻止。

#### Scenario: 惯性滚动中触屏打断

- **WHEN** 用户在 fling 动画进行中触摸屏幕
- **THEN** fling 立即停止，`isScrolling` 为假，像素余量为零，后续新输出可自动回底

#### Scenario: 惯性自然结束

- **WHEN** fling 动画自然减速至停止
- **THEN** `finishFlingAnimation` 正常发出 `onScrollingStateChanged(false)`（现有行为，不回归）
