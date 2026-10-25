# Proposal

## Why

`docs/STATUS.md`（受保护文件）仍列着四项「未被解决的问题」：上下滑动滚动缓慢卡顿闪烁、
长按选择拖过对侧手柄后另一边界异常增加且手柄与边界不再对齐、输入法弹出时按搜索定位
可能看不到匹配文本、kitten icat 图像 `clear` 后要清两次才消失。

canonical 分支 `main` 上已有这四项的修复提交，但复核发现**两处实现与事实/规范仍不一致**，
回归无判据：

1. **选区端点语义错配（未修完）**：Kotlin 侧的选区模型是**行主序线性**选区
   （`clampSelection` 按 `(行, 列)` 行主序排序），原生侧 `install_selection_impl` /
   `selection_text_impl` 也用 ghostty 的线性选区（`SELECTION_RECTANGLE = false`）。
   但下发前 Kotlin 把端点换算成了**包围盒角点**（`publishNormalizedSelectionBounds`、
   `syncDragBoundsToNativeThrottled` 都取行列的 min/max）。跨行且起始列大于结束列时，
   原生按线性语义解释角点，得到的区域在**首行左侧与末行右侧同时外扩**——
   现象正是「另一边界异常增加，且两个指针和边界不再对齐」，且复制出的文本与高亮区域不一致。
2. **搜索居中口径只扣了输入法行数（未修完）**：`visibleSearchRows` 只按
   `总行数 − 输入法行数` 折算，遗漏了「内容为跟随键盘整体上移」这一项。
   输入法遮挡超过总行数的三分之一时，命中行被推到**屏幕上缘之外**，仍然看不到。
3. **kitty 存储代数的不变式与注释相反**：实测（`ED2` 撤下虚拟放置）表明
   **图像存储生成戳不变、放置生成戳变化**。`types.rs` 的字段注释写对了，
   `tests.rs` 的用例注释却写成「上游对虚拟放置的撤除同样会推进存储生成戳」——与实测相反，
   且没有断言把这条事实钉住。
4. **滚动项缺留档**：修复与规范（`render-loop-scroll-cadence` 的「稳定滚动余量不强制
   全量重绘」）都已在 `main` 上，但没有把「哪条提交、哪条判据、还差什么」写进留档，
   下一次改动无从判断是否复发。

## What Changes

- `text-selection`：选区端点 MUST 按**行主序线性**语义下发到原生（MUST NOT 用包围盒角点）；
  高亮区域、控制柄位置与复制文本 MUST 由同一对有序端点导出。
- `text-search`：搜索居中 MUST 以**可见带**为基准——可见带由输入法遮挡、键栏占高与
  内容跟随键盘的整体上移三者共同决定；命中行 MUST 落在可见带内，MUST NOT 被推到上缘之外。
- `kitty-graphics`：撤下放置时图像存储生成戳**不变**这一事实 MUST 被断言钉住；
  渲染缓存 MUST 以放置生成戳为撤除信号。
- 留档 `status-disposition.md`：四项逐条写明根因、修复、自动化判据与仍需真机确认的部分。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `text-selection`：选区端点的行主序线性语义（高亮、手柄与取文同源）。
- `text-search`：搜索居中按可见带（含内容上移）折算。
- `kitty-graphics`：撤下放置时图像存储生成戳不变的不变式与判据。

## Impact

- `android/app/src/main/java/terminal/emulator/TerminalViewModel.kt`（选区端点下发：
  包围盒角点 → 行主序有序端点）
- `android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt`（搜索居中折算：
  可见行数 → 可见带；删除只被测试引用的死函数）
- `android/app/src/main/java/terminal/emulator/ui/TerminalScreen.kt`（居中调用点）
- `native/src/terminal/ghostty_terminal/tests.rs`（kitty 存储代数不变式断言与注释订正）
- `android/app/src/test/java/terminal/emulator/SelectionStateTest.kt`、
  `android/app/src/test/java/terminal/emulator/ui/TerminalSurfaceLogicTest.kt`
- 不改 `docs/specification/` 与 `docs/STATUS.md`。
