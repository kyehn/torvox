# 空闲帧光标行上报

## Why

实测取证（`ImeDiagTest#dumpPanInputs`）：120 行输出定居后弹出 IME，`renderWithNewOutput`
返回 `renderCount=0 renderCursor=-1`，`cursorRowFlow` 恒 `-1`，但直查
`getCursorViewportPacked` 光标在第 44 行（45×48 网格底部）。IME 跟随平移按
`computeTerminalPanPx` 在光标未知时返回 null（保持旧平移），内容较多时终端不上移，
`contentManyImePopupMovesUpBottomIdentical` 差分为零而失败。

根因：`renderWithNewOutput` 把光标采样放在 `if count > 0` 门内。空闲帧
`render_inner` 返回 0（无新单元数据，无需 GPU 呈现——正确），但光标行查询
`Query::RenderCursor` 被同一门控连带跳过。空闲时终端无新输出正是最需要光标坐标的时刻
（IME 弹出/定居后、历史浏览时）。

## What Changes

- `renderWithNewOutput`：光标行采样与 `new_output` 消费移出 `count > 0` 门，空闲帧同样上报。
- 代价：空闲帧复用本帧已渲染缓存，不新增发往 VT 线程的同步查询。
- 错误路径（count<0）仍回 0xFFFF。
- 影响面：仅 `cursorRowFlow` 在空闲时也能更新；渲染呈现逻辑不动；Kotlin 侧无需改动
  （变更检测 `if (cursorRow != entry.cursorRow)` 已存在，无值变不触发重组）。

## Non-Goals

- 不改变 `render_inner` 的空闲返回 0 语义（省 GPU 呈现是正确行为）。
- 不引入独立光标查询 JNI——复用已合并的单次穿越。
