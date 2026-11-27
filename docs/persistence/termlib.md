# termlib 研究留档

## 定位

ConnectBot 的 Jetpack Compose 终端显示组件，通过 JNI 包装 libvterm。它是纯显示层：PTY、SSH 与网络收发由宿主负责，`writeInput` 喂字节、`onKeyboardInput` 回字节。本批仓库中与本地终端层最直接可比。

## 关键实现与文件锚点

- `lib/src/main/java/org/connectbot/terminal/TerminalNative.kt`：`LIBRARY_NAME = "jni_cb_term"`、`lifetimeLock`、`inNativeCall`。
- `lib/src/main/cpp/Terminal.{cpp,h}`：`writeInput`、`beginCursorBatch`、`finishCursorBatch`。
- `TerminalEmulator.kt`：`damageLock`、`pendingDamageRegions`、`nextSnapshotNanos`、`requestProcessPendingUpdatesLocked`。
- `TerminalDispatcher.kt`：`queue: ConcurrentLinkedQueue<Runnable>`。
- `TerminalSnapshot.kt` / `ScrollbackHistory.kt` / `PackedCells.kt`：`sameShaping`、`singleAscii`。
- `CellData.kt`：`CODE_POINTS = 16`、`STRIDE = CODE_POINTS + 4`、`BYTES = STRIDE * 4`。
- `TerminalShaping.kt`：`byteLimit = 256 * 1024`、`MAX_PARAGRAPH_ROWS = 500`、`MAX_BYTES = 512 * 1024`。
- `TerminalTextPaint.kt`：`glyphAdvances = FloatArray(512)`、`preparedViewport`、`measureCell`。
- `TerminalAsciiGlyphs.kt`：`ids = IntArray(128)`、`glyphs = arrayOfNulls<Glyph>(127)`。
- `TerminalBoxDrawing.kt`：`connections[cp - 0x2500]`、`0x2580` 起块元素。
- `TerminalOutputJournal.kt`：`MAX_CHARACTERS = 8192`、`finished.size > 2048` 修剪。
- `LiveOutputRegion.kt`：`AccessibleOutputFeedback`。
- `KeyboardHandler.kt` / `ImeInputView.kt`：键位分发与 IME 入口。
- `tools/generate-unicode.py`：`--download` 刷新缓存、`--check` 校验生成物。
- `benchmark/run-frames.py`、`macrobenchmark/`、`docs/rendering.md`：帧采样、宏基准与渲染口径。

## 并发与输入顺序

`TerminalNative` 用非重入 `lifetimeLock` 加 `inNativeCall` 守卫，禁止回调同步重入本机方法（会死锁）；JNI 名为 `jni_cb_term`，跨 classloader 冲突时复制 so 再加载。所有 native 调用被串行化。

`TerminalEmulatorImpl` 把 damage、光标、属性变化收敛到 `damageLock`：`pendingDamageRegions` 合并重叠或相邻行区域，超过 100 个区域时直接退化为整屏脏，避免 O(n²)。`requestProcessPendingUpdatesLocked` 做速率限制，`nextSnapshotNanos` 留 12 ms 余量，防止一次 PTY 读触发多次 Compose 重绘。

C++ `Terminal::writeInput` 在 `vterm_input_write` 外包裹 `beginCursorBatch`/`finishCursorBatch`：`termMovecursor` 在批次内只记录最终位置与首次 oldpos，批次结束才调用 Java 回调。文档描述的“一次 write 内光标通知合并”就落在这里，Kotlin 侧 `moveCursor` 自身仍是逐次置 `cursorMoved`。

`TerminalDispatcher` 用 `ConcurrentLinkedQueue`，每批最多跑 64 个任务再让出线程池；`call` 在已在执行线程时直接跑，避免自锁。Compose 侧经 `QueuedTerminal` 走异步入口，公开的模拟器 API 保持同步语义。键事件、IME commit、`pasteText` 共用同一个 FIFO；paste 是不可分命令组，UTF-8 可分块但后续键入不能插入，且遵守 bracketed paste。宿主要经 `onPasteRequest` 取剪贴板再调 `pasteText`，不能绕过这条链路直接写传输。

## 渲染数据

列数来自 native parser，与 glyph advance 解耦。`TerminalSnapshot` 是不可变快照，`ScrollbackHistory` 按页共享未变行，`PackedCells` 复用未变 cell 数据。Unicode 17.0.0 的 emoji 表由 `tools/generate-unicode.py` 生成并签入，`--check` 可复现；native/JNI cell 容纳 16 个码点，更长的组合继续放进下一个 cell。

渲染先画全部背景（含选区）再画全部前景；完整 cell 文本在列原点一起 shaping，advance 过大才横向压缩，斜体 overhang 保留。每行 display list 带视口边界，改动不保留整屏位图；图形层让光标反色两遍复用录制结果。Latin-1 单字形 advance 每字体每字号 2 KiB 缓存，粗体与常规分开；每个 shaping engine 上限 256 KiB，prepared viewport 上限 1 MiB，paint 缓存 192 个字形 display list 与 512 KiB RenderNode 估算内存。

API 31+ 由 `TerminalAsciiGlyphs` 单独 shaping 可打印 ASCII，按属性 flags 与前景色分组、字体不一致即断开，单次 `drawGlyphs` 最多 128 个字形；RTL 走 terminal-wg 隐式 LTR level-1，超过 500 行（`MAX_PARAGRAPH_ROWS`）跳过段落级 BiDi，老版本保留逐 cell 路径。盒绘制 U+2500–U+257F 与块元素 U+2580–U+259F 用 cell 相对 Canvas 几何，shade 用约 25%/50%/75% 覆盖的网点；Powerline、Braille 仍走字体。`BoxDrawingGoldenTest` 用 Roborazzi 固化 160 个字符及连接处。

无障碍部分提供冻结屏幕与历史的审阅模式：逐行、逐词、逐链接导航，按需朗读格式，选区可从审阅光标开始，软换行合并而硬换行保留；OSC 133 提供 prompt/命令边界。实时反馈用固定 300 ms 窗口，待朗读文本由 `TerminalOutputJournal.MAX_CHARACTERS` 限为 8192 个 UTF-16 单元（上游注释记作 16 KiB 存储），超限置 `overflow` 标志并按行丢弃最旧内容。这些是本地规范未声明功能，只吸收设计知识。

## 已核实局限

- `SelectionController` 的接口注释说 `selectAllVisible()` 只选当前视口，但默认实现直接 `= selectAll()`，调用方拿到的是整个滚动历史；实现类另有带参数的 `selectAllVisible(rows, cols, firstVisibleRow)`，两个入口语义不一致。
- 预 shaping 会让整帧（包括其中的 ASCII 行）等待，增加输入到呈现的延迟，不是免费优化。
- vendored libvterm 与生成的 Unicode 表需要随上游同步；无障碍按 Android 26/31/37 分支，移植时容易漏低版本路径。

## 与本仓对照

- 本仓 `native/src/terminal/output_processor.rs` 自己扫 OSC 52 读取请求，等于 VT 之外的第二份解析状态，且 `reset()` 必须在 RIS 后调用；termlib 把 OSC 处理放在 parser 回调链上，没有这份重复状态。这是本仓特有约束，不能照搬，但要继续守住重置路径的测试。
- `render/cell_builder.rs` 的 `compute_dirty_bands` 与 `diff_dirty_rows_into` 对应 termlib 的 damage 合并；脏行复用必须和图集代际守卫一起看，拆开就会显示旧字形。
- `ui/ImeConnection.kt` 的 paste/IME 与键入共用队列、`MAX_SURROUNDING_DELETES`/`MAX_FORWARD_DELETES` 对不可信长度设上界，与 termlib 的 FIFO 与不可分命令组是同一类问题，适合作为回归测试目标。
- 冻结快照加独立导航的无障碍审阅属于本地规范未定义功能，不实现。
