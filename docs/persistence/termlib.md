# termlib 上游审查（重点）

- 上游：https://github.com/connectbot/termlib @ `bd730de`（2026-11-24）；Apache-2.0。
- 定位：ConnectBot 的 Android 终端库，JNI 包装 libvterm，提供屏幕、滚动、选择、链接、无障碍与 Compose 组件；本批仓库中与本地终端层最直接可比。

## 架构与数据流

- `TerminalEmulator` 持有 VT 状态、屏幕、scrollback 与选择；`TerminalNative` 是 JNI 边界；`Terminal` 是 Compose 入口。
- `ScrollbackHistory`、`PackedCells`、`TerminalSnapshot`、`SelectionManager` 构成快照模型：后台合并受损单元范围、复用未变 packed row、在快照间共享未变滚动页，Compose 绘制时只读行内容。
- `docs/rendering.md` 规定：`writeInput` 仍在调用方 IO 线程同步解析，返回后输入缓冲可复用；API 31+ 且快照含复杂文本、视口贴底、单元宽度为正时，后台 dispatcher 预 shaping，`StateFlow` 合并等待期间的更新。代价是整帧（含 ASCII 行）输入到呈现延迟上升。
- 一次 `writeInput` 内的光标移动通知被合并：libvterm 保留全部逻辑移动，绘制只收最终位置；写外回调仍即时。组合、布局、绘制不持有终端或图像库锁。
- 键事件、IME commit、`pasteText(text)` 共用一个 FIFO；paste/IME 是不可分命令组，UTF-8 可分块但后续键入不能插入，paste 遵守 bracketed paste。
- 列数由 native parser 决定，与 glyph advance 解耦；native/JNI 单元存 16 个码点，更长组合进入下一单元。Unicode 17.0.0 表由 `tools/generate-unicode.py` 生成并可用 `--check` 复现。

## 绘制与无障碍

- 所有背景（含选择）先于所有前景；完整单元文本在列原点一起 shaping，过大 advance 横向压缩，斜体 overhang 保留。每行 display list 带视口边界，避免屏尺寸位图；图形层让光标反色两遍复用录制结果。
- API 31+ 单独 shaping 可打印 ASCII，按字体/颜色分组最多 128 个；选择重着色、装饰、链接和需压缩字形走逐单元路径。Latin-1 advance 每字体每字号 2 KiB 缓存，粗/常规分开；shape 引擎 256 KiB、prepared viewport 1 MiB 是显式上限。
- Box drawing（U+2500–U+257F）与块元素（U+2580–U+259F）用 cell 相对 Canvas 几何而非字体轮廓；Powerline/Braille 仍走字体。`BoxDrawingGoldenTest` 用 Roborazzi 固化 160 个字符与连接处。
- `docs/ACCESSIBILITY.md` 定义冻结屏幕/历史的审阅模式：逐行、逐词、逐链接导航，按需朗读格式，选择可从审阅光标开始，软换行合并而硬换行保留，状态可随会话保存。

## 已核实问题

- `SelectionManager.selectAllVisible()` 直接委托 `selectAll()`，与 `SelectionController` 接口注释“只选当前视口”不一致：调用方得到整个滚动历史。
- 预 shaping 的延迟是设计权衡而非免费优化；滚动或视口不在底部时会按需 shaping。
- vendored libvterm 与 Unicode 生成表需随上游同步；无障碍 API 按 Android 26/31/37 分版本分支，移植易漏低版本路径。

## 对 torvox 的取舍（优先级最高）

1. 对照 `native/src/terminal/output_processor.rs`：补“一次 write 内合并光标可见性更新”的回归测试，而不是新增通知类型。
2. 对照 `native/src/render/cell_builder.rs` 的 `compute_dirty_bands` / `diff_dirty_rows_into`：保持解析器列数为唯一列来源，脏行复用与图集代际守卫不可拆开。
3. 对照 `android/app/src/main/java/terminal/emulator/ui/ImeConnection.kt`：paste/IME 与键入共用 FIFO、不可信长度钳位应作为测试目标。
4. 冻结快照 + 独立导航的无障碍审阅属于本地规范未定义功能，不实现；只保留其设计知识。
- 跳过：未运行 Roborazzi 金图与 Android 仪器测试（需 SDK/设备）；风险是视觉与触摸行为只按文档和源码核对。
