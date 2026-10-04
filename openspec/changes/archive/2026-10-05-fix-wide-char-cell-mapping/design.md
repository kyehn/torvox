# 设计：把单元格↔字符映射交回网格

## 1. 为什么原生查询是唯一正确解

「列 c 属于哪个字符的起始列」有且只有一个权威答案：网格单元的 `CellWide`
（`Wide` / `SpacerTail` / `SpacerHead` / `Narrow`）。行文本是**派生**表示，
宽字符尾格在其中的 `' '` 与真空白不可区分，因此：

| 候选 | 判定吸附 | 判定空白 | 判定组合记号/ZWJ 零宽 |
| --- | --- | --- | --- |
| 保持手写 wcwidth（现状） | 仅宽字符对，其余一律错 | 宽字符后整体错位 | 否（恒按一格计） |
| 行文本按列直取 | 否 | 是 | 否（但不影响判定） |
| 网格 `CellWide`（本变更） | 是 | 需配合列直取 | 是（Ghostty 自身判定） |

只有第三项同时正确。因此 `isWhitespaceCell` 用「先吸附到字符起始列，再按列直取
字符」两步走：长按宽字符右半格先归到该字符的起始列，再读该列字符——非空白，
走选词；长按真空白则吸附为空操作，仍判空白。**菜单语义与列映射就此同源。**

## 2. 查询契约

`wide_char_tail_cols(row) -> Vec<u32>`：该行中作为宽字符后半格（`SpacerTail`）的列号，升序。

- 单元为 `SpacerTail` → 收录该列。
- 其余（`Wide` 起始格、`Narrow`、`SpacerHead`、读取失败）→ 不收录。

调用方据此吸附：`tail.contains(col) ? col - 1 : col`。

`SpacerHead` 不收录：它出现在**续行**的行首，该行前一列属于软换行前的上一行，
把它当尾格会把选区跨过折行边界。`cell_columns`（既有，`internal.rs`）只把
`SpacerHead`/`SpacerTail` 一并判为无列宽单元；本查询需要区分二者，故不复用它。

失败路径（通道满/超时）沿用 `GhosttyTerminal::query` 既有的 `fallback` 参数，
此处传空 vec——等价于「无吸附」，保持原列，不猜。

### 2.1 为什么按行而非按列

初版是逐列的 `cell_char_start_col(row, col)`。自查发现它把一次发往 VT 线程的
**同步**往返（`QUERY_TIMEOUT_MS = 500ms`，VT 循环空闲时每 50ms 才排一次查询，
故典型延迟 0–50ms）压到了**手柄拖动的每个 MotionEvent MOVE** 上——而旧实现
因拖动会话内命中行缓存，整个拖动只发一次 `scrollbackLine`。改按行后，一次查询
覆盖整行，拖动内每个 MOVE 只是对 ≤ 列数个 int 的线性查找；缓存的生命周期
（拖动会话标志 + 行号）不变，只是缓存对象由整行文本换成列号数组。

## 3. 删除项

`util/TextWidth.kt` 的三个函数（`isWideBmp` / `isWideAstral` / `isWideCodePoint`）
唯一调用点是 `charIndexAtCellColumn`。该函数删除后表整体失去调用点，一并删除
（含 `TextWidthTest.kt`）。这同时消除了一份**不完整**的手写宽度表——它把组合记号、
ZWJ（U+200D）、变体选择符（U+FE00..FE0F）一律按一格计，而 Ghostty 按零宽处理，
只要网格是唯一来源就不会再出现这类分歧。

## 4. 缓存状态的取舍

原状用 5 处状态（`cachedWideCharLineRow` / `cachedWideCharLine` /
`cachedWideCharTimeMs` / `dragWideCharCacheSession` / `WIDE_CHAR_CACHE_TTL_MS`）
缓存整行文本，让拖动期间零 JNI 调用。改按行查询后缓存的生命周期不变（拖动会话
标志 + 行号），但缓存对象由整行 `String` 换成 `IntArray`，并删掉 TTL——两个调用点
（`dragTargetFromTouch` / `updateDragHandleForCell`）都在拖动会话内，TTL 在旧实现
里因 `dragWideCharCacheSession` 恒真而从未生效。
