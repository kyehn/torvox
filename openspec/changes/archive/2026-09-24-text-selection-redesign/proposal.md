# Proposal

## Why

`DESIGN.md` 文本选择条款与当前实现存在偏差：拖动时菜单不隐藏、全选只取视口矩形且含空白区、菜单可遮挡选区、OSC 8 非 URL 超链接点击无效、打开文件附加未声明的位置限制、菜单样式硬编码不用主题；同时存在 7 类死代码与方向键移锚、SmartCopy 等未声明行为（STYLE.md 禁止），词/行/全选为 Kotlin 自实现而未合理使用上游 `GHOSTTY_TERMINAL_OPT_SELECTION` 与 `select_word/select_line/select_all` 语义 API。

## What Changes

- 上游语义接入：长按选词、双击/三击/四击的词/行/全选改经 `select_word`/`select_line`/`select_all` 派生并安装为终端属主选区（返回界限驱动控制柄与菜单），删除 Kotlin 自实现的词/行/视口全选几何。
- 全选改为覆盖全部可选内容且仅含内容区域（上游 `select_all` 语义），全选后复制正常。
- 菜单重锚定：锚定改纯函数，保证贴边翻转、选区变更（全选）后立即重显、两侧无空间时隐藏，任何时刻不与选区矩形相交；抓柄拖动即隐藏、抬手按新几何重显。
- 菜单样式改 Material 3 主题属性（`colorSurface`/`colorOnSurface`），不再硬编码颜色。
- 打开链接补 OSC 8 回退：选中文本非 URL 形态时取超链接 URI，保证一次点击即生效；打开文件删除未声明的位置限制，保留点击时存在检查与写权限授予。
- 对齐参考实现：边缘滚动改每次触点移动恰好 1 行（删除 30ms 定时循环，termux/ghostty-android 同款）；单击消除选择后重置多击计数。
- 删除死代码：ModifierBar 第二套选择菜单、测试专用菜单定位算法、`expandAndSetSelection`/`SelectionExpander` 链、`SelectionMode`/mode/rectangle 通道、`selectionBackground` 死通道、重复重显函数。
- 删除未声明行为：方向键移动选择锚点、SmartCopy 复制后处理。
- 调整测试：删除仅覆盖死代码的测试，新增锚定不遮挡、内容区域全选、OSC 8 回退、拖动隐藏菜单断言。

## Capabilities

### New Capabilities

- `text-selection`: 文本选择的启动（长按/多击）、控制柄拖动与边缘滚动、选择菜单内容与锚定、全选与复制、打开链接/文件语义

### Modified Capabilities

（无：`openspec list --specs` 现有能力与文本选择无交集）

## Impact

- Kotlin：`ui/TerminalSurface.kt`、`ui/TerminalScreen.kt`、`ui/ModifierBar.kt`、`TerminalViewModel.kt`、`runtime/TerminalRuntime.kt`、`bridge/*`，删除 `ui/SmartCopy.kt`、`bridge/SelectionExpander.kt`
- native：`android/ffi.rs`（新增三个选择语义查询、去 mode/rectangle 参数）、`terminal/ghostty_terminal/{public_api,commands,internal}.rs`
- 测试：单测删 `SelectionExpanderTest`/`SmartCopyTest`/`TerminalScreenMenuTest`、裁剪 `SelectionStateTest`、增锚定与 OSC 8 回退用例；仪器调整 `MultiTapSelectionInstrumentedTest`/`SelectionDragQuantifiedTest`/`SelectionEspressoTest`、cucumber `SelectionSteps`
