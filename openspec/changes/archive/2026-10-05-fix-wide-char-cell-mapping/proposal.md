# 修复宽字符的单元格↔字符映射错位

## Why

`TerminalSurface` 用**行文本**反推「单元格列 → 字符下标」：手写 wcwidth 表
（`util/TextWidth.kt`）逐码点累加宽度，`charIndexAtCellColumn` 走一遍列号映射。
但原生 `read_line_text_impl`（`internal.rs`）的契约恰恰相反——它**每个网格列恰好
产出一个字符**（其注释即为此不变式，`SearchMatch.start_col` 依赖它），宽字符的尾格
落一个 `' '`。实测（`cargo test` 探针，24x80，`vt_write("中文AB")`）：

```text
cols=80 chars=6 text="中 文 AB"
```

字符下标恒等于列号：`'中'`→列 0、尾格 `' '`→列 1、`'文'`→列 2、尾格 `' '`→列 3、
`'A'`→列 4、`'B'`→列 5。

Kotlin 的累加模型在第一个宽字符之后就整体左移一格，用户可见后果：

- `isWhitespaceCell("中 文 AB", col = 2)` 读到列 1 的尾格空格 → 判为空白 → **长按
  「文」弹出仅粘贴菜单**，而非选词（`DESIGN.md:171`；spec text-selection
  「长按非空文本 → 选词」）。行内每出现一个宽字符，错位就再累加一格。
- `snapColToWideChar("中 文 AB", col = 3)` 比较出下标 2≠1 → 不吸附，**宽字符「文」
  被选区从中间切开**（`DESIGN.md:173`「支持宽字符吸附」；spec
  「手柄停在双宽字符的后半格 → 吸附至该字符起始列」）。

两者都被既有单测当成正确行为钉住（`TerminalSurfaceLogicTest` 用 `"中文AB"`、
`isWhitespaceCell walks cells not chars for wide glyphs` 用 `"中a"`，都假定行文本
是「宽字符只占一个字符下标」的紧凑文本），因此测试与实现互相背书，缺陷从未暴露。

根因不是某一行的算错，而是**违反 DESIGN「以 Ghostty 作为终端状态的单一来源」**：
网格单元占几列只有网格知道，而宽字符尾格在行文本里与真空白同为 `' '`，行文本
**根本不足以**判定吸附——Kotlin 侧那份手写宽度表是在重复实现 Ghostty 已有的事实，
且实现得不全（组合记号、ZWJ、变体选择符均为零宽，一律按一格计）。

## What Changes

- 原生新增只读查询 `wide_char_tail_cols(row)`：经 `grid_ref().cell().wide()` 标出该行
  全部 `SpacerTail` 列。复用既有 `absolute_point` 空间解析，无新逻辑、不新增状态。
  按行而非按列：吸附在手柄拖动的每个 MOVE 都要做，按列查询会把一次发往 VT 线程的
  同步往返压到每个触摸帧上（典型延迟 0–50ms，见 design 2.1）。
- `TerminalSurface` 的宽字符吸附改走该查询；长按分类先把落点吸附到字符起始列再判空白。
  跨 JNI 的整行文本拷贝换成列号数组，TTL 一并删除（两个调用点都在拖动会话内，
  旧 TTL 因会话标志恒真而从未生效）。
- `isWhitespaceCell` 改为按列直接取字符（`line[col]`），并删除 `charIndexAtCellColumn`、
  `snapColToWideChar` 与整个 `util/TextWidth.kt`（含其单测）——手写 wcwidth 失去最后
  一个调用点。
- 修正把错误模型钉死的单测，改为按「字符下标 == 列号」的原生契约构造用例。

## Capabilities

### Modified Capabilities

- `text-selection`：长按空白判定与宽字符吸附的列映射来源。

## Impact

- 原生：`ghostty_terminal/{commands,internal,public_api}.rs`、`android/ffi.rs`。
- Kotlin：`bridge/{TerminalQueryPort,NativeQueryPort,Bridge,NativeBridge}.kt`、
  `ui/TerminalSurface.kt`；删除 `util/TextWidth.kt` 与 `util/TextWidthTest.kt`。
- 不改保护文件。行为只向 spec 已声明的语义收敛，不新增功能。
