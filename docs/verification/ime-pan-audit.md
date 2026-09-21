# T2 IME 平移修复审计（404b21c / 32feec6）

## 结论

- T2 性能结构成立：`WindowImeBottomPx` 叶节点读取 insets、位移经 `snapshotFlow` + 布局期 `offset` 应用，动画帧不整屏重组。
- `computeTerminalPanPx` 公式正确：光标最小平移、钳制 `[0, imePx]`、光标隐藏返回 `null` 保持上次位置、键盘关闭返回 0。
- 发现一处实质回归并已修复（047faa1）：`barPanPx` 的 live 写入被放在 `delay(48ms)` 定居等待之后，`collectLatest` 语义下动画期间修饰键栏冻结、定居后跳变，与 delta spec“位移逐帧跟随”矛盾。修复为 live 写入先行、定居延迟随后。
- 顺带清理 T2 残留未使用变量 `val density`（主组合体；`WindowImeBottomPx` 内仍在使用）。

## 语义核对

- 位移来源：终端区 `heldTerminalPanPx` 由 `computeTerminalPanPx(followedCursorRow, cellHeight, boxHeight, barPanPx, reservedBarPx)` 驱动；修饰键栏 `barPanPx` 整体跟随键盘。
- 退格/回车后光标移动：`followedCursorRow` 订阅 `cursorRowFlow`，行号变化即重算 pan（光标上移→位移缩小，无需归位逻辑，公式自然收敛）。
- 隐藏归位：`barPx<=0 → heldTerminalPanPx=0`；键盘关闭时 `followedCursorRow` 重置为 `CURSOR_ROW_UNKNOWN`。
- scrollOffset 交互：历史浏览时光标行上报 `-1`（hidden bits），公式返回 `null` 保持 pan，不抢夺视图；`shouldResetScroll` 路径未受 T2 影响。

## 新增回归测试（TerminalImePanTest，7→11）

- `pan_grows_with_live_frame_during_show`：中间帧 550→250px、定居 1100→800px（冻结 settled 值会在此失败）。
- `pan_shrinks_when_cursor_moves_up`：同一键盘高度光标上移位移跟随缩小。
- `pan_follows_live_value_during_hide`：隐藏中间帧 live=100 时位移已回落到 100。
- `pan_never_exceeds_keyboard_height`：远超可视区光标钳制在键盘高度内。

## 验证

- `:app:testDebugUnitTest --tests terminal.emulator.ui.TerminalImePanTest --offline`：BUILD SUCCESSFUL，11 tests / 0 failures。
- 未触及 native，不跑 `cargo test`（约束：不改 native 渲染逻辑）。
