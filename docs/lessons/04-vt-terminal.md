# VT/Terminal 经验教训

> **Note**: CSI handling is now managed by GhosttyTerminal internally. This lesson describes a historical bug in the old vte-based parser before the Ghostty migration.

## 1. CSI cursor_position 1-indexed 未转换导致光标错位

### 问题
VT100/ANSI 规范定义的 `cursor_position`（`ESC[#;#H` 或 `ESC[#;#f`）参数是 **1-indexed** 的（行列从 1 开始），但内部 Grid 数据结构是 **0-indexed** 的（行列从 0 开始）。代码直接接受 1-indexed 参数但忘记转换为 0-indexed，导致光标总是偏右 1 列、偏下 1 行。

这个 bug 在某些情况下看起来"几乎正确"（比如在 80 列终端中，第一行第一列和最后一行最后一列之间的差异不明显），但在边界条件（如第 1 行第 1 列定位到第 0 行第 0 列，第 25 行第 80 列超出网格范围）下暴露出来。

### 根因
VT 规范使用 1-indexed 坐标（"第一行"是行 1），而内部数据结构使用 0-indexed 坐标（"第一行"是行 0）。这是一个经典的 **off-by-one** 问题，由"规范坐标"和"实现坐标"的不匹配引起。

### DEC 模式路由绕过

### 问题
`CsiHandler::process_csi` 中 `'h'`（设置模式）和 `'l'`（重置模式）序列直接调用了 `set_dec_mode`，**绕过了** `process_dec_mode` 方法。这意味着以下 6 个 DEC 模式从未被 CSI H/L 序列正确设置：

| 模式 | 功能 |
|------|------|
| 25 | cursor_visible (光标可见) |
| 7 | auto_wrap (自动换行) |
| 6 | origin_mode (原点模式) |
| 47 | alternate_screen (备用屏幕) |
| 1004 | bracketed_paste (粘贴括号模式) |
| 1047/1048/1049 | 备用屏幕缓冲区操作 |

### 修复
1. cursor_position: 参数解析后减 1 转换为 0-indexed，再传递给 Grid
2. DEC modes: 'h'/'l' handler 通过 `process_dec_mode` 路由，而不是直接调用底层 setter
3. 添加 31 个 VT 序列正确性测试

### 教训
- VT 规范参数通常是 1-indexed，内部表示是 0-indexed，转换点要明确
- CSI handler 的代码路径应通过统一的方法处理，不要跳过中间的验证/转换逻辑
- 直接调用底层 setter 绕过了参数验证、模式依赖检查、同步更新等逻辑
- VT 解析器的每个 action handler 都应该走完整的处理链

### 相关提交
- `8ce6b152ea`: fix(csi): route DEC modes through process_dec_mode + fix cursor_position 1-indexing

## 2. Keyboard Encoding — SS3 vs CSI Modifier Encoding 12 Bugs

### 问题/Issue
12 keyboard encoding bugs in `encode_legacy_special` function. Code used SS3 (single-shift) sequences for function keys with modifiers, but VT spec requires CSI sequences when modifiers are present. Example: Shift+F1 was encoded as `ESC O 1;2 P` (SS3+mod), correct is `ESC [ 1;2 P` (CSI). List of bugs: Alt+Enter, Shift+Tab, Shift+PageUp, Alt+F1, Shift+F1, App mode+Ctrl+Up, Shift+Insert, Alt+Backspace, Shift+F5, Alt+PageUp, Ctrl+PageDown, Ctrl+Backspace.

### 根因/Root cause
Misunderstanding of VT keyboard encoding rules. Function keys without modifiers use SS3 (ESC O prefix). Function keys **with** modifiers must switch to CSI (ESC [ prefix) with parameter for modifier.

### 修复/Fix
`encode_legacy_special` now checks for modifiers; if present, uses CSI format directly instead of SS3.

### 教训/Lesson
VT keyboard encoding has three layers: (a) no-modifier function keys use SS3, (b) modified function keys switch to CSI, (c) the modifier parameter uses the same bitmask across all sequences. C1_SS3 is only for the no-modifier case.

### 相关提交
- `c1c450ee`: fix(keyboard): SS3 vs CSI modifier encoding for 12 function key bugs

## 3. erase_in_display/erase_in_line 错误移动了光标位置

### 问题/Issue
Erase operations incorrectly reset cursor to (0,0) for all modes. Per VT spec, erase operations never move the cursor — they only clear content. Also `cursor_horizontal_tab` (HT) loop iterated to cols inclusive, causing OOB for 0-indexed grid; should be cols-1.

### 根因/Root cause
Code confused "clearing then going to home" (an application-level pattern) with VT spec behavior. Erase and cursor movement are independent VT operations.

### 修复/Fix
All erase methods preserve cursor position. HT bounds fixed to cols-1.

### 教训/Lesson
VT erase operations (ED/EL) never move the cursor. Only explicit cursor positioning sequences (CUP, CUD, CUU, etc.) change cursor position. Bounds loops must use exclusive upper bounds (0..cols, not 0..=cols).

### 相关提交
- `951515ff`: fix(terminal): erase_in_display preserves cursor position
- `e5833f18`: fix(terminal): HT bounds to cols-1

## 4. SGR 属性积累 vs 替换

### 问题/Issue
`apply_sgr` replaced attributes instead of accumulating them. Each SGR sequence discarded previously set attributes. Example: `ESC[1m` (bold) then `ESC[31m` (red fg) → result was only red, not bold+red. Also `build_dumped_grid` only handled Rgb colors, ignoring Palette-indexed colors.

### 根因/Root cause
`apply_sgr` was implemented as "set attribute X" instead of "add attribute X to existing set". SGR is designed to be cumulative — bold + color + underline are independent dimensions.

### 修复/Fix
`apply_sgr` now preserves existing attributes and only modifies the ones specified. `build_dumped_grid` handles all color types (Rgb, Palette, Default).

### 教训/Lesson
SGR attributes are independent dimensions. Each SGR command only changes the attributes it specifies; others must be preserved. Handle all color variants (Rgb, Palette-indexed, Default), not just the common case.

### 相关提交
- `02ec90d9`: fix(terminal): SGR cumulative attribute application
- `9e03d135`: fix(terminal): handle all color variants in build_dumped_grid
