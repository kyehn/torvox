## Why

实测（向 `Key_CTRL` 注入 `DOWN`+`CANCEL`）：修饰键栏按键把系统手势认领触摸时送达的 `ACTION_CANCEL` 当成抬手处理，按键被触发。全面屏手势（底部上滑回桌面、侧缘返回）在按键上起滑并中途被认领时会误触修饰键——违反 `DESIGN.md`“修饰键不应该和全面屏手势冲突，不应该被上滑手势触发”“底部上滑绝不能触发按键”。超出 touchSlop 的正常上滑已被既有滑出取消拦住，唯独取消路径漏网。

## What Changes

- `ExtraKeyButton` 手势处理三个分支（普通点按、自动重复、长按副动作）经 `PointerEvent.motionEvent` 识别 `ACTION_CANCEL`，取消即吞掉：不触发按键、不触发长按副动作。
- 仪器测试：View 派发 `DOWN`+`CANCEL` 不触发 `Key_CTRL`（先红后绿），`DOWN`+`UP` 对照仍触发。

## Capabilities

### New Capabilities

- `modifier-bar-gesture-cancel`: 修饰键栏触摸取消绝不触发按键，键栏位于系统底部手势区之上。

### Modified Capabilities

无。

## Impact

- `ModifierBar.kt` 手势处理三处分支与一个私有判定函数。
- `ModifierBarTest.kt` 新增两项仪器测试。
