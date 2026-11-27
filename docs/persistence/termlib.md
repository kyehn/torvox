# termlib 深度审查

- 来源：https://github.com/connectbot/termlib
- 审查快照：2026-11-27，默认分支工作树；不记录提交哈希。
- 覆盖方式：枚举全部受版本控制文件后逐文件完整读入并计算 SHA-256；核心实现按声明与关键片段逐行复核。二进制与许可证/锁文件/生成表仍完整读入并登记哈希，但不逐行复述内容。
- 本地核对：下列本地文件均已实际打开阅读，核对结论只落在既有结构上。

## 审查结论

仓库定位：ConnectBot 维护的 Android 终端库（包 `org.connectbot.terminal`），以 JNI 包装 libvterm，提供屏幕、滚动、选择、链接、无障碍与 Compose 组件。它是这批仓库中与本地终端层最直接可比的库，也是理解“完整状态机 + 完整 UI 契约”的最佳样本。

### 文件与层次

- `lib/src/main/java/org/connectbot/terminal/`：公开 API 与实现。`Terminal.kt` 是 Compose 入口；`TerminalEmulator.kt` 持有 VT 状态、屏幕、滚动与选择；`TerminalView/TerminalRenderer/TerminalShaping/TerminalBoxDrawing.kt` 负责绘制；`TerminalNative.kt` 是 JNI 边界；`PackedCells.kt`、`ScrollbackHistory.kt`、`TerminalSnapshot.kt`、`SelectionManager.kt` 构成快照模型；`OscParser.kt`、`HyperlinkState.kt` 处理链接；`KeyboardHandler.kt`、`TerminalInputConnection.kt` 处理输入；`Accessibility*.kt`、`Review*.kt` 处理无障碍审阅。
- `lib/src/main/cpp/libvterm/`：vendored VT 解析器，`src/` 是状态机，`src/vterm*.c` 管理屏幕、滚动、回调，`src/encoding/` 与 `src/unicode/` 提供 UTF-8、宽字符与 Unicode 数据。
- `lib/src/androidTest/`、`lib/src/test/`、`lib/src/testFixtures/`：仪器测试、JVM 测试与夹具；`benchmark/`：帧基准；`tools/generate-unicode.py`：从固定 Unicode 输入生成宽度/emoji 表；`docs/rendering.md`、`docs/ACCESSIBILITY.md`、`docs/logging.md`：行为契约。

### 输入与线程模型

`docs/rendering.md` 明确：解析仍同步发生在调用方 IO 线程，`writeInput` 返回后输入缓冲区可复用；后台 dispatcher 合并快照构建，按批传输受损单元范围，复用未变 packed row，并在快照间共享未变滚动页。Compose 只在绘制时读行内容。

- 普通路径在 Compose frame clock 上应用最新快照。API 31+ 且屏幕含复杂文本、视口在底部、单元宽度为正时，收集器在后台 dispatcher 预 shaping，准备完成后再发布快照；准备期间到来的更新会被 `StateFlow` 合并。代价是包括 ASCII 行在内的整帧延迟可能上升。
- 预排版只适用于同一快照、同一单元宽度、同一视口位置；滚动或视口不在底部时按需 shaping。
- 一次 `writeInput` 内的光标移动通知被合并：libvterm 保留所有逻辑移动，绘制只收到最终位置、可见性和批次起点；写外的回调仍即时。
- 组合、布局、绘制不持有终端或图像库锁；键盘/IME 提交、resize、单元尺寸更新异步。键事件、IME 提交和 `pasteText(text)` 共用一个 FIFO，paste/IME 是不可分命令组，UTF-8 可分传输块，但后续键入不能插入其间；paste 遵守 bracketed paste。
- 图像解码与动画在 UI 线程外合并，绘制只读已发布 bitmap/drawable； retired 资源保留在解码预算内直到弱引用通知，内存压力会推迟解码而不是超预算。

### 屏幕与文本模型

- 列数由 native parser 决定，与 glyph advance 解耦；Unicode 17.0.0 生成 emoji 属性与序列，输入框与生成表都 checked in，`--check` 可验证可复现性。
- 默认 emoji 表示与完整序列占两列，文本表示保持基础宽度；不完整序列立即显示并可能扩展前一单元，输入块边界不终止序列，终端命令才终止。变宽在 autowrap 开启时在右边界换行，变窄不会撤销旧换行。
- native/JNI 单元可存 16 个码点，更长的组合输入进入下一单元而不是丢弃；生成表按该容量校验。
- 绘制顺序是所有背景先于所有前景（含选择背景）；完整单元文本在单元列原点一起 shaping；过大 advance 横向压缩，斜体 overhang 保留；装饰按终端列对齐。每行绘制显示列表带视口边界，避免屏尺寸位图；图形层让光标反色两遍复用录制结果。
- Latin-1 单字形 advance 每字体每字号 2 KiB 缓存，粗/常规分开；API 31+ 对可打印 ASCII 逐字形 shaping，同字体同色相邻最多 128 个一组，按原列定位；选择重着色、装饰、链接和需压缩的字形走逐单元路径。

### 选择、滚动与 OSC 8

- `ScrollbackHistory.kt` 与 `PackedCells.kt` 把历史做成可共享页的无对象单元数组；`TerminalSnapshot.kt` 在快照中固定行与滚动视图，后台 shaping 只作用于快照。
- `SelectionManager.kt` 用快照坐标记录选择，配合 `ReviewSelection*.kt` 把软换行合并、硬换行保留；链接状态把 OSC 8 的 id/URI 与单元关联，`OscParser.kt` 解析 OSC 序列。
- `docs/ACCESSIBILITY.md` 定义了完整的“审阅模式”：冻结屏幕和历史后 TalkBack 可逐行、逐词、逐链接导航，格式化信息按需朗读，选择可从审阅光标开始，软换行合并而硬换行保留；审阅状态可随会话保存，弹窗期间可静音播报。
- 96 个测试文件覆盖 JNI、快照、选择、无障碍、渲染等；18 处 TODO/FIXME 说明仍有未完成点，但合同文档与测试密度是这批仓库中最高的。

### 验证与风险

- 后台 shaping 会抬高复杂快照的输入到呈现延迟，文档明确承认；这是“预 shaping 换帧稳定”的取舍，不是免费优化。
- vendored libvterm 与 Unicode 生成表需要随上游同步；生成器与 `--check` 是必要维护工具，不能只复制生成结果。
- 大量无障碍 API 面向 Android 26/31/37 分版本行为，移植时容易漏掉低版本分支。

### 本地取舍（优先级最高）

1. 本地 `OutputProcessor::write` 已按 OSC 与 CSI 分派；应补上“一次 write 内合并光标可见性更新”的做法，减少每帧多次通知。
2. 本地 `native/src/render/` 若存在按字形 advance 推导列宽的路径，应以 parser 列数为准；这与本地 Ghostty 列模型一致。
3. 本地输入链 `InputFfi::key_down/key_up` → `TerminalInputEncoder.appendKey` → 核心，已有 Kitty 编码；termlib 的“paste/IME 与键入共 FIFO、paste 不可分”应作为回归测试目标，而不是新增抽象。
4. 滚动共享页与快照复用是明确可借鉴的数据结构；但本地已有自有渲染缓存，先对齐语义再谈替换。
5. 无障碍审阅状态属于本仓规范未定义功能，不实现；只保留其“冻结快照 + 独立导航”的设计知识。

## 本地代码核对记录

| 本地文件 | 核对点 |
| --- | --- |
| `native/src/terminal/output_processor.rs` | 对照一次 write 内合并光标/选择通知的缺口 |
| `native/src/render/cell_builder.rs` | 对照列数来自解析器、脏行复用与图集代际守卫 |
| `native/src/render/font/mod.rs` | 对照字形缓存容量与精确光栅键 |
| `native/src/render/pipeline.rs` | 对照 std140 布局与逐帧 uniform 更新 |
| `native/src/render/mod.rs` | 对照无 Canvas 回退与可恢复 GPU 错误分类 |
| `native/src/terminal/ghostty_terminal/public_api.rs` | 对照 `sanitize_vt_input` 的过滤范围 |
| `native/src/terminal/ghostty_terminal/internal/input_filter.rs` | 对照 Kitty 探测/进位与 CSI 饱和 |
| `native/src/terminal/ghostty_terminal/key_encoding_tests.rs` | 对照 DECCKM/modifyOtherKeys/Kitty 编码事实 |
| `android/app/src/main/java/terminal/emulator/ui/TerminalInputEncoder.kt` | 对照 Kitty 键盘编码覆盖矩阵 |
| `android/app/src/main/java/terminal/emulator/ui/ImeConnection.kt` | 对照 paste/IME 与键入共用 FIFO 及不可信长度钳位 |
| `android/app/src/main/java/terminal/emulator/ui/TerminalScreen.kt` | 对照快照/视口偏移与搜索防抖 |
| `android/app/src/main/java/terminal/emulator/bridge/NativeBridge.kt` | 对照批量输入与渲染契约 |

## 文件覆盖清单

状态含义：`TEXT` 为逐行复核范围内的文本文件；`BINARY` 为含 NUL 的二进制资源；`DECLARED_EXCLUSION` 为许可证、锁文件或生成表。三者都经过完整读入与哈希登记。

| 路径 | 状态 | 字节 | 行 | SHA-256（前 12 位） |
| --- | --- | ---: | ---: | --- |
| `.editorconfig` | TEXT | 610 | 19 | `e400afad5a39` |
| `.gitattributes` | TEXT | 42 | 1 | `67ca32a33e9d` |
| `.github/ISSUE_TEMPLATE/release-branch.yml` | TEXT | 1119 | 36 | `01c1b7604642` |
| `.github/ISSUE_TEMPLATE/release.yml` | TEXT | 1586 | 47 | `f72c0d04e8f2` |
| `.github/docs.json` | TEXT | 507 | 29 | `b85d9bc24af4` |
| `.github/scripts/docs.init.gradle.kts` | TEXT | 2688 | 56 | `db00610f198d` |
| `.github/workflows/actions.lock` | DECLARED_EXCLUSION | 3510 | 94 | `54aed3f263ec` |
| `.github/workflows/ci.yml` | TEXT | 5223 | 146 | `171fa830eb2a` |
| `.github/workflows/docs.yml` | TEXT | 2597 | 96 | `44752edc67de` |
| `.github/workflows/release.yml` | TEXT | 751 | 33 | `fb4b299cb2c1` |
| `.gitignore` | TEXT | 271 | 19 | `7a9185eae7c1` |
| `CHANGELOG.md` | TEXT | 3379 | 87 | `050fe3bc61c4` |
| `LICENSE` | DECLARED_EXCLUSION | 11358 | 202 | `cfc7749b96f6` |
| `README.md` | TEXT | 1065 | 36 | `c6f56bcc7953` |
| `benchmark/README.md` | TEXT | 12005 | 216 | `d56542852f0a` |
| `benchmark/capture-cacafire.py` | TEXT | 1152 | 35 | `45392b9cdfb8` |
| `benchmark/profile-frames.py` | TEXT | 3978 | 69 | `8aaf4fce14dd` |
| `benchmark/run-frames.py` | TEXT | 6416 | 100 | `01bcc868b150` |
| `benchmark/summarize-frames.py` | TEXT | 10898 | 196 | `73d5956dc84d` |
| `build.gradle.kts` | TEXT | 1660 | 66 | `73550381c734` |
| `docs/ACCESSIBILITY.md` | TEXT | 14327 | 219 | `969d5acd8781` |
| `docs/logging.md` | TEXT | 301 | 7 | `e71be71371d5` |
| `docs/rendering.md` | TEXT | 12150 | 206 | `ff0b8c0bb218` |
| `gradle.properties` | TEXT | 335 | 11 | `06b590cbd012` |
| `gradle/libs.versions.toml` | TEXT | 3504 | 70 | `73d09f5c802e` |
| `gradle/wrapper/gradle-wrapper.jar` | BINARY | 49154 | 185 | `621008afc4f5` |
| `gradle/wrapper/gradle-wrapper.properties` | TEXT | 281 | 9 | `e0b173cf06bd` |
| `gradlew` | TEXT | 8656 | 248 | `a5a5c199ba02` |
| `gradlew.bat` | TEXT | 4888 | 112 | `f17917f8dbe6` |
| `lib/README.md` | TEXT | 4003 | 87 | `9e4be5509c9f` |
| `lib/api.txt` | TEXT | 24453 | 409 | `81270fe0b1d0` |
| `lib/build.gradle.kts` | TEXT | 9214 | 302 | `aa8083101697` |
| `lib/consumer-rules.pro` | TEXT | 230 | 11 | `6b8ea80656c2` |
| `lib/dokka/templates/includes/footer.ftl` | TEXT | 992 | 23 | `b3ad9bfbad91` |
| `lib/proguard-rules.pro` | TEXT | 321 | 16 | `6974933eb3d7` |
| `lib/src/androidTest/java/org/connectbot/terminal/AndroidBoxDrawingTest.kt` | TEXT | 1495 | 40 | `fca61de7e22a` |
| `lib/src/androidTest/java/org/connectbot/terminal/AndroidFontFallbackTest.kt` | TEXT | 3016 | 71 | `900fbafbd09a` |
| `lib/src/androidTest/java/org/connectbot/terminal/AndroidInlineImageTest.kt` | TEXT | 305 | 12 | `7ebfc8aef9b6` |
| `lib/src/androidTest/java/org/connectbot/terminal/AndroidInversionTest.kt` | TEXT | 303 | 12 | `bd50e8b55fa3` |
| `lib/src/androidTest/java/org/connectbot/terminal/AndroidMemoryTest.kt` | TEXT | 5080 | 113 | `798deab2baf3` |
| `lib/src/androidTest/java/org/connectbot/terminal/AndroidRenderBenchmarkTest.kt` | TEXT | 14861 | 285 | `2e3aa9bea5fc` |
| `lib/src/androidTest/java/org/connectbot/terminal/AndroidShapingTest.kt` | TEXT | 299 | 12 | `d2bbd841b5c8` |
| `lib/src/androidTest/java/org/connectbot/terminal/AndroidTerminalLifetimeTest.kt` | TEXT | 309 | 12 | `c107e8148f54` |
| `lib/src/androidTest/java/org/connectbot/terminal/AndroidUtf8BoundaryTest.kt` | TEXT | 1171 | 34 | `306a477a2a59` |
| `lib/src/androidTest/java/org/connectbot/terminal/TerminalAccessibilityDeviceTest.kt` | TEXT | 29150 | 555 | `b18c44daffc8` |
| `lib/src/androidTest/java/org/connectbot/terminal/TerminalImeLifecycleTest.kt` | TEXT | 8735 | 200 | `27cec91161c9` |
| `lib/src/androidTest/java/org/connectbot/terminal/TerminalRedrawTest.kt` | TEXT | 10648 | 223 | `dfbc893e6f13` |
| `lib/src/main/cpp/CMakeLists.txt` | TEXT | 1428 | 53 | `45ea9b5c948c` |
| `lib/src/main/cpp/Terminal.cpp` | TEXT | 46873 | 1205 | `eaf4802ab892` |
| `lib/src/main/cpp/Terminal.h` | TEXT | 9010 | 221 | `4c790d8e41d3` |
| `lib/src/main/cpp/libvterm/CONTRIBUTING` | TEXT | 643 | 22 | `fd7e18ecb4ad` |
| `lib/src/main/cpp/libvterm/LICENSE` | DECLARED_EXCLUSION | 1098 | 23 | `66c627cf44f0` |
| `lib/src/main/cpp/libvterm/Makefile` | TEXT | 2823 | 112 | `55b00d22e669` |
| `lib/src/main/cpp/libvterm/README.termlib` | TEXT | 2104 | 44 | `92055aea1916` |
| `lib/src/main/cpp/libvterm/bin/unterm.c` | TEXT | 6815 | 280 | `84de7a73a63d` |
| `lib/src/main/cpp/libvterm/bin/vterm-ctrl.c` | TEXT | 7603 | 354 | `254819b7e489` |
| `lib/src/main/cpp/libvterm/bin/vterm-dump.c` | TEXT | 6169 | 245 | `9cdd71fa94d5` |
| `lib/src/main/cpp/libvterm/include/vterm.h` | TEXT | 22072 | 655 | `4d5989be2818` |
| `lib/src/main/cpp/libvterm/include/vterm_keycodes.h` | TEXT | 1106 | 61 | `2a8a667b8f82` |
| `lib/src/main/cpp/libvterm/src/compact-screen.h` | TEXT | 1179 | 33 | `90a36f2b5c85` |
| `lib/src/main/cpp/libvterm/src/emoji.inc` | TEXT | 162128 | 6313 | `3547cc8c964e` |
| `lib/src/main/cpp/libvterm/src/encoding.c` | TEXT | 6655 | 257 | `b121ea793c46` |
| `lib/src/main/cpp/libvterm/src/encoding/DECdrawing.inc` | TEXT | 757 | 36 | `d8c25047d5f4` |
| `lib/src/main/cpp/libvterm/src/encoding/uk.inc` | TEXT | 119 | 6 | `9dad74b7ced6` |
| `lib/src/main/cpp/libvterm/src/fullwidth.inc` | TEXT | 2544 | 111 | `41c4b4a0722c` |
| `lib/src/main/cpp/libvterm/src/keyboard.c` | TEXT | 6352 | 233 | `147c9ea9709d` |
| `lib/src/main/cpp/libvterm/src/mouse.c` | TEXT | 2507 | 99 | `2824a884d4ac` |
| `lib/src/main/cpp/libvterm/src/parser.c` | TEXT | 13405 | 482 | `5b5e5f4d0059` |
| `lib/src/main/cpp/libvterm/src/pen.c` | TEXT | 17945 | 619 | `28f7575ba56a` |
| `lib/src/main/cpp/libvterm/src/rect.h` | TEXT | 2022 | 56 | `8ac51df5e5c7` |
| `lib/src/main/cpp/libvterm/src/screen.c` | TEXT | 48133 | 1573 | `1a64df2eb4bf` |
| `lib/src/main/cpp/libvterm/src/state.c` | TEXT | 64645 | 2400 | `5df179ddca90` |
| `lib/src/main/cpp/libvterm/src/unicode.c` | TEXT | 18380 | 428 | `6bbfb0fbe885` |
| `lib/src/main/cpp/libvterm/src/utf8.h` | TEXT | 1040 | 39 | `2a1c6ffd176d` |
| `lib/src/main/cpp/libvterm/src/vterm.c` | TEXT | 10703 | 430 | `f8620e99da94` |
| `lib/src/main/cpp/libvterm/src/vterm_internal.h` | TEXT | 7549 | 309 | `c55a7caf07c9` |
| `lib/src/main/cpp/libvterm/t/02parser.test` | TEXT | 4465 | 298 | `d5cd310f18b1` |
| `lib/src/main/cpp/libvterm/t/03encoding_utf8.test` | TEXT | 3221 | 122 | `d1071691a725` |
| `lib/src/main/cpp/libvterm/t/10state_putglyph.test` | TEXT | 1410 | 74 | `5f288b5b43a5` |
| `lib/src/main/cpp/libvterm/t/11state_movecursor.test` | TEXT | 2955 | 224 | `77d15102a643` |
| `lib/src/main/cpp/libvterm/t/12state_scroll.test` | TEXT | 2623 | 160 | `57a1a30635d3` |
| `lib/src/main/cpp/libvterm/t/13state_edit.test` | TEXT | 4844 | 308 | `5d07f98b41c9` |
| `lib/src/main/cpp/libvterm/t/14state_encoding.test` | TEXT | 1371 | 105 | `41429a38503a` |
| `lib/src/main/cpp/libvterm/t/15state_mode.test` | TEXT | 1415 | 86 | `2095f7c86123` |
| `lib/src/main/cpp/libvterm/t/16state_resize.test` | TEXT | 778 | 48 | `584843375d86` |
| `lib/src/main/cpp/libvterm/t/17state_mouse.test` | TEXT | 3509 | 191 | `8af89309fd60` |
| `lib/src/main/cpp/libvterm/t/18state_termprops.test` | TEXT | 694 | 42 | `43ec314a43a5` |
| `lib/src/main/cpp/libvterm/t/20state_wrapping.test` | TEXT | 1259 | 69 | `801c8b7b3c7b` |
| `lib/src/main/cpp/libvterm/t/21state_tabstops.test` | TEXT | 898 | 60 | `897a3e0898f2` |
| `lib/src/main/cpp/libvterm/t/22state_save.test` | TEXT | 1011 | 64 | `a70943d1abe1` |
| `lib/src/main/cpp/libvterm/t/25state_input.test` | TEXT | 2594 | 173 | `a11ca93cdc8f` |
| `lib/src/main/cpp/libvterm/t/26state_query.test` | TEXT | 1002 | 63 | `f210ec7526c4` |
| `lib/src/main/cpp/libvterm/t/27state_reset.test` | TEXT | 430 | 32 | `d053a52d704e` |
| `lib/src/main/cpp/libvterm/t/28state_dbl_wh.test` | TEXT | 1247 | 61 | `e9798638119c` |
| `lib/src/main/cpp/libvterm/t/29state_fallback.test` | TEXT | 408 | 31 | `4bfb89d4dbe2` |
| `lib/src/main/cpp/libvterm/t/30state_pen.test` | TEXT | 2507 | 141 | `a66342d365f9` |
| `lib/src/main/cpp/libvterm/t/31state_rep.test` | TEXT | 2739 | 133 | `d3bf2476cf9e` |
| `lib/src/main/cpp/libvterm/t/32state_flow.test` | TEXT | 493 | 28 | `80974362b5ae` |
| `lib/src/main/cpp/libvterm/t/40state_selection.test` | TEXT | 1904 | 89 | `685be81b6c91` |
| `lib/src/main/cpp/libvterm/t/60screen_ascii.test` | TEXT | 1496 | 69 | `3a5b143e21c5` |
| `lib/src/main/cpp/libvterm/t/61screen_unicode.test` | TEXT | 2522 | 59 | `a1dd6abcbca5` |
| `lib/src/main/cpp/libvterm/t/62screen_damage.test` | TEXT | 2972 | 155 | `45de9f6bd3b2` |
| `lib/src/main/cpp/libvterm/t/63screen_resize.test` | TEXT | 2775 | 134 | `fb62d3c2d353` |
| `lib/src/main/cpp/libvterm/t/64screen_pen.test` | TEXT | 2445 | 71 | `bcdae038fdb9` |
| `lib/src/main/cpp/libvterm/t/65screen_protect.test` | TEXT | 242 | 16 | `995eb7d95674` |
| `lib/src/main/cpp/libvterm/t/66screen_extent.test` | TEXT | 245 | 11 | `93ad0670f39a` |
| `lib/src/main/cpp/libvterm/t/67screen_dbl_wh.test` | TEXT | 1003 | 38 | `c0a10b2e1675` |
| `lib/src/main/cpp/libvterm/t/68screen_termprops.test` | TEXT | 254 | 17 | `c4afa5ca1908` |
| `lib/src/main/cpp/libvterm/t/69screen_pushline.test` | TEXT | 508 | 17 | `d744ce3da9cd` |
| `lib/src/main/cpp/libvterm/t/69screen_reflow.test` | TEXT | 2426 | 120 | `13d4f9c3b52a` |
| `lib/src/main/cpp/libvterm/t/69screen_sb_clear.test` | TEXT | 61 | 6 | `e678c4bc22f1` |
| `lib/src/main/cpp/libvterm/t/90vttest_01-movement-1.test` | TEXT | 2764 | 87 | `10e94d5d43d0` |
| `lib/src/main/cpp/libvterm/t/90vttest_01-movement-2.test` | TEXT | 2728 | 40 | `3f31d3fcc24f` |
| `lib/src/main/cpp/libvterm/t/90vttest_01-movement-3.test` | TEXT | 559 | 21 | `c24eb0915a2e` |
| `lib/src/main/cpp/libvterm/t/90vttest_01-movement-4.test` | TEXT | 1022 | 36 | `02f71881c709` |
| `lib/src/main/cpp/libvterm/t/90vttest_02-screen-1.test` | TEXT | 339 | 18 | `56393380e0cf` |
| `lib/src/main/cpp/libvterm/t/90vttest_02-screen-2.test` | TEXT | 490 | 29 | `18bde01b6221` |
| `lib/src/main/cpp/libvterm/t/90vttest_02-screen-3.test` | TEXT | 181 | 16 | `67443dc9aea3` |
| `lib/src/main/cpp/libvterm/t/90vttest_02-screen-4.test` | TEXT | 188 | 17 | `f013a8d13878` |
| `lib/src/main/cpp/libvterm/t/92lp1640917.test` | TEXT | 257 | 13 | `7ac942700e4c` |
| `lib/src/main/cpp/libvterm/t/harness.c` | TEXT | 32336 | 1256 | `32f5469bf970` |
| `lib/src/main/cpp/libvterm/t/run-test.pl` | TEXT | 6678 | 234 | `a0883abb508a` |
| `lib/src/main/cpp/libvterm/vterm.pc.in` | TEXT | 174 | 8 | `ec1c59df4bdd` |
| `lib/src/main/java/org/connectbot/terminal/AccessibilityOverlay.kt` | TEXT | 1541 | 47 | `926bae783a27` |
| `lib/src/main/java/org/connectbot/terminal/AccessibilityStateManager.kt` | TEXT | 2886 | 77 | `1f1fe1a8dba2` |
| `lib/src/main/java/org/connectbot/terminal/AnimatedImage.kt` | TEXT | 2870 | 73 | `feaa3b627e5f` |
| `lib/src/main/java/org/connectbot/terminal/CellData.kt` | TEXT | 2322 | 58 | `8e3e1450d965` |
| `lib/src/main/java/org/connectbot/terminal/ColorCache.kt` | TEXT | 5102 | 175 | `fd964816c045` |
| `lib/src/main/java/org/connectbot/terminal/ComposeController.kt` | TEXT | 1778 | 63 | `25e83842d9d8` |
| `lib/src/main/java/org/connectbot/terminal/ComposeMode.kt` | TEXT | 2486 | 88 | `c69334d61154` |
| `lib/src/main/java/org/connectbot/terminal/DelKeyMode.kt` | TEXT | 1096 | 34 | `03ecd98b8869` |
| `lib/src/main/java/org/connectbot/terminal/ImageAnimationBounds.kt` | TEXT | 2699 | 73 | `88a295c22066` |
| `lib/src/main/java/org/connectbot/terminal/ImageBytes.kt` | TEXT | 6560 | 186 | `20422763d846` |
| `lib/src/main/java/org/connectbot/terminal/ImageDimensions.kt` | TEXT | 6438 | 125 | `dbcfe77c3dd4` |
| `lib/src/main/java/org/connectbot/terminal/ImagePresentation.kt` | TEXT | 1866 | 57 | `4524b1b5fd66` |
| `lib/src/main/java/org/connectbot/terminal/ImeInputView.kt` | TEXT | 35925 | 831 | `c5bb0b1febd3` |
| `lib/src/main/java/org/connectbot/terminal/ImeShortcutInputMode.kt` | TEXT | 1036 | 29 | `e2709e6cde10` |
| `lib/src/main/java/org/connectbot/terminal/InlineImageConsentGate.kt` | TEXT | 2145 | 72 | `3ccd1f8fa9de` |
| `lib/src/main/java/org/connectbot/terminal/InlineImageLimits.kt` | TEXT | 961 | 26 | `3723e0b32e08` |
| `lib/src/main/java/org/connectbot/terminal/InlineImageProtocol.kt` | TEXT | 30826 | 665 | `4d060f850642` |
| `lib/src/main/java/org/connectbot/terminal/InlineImageStore.kt` | TEXT | 34304 | 783 | `a9cb656f4eae` |
| `lib/src/main/java/org/connectbot/terminal/InlineImages.kt` | TEXT | 1311 | 45 | `0a2b191fc93f` |
| `lib/src/main/java/org/connectbot/terminal/KeyboardHandler.kt` | TEXT | 26470 | 730 | `ca06b692e67d` |
| `lib/src/main/java/org/connectbot/terminal/KittyPlaceholder.kt` | TEXT | 3076 | 52 | `111e76e66ca2` |
| `lib/src/main/java/org/connectbot/terminal/LiveOutputRegion.kt` | TEXT | 2093 | 50 | `94f897f21d37` |
| `lib/src/main/java/org/connectbot/terminal/ModifierManager.kt` | TEXT | 1576 | 50 | `cd903a591180` |
| `lib/src/main/java/org/connectbot/terminal/OscParser.kt` | TEXT | 14092 | 386 | `5741492147f2` |
| `lib/src/main/java/org/connectbot/terminal/PackedCells.kt` | TEXT | 19152 | 407 | `7df462bb1be2` |
| `lib/src/main/java/org/connectbot/terminal/RightAltMode.kt` | TEXT | 1556 | 39 | `e8db1123972a` |
| `lib/src/main/java/org/connectbot/terminal/ScreenTransfer.kt` | TEXT | 3309 | 82 | `0db8eaa6d493` |
| `lib/src/main/java/org/connectbot/terminal/ScrollController.kt` | TEXT | 1510 | 49 | `7e10c018f381` |
| `lib/src/main/java/org/connectbot/terminal/ScrollbackHistory.kt` | TEXT | 3054 | 97 | `26c85786b178` |
| `lib/src/main/java/org/connectbot/terminal/SelectionManager.kt` | TEXT | 21963 | 577 | `f7c2f813464d` |
| `lib/src/main/java/org/connectbot/terminal/SelectionTouch.kt` | TEXT | 6070 | 128 | `7c2900244d48` |
| `lib/src/main/java/org/connectbot/terminal/SemanticType.kt` | TEXT | 2706 | 86 | `49657c031a7a` |
| `lib/src/main/java/org/connectbot/terminal/Terminal.kt` | TEXT | 147460 | 3121 | `82c336e24064` |
| `lib/src/main/java/org/connectbot/terminal/TerminalAccessibility.kt` | TEXT | 11155 | 274 | `23a4972fa552` |
| `lib/src/main/java/org/connectbot/terminal/TerminalAccessibilityControls.kt` | TEXT | 10518 | 204 | `831f96ea533c` |
| `lib/src/main/java/org/connectbot/terminal/TerminalAccessibilityView.kt` | TEXT | 51932 | 1042 | `7aba83afa89c` |
| `lib/src/main/java/org/connectbot/terminal/TerminalAccessibleText.kt` | TEXT | 1534 | 40 | `857b3644b338` |
| `lib/src/main/java/org/connectbot/terminal/TerminalAsciiGlyphs.kt` | TEXT | 2850 | 73 | `42dd4ec551d5` |
| `lib/src/main/java/org/connectbot/terminal/TerminalBoxDrawing.kt` | TEXT | 18956 | 484 | `f15082d17168` |
| `lib/src/main/java/org/connectbot/terminal/TerminalCallbacks.kt` | TEXT | 6787 | 172 | `6463fb234d69` |
| `lib/src/main/java/org/connectbot/terminal/TerminalDispatcher.kt` | TEXT | 3308 | 92 | `834766441af0` |
| `lib/src/main/java/org/connectbot/terminal/TerminalEmulator.kt` | TEXT | 61330 | 1549 | `1e471f690ef1` |
| `lib/src/main/java/org/connectbot/terminal/TerminalExtendedSelection.kt` | TEXT | 2029 | 47 | `5836a82dd0ba` |
| `lib/src/main/java/org/connectbot/terminal/TerminalFrameTrace.kt` | TEXT | 1196 | 35 | `5d9fc914fe2a` |
| `lib/src/main/java/org/connectbot/terminal/TerminalInversion.kt` | TEXT | 3934 | 113 | `ad97dba8657c` |
| `lib/src/main/java/org/connectbot/terminal/TerminalLine.kt` | TEXT | 8042 | 196 | `d4ceb5d6886e` |
| `lib/src/main/java/org/connectbot/terminal/TerminalNative.kt` | TEXT | 10675 | 255 | `4c7ab8b37714` |
| `lib/src/main/java/org/connectbot/terminal/TerminalOutputJournal.kt` | TEXT | 7004 | 167 | `10296d2a32ab` |
| `lib/src/main/java/org/connectbot/terminal/TerminalReviewFormatting.kt` | TEXT | 3203 | 81 | `5f0cc53c7fea` |
| `lib/src/main/java/org/connectbot/terminal/TerminalReviewLinks.kt` | TEXT | 1070 | 21 | `fa733e5d29f9` |
| `lib/src/main/java/org/connectbot/terminal/TerminalReviewSelection.kt` | TEXT | 6367 | 143 | `38b2a855731e` |
| `lib/src/main/java/org/connectbot/terminal/TerminalScreenState.kt` | TEXT | 9928 | 261 | `33c56b306e90` |
| `lib/src/main/java/org/connectbot/terminal/TerminalShaping.kt` | TEXT | 32223 | 698 | `37f6d391319e` |
| `lib/src/main/java/org/connectbot/terminal/TerminalSnapshot.kt` | TEXT | 4172 | 113 | `1a10b1877d52` |
| `lib/src/main/java/org/connectbot/terminal/TerminalTextDecoder.kt` | TEXT | 1993 | 55 | `a8d9f4515b9e` |
| `lib/src/main/java/org/connectbot/terminal/TerminalTextPaint.kt` | TEXT | 3946 | 97 | `e8f79e80cc98` |
| `lib/src/main/java/org/connectbot/terminal/UrlDetection.kt` | TEXT | 7440 | 182 | `9f8ca95b1b4d` |
| `lib/src/main/res/values-af/accessibility.xml` | TEXT | 7756 | 76 | `e851ac27d257` |
| `lib/src/main/res/values-af/strings.xml` | TEXT | 703 | 12 | `c531eb526fcc` |
| `lib/src/main/res/values-ar/accessibility.xml` | TEXT | 8873 | 80 | `131ea9c7fb3f` |
| `lib/src/main/res/values-ar/strings.xml` | TEXT | 773 | 12 | `dbd9e3f03060` |
| `lib/src/main/res/values-be/accessibility.xml` | TEXT | 9317 | 78 | `bcd969364abd` |
| `lib/src/main/res/values-be/strings.xml` | TEXT | 836 | 12 | `0092aa421511` |
| `lib/src/main/res/values-bg/accessibility.xml` | TEXT | 9381 | 76 | `5524d60ae00f` |
| `lib/src/main/res/values-bg/strings.xml` | TEXT | 818 | 12 | `791a703734bf` |
| `lib/src/main/res/values-ca/accessibility.xml` | TEXT | 8073 | 77 | `efbe65bf6ca0` |
| `lib/src/main/res/values-ca/strings.xml` | TEXT | 724 | 12 | `7e83b5fbe0d0` |
| `lib/src/main/res/values-cs/accessibility.xml` | TEXT | 8139 | 78 | `18c4c19c6519` |
| `lib/src/main/res/values-cs/strings.xml` | TEXT | 717 | 12 | `3ad4a8517f3c` |
| `lib/src/main/res/values-da/accessibility.xml` | TEXT | 7686 | 76 | `ec0b5d4a3e67` |
| `lib/src/main/res/values-da/strings.xml` | TEXT | 708 | 12 | `ab351c47b74b` |
| `lib/src/main/res/values-de/accessibility.xml` | TEXT | 7813 | 76 | `1d5b450596e9` |
| `lib/src/main/res/values-de/strings.xml` | TEXT | 730 | 12 | `85ecc8cceb47` |
| `lib/src/main/res/values-el/accessibility.xml` | TEXT | 9712 | 76 | `3608671bbc08` |
| `lib/src/main/res/values-el/strings.xml` | TEXT | 893 | 12 | `1d366a5d7f36` |
| `lib/src/main/res/values-en-rCA/strings.xml` | TEXT | 692 | 12 | `b64c830847b6` |
| `lib/src/main/res/values-en-rGB/strings.xml` | TEXT | 692 | 12 | `b64c830847b6` |
| `lib/src/main/res/values-es/accessibility.xml` | TEXT | 7977 | 77 | `d140267a64f1` |
| `lib/src/main/res/values-es/strings.xml` | TEXT | 725 | 12 | `3d92edbefa08` |
| `lib/src/main/res/values-eu/accessibility.xml` | TEXT | 7811 | 76 | `0548beef399a` |
| `lib/src/main/res/values-eu/strings.xml` | TEXT | 729 | 12 | `4871c8548733` |
| `lib/src/main/res/values-fa/accessibility.xml` | TEXT | 8593 | 76 | `db8382b351b4` |
| `lib/src/main/res/values-fa/strings.xml` | TEXT | 795 | 12 | `abe218e139dd` |
| `lib/src/main/res/values-fi/accessibility.xml` | TEXT | 7813 | 76 | `93c8c090075f` |
| `lib/src/main/res/values-fi/strings.xml` | TEXT | 714 | 12 | `8b2c111ef89c` |
| `lib/src/main/res/values-fr/accessibility.xml` | TEXT | 8054 | 77 | `e5d1e03d59a8` |
| `lib/src/main/res/values-fr/strings.xml` | TEXT | 731 | 12 | `c9c03d528b4a` |
| `lib/src/main/res/values-gl/accessibility.xml` | TEXT | 7911 | 76 | `71c3439f8068` |
| `lib/src/main/res/values-gl/strings.xml` | TEXT | 724 | 12 | `81d6caa9e504` |
| `lib/src/main/res/values-he/accessibility.xml` | TEXT | 8262 | 77 | `4c539f5ef8e1` |
| `lib/src/main/res/values-he/strings.xml` | TEXT | 762 | 12 | `917fdcd1b8f6` |
| `lib/src/main/res/values-hr/accessibility.xml` | TEXT | 7855 | 77 | `0ea18ee2bf29` |
| `lib/src/main/res/values-hr/strings.xml` | TEXT | 714 | 12 | `80069cc92d0c` |
| `lib/src/main/res/values-hu/accessibility.xml` | TEXT | 8029 | 76 | `761dd161c8e1` |
| `lib/src/main/res/values-hu/strings.xml` | TEXT | 747 | 12 | `dc83427dd693` |
| `lib/src/main/res/values-id/accessibility.xml` | TEXT | 7690 | 75 | `af287fe611b1` |
| `lib/src/main/res/values-id/strings.xml` | TEXT | 709 | 12 | `2d1f2d0d3ae9` |
| `lib/src/main/res/values-in/accessibility.xml` | TEXT | 7690 | 75 | `af287fe611b1` |
| `lib/src/main/res/values-in/strings.xml` | TEXT | 709 | 12 | `2d1f2d0d3ae9` |
| `lib/src/main/res/values-is/accessibility.xml` | TEXT | 7604 | 76 | `bb8ffa59f651` |
| `lib/src/main/res/values-is/strings.xml` | TEXT | 709 | 12 | `9d7c79764cd2` |
| `lib/src/main/res/values-it/accessibility.xml` | TEXT | 8110 | 77 | `855efc5582bd` |
| `lib/src/main/res/values-it/strings.xml` | TEXT | 737 | 12 | `07e4d38fcff5` |
| `lib/src/main/res/values-iw/accessibility.xml` | TEXT | 8262 | 77 | `4c539f5ef8e1` |
| `lib/src/main/res/values-iw/strings.xml` | TEXT | 762 | 12 | `917fdcd1b8f6` |
| `lib/src/main/res/values-ja/accessibility.xml` | TEXT | 8077 | 75 | `5824d219c70d` |
| `lib/src/main/res/values-ja/strings.xml` | TEXT | 762 | 12 | `ba535439dcdf` |
| `lib/src/main/res/values-ka/accessibility.xml` | TEXT | 10864 | 76 | `3dd061efd9f0` |
| `lib/src/main/res/values-ka/strings.xml` | TEXT | 965 | 12 | `1334ab5b7e1a` |
| `lib/src/main/res/values-ko/accessibility.xml` | TEXT | 7838 | 75 | `cad2d6715428` |
| `lib/src/main/res/values-ko/strings.xml` | TEXT | 728 | 12 | `e7309667b0ad` |
| `lib/src/main/res/values-lo/accessibility.xml` | TEXT | 9697 | 75 | `815033069ea1` |
| `lib/src/main/res/values-lo/strings.xml` | TEXT | 885 | 12 | `d5dd60e113df` |
| `lib/src/main/res/values-lt/accessibility.xml` | TEXT | 8200 | 78 | `46ee08d2fc16` |
| `lib/src/main/res/values-lt/strings.xml` | TEXT | 750 | 12 | `2929796966e1` |
| `lib/src/main/res/values-lv/accessibility.xml` | TEXT | 8038 | 77 | `2f75cbbe376a` |
| `lib/src/main/res/values-lv/strings.xml` | TEXT | 732 | 12 | `114f42025d03` |
| `lib/src/main/res/values-mk/accessibility.xml` | TEXT | 9494 | 76 | `09c476098412` |
| `lib/src/main/res/values-mk/strings.xml` | TEXT | 795 | 12 | `4310751a45f8` |
| `lib/src/main/res/values-nb/accessibility.xml` | TEXT | 7689 | 76 | `21a2430ad7d2` |
| `lib/src/main/res/values-nb/strings.xml` | TEXT | 699 | 12 | `51e9101194cd` |
| `lib/src/main/res/values-ne/accessibility.xml` | TEXT | 10323 | 76 | `4c0e4ca0fc69` |
| `lib/src/main/res/values-ne/strings.xml` | TEXT | 969 | 12 | `6c11fd446709` |
| `lib/src/main/res/values-nl/accessibility.xml` | TEXT | 7754 | 76 | `983c8aaa3b8b` |
| `lib/src/main/res/values-nl/strings.xml` | TEXT | 718 | 12 | `3540f582ec2c` |
| `lib/src/main/res/values-pl/accessibility.xml` | TEXT | 8104 | 78 | `07487d66c541` |
| `lib/src/main/res/values-pl/strings.xml` | TEXT | 716 | 12 | `714b5cf9b3df` |
| `lib/src/main/res/values-pt-rBR/accessibility.xml` | TEXT | 7934 | 77 | `2e09d791a148` |
| `lib/src/main/res/values-pt-rBR/strings.xml` | TEXT | 725 | 12 | `3ba923155f4b` |
| `lib/src/main/res/values-pt/accessibility.xml` | TEXT | 7956 | 77 | `0a48574824ea` |
| `lib/src/main/res/values-pt/strings.xml` | TEXT | 721 | 12 | `f6e6e152a804` |
| `lib/src/main/res/values-ro/accessibility.xml` | TEXT | 8070 | 77 | `58152cd05aa2` |
| `lib/src/main/res/values-ro/strings.xml` | TEXT | 727 | 12 | `b4313a75415e` |
| `lib/src/main/res/values-ru/accessibility.xml` | TEXT | 9332 | 78 | `2ed6a872335c` |
| `lib/src/main/res/values-ru/strings.xml` | TEXT | 846 | 12 | `261ed48cdfe7` |
| `lib/src/main/res/values-sk/accessibility.xml` | TEXT | 8145 | 78 | `f37097ac5cd9` |
| `lib/src/main/res/values-sk/strings.xml` | TEXT | 736 | 12 | `e661858d8f09` |
| `lib/src/main/res/values-sl/accessibility.xml` | TEXT | 7899 | 78 | `6190ed05ea45` |
| `lib/src/main/res/values-sl/strings.xml` | TEXT | 711 | 12 | `5edc1e7313d1` |
| `lib/src/main/res/values-sr/accessibility.xml` | TEXT | 9204 | 77 | `824218c8c549` |
| `lib/src/main/res/values-sr/strings.xml` | TEXT | 780 | 12 | `125ad90322b2` |
| `lib/src/main/res/values-sv/accessibility.xml` | TEXT | 7621 | 76 | `afb4f62dbba9` |
| `lib/src/main/res/values-sv/strings.xml` | TEXT | 707 | 12 | `d2efcc3c7c96` |
| `lib/src/main/res/values-ta/accessibility.xml` | TEXT | 11056 | 76 | `ecf80fdd36d5` |
| `lib/src/main/res/values-ta/strings.xml` | TEXT | 953 | 12 | `c9ae8a3c9127` |
| `lib/src/main/res/values-th/accessibility.xml` | TEXT | 10221 | 75 | `d0486e0b66e0` |
| `lib/src/main/res/values-th/strings.xml` | TEXT | 900 | 12 | `ad62e210779b` |
| `lib/src/main/res/values-tk/accessibility.xml` | TEXT | 7845 | 76 | `1b200292dd30` |
| `lib/src/main/res/values-tk/strings.xml` | TEXT | 726 | 12 | `679e163e2153` |
| `lib/src/main/res/values-tr/accessibility.xml` | TEXT | 7808 | 76 | `df5bbeca3b5f` |
| `lib/src/main/res/values-tr/strings.xml` | TEXT | 722 | 12 | `fc5c769a0e36` |
| `lib/src/main/res/values-uk/accessibility.xml` | TEXT | 9441 | 78 | `243a74717302` |
| `lib/src/main/res/values-uk/strings.xml` | TEXT | 832 | 12 | `edf4ad157448` |
| `lib/src/main/res/values-vi/accessibility.xml` | TEXT | 8301 | 75 | `d2e0433322a1` |
| `lib/src/main/res/values-vi/strings.xml` | TEXT | 756 | 12 | `c434ab0973a4` |
| `lib/src/main/res/values-zh-rCN/accessibility.xml` | TEXT | 7416 | 75 | `9da30e0e5b63` |
| `lib/src/main/res/values-zh-rCN/strings.xml` | TEXT | 705 | 12 | `7d460b01917d` |
| `lib/src/main/res/values-zh-rHK/accessibility.xml` | TEXT | 7458 | 75 | `4d31e360d3bd` |
| `lib/src/main/res/values-zh-rHK/strings.xml` | TEXT | 705 | 12 | `43e570eea4fb` |
| `lib/src/main/res/values-zh-rTW/accessibility.xml` | TEXT | 7458 | 75 | `4d31e360d3bd` |
| `lib/src/main/res/values-zh-rTW/strings.xml` | TEXT | 705 | 12 | `43e570eea4fb` |
| `lib/src/main/res/values/accessibility.xml` | TEXT | 13642 | 171 | `7aece135db41` |
| `lib/src/main/res/values/strings.xml` | TEXT | 2085 | 32 | `fcf83176762e` |
| `lib/src/sharedTest/java/org/connectbot/terminal/EmojiSequenceTest.kt` | TEXT | 5246 | 123 | `2e4526d17a90` |
| `lib/src/sharedTest/java/org/connectbot/terminal/InlineImageRenderChecks.kt` | TEXT | 8602 | 176 | `364a604872a0` |
| `lib/src/sharedTest/java/org/connectbot/terminal/NativeBoundaryTest.kt` | TEXT | 19052 | 429 | `6ebb9c02f373` |
| `lib/src/sharedTest/java/org/connectbot/terminal/TerminalEmulatorIdlingResource.kt` | TEXT | 1199 | 36 | `7d5635833e40` |
| `lib/src/sharedTest/java/org/connectbot/terminal/TerminalInversionChecks.kt` | TEXT | 8775 | 185 | `d527aae18b3a` |
| `lib/src/sharedTest/java/org/connectbot/terminal/TerminalLifetimeChecks.kt` | TEXT | 11130 | 252 | `e542df10d90c` |
| `lib/src/sharedTest/java/org/connectbot/terminal/TerminalShapingChecks.kt` | TEXT | 29115 | 612 | `0fe601defc7d` |
| `lib/src/test/java/org/connectbot/terminal/AccessibilityBlankRowsTest.kt` | TEXT | 6697 | 141 | `3c258938bdbb` |
| `lib/src/test/java/org/connectbot/terminal/AccessibilityBoundsTest.kt` | TEXT | 1551 | 39 | `a61bcc4f821c` |
| `lib/src/test/java/org/connectbot/terminal/AccessibilityLinksAndTouchTest.kt` | TEXT | 14443 | 290 | `76f0ad186d02` |
| `lib/src/test/java/org/connectbot/terminal/AccessibilityOverlayTest.kt` | TEXT | 11870 | 217 | `1b18bcb25d5f` |
| `lib/src/test/java/org/connectbot/terminal/AccessibilitySessionStateTest.kt` | TEXT | 5237 | 105 | `3f4588c492af` |
| `lib/src/test/java/org/connectbot/terminal/BoundaryBenchmarkTest.kt` | TEXT | 4931 | 114 | `9ad81c3f370b` |
| `lib/src/test/java/org/connectbot/terminal/BoxDrawingGoldenTest.kt` | TEXT | 17758 | 339 | `52fda62b9a04` |
| `lib/src/test/java/org/connectbot/terminal/ClearLogsRule.kt` | TEXT | 496 | 20 | `8c849ffbb78e` |
| `lib/src/test/java/org/connectbot/terminal/ComplexScriptGoldenTest.kt` | TEXT | 9178 | 190 | `dd910c83a345` |
| `lib/src/test/java/org/connectbot/terminal/ComposeModeTest.kt` | TEXT | 5173 | 204 | `769032df2893` |
| `lib/src/test/java/org/connectbot/terminal/CursorAndModeEscapeTest.kt` | TEXT | 11747 | 279 | `9cff2da00fe0` |
| `lib/src/test/java/org/connectbot/terminal/GranularNavigationTest.kt` | TEXT | 1867 | 36 | `09010b1f8e95` |
| `lib/src/test/java/org/connectbot/terminal/HandleDragTest.kt` | TEXT | 7692 | 216 | `0bf19f848a5a` |
| `lib/src/test/java/org/connectbot/terminal/ImageDimensionsTest.kt` | TEXT | 4801 | 96 | `e375fd37025a` |
| `lib/src/test/java/org/connectbot/terminal/ImeExtractedTextMonitorTest.kt` | TEXT | 7764 | 185 | `92de37231f29` |
| `lib/src/test/java/org/connectbot/terminal/ImeInputViewTest.kt` | TEXT | 49828 | 1302 | `700233c0de1e` |
| `lib/src/test/java/org/connectbot/terminal/ImeLifecycleTest.kt` | TEXT | 4535 | 130 | `2b75e5e203e6` |
| `lib/src/test/java/org/connectbot/terminal/InlineImageBenchmarkTest.kt` | TEXT | 3090 | 77 | `3d38329dc6b2` |
| `lib/src/test/java/org/connectbot/terminal/InlineImageGoldenTest.kt` | TEXT | 3869 | 86 | `81a09dccce6f` |
| `lib/src/test/java/org/connectbot/terminal/InlineImageRendererTest.kt` | TEXT | 3258 | 72 | `9f00af0d6f78` |
| `lib/src/test/java/org/connectbot/terminal/InlineImageTest.kt` | TEXT | 43556 | 968 | `3dc190c5b573` |
| `lib/src/test/java/org/connectbot/terminal/KeyboardHandlerTest.kt` | TEXT | 39195 | 1032 | `71c2b5459b4b` |
| `lib/src/test/java/org/connectbot/terminal/MagnifierOffsetTest.kt` | TEXT | 5101 | 149 | `bcac501f4321` |
| `lib/src/test/java/org/connectbot/terminal/OscMalformedUtf8Test.kt` | TEXT | 8326 | 194 | `1d0b61d6c7a7` |
| `lib/src/test/java/org/connectbot/terminal/OscParserTest.kt` | TEXT | 19742 | 577 | `300f928ae884` |
| `lib/src/test/java/org/connectbot/terminal/OscSequenceTest.kt` | TEXT | 8724 | 265 | `3aef77f3c929` |
| `lib/src/test/java/org/connectbot/terminal/PipelineThreadingTest.kt` | TEXT | 6575 | 161 | `42528f35916c` |
| `lib/src/test/java/org/connectbot/terminal/ReadLastOutputTest.kt` | TEXT | 9083 | 291 | `53d30a21b52e` |
| `lib/src/test/java/org/connectbot/terminal/ReviewModeTest.kt` | TEXT | 4531 | 98 | `ff7f0a9f947e` |
| `lib/src/test/java/org/connectbot/terminal/ScrollbackClearTest.kt` | TEXT | 4400 | 118 | `1afc84d672b6` |
| `lib/src/test/java/org/connectbot/terminal/ScrollbackHistoryTest.kt` | TEXT | 2420 | 67 | `c4340ddec177` |
| `lib/src/test/java/org/connectbot/terminal/SelectionControllerTest.kt` | TEXT | 12301 | 391 | `0f88dc4c868a` |
| `lib/src/test/java/org/connectbot/terminal/SelectionManagerTest.kt` | TEXT | 30365 | 788 | `560a4994912c` |
| `lib/src/test/java/org/connectbot/terminal/SelectionTouchTest.kt` | TEXT | 8104 | 166 | `bdd1b61606dd` |
| `lib/src/test/java/org/connectbot/terminal/SemanticTypeTest.kt` | TEXT | 10987 | 302 | `4b36fada09ba` |
| `lib/src/test/java/org/connectbot/terminal/ShellIntegrationTest.kt` | TEXT | 15307 | 402 | `714d93569821` |
| `lib/src/test/java/org/connectbot/terminal/TerminalAccessibleOutputTest.kt` | TEXT | 2365 | 55 | `555fbeeb01f9` |
| `lib/src/test/java/org/connectbot/terminal/TerminalCursorContextTest.kt` | TEXT | 1525 | 54 | `c9098cd4bdc8` |
| `lib/src/test/java/org/connectbot/terminal/TerminalGestureTest.kt` | TEXT | 26978 | 666 | `328835a95f95` |
| `lib/src/test/java/org/connectbot/terminal/TerminalGlyphTest.kt` | TEXT | 5275 | 131 | `fe3dcb030d3e` |
| `lib/src/test/java/org/connectbot/terminal/TerminalInversionTest.kt` | TEXT | 456 | 16 | `8e6b682d74fb` |
| `lib/src/test/java/org/connectbot/terminal/TerminalLifetimeTest.kt` | TEXT | 3804 | 97 | `294eebacf975` |
| `lib/src/test/java/org/connectbot/terminal/TerminalMemoryTest.kt` | TEXT | 7037 | 163 | `68c611742aae` |
| `lib/src/test/java/org/connectbot/terminal/TerminalOutputJournalTest.kt` | TEXT | 3961 | 87 | `94b2a1837254` |
| `lib/src/test/java/org/connectbot/terminal/TerminalPageGestureTest.kt` | TEXT | 8235 | 221 | `b402ed07b00e` |
| `lib/src/test/java/org/connectbot/terminal/TerminalRendererGoldenTest.kt` | TEXT | 4567 | 105 | `bf9f51381548` |
| `lib/src/test/java/org/connectbot/terminal/TerminalReviewSelectionTest.kt` | TEXT | 5967 | 114 | `d8c6e14e88aa` |
| `lib/src/test/java/org/connectbot/terminal/TerminalReviewSessionTest.kt` | TEXT | 3165 | 83 | `c914d5eac82f` |
| `lib/src/test/java/org/connectbot/terminal/TerminalScreenStateScrollTest.kt` | TEXT | 21703 | 576 | `de04d4789d10` |
| `lib/src/test/java/org/connectbot/terminal/TerminalScreenStateUrlTest.kt` | TEXT | 12371 | 340 | `66d910b29015` |
| `lib/src/test/java/org/connectbot/terminal/TerminalShapingLegacyTest.kt` | TEXT | 807 | 26 | `c7d382e60fd1` |
| `lib/src/test/java/org/connectbot/terminal/TerminalShapingTest.kt` | TEXT | 452 | 16 | `def4538ed611` |
| `lib/src/test/java/org/connectbot/terminal/TerminalTextPaintTest.kt` | TEXT | 1591 | 44 | `fefc5b78a12e` |
| `lib/src/test/java/org/connectbot/terminal/TerminalUrlExtractionTest.kt` | TEXT | 5687 | 179 | `cb2e416ed6af` |
| `lib/src/test/resources/glyph-fixtures/Fallback.ttf` | BINARY | 788 | 5 | `e750fb9cac45` |
| `lib/src/test/resources/glyph-fixtures/Primary.ttf` | BINARY | 800 | 5 | `b8c7a8993936` |
| `lib/src/test/resources/robolectric.properties` | TEXT | 7 | 1 | `fda2c8e01718` |
| `lib/src/test/roborazzi/box-drawing-13.png` | BINARY | 40146 | 96 | `41cd722c65be` |
| `lib/src/test/roborazzi/box-drawing-16.png` | BINARY | 55859 | 95 | `e82a5072eefa` |
| `lib/src/test/roborazzi/box-drawing-21.png` | BINARY | 77141 | 107 | `76145d0971a5` |
| `lib/src/test/roborazzi/complex-script-boundaries.png` | BINARY | 53597 | 157 | `8ba9cfdac7ef` |
| `lib/src/test/roborazzi/inline-images-golden.png` | BINARY | 12647 | 40 | `b85a6091cc01` |
| `lib/src/test/roborazzi/shaping-arabic.png` | BINARY | 29868 | 123 | `a9e2287923e0` |
| `lib/src/test/roborazzi/shaping-bengali.png` | BINARY | 22105 | 66 | `47d8b0390f55` |
| `lib/src/test/roborazzi/shaping-devanagari.png` | BINARY | 24007 | 80 | `620d7fb9afad` |
| `lib/src/test/roborazzi/shaping-gujarati.png` | BINARY | 23251 | 76 | `b949ccb64511` |
| `lib/src/test/roborazzi/shaping-gurmukhi.png` | BINARY | 20050 | 96 | `75e5ed369847` |
| `lib/src/test/roborazzi/shaping-kannada.png` | BINARY | 22478 | 71 | `745d30996e9c` |
| `lib/src/test/roborazzi/shaping-khmer.png` | BINARY | 22270 | 66 | `d757b58b3e6b` |
| `lib/src/test/roborazzi/shaping-lao.png` | BINARY | 21619 | 70 | `fd9a40c935a3` |
| `lib/src/test/roborazzi/shaping-malayalam.png` | BINARY | 23749 | 72 | `47feb19b9a66` |
| `lib/src/test/roborazzi/shaping-myanmar.png` | BINARY | 24053 | 79 | `5dd1b4252f42` |
| `lib/src/test/roborazzi/shaping-nko.png` | BINARY | 21803 | 68 | `9c286ae7b54e` |
| `lib/src/test/roborazzi/shaping-odia.png` | BINARY | 21552 | 66 | `71490089d828` |
| `lib/src/test/roborazzi/shaping-sinhala.png` | BINARY | 23434 | 88 | `e5319f766d52` |
| `lib/src/test/roborazzi/shaping-syriac.png` | BINARY | 24504 | 52 | `466ed5c86d09` |
| `lib/src/test/roborazzi/shaping-tamil.png` | BINARY | 21142 | 72 | `3866b10c64ea` |
| `lib/src/test/roborazzi/shaping-telugu.png` | BINARY | 21732 | 92 | `0d80acdba4df` |
| `lib/src/test/roborazzi/shaping-thaana.png` | BINARY | 22149 | 79 | `69cf1fb5b68b` |
| `lib/src/test/roborazzi/shaping-thai.png` | BINARY | 22628 | 61 | `7b6edc774bde` |
| `lib/src/test/roborazzi/shaping-tibetan.png` | BINARY | 23430 | 50 | `57465b2789a8` |
| `lib/src/test/roborazzi/terminal-renderer-golden.png` | BINARY | 37038 | 131 | `4799786c95f2` |
| `macrobenchmark/build.gradle.kts` | TEXT | 1261 | 43 | `ee76a74c77a6` |
| `macrobenchmark/src/main/java/org/connectbot/terminal/benchmark/TerminalFrameBenchmark.kt` | TEXT | 4273 | 98 | `74ffaba0ff62` |
| `mise.toml` | TEXT | 25 | 2 | `860d6776d470` |
| `renovate.json5` | TEXT | 117 | 6 | `55dfe6c56b39` |
| `settings.gradle.kts` | TEXT | 376 | 21 | `2982ef538654` |
| `test-app/build.gradle.kts` | TEXT | 2136 | 78 | `752deb4669b3` |
| `test-app/proguard-rules.pro` | TEXT | 337 | 11 | `26377eab265f` |
| `test-app/src/benchmark/AndroidManifest.xml` | TEXT | 248 | 6 | `254242bdee07` |
| `test-app/src/benchmark/java/org/connectbot/terminal/testapp/FrameBenchmarkActivity.kt` | TEXT | 11732 | 240 | `de5ec75100e2` |
| `test-app/src/main/AndroidManifest.xml` | TEXT | 751 | 21 | `1267a21b2c1a` |
| `test-app/src/main/assets/fonts/0xProtoNerdFontMono-Regular.ttf` | BINARY | 2412316 | 30586 | `e98bf52027a5` |
| `test-app/src/main/java/org/connectbot/terminal/testapp/Icons.kt` | TEXT | 6335 | 173 | `9e47707243b0` |
| `test-app/src/main/java/org/connectbot/terminal/testapp/MainActivity.kt` | TEXT | 1467 | 42 | `31738bf34e5b` |
| `test-app/src/main/java/org/connectbot/terminal/testapp/ShellScreen.kt` | TEXT | 31767 | 744 | `4ee1e472c821` |
| `test-app/src/main/java/org/connectbot/terminal/testapp/ui/theme/Theme.kt` | TEXT | 688 | 23 | `174871865edf` |
| `test-app/src/main/res/drawable/ic_launcher_foreground.xml` | TEXT | 445 | 13 | `ca8e51bb7eaf` |
| `test-app/src/main/res/mipmap-anydpi-v26/ic_launcher.xml` | TEXT | 268 | 5 | `5f3b1ed2282a` |
| `test-app/src/main/res/raw/.placeholder` | TEXT | 0 | 0 | `e3b0c44298fc` |
| `test-app/src/main/res/raw/test_attributes.txt` | TEXT | 872 | 27 | `76210ab69379` |
| `test-app/src/main/res/raw/test_output.txt` | TEXT | 8273 | 47 | `c47a227a43c9` |
| `test-app/src/main/res/raw/test_scroll.txt` | TEXT | 5249 | 102 | `0417fd94f66a` |
| `test-app/src/main/res/raw/test_unicode.txt` | TEXT | 1604 | 47 | `b2078e0b9ad0` |
| `test-app/src/main/res/values/colors.xml` | TEXT | 121 | 4 | `b54bc246c11f` |
| `test-app/src/main/res/values/strings.xml` | TEXT | 138 | 4 | `fad451ad474e` |
| `test-app/src/main/res/values/themes.xml` | TEXT | 156 | 4 | `a9a537ce95ba` |
| `tools/generate-font-fixtures.py` | TEXT | 1844 | 44 | `cff59bc88ffc` |
| `tools/generate-unicode.py` | TEXT | 4492 | 110 | `b307caf9fc78` |
| `tools/talkback-touch-bridge.py` | TEXT | 5371 | 118 | `46b15ea6fc47` |
| `tools/unicode/17.0.0/emoji-data.txt` | TEXT | 107324 | 1288 | `2cb2bb9455cd` |
| `tools/unicode/17.0.0/emoji-sequences.txt` | TEXT | 194868 | 1514 | `12cc8267dc33` |
| `tools/unicode/17.0.0/emoji-variation-sequences.txt` | TEXT | 38315 | 757 | `bb3d09ef03f2` |
| `tools/unicode/17.0.0/emoji-zwj-sequences.txt` | TEXT | 277215 | 1675 | `5b25441daed2` |

## 复算命令

```sh
git clone --depth 1 https://github.com/connectbot/termlib /tmp/opencode/external/termlib
git -C /tmp/opencode/external/termlib ls-files -z | xargs -0 sha256sum
git -C /tmp/opencode/external/termlib ls-files | wc -l
```
