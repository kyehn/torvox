# 任务

- [x] 建 change（proposal/tasks + `ime-editor-contract` 增量）
- [x] `ComposingBaseline` 增加 `replacedCursor`，`reanchorRegion` 改为 `acceptsRegionReanchor`
- [x] `TerminalSurface.replaceText` 改走重放计划，`setComposingRegion` 改为零字节零位移
- [x] 移除失去调用方的 `ComposingBaseline.compose`（死代码）
- [x] 新增单测：范围替换的光标落点、子区间重锚被拒、整段重锚受理、范围替换的重放计划
- [x] `ComposingBaselineTest` / `ComposingDiffTest` 全绿
- [x] 规范补上「组字区重锚不改文本」的要求，并更正范围替换与重锚的措辞
