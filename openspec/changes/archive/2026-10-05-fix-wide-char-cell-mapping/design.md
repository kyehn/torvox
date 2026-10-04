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

`cell_char_start_col(row, col) -> u32`：

- `col == 0` 或网格坐标不可解析 → 原样返回 `col`（列 0 无前驱，越界无从吸附）。
- 单元为 `SpacerTail` → 返回 `col - 1`。
- 其余（`Wide` 起始格、`Narrow`、`SpacerHead`、读取失败）→ 原样返回 `col`。

`SpacerHead` 不左移：它出现在**续行**的行首，该行前一列属于软换行前的上一行，
把它左移会把选区跨过折行边界。`cell_columns`（既有，`internal.rs`）只把
`SpacerHead`/`SpacerTail` 一并判为无列宽单元；本查询需要区分二者，故不复用它。

失败路径（通道满/超时）沿用 `GhosttyTerminal::query` 既有的 `fallback` 参数，
此处传 `col` 本身——查询失败时保持原列，不猜。

## 3. 删除项

`util/TextWidth.kt` 的三个函数（`isWideBmp` / `isWideAstral` / `isWideCodePoint`）
唯一调用点是 `charIndexAtCellColumn`。该函数删除后表整体失去调用点，一并删除
（含 `TextWidthTest.kt`）。这同时消除了一份**不完整**的手写宽度表——它把组合记号、
ZWJ（U+200D）、变体选择符（U+FE00..FE0F）一律按一格计，而 Ghostty 按零宽处理，
只要网格是唯一来源就不会再出现这类分歧。

## 4. 被删除的缓存状态

现状为让吸附在拖动期间不发 JNI 请求，缓存整行文本 + 500ms TTL + 拖动会话标志
（`cachedWideCharLineRow` / `cachedWideCharLine` / `cachedWideCharLineAtMs` /
`dragWideCharCacheSession` / `WIDE_CHAR_CACHE_TTL_MS`，共 5 处状态）。新查询只传
两个整数、不回传行文本，缓存失去存在理由，连同拖动会话的置位/清除一并删除。
