## Context

动机见 proposal.md。现状（均经代码与真机实测确认）：

- Kotlin `TerminalSurface.kt` 手势 `onScroll` 已有 `scrollAccumulatorPx` 亚行累积（`onDown` 清零，`distanceY` 累加，满 `cellHeight` 取整进位到 `scrollOffset`，余量保留）。当前余量**只累积不渲染**，即按行跳变的根因。
- 行通道：`scrollOffset: Int` → `onScrollChanged` → `runtime.setScrollOffset` → JNI `setScrollOffset` → `scroll_viewport(-delta)`（ghostty `Delta(isize), up is negative`，方向已实测正确）。
- fling：`OverScroller + postOnAnimation` 逐帧 `doFlingStep` 推行偏移（`TerminalSurface.kt:1386-1404`）。
- 渲染：wgpu cell 实例管线，行偏移改变时重推 CellData；`forceRender` 可逐帧请求呈现。

## Goals / Non-Goals

**Goals:**

- 行内余量实时渲染（跟手），跨行进位，松手对齐整行。
- 方向、fling 物理、行通道符号零改动。

**Non-Goals:**

- 不改 atlas、grid、字形度量、不改 `scroll_viewport` 语义。
- 不引入新滑动物理引擎（沿用 OverScroller）。
- 本变更不修方向/闪烁/CJK（实测均无问题，见 proposal）。

## Decisions

### D1: 复用现有 `scrollAccumulatorPx` 作为像素余量源，不新增状态

- Why：余量已存在且语义正确（跨行进位/手势起始清零），渲染侧只需消费它。零新状态即零新复杂度。
- Alternative（已排除）：Kotlin 另建像素通道——重复状态，两处余量 drift 风险。

### D2: 渲染侧新增 Y 像素偏移（钳制在一行内），行通道不动

- Why：行偏移仍走 `setScrollOffset`（VT 线程侧 CellData 正确性依赖它）；像素偏移只影响呈现位置，不影响行语义。偏移量钳制 `[0, cellHeight)`，超界即 bug（由进位逻辑保证）。
- 具体承载（二选一，实现时按 cell 管线现状定，见 Open Questions）：shader uniform 整体平移 vs cell 实例顶点 Y 偏移。推荐前者（单 uniform 改动最小），若管线无全局 uniform 则选后者。

### D3: 余量同步走现有 `forceRender` 节奏，不新增线程/回调

- Why：`doFlingStep` 每 vsync 已调 `forceRender`；拖动路径 `onScrollChanged` 同理。像素偏移随附在下一次呈现消费，无额外唤醒。
- 松手对齐：在 `stopFlingAnimation`/手势抬起处将余量归零并请求末帧（就近取整或回弹，实现时二选一，行为差异仅一帧）。

## Risks / Trade-offs

- [Risk] 顶部/底部边界余量（首行上无内容、末行下无内容时偏移露黑边）→ Mitigation：边界时余量钳制为 0（与现有 `coerceIn(0, scrollbackLen)` 行钳制同理），实现任务含边界用例。
- [Risk] cell 顶点偏移方案需改实例布局 → Mitigation：优先 uniform 方案；若不可行，顶点方案限定在 Y 平移一维，不碰 atlas UV/尺寸。
- [Risk] 高频 JNI 传像素值开销 → Mitigation：余量变化才发送（`target != scrollOffset` 既有门控模式），或复用现有 `onScrollChanged` 附带。

## Migration Plan

- 无需迁移：像素偏移缺省 0，未设置时逐像素等价于现状；回滚即 revert。
- 验证：maestro 慢速拖动截图位移连续（OCR 行块 Y 坐标渐变）；Rust 帧计时同级；方向 OCR 复核。

## Open Questions

- Q1：cell 管线全局 Y 平移用 uniform 还是实例顶点——实现前读 `cell_builder.rs` 实例布局后定（不影响 spec 与任务分解）。
- Q2：松手对齐取整 vs 回弹——实现时二选一，spec 已允许任一收敛行为。
