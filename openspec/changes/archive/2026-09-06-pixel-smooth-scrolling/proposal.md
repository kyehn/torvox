## Why

规范（`docs/specification/DESIGN.md` 终端页面）要求“按像素流畅滚动（参考 ghostty-android-terminal）”。当前滚动按整行跳变（`scrollOffset: Int` 行单位，`scrollAccumulatorPx` 亚行余量仅累积不渲染），慢速拖动时内容逐行跳动，不符合规范。实测确认：滚动方向正确（上滑看历史，与 termux 一致，不改）、页面切换无闪烁（设置往返逐像素一致，不改）——本变更只补像素滚动缺口。

## What Changes

- 渲染侧新增视口 Y 像素偏移量的接收与应用（范围限制在一行高度内，行偏移仍走现有 `setScrollOffset` 行通道）。
- Kotlin 手势侧将 `scrollAccumulatorPx` 的亚行余量同步给渲染侧，拖动/ fling 逐帧更新像素偏移，跨行时行通道进位、像素余量归零。
- 行数不变时只重绘偏移（不重排、不重算 grid、不清 atlas），保证与现有帧率一致。

## Capabilities

### New Capabilities

- `terminal/pixel-scroll`: 手势滚动时内容按像素跟手平移（行内余量实时渲染），跨行进位后余量归零；松手对齐到整行。

### Modified Capabilities

（无。`openspec/specs/` 现有 `font/cjk-fallback` 不受影响。）

## Impact

- 影响代码：`native/src/render/`（cell 实例 Y 偏移 uniform/顶点）、`native/src/android/ffi.rs`（像素偏移 JNI 入口）、`TerminalSurface.kt`（余量同步 + fling 像素步进）。
- 无数据格式、无设置项、无 API 破坏；像素偏移缺省为 0，旧行为是新行为的特例（失败/未设置时与现状一致）。
- 规范依据：`docs/specification/DESIGN.md` 终端页面·按像素流畅滚动条款。
- 非目标（实测已排除，不在本变更内）：滚动方向（实测正确）、页面切换闪烁（实测无复现）、CJK 渲染速度（实测 avg/p95=0ms）。
