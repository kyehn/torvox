# 参考

本文件记录外部参考项目的**已核实**事实。引用前必须先读参考源码确认，不得凭印象扩写。与本仓实现不一致之处已在下文逐条标注取舍理由。

## termux-app

- 文本选择控制器 `terminal-view/.../textselection/TextSelectionCursorController.java`：状态是固定起点 `mSelX1/mSelY1` 加拖动端 `mSelX2/mSelY2`（`:30`），**没有锚点对象，也没有越过时的归属权交换**；`getSelectors`（`:362-371`）与 `getSelectedText`（`:374-376`）把坐标原样传给模拟器，由模拟器处理反向选择。抓取起点手柄时把起点重锚到原终点（`:240` `mSelX1 = mSelX2;`）。手柄拖动没有状态机，只有 `TextSelectionHandleView.java:30` 的 `mIsDragging` 与 `:291-319` 的 `MotionEvent` 分派。**不要把「锚点交叉翻转」记在 termux 名下**，那是 termlib 的特性。
- 宽字符吸附 `getValidCurX`（`TextSelectionCursorController.java:307-336`）：累加 `WcWidth.width` 判定落点，命中宽字符中段时返回该字符右边界，双侧手柄均应用（`:261`、`:301`）。
- 选择菜单不遮挡的实现是 `onGetContentRect`（`:194-212`）：x 为列乘字体宽度并在 `x1 > x2` 时交换左右（`:200-204`），y 为 `(行 - 1 - getTopRow())` 乘行高（`:197`），上下边界再加手柄高度并按终端底部钳制（`:206-210`）。注意它与实际定位手柄的 `TerminalView.getPointY`（`TerminalView.java:1062-1064`，`(cy - mTopRow) * fontLineSpacing`）差一行，本仓按后者对齐。
- 菜单在拖动时隐藏、抬手后恢复：`TerminalView.java:1507-1518` `updateFloatingToolbarVisibility`，MOVE 隐藏、UP/CANCEL 恢复，延时取 `ViewConfiguration.getDoubleTapTimeout()`（`:1494`）。
- 300ms 防误关在控制器的 `hide()`（`TextSelectionCursorController.java:57-64`），比较 `mShowStartTime`；手柄 `PopupWindow`（`TextSelectionHandleView.java:23`、`:69-70`、`:134`，`TYPE_APPLICATION_SUB_PANEL`）本身无计时保护。
- `WcWidth`（`terminal-emulator/.../WcWidth.java`）：文件头声明 `Implementation of wcwidth(3) for Unicode 15`（`:3-4`），但树内无版本常量与生成脚本，另有同步告警（`:8-12`）。入口 `width(int)`（`:514`）与 `width(char[], int)`（`:536`），对不可打印字符返回 0（`:5`）。
- `TextStyle` 64 位打包（`TextStyle.java:5-6`）：16 标志位（已用 11）+ 24 位前景 + 24 位背景（`:9-13`），编码器 `encode(int,int,int)`（`:51-56`）；`TerminalRow.java:49` 每列一个 `long`。
- 取选择文本按 wrap 感知拼接：`TerminalBuffer.java:167-168` 只在「不 join 且非 wrap 行」时补 `\n`；三模式 `joinBackLines`（`:121`）/`joinFullLines`（`:125`）/`getTranscriptTextWithFullLinesJoined`（`:113-115`）；wrap 行保留尾随空格（`:151-154`）。
- 列转 char 走宽字符换算：`TerminalRow.java:94-110` `findStartOfColumn`，`TerminalBuffer.java:144-147` 选中宽字符起点时跳到 `findStartOfColumn(x2 + 1)`。
- 长按非空白位置扩展为整个空白分隔词：`setInitialTextSelectionPosition`（`:93-107`）；`getWordAtLocation`（`TerminalBuffer.java:173-198`）先走完整个 wrap 行。
- 会话抽屉行格式：`TermuxSessionsListViewController.java:71` `"[" + (position+1) + "] "`，`:77` 粗体 span 覆盖编号加会话名，`:78` 换行后斜体 span 覆盖终端标题；另有死亡会话删除线（`:85-88`）与非零退出码红色（`:90`）。点击切换并关闭抽屉：`:98` `setCurrentSession(...)`、`:99` `closeDrawers()`。
- 会话关闭**不是**抽屉行内操作：走死亡会话回车（`TermuxTerminalViewClient.java:244-246`、`:463-465`）与前台服务通知。
- **长按重命名存在**：`TermuxSessionsListViewController.java:103-107` `onItemLongClick` 调 `renameSession`（`TermuxTerminalSessionActivityClient.java:344-355` 弹 `TextInputDialogUtils.textInput`），键盘入口 `TermuxTerminalViewClient.java:265`、`:485`。本仓不实现重命名是产品取舍，不是「termux 没有」。
- 反例：termux-app 的 CPU 渲染路径是**按 run 批量**而非逐格——`TerminalRenderer.java:260` `canvas.drawTextRun`（API 23+），`:262` 退回按 run 的 `drawText`；run 在样式/光标/选择变化处断开（`:137`），并在实测宽度与 wcwidth 不符时按 run 缩放校正（`:133-135`、`:217-223`）。可借鉴的是 run 批处理与逐 run 宽度校正。
- 反例：termux-app **没有** OSC 8 超链接、全选、打开文件、终端内搜索、脏跟踪的任何实现。声称「参考 Termux」的这些能力在 termux-app 中无对应物，不得据此推断。
  - 反例：termux-app 不使用 `LD_PRELOAD` 做任何重定向（全仓无此字样），也不存在 `termux-exec` 这类程序。

## ghostty-android-terminal

- 选择状态由模拟器拥有：`term/TerminalEmulator.java:193-196`「The terminal owns it (tracked refs)」、`cpp/terminal_jni.c:1460`；视图只保存手势与工具栏状态。多击计数 `ui/TerminalView.java:1071` `tapCount = continues ? tapCount + 1 : 1;`。
- 菜单锚定 `ActionMode.Callback2`（`ui/TerminalView.java:1438`）+ `onGetContentRect`（`:1484`）。
- 几何缓存键 `selectionGeometryKey`（`ui/TerminalView.java:1172`），每坐标 12 位，`Long.MIN_VALUE` 作哨兵。
- 边缘滚动 `ui/TerminalView.java:1267-1275` `dragSelectionTo` 每次移动一行。
- 初始 winsize 在 `fork()` 之前写入像素字段：`cpp/pty_jni.c:102-106` 设 `ws_xpixel = cols * cell_w`、`ws_ypixel = rows * cell_h`，`:106` `ioctl`，`:113` 才 `fork`；resize 同字段在 `:348-352`。
- PTY 摄取前剥离 NUL 并走 `memchr` 快路径：`cpp/terminal_jni.c:325` `if (len > 0 && memchr(bytes, 0, (size_t)len))`，命中才分配拷贝（`:326-333`），否则无拷贝直送（`:322-323`、`:335`）。
  - 本仓**有意不同**：ghostty 的 kitty 图像存储对 NUL 敏感，本仓不剥离（`native/src/terminal/ghostty_terminal/tests.rs:2032-2044` 断言孤立 NUL 不影响存储），三处剥离均为标量循环（`native/src/terminal/session.rs:346-349`、`native/src/terminal/ghostty_terminal/public_api.rs:148-152`、`:180-183`），无 `memchr`。改动前必须重跑该测试。
- 搜索覆盖层不改终端尺寸以免 `SIGWINCH`：`ui/SearchBarView.java:35-37` 注释自述；防抖 `:54` `DEBOUNCE_MS = 150`；高亮复用 selection 槽位 `term/TerminalEmulator.java:249-250`。
  - 本仓**有意不同**：高亮走独立覆盖层颜色（`android/.../ui/SearchHighlightColors.kt`），不复用选择槽位。
- 对照：`ui/TerminalFontStore.java:22-25` 四槽 `DEFAULT=0 / ITALIC=1 / BOLD=2 / BOLD_ITALIC=3`。本仓**有意只有一槽**（`DESIGN.md` 字体一节：只有一项主字体选择）。

## Haven

- `cursorKeyAppMode` 跟踪 DECCKM：`feature/terminal/.../MouseModeTracker.kt:57-59`、`:66` `private const val DECCKM = 1`、`:148-150` 状态机解析。
- alt 屏滑动转方向键时按应用光标模式区分 SS3/CSI：`feature/terminal/.../TerminalScreen.kt:2441-2443` `val prefix = if (appMode) "\u001bO" else "\u001b["`，按住滑动版本 `:2450-2462`；测试 `SwipeArrowsTest.kt:39`。
  - 只可借鉴**编码分支**。`MouseModeTracker` 是 Haven 自写的 VT 状态机，从原始 PTY 字节解析模式位，与 `DESIGN.md`「以 Ghostty 作为终端状态单一来源、不重复实现 Ghostty 已有功能」冲突，不得引入。
- Popup 内 `startActionMode(TYPE_FLOATING)` 静默 no-op：`feature/terminal/.../FloatingTextInputDialog.kt:218-220` 注释（Popup 窗口无真实 DecorView）。同文件 `:200-214` 记录其选择菜单路径依赖 Compose 版本开关，本仓不得照搬版本相关分支。

## termlib

- `applyHandleDrag` 实现锚点语义加交叉翻转：`lib/src/main/java/org/connectbot/terminal/Terminal.kt:2183`，`:2170-2172` 注释说明手柄越过锚点后归属权翻转且锚点恢复到交叉前列，避免跳到拖动列；测试 `HandleDragTest.kt:57`、`:157`、`:199`。
- 多行反向判定：`SelectionManager.kt:106-140` `contains` 与 `getStartPosition`/`getEndPosition` 按 `startRow < endRow` 分支；`:474-482` 注释记录 `minOf/maxOf` 在「向下且向左」拖拽时与高亮不一致。
- resize 钳制选择：`SelectionManager.kt:375` `clampToDimensions(rows, cols)`，**须调用方主动调用**，`TerminalEmulator.resize` 不会自动执行。
- URL 尾随标点修剪：`UrlDetection.kt:20-30` 混合策略——非括号字符用朴素集合 `TRAILING_DETECTED_URL_PUNCTUATION = setOf('.', ',', ';', ':', '!')`（`:9`），`)` 与 `]` 走括号配对计数 `countOpenLessThanClose`（`:32-43`）。「括号配对计数」只对括号成立，不是全字符级。
- `TerminalInputConnection`：`ImeInputView.kt:293-296`；IME 显示控制 `:341-370` `getExtractedText`/`buildExtractedText`、`:308-314` `GET_EXTRACTED_TEXT_MONITOR`、`:325-339` `beginBatchEdit` 深度计数。修饰键掩码在 `KeyboardHandler.kt:92-94`、`:414-418`，由连接查询。
  - 反例：termlib 用 `TYPE_NULL` 与 `TYPE_TEXT_VARIATION_PASSWORD` / `VISIBLE_PASSWORD` 隐藏数字行键盘（`ImeInputView.kt:193-198`），并按是否按住 Ctrl/Alt 在 `ImeShortcutInputMode.DISABLED / TYPE_NULL / FORCE_ASCII`（`ImeShortcutInputMode.kt:20-29`）间切换编辑器类型。这些都限制输入法特性，与 `DESIGN.md`「支持全功能输入法（不限制输入法特性）」冲突。termlib 并**无**面向用户的「普通/密码」开关，不要这样标注。

## zed-android-port

- 前台进程组经 `tcgetpgrp` 获取：`crates/terminal/src/pty_info.rs:42-44`；`handle` 取自 `pty.file().as_raw_fd()`（`:31-36`）。返回 0 表示 PTY 尚未设置前台组。
- 回退到 shell 子进程并读 name/cwd/argv：`pty_info.rs:33` `fallback_pid = pty.child().id()`、`:171-182` `load()`、`:82-86` `ProcessInfo`、`:98-100` 刷新策略。注意 `get_child()`（`:137-140`）只被 `kill_child_process` 使用，展示路径的回退是间接的。
- kill 先 `killpg` 前台组再 `kill` shell：`crates/terminal/src/terminal.rs:2281-2283` 注释与调用顺序；`pty_info.rs:142-149` `killpg(SIGKILL)`、`:155-156` `kill_child_process`。

## wgpu-in-app

- `get_current_texture` 处理：参考项目是 **4 个 match 臂**（`app-surface/src/lib.rs:222-236`）——`Success｜Suboptimal` 直用（`:223-224`）；`Timeout｜Outdated｜Lost` 重新 configure 并**重试一次**（`:225-231`）；`Occluded => return None`（`:235`）；`Validation => panic!`（`:236`）。**不是五分支。**
- wgpu 30 的对应枚举是 `CurrentSurfaceTexture`（`wgpu/src/api/surface_texture.rs:48-78`），共 7 个变体 `Success`(`:50`) / `Suboptimal`(`:53`) / `Timeout`(`:57`) / `Occluded`(`:62`) / `Outdated`(`:66`) / `Lost`(`:68`) / `Validation`(`:78`)；不存在 `AcquireError` 类型，也没有 `Surface::acquire`。
- `Occluded` 在 Android/Vulkan 上不可达：唯一产出点是 Metal 后端（`wgpu-hal/src/metal/surface.rs:368`）；Vulkan swapchain 只映射 `TIMEOUT` 与 `ERROR_SURFACE_LOST_KHR`（`wgpu-hal/src/vulkan/swapchain/native.rs:489-493`），另一处只映射 `ERROR_OUT_OF_DATE_KHR` / `ERROR_SURFACE_LOST_KHR`（`:617-618`）。**不要为它设计分支。**
- Android `view_formats` 取单格式：`app-surface/src/lib.rs:329-339` Android 分支 `vec![format]`（`:339`）。仅 webgl 分支为空。
  - 本仓**有意相反**：`native/src/render/context.rs:491` `view_formats: vec![]`（平台缺少 `SURFACE_VIEW_FORMATS` downlevel flag，非空列表会导致 configure 失败），并取首个非 sRGB 格式（`:475-483`）而非 `caps.formats[0]`。注释中的平台矩阵描述须与参考源码一致。
- `ANativeWindow` 引用计数 RAII：`app-surface/src/android.rs:51` `Arc<Mutex<*mut ndk_sys::ANativeWindow>>`，`:90-93` `impl Drop for NativeWindow` 调 `ANativeWindow_release`；`:64` 注释说明 `ANativeWindow_fromSurface` 会 `+1` 引用计数。
  - 本仓未做 RAII：`native/src/render/context.rs:380-384` 依赖调用方保证存活，释放是手动 FFI 调用（`native/src/android/ffi.rs:116` 声明、`:1526-1528` 调用）。若改为 RAII 须同步改 JNI 侧所有权约定。
- 零尺寸钳制：`app-surface/src/lib.rs:71-73` `normalize_view_size`（`max(1)`），单测 `:451`。
- `resize` 幂等：`app-surface/src/lib.rs:138-140` 尺寸相同直接 `return false`，单测 `:474`。
- 反例：JNI 导出用 `jni_fn` 宏——`wgpu-in-app/src/ffi/android.rs:6`、`:11`、`:22`、`:29`、`:37`，依赖声明 `Cargo.toml:40`。
- 反例：参考项目**用** `Box::into_raw` 裸指针传递 JNI 句柄（`wgpu-in-app/src/ffi/android.rs:18`，恢复 `:24`/`:32`，释放 `:39`；iOS 侧 `src/ffi/ios.rs:17`），没有全局注册表。本仓用全局注册表（`native/src/android/ffi.rs:139` `SESSION_REGISTRY`、`:319` `REQUEST_REGISTRY`），**本仓在此项上领先参考项目，不得反向对齐**。

## zelland

- surface 就绪竞态用 `PENDING_SIZE` 独立存尺寸：`src-tauri/src/renderer/mod.rs:112`，写入 `:114-115`（调用方 `renderer/android.rs:90`），消费 `:320-321`；理由见 `WGPU_FIXES.md:243`、`:274`。
  - 本仓有等价机制但分层不同：`android/.../runtime/TerminalRuntime.kt:282-286` 的 `pendingSurface` / `pendingSurfaceWidth` / `pendingSurfaceHeight`，设置于 `:2095-2097`、`:3501-3502`，读取于 `:3238-3239`；零尺寸推迟在 `ui/TerminalSurface.kt:2947`。
- 鼠标映射须用实时 cell 尺寸：`src-tauri/src/terminal.rs:105-108` 注释，编译期常量仅作回退；`renderer/mod.rs:1094-1097` `get_cell_size()`。

## termux-kotlin-app

- 字符串转 argv 的四态机 `ArgumentTokenizer`：`termux-shared/.../shell/ArgumentTokenizer.kt:51-54` `NO_TOKEN_STATE=0 / NORMAL_TOKEN_STATE=1 / SINGLE_QUOTE_STATE=2 / DOUBLE_QUOTE_STATE=3`，`:84-88` 消费。
  - **来源不是 BSD**：文件头 `:1-39` 是 DrJava / JavaPLT（Rice University）Apache-2.0 版权块，类注释同源。这是 termux-kotlin-app 对 termux-shared 的再实现。
  - **用途不相关**：唯一调用方是 `AmSocketServer.kt`，与终端启动路径无关。本仓若拆分 Shell 启动参数必须自行声明调用点（`DESIGN.md` Shell 启动入口一节）。
- 该仓库 `terminal-view/` 与 `terminal-emulator/` **没有任何终端内搜索实现**。唯一匹配高亮是命令面板的 `app/.../commandpalette/CommandPalette.kt:314`、`:331` `highlightMatches`，无匹配计数、无当前/定位区分。搜索设计只能参考 ghostty-android-terminal。

## console

- 收窄搜索用 `contains` 而非 `startsWith`：`src/kgx-tab.c:234-235` `g_strrstr (priv->last_search, search)`，理由在 `:218-233`。
  - **该技巧是 VTE 专属**：同处 `:218-219` 自述根因是「VTE doesn't automatically highlight the search match and doesn't have an API to do that」。本仓以 Ghostty 为状态源，这类缺陷不会出现；引用仅作意图参考，不得当作可移植技术。
- `Copy` 无选择时置灰而非隐藏：`src/kgx-terminal.c:705-709` `gtk_widget_action_set_enabled(..., "term.copy", vte_terminal_get_has_selection(...))`，注册于 `:914`，快捷键 `<shift><primary>c` 见 `src/kgx-application.c:176-177`。

## PTY 与进程

- fork 前预构建 `CStrings`、`child` 只做 AS-safe 调用。本仓边界声明见 `native/src/terminal/pty.rs:1-2`。
- spawn 前先应用初始 winsize。本仓当前在 spawn 时播种 `ws_xpixel: 0, ws_ypixel: 0`（`native/src/terminal/pty.rs:152-157`），改在每次 grid resize 后推送真实像素（`android/.../ui/TerminalSurface.kt:463` → `bridge/Bridge.kt:193`）。这与 ghostty-android-terminal 的「fork 前写像素」不同，是本仓的取舍：首帧布局未完成时写估计像素会让 shell 拿到错误宽度。

## 输入与 IME

- IME composing 增量 diff 同步，避免全量重设。
- `InputEvent` 触摸状态机与多会话委托函数集为 `host` 可测纯逻辑。

## 文本与字体

- CJK/emoji 分类边界测试对照字体分类，防区间过度扩张致 `tofu`。
- `WcWidth` 按 Unicode 15 判定宽字符（见 termux-app 一节，树内无版本常量，改表需同步上游）。

## 修饰键栏

- `DESIGN.md` 修饰键栏一节的 2 行 7 列、粘滞键、左滑进入输入框、不与全面屏手势冲突四项，**在 termux-app 与 ghostty-android-terminal 中均无直接对应实现**。默认布局以 termux-app 的 `res/xml/shortcuts.xml` 之外的实际键位表为准，实现前必须先读参考源码确认，不得凭空推断。
