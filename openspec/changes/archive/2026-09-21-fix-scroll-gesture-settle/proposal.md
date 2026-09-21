## Why

慢速拖放抬指（无 fling、无 tap 回调）后 `isScrolling` 与亚行余量永久残留：内容恒定错位半行、新输出不再自动回底，体感为飘移虚浮。UP/CANCEL 分支只清选择状态，不收尾滚动。

## What Changes

- `ACTION_UP/CANCEL` 时若 `isScrolling` 为真，复用与点抬手一致的收尾：余量归零并推送渲染、发滚动结束。
- 不碰手势数学与惯性物理。

## Capabilities

### New Capabilities

- `scroll-gesture-settle`: 手势结束时滚动状态必收尾。

### Modified Capabilities

无。

## Impact

- `TerminalSurface.kt` 的 `onTouchEvent` UP/CANCEL 分支。
- 单元测试覆盖 `applyScrollDistance` 不变；收尾语义由手势单测锁定。
