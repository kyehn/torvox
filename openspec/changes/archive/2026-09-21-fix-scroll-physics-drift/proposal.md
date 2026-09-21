# fix-scroll-physics-drift Proposal

## Why

用户报"上下滑动不跟手、飘移、虚浮"。已落地一刀（384ae20 滚动手势收尾）治状态残留，但剩余两类物理层根因未解决：

1. **R3 floor 非对称**：`applyScrollDistance` 中 `floor(accumulator / cellHeight)` 在正/负余量下行为不对称（`floor(0.9)=0` 但 `floor(-0.9)=-1`），连续拖动累加出 1 行级漂移。
2. **R4 fling 收尾缺失**：`stopFlingAnimation()` 在 ACTION_DOWN 中断 fling 时未清 `scrollActive`/未发 `onScrollingStateChanged(false)`，导致 render-loop 的 `shouldResetScroll` 判断失准。

R1（双通道时差）经代码审查确认已通过 render-loop 单帧消费 `entry.scrollOffset` + `entry.scrollRemainderPx` 同步，无需改动。

## What Changes

### R3: `applyScrollDistance` 取整对称化

将 `floor((accumulator / cellHeight).toDouble()).toInt()` 替换为 `(accumulator / cellHeight).toInt()`（截断趋向零），使正/负方向的亚行阈值对称：±0.9px 都不触发行移动，±1.1px 都触发 1 行移动。

**文件**: `TerminalSurface.kt` — `applyScrollDistance` 纯函数  
**文件**: `ScrollDistanceTest.kt` — 新增对称截断测试用例

### R4: fling 中断收尾

`stopFlingAnimation()` 在 `flingScroller.isFinished == false` 时，除 force-finished + removeCallbacks 外，还须：清 `scrollAccumulatorPx`、清 `setScrollRemainderPx(0f)`、发 `onScrollingStateChanged(false)`，使 render-loop 的 `shouldResetScroll` 在新输出到达时不被旧 fling 残留的 `scrollActive` 阻止。

**文件**: `TerminalSurface.kt` — `stopFlingAnimation` 方法

## Verification

```bash
cd android && ./gradlew ':app:testDebugUnitTest' \
  --tests 'terminal.emulator.ui.ScrollDistanceTest' \
  --tests 'terminal.emulator.ui.GridToScreenTest'
```
