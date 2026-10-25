# Design

## 一、选区端点：行主序线性，不是包围盒

### 事实

- Kotlin 侧：`SelectionState.applyHandleDrag` 返回 `(startRow, startCol, endRow, endCol)`，
  `clampSelection` 按 `(行, 列)` 行主序排序保证 `start ≤ end`；
  `normalizeSelectionAnchors` 同样是**行主序**归一（列只在同一行内参与比较）。
- 原生侧：`install_selection_impl` / `selection_text_impl` 用
  `libghostty_vt::selection::Selection::new(start_ref, end_ref, SELECTION_RECTANGLE)`，
  且 `SELECTION_RECTANGLE = false`（`internal.rs:68`）。上游文档明确：
  `rectangle = false` 时两端点描述**线性选区**，端点保留方向、可反转。
- 线性语义下，`(r1, c1) → (r2, c2)`（`r1 < r2`）覆盖：首行 `c1..行尾`、
  中间整行、末行 `行首..c2`。

### 缺陷

下发前的归一取了**包围盒**：

```
loRow = min(start.row, end.row); hiRow = max(...)
loCol = min(start.col, end.col); hiCol = max(...)
```

起始列大于结束列时（`start = (10, 5)`、`end = (12, 2)`），包围盒角点是
`(10, 2) → (12, 5)`；原生按线性语义解释它，覆盖首行 `2..行尾`、末行 `行首..5`，
而正确区域是首行 `5..行尾`、末行 `行首..2`。于是：

- 首行左侧多出 `2..4` 三格；
- 末行右侧多出 `3..5` 三格；
- 控制柄仍按 Kotlin 的有序端点画在 `(10, 5)` 与 `(12, 2)`，与高亮边界错位；
- `extractSelectedText` 走的是 `normalizeSelectionAnchors`（行主序），复制出的文本
  与屏幕上的高亮区域**不是同一块**。

这正是 `docs/STATUS.md` 第二项的描述。

### 决策

1. **单一有序端点对**：选区在任何跨边界传输中都只以 `(startRow, startCol, endRow, endCol)`
   的行主序有序形式存在。`publishNormalizedSelectionBounds` 与
   `syncDragBoundsToNativeThrottled` 改用 `normalizeSelectionAnchors`（已在仓内，
   `TerminalSurface.kt`，行主序语义），不再各自取 min/max。
2. **包围盒只用于「菜单锚定」**：菜单必须不与选区相交，用包围盒是对的，保留在
   `showSelectionMenu` 一侧，不回流到选区语义。
3. **控制柄位置不变**：`TerminalScreen` 的手柄折算在有序端点下本就落在
   `(startRow, startCol)` 左缘与 `(endRow, endCol)` 右缘，与线性高亮一致；
   跨行且起始列大于结束列时不再需要 `selStart.row > selEnd.row` 的分支。
4. **不引入矩形选区**：上游 `Selection::new` 支持 `rectangle = true`，但 termux
   `getSelectedText` 与 ghostty 线性选区同源，矩形语义会与「手柄在首格左缘、
   末格右缘」的既有契约冲突，也会改变复制文本的折行拼接结果。

### 判据

- 纯函数层：`publishNormalizedSelectionBounds` 的有序化由 `normalizeSelectionAnchors`
  承担，用既有 `NormalizeSelectionAnchorsTest` 覆盖；新增用例断言跨行且列倒置时
  下发给原生的是 `(r1, c1) → (r2, c2)`，不是 `(r1, min) → (r2, max)`。
- 端到端：仪器化用例断言「拖到起始列更靠右的位置后，高亮首行的左缘等于拖动锚点列」。

## 二、搜索居中：以可见带为基准

### 事实

输入法弹出时，主屏**不改网格**（改网格会重排闪烁），而是把终端 Surface 整体
`translationY = -shift` 上移 `shift` 像素跟随键盘（`applyImeShift`）。
`shift = computeImeSurfaceShift(...)`：

```
visibleContentPx = surfaceHeight - modifierBarHeight - imeBottom
shift = (max(contentBottom, cursorBottom) - visibleContentPx).coerceIn(0, imeBottom)
```

命中行落在视口行 `v` 时，它的**屏幕纵坐标**是 `v * cellHeight - shift`；
屏幕上的可见带是 `[0, visibleContentPx]`。

### 缺陷

`centerSelectedRow` 用 `visibleRows / 2`（`visibleRows = 总行数 − 输入法行数`）作为
命中行在视口中的位置，遗漏了 `shift` 这一项。`shift ≈ imeBottom` 时：

```
screenY = (总行数 − 输入法行数) / 2 * cellHeight − imeBottom
```

输入法遮挡超过总行数的三分之一时该值为负——命中行被推到**屏幕上缘之外**，仍然看不到。

### 决策

1. **可见带是唯一基准**：命中行的目标屏幕纵坐标取可见带正中，
   `targetScreenY = visibleContentPx / 2`；反解出视口行
   `v = (targetScreenY + shift) / cellHeight`。
2. **折算成纯函数**：`centerSearchViewportRow(visibleContentPx, shiftPx, cellHeightPx)`
   承担全部折算，测试直接驱动它。
3. **删除死代码**：`visibleSearchRows` 与 `centerSearchRow` 里 `centerSearchRow`
   只被测试引用（生产代码走 `getVisibleRows`），本轮一并删除，避免「测了个没人用的函数」。
4. **不引入反馈环**：`shift` 取当前已施加值（`appliedImeShiftPx`）。`scrollToRow`
   之后 `applyImeShift` 会按新的偏移重算，但显出量（`imeShiftRevealPx`）只在用户
   手动滚到回滚区顶端之外时才非零，搜索定位不产生该量，故不存在反馈。

### 判据

- 纯函数用例：无输入法时命中行落在视口中点；有输入法且内容未满时命中行落在
  可见带正中；输入法遮挡超过总行数三分之一时命中行仍在可见带内（不落到上缘之外）。

## 三、kitty 存储代数：实测事实与注释对齐

### 实测

```
PROBE storage_gen before=2 after=2 placement_gen before=1 after=2
```

（虚拟放置 → ED2；`storage_gen` = `Graphics::generation()`，
`placement_gen` = `RenderState::image_placements_generation()`。）

即：**撤下放置不推进图像存储生成戳，推进的是放置生成戳**。
`types.rs` 的字段注释与此一致，`tests.rs` 的用例注释与之相反。

### 决策

1. **订正注释**：`tests.rs` 里「上游对虚拟放置的撤除同样会推进存储生成戳」改为
   实测事实。
2. **加断言钉住**：在既有两个 ED2 用例里断言 `kitty_generation` 不变、
   `kitty_placement_generation` 变化。上游若改变该语义，用例会转红并指出需要改哪里。
3. **渲染侧不动**：`kitty_cache_invalidation` 已按
   `placement_generation_changed && viewport_shift_rows == 0` 走「重查放置 + 整帧重绘」，
   与实测事实一致。

## 四、滚动项：留档，不改行为

`main` 上相关提交与判据：

| 提交 | 内容 | 判据 |
| --- | --- | --- |
| `42e2435f` | 健康 vsync 下活跃闭锁只作安全网，消除一周期双帧 | JVM `RenderLatchCadenceTest` |
| `d72e7476` | 仅在搜索可见时镜像滚动偏移，滚动事件不再触发整屏重组 | Compose 重组计数 |
| `2f7bbf6d` | 滚动余量稳定时停止整屏重绘 | Rust 判据表 |
| `32fddfe0` | 上述判据表更新为稳定余量语义 | 同上 |

规范落点：`render-loop-scroll-cadence` 的「滚动更新由 vsync 合并」与
「稳定滚动余量不强制全量重绘」。本轮只补留档，不再改行为——真机复测需要设备，
本环境不具备（见 `status-disposition.md` 的剩余项）。
