# 深度研究：connectbot/termlib

> 研究日期：2026-08-06 | 克隆位置：`repositories/refs/termlib`（depth 1）
> 语言：Kotlin（Compose 状态）+ C（libvterm）；Apache 2.0
> 定位：ConnectBot 的现代 Kotlin 终端库。**唯一用 Compose 状态驱动选择的参考项目**——与本项目（Compose UI）架构最接近

## 1. 项目结构

```
lib/src/main/java/org/connectbot/terminal/
├── SelectionManager.kt (448)   # 选择状态机（mutableStateOf 驱动）
├── TerminalScreenState.kt (393) # 屏幕状态（snapshot/lines/scrollback）
├── SemanticType.kt (86)        # OSC 133 语义分段（无障碍导航）
├── TerminalNative.kt (258)     # JNI 封装（libvterm）
├── TerminalEmulator.kt / Terminal.kt / TerminalCallbacks.kt
├── ImeInputView.kt / KeyboardHandler.kt / ModifierManager.kt
├── ScrollController.kt / UrlDetection.kt / OscParser.kt
└── lib/src/main/cpp/libvterm/   # vendored libvterm（C 解析器）
```

## 2. 选择系统：`SelectionManager.kt`

### 2.1 设计：Compose 状态驱动

```kotlin
internal class SelectionManager {
    var mode by mutableStateOf<SelectionMode>(SelectionMode.NONE); private set
    var selectionRange by mutableStateOf<SelectionRange?>(null); private set
    var isSelecting by mutableStateOf(false); private set
}
```

选择范围是**纯 Kotlin 状态**（`mutableStateOf`），Compose UI 自动重组。**对比 torvox**：torvox 的 SelectionState 在 ViewModel（同为 Compose 状态），架构一致。

### 2.2 SelectionRange.contains（多行范围判定算法）

`SelectionManager.kt:103-126`：
```kotlin
fun contains(row: Int, col: Int): Boolean {
    val minRow = minOf(startRow, endRow)
    val maxRow = maxOf(startRow, endRow)
    if (row !in minRow..maxRow) return false
    if (startRow == endRow) {
        val minCol = minOf(startCol, endCol)
        val maxCol = maxOf(startCol, endCol)
        return col in minCol..maxCol
    }
    return when (row) {
        minRow -> col >= if (startRow < endRow) startCol else endCol
        maxRow -> col <= if (startRow < endRow) endCol else startCol
        else -> true
    }
}
```
**正确处理反向选择**（startRow > endRow 时首行从 endCol 开始）。**对比 torvox**：torvox 的 `SelectionRange::contains`（Rust `cell_builder.rs`）语义相同，但需自查反向选择处理（Kotlin 侧 `startSelection` 是否规范化端点）。

### 2.3 模式切换：toggleMode 循环

```kotlin
fun toggleMode(...) {
    mode = when (mode) {
        CHARACTER -> WORD; WORD -> LINE; LINE -> CHARACTER; NONE -> CHARACTER
    }
    adjustSelectionForMode(cols, snapshot, scrollbackPosition)
}
```
`adjustSelectionForMode`（:288-320）：LINE 模式扩到整行（`startCol=0, endCol=cols-1`）；WORD 模式用 `findWordBoundaries(line, col)` 从 snapshot 行扩展。scrollbackPosition 参数处理滚动偏移（`getSnapshotLine` :322-327 从 scrollback 数组取行）。

**对比 torvox**：torvox 的 SelectionMode 有 5 种（Char/Word/Line/Semantic/Block），termlib 只有 3 种 + 循环切换。termlib 的"选择中切换模式自动调整范围"（adjustSelectionForMode）是 torvox 缺失的 UX——torvox 切换模式后范围不自动扩展。

### 2.4 键盘移动选择（无障碍导航）

`moveSelectionUp/Down/Left/Right`（:182-236）：
- `isSelecting` 时：只移动 end 点（类似手柄拖动）
- 结束后：移动整个范围（start+end 一起）

**对比 torvox**：torvox **没有**键盘移动选择的功能（只有触摸手柄）。termlib 的模式支撑了硬件键盘/无障碍服务的选择导航。这是功能缺口（用户未明确要求，但属于"选择系统完整性"）。

### 2.5 clampToDimensions

`:274-287`：终端 resize 时钳制选择范围（`coerceAtMost(rows-1, cols-1)`）。**对比 torvox**：torvox 的 ViewModel `updateState` 在 resize 后是否钳制 selection？需自查（resize 后选区越界会导致渲染 panic 或错误反色）。

### 2.6 findWordBoundaries

`:329-331+`：`isWordChar(char) = char.isLetterOrDigit() || char == '_'` —— 与 torvox 的 SelectionExpander 语义不同（torvox 支持 URL/标点处理，更丰富；termlib 只按 word char 扩展，更保守）。

## 3. SemanticType：OSC 133 语义分段（创新功能）

`SemanticType.kt`：
```kotlin
enum class SemanticType {
    DEFAULT, PROMPT, COMMAND_INPUT, COMMAND_OUTPUT, COMMAND_FINISHED, ANNOTATION, HYPERLINK
}
data class SemanticSegment(startCol, endCol, semanticType, metadata, promptId)
```
- 解析 OSC 133 A/B/C/D 序列，把终端行分段为 prompt/命令/输出
- 支撑"跳到下一个 prompt"无障碍导航 + screen reader 上下文
- OSC 8 HYPERLINK 与 URL 检测（`UrlDetection.kt`）

**对比 torvox**：torvox 无此功能（libghostty-vt 内部有 shell integration 支持，但未暴露到 Kotlin）。**这是差异化功能候选**（用户未要求，但符合"终端现代化"方向）。

## 4. 测试基础设施（值得借鉴）

termlib 有 **19 个测试文件**，覆盖：
- `SelectionManagerTest.kt`、`SelectionControllerTest.kt`、`HandleDragTest.kt`（选择状态机、拖拽）
- `TerminalScreenStateScrollTest.kt`、`TerminalScreenStateUrlTest.kt`
- `SemanticTypeTest.kt`、`TerminalUrlExtractionTest.kt`
- `TerminalRendererGoldenTest.kt`（golden 渲染对比！）
- `ComposeModeTest.kt`、`AccessibilityOverlayTest.kt`、`GranularNavigationTest.kt`
- `ImeInputViewTest.kt`、`KeyboardHandlerTest.kt`、`OscParserTest.kt`

**对比 torvox**：torvox 的 Kotlin 测试覆盖（InputBatchBuffer/InputCoalescer/SelectionExpander 等）已有，但缺 **golden 渲染测试**（termlib 有 `TerminalRendererGoldenTest`）和 **拖拽专项测试**（HandleDragTest）。

## 5. 依赖清单

| 依赖 | 用途 | 本项目适用性 |
|------|------|--------------|
| libvterm（C，vendored） | VT 解析 | 不适用（libghostty-vt 更完整，支持 kitty graphics） |
| Compose runtime | 状态驱动 | **已用** |
| 无其他重大依赖 | — | — |

## 6. 项目文档吸收价值

- README 描述架构分层（Terminal/Emulator/ScreenState/Native）
- 测试命名规范（功能+场景）

## 7. 代码注释引用（待加入 torvox 代码）

```
SelectionManager.kt (torvox) 或 ViewModel:
// 选择状态机参考 termlib SelectionManager.kt（Compose mutableStateOf 驱动）
// contains 多行算法 :103-126（反向选择处理）
// adjustSelectionForMode :288-320（模式切换自动扩展范围）
// clampToDimensions :274-287（resize 钳制——torvox 需自查）
// moveSelection* :182-236（键盘/无障碍选择导航——torvox 缺失）
```

## 8. 结论

termlib 是**架构最接近的 Compose 参考**。三个明确缺口/借鉴点：
1. **resize 时钳制选择范围**（clampToDimensions）——torvox 需自查
2. **模式切换自动调整范围**（adjustSelectionForMode）——torvox 切换 SelectionMode 后范围不扩展
3. **键盘移动选择**（无障碍导航）——功能缺口
4. OSC 133 语义分段（SemanticType）——差异化功能候选

## deep-v4 增量（复核第 1 轮：测试套件 + applyHandleDrag + UrlDetection）

### applyHandleDrag（Terminal.kt:1899-1935，纯函数）

锚点语义 + **crossing 翻转**：拖拽手柄越过锚点时，交换手柄所有权，静止手柄恢复到越界前位置（HandleDragResult(newRow,newCol,anchorRow,anchorCol,newIsMovingStart)）。

**torvox 对照**：TerminalSurface.kt handleDragState/dragAnchorRow/Col（:1086-1122）用"锚点+位移 coerceIn"模型——**无 crossing 翻转**（拖过头会被夹住）。行为差异 P1：用户拖过另一头时 termlib 交换手柄（更符合直觉），torvox 停留在边界。

**测试对照**：HandleDragTest.kt（no-cross/same-position/crossing 同行/跨行/回退场景全覆盖）——torvox 无对应纯函数测试。

### UrlDetection.kt（:12-35）

- `TRAILING_DETECTED_URL_PUNCTUATION = {., ,, ;, :, !}`：URL 尾随标点修剪
- `countOpenLessThanClose`：括号配对计数——`)` 只在闭合计数 > 打开计数时修剪（`foo(bar)` 的 `)` 保留，`foo(bar)).` 修剪多余的 `)`）

**torvox 对照**：SelectionExpander.findUrlStart/expandWord（:22-80）URL 展开到空白为止——**未修剪尾随标点/括号**。功能缺口 P0：`https://x.com/a).` 会选中 `).`。round-214 只修了"前导引号/词收缩"，尾随侧未处理。

### 测试套件清单（27 个测试文件，未覆盖项）

| 测试 | 对应 torvox |
|------|------------|
| HandleDragTest.kt | 拖拽纯函数测试（缺） |
| SelectionControllerTest.kt | 选择控制器测试（缺） |
| TerminalRendererGoldenTest.kt | **渲染黄金测试**（torvox 有截图测试但无 golden 模式） |
| TerminalUrlExtractionTest.kt / TerminalScreenStateUrlTest.kt | URL 检测测试（缺） |
| MagnifierOffsetTest.kt | 放大镜偏移（torvox 无放大镜） |
| GranularNavigationTest / ReviewModeTest / AccessibilityOverlayTest | 无障碍/审查模式（torvox 无） |
| ReadLastOutputTest.kt | shell-integration 读取（torvox 无） |
| ShellIntegrationTest.kt / OscSequenceTest / OscParserTest / ComposeModeTest | OSC/合成模式（torvox 有 vt 层但 Kotlin 侧无） |
| KeyboardHandlerTest.kt / CursorAndModeEscapeTest.kt / RightAltMode | 键盘/光标转义（部分有） |
| TerminalScreenStateScrollTest / ScrollbackClearTest / LiveOutputRegion | 滚动状态（torvox 滚动有但无状态单测） |

**结论**：termlib 的"纯函数 + 单测全覆盖"模式是 torvox 选择/URL/拖拽逻辑的测试模板（P1）。

## deep-v5 增量（复核第 2 轮：Terminal.kt 2506 行函数清单精读）

### drawComposeOverlay（:2293-2370）——IME 合成文本叠加层

- **空 buffer 提前 return**（:2314-2316 注释）：避免 Enter 后合成区残留方块
- 列宽截断（CJK 全宽 2 列，:2318-2323）
- 背景矩形 + 白字（重置 fakeBold/skew/underline/strike）+ **光标条在文本末尾**（:2362-2368）
- TextPaint 状态保存/恢复

**torvox 对照**：InputConnection 有 setComposingText/commitText（TerminalSurface.kt:438/:491，转发 PTY），但**无 compose 叠加层绘制**——合成期间依赖 shell 回显（ghostty/termux 同）。**功能差异 P2**：termlib 方案提供组合期间视觉反馈（背景+光标条），torvox 无。记录不吸收（设计选择）。

### findOptimalFontSize（:2469-2506）——二分搜索最大适配字号

目标 rows×cols + 可用宽高 → 二分找最大字号（`FONT_SIZE_SEARCH_EPSILON` 收敛）。**torvox 对照**：recomputeGridFromFontMetrics 是"字号→网格"正向；termlib 是"目标网格→字号"反向。互补算法，P2 记录（torvox 字号设置 UI 若做"按目标行列自动字号"可参考）。

### drawCurlyUnderline（:1808-1850）——波浪下划线（拼写检查式）

zigzag path，每字符同相位（无缝重复），`charWidth / CURLY_UNDERLINE_CYCLES_PER_CHAR` 波长。**torvox 对照**：无 curly 样式标志（cell_builder 仅有 underline 位）；libghostty-vt 是否暴露 double/curly 变体未知（vendored 源不在本地）——**P3 待查**。

### 其余确认

- drawDoubleUnderline（:1794）：双线下划线——torvox pack_style_flags 有 double_underline 位（已覆盖）
- magnifierOffset/MagnifyingGlass（:2021-2148）：放大镜（torvox 无，P3）
- drawCursor（:2190）：光标绘制（torvox shader 光标已覆盖）
- codepointColumns/visualColumnWidth（:2372-2398）：宽字符列宽（torvox 有等价值）
