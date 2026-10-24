# Proposal

## Why

`docs/STATUS.md`（受保护文件）仍列着四项「未被解决的问题」：上下滑动滚动缓慢卡顿闪烁、
文本搜索的匹配与当前匹配高亮未区分、kitten icat 显示图像后 `clear` 需清两次才撤下图像、
长按选择后手柄越过对侧手柄时选区异常变动且手柄与边界不再对齐。

这四项此前只有「已改」的说法，没有可复核的判据与规范落点：行为要求散落在实现里，
回归靠肉眼，故同一类缺陷会在下一次改动中原样复现。

## What Changes

- `text-selection`：补齐手柄交叉的完整语义——触点折算的参考点在整次拖动内固定，
  被拖的边界必须停在手指所在格，未拖动的一侧必须钉在交叉前的位置；手柄与选区
  边界 MUST 始终对齐。
- `kitty-graphics`：补齐两条行为要求——清屏（ED2）MUST 在同一次清除内撤下图像放置
  并清掉屏幕上的图像像素；占位符扫描 MUST 按绝对行复用，回看历史不得全量重扫。
- `text-search`：行为与既有规范一致（当前命中 alpha 255、其余 160、视口外命中两侧
  裁剪），本轮只把折算与配色抽成可表驱动验证的纯函数并补齐判别力测试，规范不动。
- 新增留档 `status-disposition.md`：四项逐条写明当前状态、自动化判据（用例名与计量）、
  仍需真机确认的部分与原因；原始日志行、时间戳、PID 与设备标识不入库。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `text-selection`：手柄交叉时的触点折算与边界归属语义（参考点固定、被拖边界跟手、
  未拖动侧钉住、手柄与边界对齐）。
- `kitty-graphics`：清屏即撤下图像放置并清除像素；占位符扫描按绝对行复用。

## Impact

- `android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt`（拖动触点折算：
  参考点抓柄时锁定，边缘滚动路径改用同一折算）
- `android/app/src/main/java/terminal/emulator/TerminalViewModel.kt`（搜索高亮折算
  抽成纯函数，行为不变）
- `native/src/android/ffi.rs`（搜索高亮折算纯函数 + 表驱动测试）
- `native/src/terminal/ghostty_terminal/`（ED2 撤下放置的不变式与占位符扫描缓存的用例）
- `android/app/src/test/java/terminal/emulator/ui/DragTargetCellTest.kt`（新增）
- `android/app/src/androidTest/java/terminal/emulator/SelectionDragQuantifiedTest.kt`、
  `android/app/src/androidTest/java/terminal/emulator/ui/KittyGraphicsPlacementTest.kt`
- 不改 `docs/specification/` 与 `docs/STATUS.md`。
