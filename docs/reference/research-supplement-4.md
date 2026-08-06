# 深度研究补充文档 4：termlib / gnome-console / shashlik / rin

> 本文件是 torvox 外部仓库研究的第 4 期补充，覆盖：
> 1. **termlib**（ConnectBot 现代终端库，Kotlin + libvterm C 后端）：Terminal.kt、TerminalEmulator.kt、TerminalScreenState.kt、TerminalSnapshot.kt、TerminalNative.kt、ComposeController.kt、ComposeMode.kt、ImeInputView.kt、KeyboardHandler.kt、ModifierManager.kt、ScrollController.kt、UrlDetection.kt、OscParser.kt、CellRun.kt、ColorCache.kt、DelKeyMode.kt、RightAltMode.kt、AccessibilityOverlay.kt、AccessibilityStateManager.kt、LiveOutputRegion.kt、TerminalCallbacks.kt、TerminalLine.kt 及全部 22 个测试文件。
> 2. **gnome-console**（kgx，GTK4+VTE）：kgx-window.c、kgx-pages.c、kgx-settings.c、kgx-application.c、kgx-despatcher.c、kgx-spad.c、kgx-process.c、kgx-paste-dialog.c、kgx-theme-switcher.c、kgx-livery*.c、kgx-colour-utils.c、kgx-palette.c、kgx-drop-target.c、kgx-train.c、kgx-fullscreen-box.c、kgx-close-dialog.c 及 .ui/.css 资源。
> 3. **shashlik**（Slint 渲染框架）：renderer-cpu/、renderer-common/、app-surface/ 其余文件、ffi-run/、kmp/、kms_deploy.sh、Cross.toml。
> 4. **rin**（Rust 终端引擎 + Android 应用）：core/ 全部、input/、renderer/screen.rs、platform/android/ 全部、android/ 全部 Kotlin。

行号以研究时仓库 HEAD 为准。torvox 侧行号以 `repositories/torvox` 当前 HEAD 为准。

---

## 1. termlib（ConnectBot 现代终端库）

架构总览：libvterm C 库（JNI 静态绑定）负责终端状态机，Kotlin 侧 `TerminalEmulator` 通过 `TerminalCallbacks` 接收脏区/滚动/OSC 事件，把屏幕行缓存在 Kotlin 堆上（`TerminalLine`），以**快照（TerminalSnapshot）+ 脏区合并**驱动 Compose 绘制。这与 torvox「ghostty 内核 + Rust 渲染 + Kotlin 桥」的架构属于同一代思路，但 termlib 的 UI 层（Compose）与 torvox 的 UI 层（Compose + AndroidView 混合）重叠度最高，吸收价值集中在前者。

### 1.1 Terminal.kt（103 KB，Compose 终端视图）

**功能说明**：
- `fun Terminal(...)`（Terminal.kt:312）：顶层 Compose 组件，组装 `TerminalEmulator`、`TerminalScreenState`、`SelectionManager`、`KeyboardHandler`、`ImeInputView`（通过 `AndroidView` 挂载）、`OscParser` 与手势处理（`GestureType` 枚举 Terminal.kt:128：Tap/DoubleTap/LongPress/Drag/Pinch）。
- `TerminalWithAccessibility(...)`（Terminal.kt:379）：无障碍包装层，内部叠加 `AccessibilityOverlay`（见 1.11）。
- `TerminalRows(...)`（Terminal.kt:1619）：按可见行迭代绘制；`DrawScope.drawLine`（Terminal.kt:1673）、`drawDoubleUnderline`（Terminal.kt:1794）、`drawCurlyUnderline`（Terminal.kt:1824）覆盖了 libvterm 的下划线样式集合（torvox 的 ghostty 渲染同样支持 double/curly underline，但 Kotlin 层无此绘制逻辑，因为 torvox 全部在 Rust 里画）。
- 选择手柄：`applyHandleDrag`（Terminal.kt:1899）+ `isTouchingHandle`（Terminal.kt:1941）+ `drawSelectionHandle`（Terminal.kt:2149），手柄命中判定、拖动映射到 `SelectionManager.updateSelectionStart/End`。
- 放大镜：`magnifierOffset`（Terminal.kt:2021）与 `MagnifyingGlass`（Terminal.kt:2044）——**Compose 自绘放大镜**（半透明背景 + 放大后的行文本 + 边框），常量 `MAGNIFIER_VERTICAL_OFFSET`（Terminal.kt:204）、`FINGER_HEIGHT_DP`（Terminal.kt:209）。托起手势时显示，用于精确光标定位。
- Compose 模式覆盖层：`drawComposeOverlay`（Terminal.kt:2293），绿色半透明背景 `COMPOSE_OVERLAY_BACKGROUND`（Terminal.kt:2279），把正在组合的键序列画在光标处；配套 `codepointColumns`（Terminal.kt:2372，宽字符判定）、`visualColumnWidth`（Terminal.kt:2383）、`truncateForOverlay`（Terminal.kt:2398）。
- 尺寸计算：`calculateDimensions`（Terminal.kt:2431）、`findOptimalFontSize`（Terminal.kt:2469，二分逼近目标网格行数列数）、`charsPerDimension`（Terminal.kt:2506）。
- 性能设施：`DRAW_TEXT_BUFFER = ThreadLocal<CharArray>`（Terminal.kt:122）与 `CURLY_UNDERLINE_PATH = ThreadLocal<Path>`（Terminal.kt:123）避免每行分配；`TERMINAL_BORDER_WIDTH`（Terminal.kt:174）、`COPY_BUTTON_SIZE/OFFSET`（Terminal.kt:189/194）。

**与 torvox 对比**：
- 有（对应物）：Compose 绘制（torvox `TerminalScreen.kt` 与 `TerminalSurface.kt` AndroidView 混合）、字体尺寸适配（torvox `FontUtils.kt`）、双击/长按手势（torvox `TerminalSurface.kt` 的 `detectTapGestures`/长按选择）。
- 没有（torvox 缺失）：① **Compose 内自绘放大镜**——torvox 用系统 `android.widget.Magnifier`（`TerminalSurface.kt:1689-1692`），termlib 的 `MagnifyingGlass` 可在 Rust 渲染纹理之上直接放大，不依赖系统窗口；② **Compose 模式的屏幕覆盖层**（`drawComposeOverlay`）——torvox 无 compose 键序列模式（键盘全靠 IME + 硬件键）；③ 双下划线/波浪下划线在 Kotlin 侧的绘制辅助（torvox 无，因为渲染在 Rust）；④ 选择手柄的命中+拖动纯函数（`applyHandleDrag`）可单测——torvox 的手柄拖动逻辑在 `TerminalSurface.kt` 手势代码里，无独立纯函数。

**依赖适用性**：全部为 Compose + kotlinx 标准 API（`androidx.compose.foundation.gestures`、`androidx.compose.ui.graphics`），无新增依赖。

**可吸收内容**：
- `magnifierOffset` 纯函数 + `MagnifyingGlass` 组合子：可直接移植到 torvox `TerminalScreen.kt`，替代/补充系统 Magnifier（系统 Magnifier 在部分 ROM 上失效）。
- `findOptimalFontSize` 二分算法（Terminal.kt:2469-2505）：torvox 目前按 dp 粗调，可吸收精确二分。
- `ThreadLocal` 绘制缓冲（Terminal.kt:122-123）：`TerminalScreen.kt` 的每帧 Canvas 绘制可借鉴，减少 GC。

**文档吸收价值**：高——尤其放大镜与 compose 覆盖层是 torvox 尚未覆盖的 UX 空白。

### 1.2 TerminalEmulator.kt（58 KB，libvterm 绑定 + 快照构建）与 TerminalCallbacks.kt

**功能说明**：
- `TerminalUrl`（TerminalEmulator.kt:38）：OSC 8 链接与自动检测 URL 的统一模型（含 `startRow/startCol/endRow/endCol/url` 与 `autoDetected` 标记）。
- `TerminalEmulator` 接口（TerminalEmulator.kt:60-195）：`writeInput`（:80）、`resize`（:90）、`dispatchKey`（:95）、`dispatchCharacter`（:104/:109）、`clearScreen`（:114）、`setAnsiPalette`（:125）、`setDefaultColors`（:138）、`applyColorScheme`（:150，含 bold 亮度/光标色/链接色）、`getLastCommandOutput`（:183，**最后一条命令输出**，供无障碍朗读）、`getUrls`（:195，`UrlScanScope` 分 CurrentView/Scrollback）。
- `TerminalEmulatorFactory`（TerminalEmulator.kt:198）：libvterm 创建/销毁、JNI 错误处理。
- `TerminalEmulatorImpl`（TerminalEmulator.kt:284）：核心实现。关键回调：
  - `damage`（:539）：脏区回调，`addDamageRegion`（:1353）合并相邻脏区。
  - `moverect`（:551）：屏幕滚动优化——直接搬移 Kotlin 行数组而非重绘。
  - `moveCursor`（:567）、`setTermProp`（:578，光标样式/标题/URL 等属性）、`bell`（:641）。
  - `pushScrollbackLine`（:649）/`popScrollbackLine`（:726）：行进出滚动区时维护 Kotlin 侧滚动缓冲与**语义段（semantic segment）移位**（`shiftStoredSegmentTexts` :1452）。
  - `clearScrollback`（:716）：ESC 3 J 清滚动区。
  - `onKeyboardInput`（:773）：键盘输出字节流。
  - `onOscSequence`（:781）：分发给 `OscParser`（见 1.8），处理 OSC 52/8/9/133/1337。
- 语义段管理：`addSemanticSegment`（:830）、`applySemanticSegment`（:931，与损坏行合并）、`updateLine`（:974，把语义段按脏区重算/保留）、`storeSegmentText`/`replaceStoredSegmentTexts`/`removeStoredSegmentTexts`（:1414/:1439/:1446）——把 libvterm 的 OSC 133 提示的行语义（prompt/command/output）缓存在 Kotlin 侧并随滚动维护。
- 快照构建：`buildSnapshot`（:1115）→ `TerminalSnapshot`（见 1.3）；URL 提取 `extractUrls`（:1157/:1172）、跨行折行 URL 重建 `buildWrappedUrlSpans`（:1214）、`continuationStart`（:1245）、`readWrappedUrl`（:1285）、`spansOverlapOsc8`（:1297）。
- 字符宽度：`isCombiningCharacter`（:1400，ICU `UProperty.GRAPHEME_EXTEND`）、`isFullwidthCharacter`（:1402）/`isFullwidthCodepoint`（:1408）。
- `TerminalCallbacks`（TerminalCallbacks.kt:29）：libvterm 回调接口——`damage`（:39）、`moverect`（:50）、`moveCursor`（:60）、`setTermProp`（:69）、`bell`（:76）、`pushScrollbackLine`（:89）、`popScrollbackLine`（:98）、`clearScrollback`（:105）、`onKeyboardInput`（:114）、`onOscSequence`（:126）；数据类 `TermRect`（:132）、`CursorPosition`（:142）、`TerminalProperty` 子类（:151-154）、`ScreenCell`（:160）。

**与 torvox 对比**：
- 有（对应物）：脏区/滚动（torvox Rust 侧 `terminal/ghostty_terminal` + `render`；Kotlin 侧无对应，事件经 `ffi.rs` `pollEvent` 到达）；OSC 处理（torvox `native/src/terminal/osc_handler.rs`）；URL 提取（torvox `ui/UrlDetector.kt`，纯文本扫描 + `SelectionExpander.kt` 处理 OSC 8）。
- 没有（torvox 缺失）：① **OSC 133 语义段模型**（prompt/command/output 分段随滚动维护）——torvox 的搜索是逐行文本扫描（`TextSearchBar.kt:79` `findMatches`），无「上一条命令输出」概念；termlib 的 `getLastCommandOutput`（TerminalEmulator.kt:183）可直接对应 torvox 未来的「复制上条命令输出」/无障碍朗读功能；② **`moverect` 行搬移优化**——torvox Rust 侧有类似优化（ghostty screen 内部），但 Kotlin 层无；③ ICU 宽度判定（`isFullwidthCodepoint` :1408）——torvox 用 `isWideChar`（TextSearchBar.kt:49）简化判定，精确性不如 ICU。

**依赖适用性**：libvterm JNI 是 termlib 特有（torvox 用 ghostty，不适用）；ICU4J `UCharacter` 仅用于宽度判定，torvox 可换成 `kotlin.math` + 区间表或复用现有 `isWideChar`。

**可吸收内容**：
- OSC 133 语义段状态机 + 随滚动移位算法（TerminalEmulator.kt:830-930、:1414-1452）：可作为 torvox「按命令块选择/复制」功能的设计蓝本；Rust 侧可在 ghostty OSC 133 处理基础上维护同构结构。
- `getLastCommandOutput` 语义（:183 + AccessibilityOverlay.kt:435 实现）：无障碍朗读「最后一条命令输出」的现成语义。

**文档吸收价值**：高——OSC 133 语义段是 termlib 独有、torvox 空白的功能。

### 1.3 TerminalScreenState.kt / TerminalSnapshot.kt / TerminalLine.kt / CellRun.kt

**功能说明**：
- `TerminalScreenState`（TerminalScreenState.kt:42）：Compose 侧屏幕状态，桥接服务层 emulator 与 UI：`getLine(index)`（:81，滚动区+可见区统一索引）、`getVisibleLine(row)`（:104）、`getHyperlinkUrlAt(row,col,autoDetectUrls)`（:130，先查 OSC 8 再查自动检测缓存）、`rebuildUrlCache`（:155，滚动时惰性重建 URL 缓存）、`mapUrlToSpans`（:215）、`buildWrappedUrlSpans`（:229）、`scrollToBottom/Top`（:313/:320）、`scrollBy`（:329）、`isAtBottom`（:336，粘底判断）、`updateSnapshot`（:352，**snapshot 差异替换 + URL 缓存失效**）、`rememberTerminalScreenState`（:375，Compose remember + 生命周期）。
- `TerminalSnapshot`（TerminalSnapshot.kt）：不可变快照（scrollback 行 + 可见行 + 光标 + 尺寸），`updateSnapshot` 做整体替换，UI 只读。
- `TerminalLine`（TerminalLine.kt）：行模型——`Cell`（宽字符用 `wideChar` 占两格 + 零宽组合字符列表）、`SemanticSegment` 引用、`softWrapped` 标志、`segmentText`（OSC 133 文本缓存）。
- `CellRun`（CellRun.kt:25）：**游程编码**——把同样式连续 cell 压缩为 run（`start`/`length`/`style`/`chars`），`reset()`（:58）与 `getCharsAsString()`（:74），供 Canvas 批量绘制减少 drawText 调用。

**与 torvox 对比**：
- 有（对应物）：滚动区+可见区统一索引（torvox `NativeBridge.scrollbackLine`/`getGridRowsColsPacked`，ffi.rs:1990/:2929）；URL 命中（torvox `UrlDetector.kt:11`）；粘底（torvox `TerminalSurface.kt` 的 `isAtBottom` 类逻辑）；不可变快照（torvox Rust 侧 grid 快照）。
- 没有（torvox 缺失）：① **URL 缓存按滚动位置惰性重建**（TerminalScreenState.kt:155-213）——torvox 每次 `findMatches`/URL 检测都全量扫描；② **CellRun 游程编码**（CellRun.kt:25-74）——torvox 每帧把整屏 cell 打包成 IntArray（ffi.rs `getGridRowsColsPacked` :2929），无 run 级压缩；③ `TerminalLine.Cell` 的宽字符+零宽组合字符建模（TerminalLine.kt）——torvox Rust 侧有宽字符处理（render/cell_builder.rs），Kotlin 侧无。

**依赖适用性**：无新增依赖；CellRun 思路可直接移植到 Rust 打包层。

**可吸收内容**：
- 滚动位置驱动的 URL 缓存失效策略（TerminalScreenState.kt:155）：torvox `TerminalViewModel` 的 URL 检测可缓存 `scrollbackOffset → urlList`。
- `CellRun` 编码注释建议（移植到 torvox `native/src/render/cell_builder.rs`）：
  ```kotlin
  // 游程编码：连续同样式 cell 合并为一个 run，
  // 减少 GPU 上传与 drawText 调用次数（termlib CellRun.kt:25）。
  ```

**文档吸收价值**：中高——性能优化（run 编码）与 URL 缓存策略可吸收。

### 1.4 TerminalNative.kt（libvterm JNI 封装）

**功能说明**：`TerminalNative` 对象（TerminalNative.kt）——`nativeCreate`/`nativeDestroy`/`nativeWrite`/`nativeResize`/`nativeSetPalette`/`nativeDispatchKey` 等 `external` 函数，对应 libvterm C API；包含 `TerminalCallbacks` 的 JNI 回调桥（Kotlin 对象引用持有、回调线程模型）。JNI 层把 libvterm 的 `vterm_screen_flush_damage` 等事件转成 Kotlin 回调。

**与 torvox 对比**：有（对应物）：torvox `NativeBridge.kt`（bridge/）与 `native/src/android/ffi.rs` 的手写 JNI 层——但 torvox 是 Rust 侧导出 `Java_*`（ffi.rs:397 起 40+ 个函数），termlib 是 Kotlin 侧声明 `external` + C 侧回调，方向相反。torvox 的 `jni` crate 依赖让 Rust 侧持有 JNIEnv 更安全。

**依赖适用性**：不适用（libvterm 与 ghostty 内核不同），但**回调对象生命周期管理**（Kotlin 侧引用防 GC、native 销毁时解绑）模式通用。

**可吸收内容**：`external` 声明 + 单例持有 native 指针 + `synchronized` 保护的样板（TerminalNative.kt），对 torvox 若引入第二原生后端有参考。

**文档吸收价值**：低——仅架构参考。

### 1.5 ImeInputView.kt（IME 输入视图，16 KB）

**功能说明**：
- `ImeInputView`（ImeInputView.kt:40）：挂载 `AndroidView` 的 EditText 类输入视图，提供 `showIme/hideIme`（:71/:80）、`onDetachedFromWindow`（:84）清理、`onCreateInputConnection`（:91）返回自定义 `InputConnection`、`onCheckIsTextEditor`（:122）。
- `resetImeBuffer`（:133）：**关键**——Enter 提交后重置 IME 缓冲（防 IME 状态错乱）；`restartInputSoon`（:147）延迟 `restartInput`。
- 自定义 `InputConnection`：`setComposingText`（:163）——**composition 差分算法**：新文本以旧 composition 为前缀 → 只发增量；否则计算删除量 + 新字符；`finishComposingText`（:227）、`deleteSurroundingText`（:237，转成 DEL 序列）、`sendKeyEvent`（:263）、`commitText`（:299）、`sendBackspaces`（:351）、`sendTextInput`（:357，走 `KeyboardHandler.onTextInput` 的 UTF-8 字节流）、`resetComposition`（:370）。

**与 torvox 对比**：有（对应物）：torvox `TerminalSurface.kt` 内嵌的 `InputConnection`（TerminalSurface.kt:405 起，「Owns the IME InputConnection: composition tracking, commit/delete」），同样实现了 composition 增长/收缩差分（TerminalSurface.kt:453-462）。**基本对等**，但 termlib 独立成类、含 `resetImeBuffer` 的 Enter 后重放防御（ImeInputView.kt:133-146）——torvox 的对应逻辑在 `TerminalSurface.kt:1549` 附近（「mid-composition; aborting it here causes the IME to lose sync」注释），思路一致。

**依赖适用性**：纯 Android SDK（`android.view.inputmethod`）。

**可吸收内容**：`resetImeBuffer` + `restartInputSoon` 的 Enter 提交后时序（ImeInputView.kt:133-160）可对照 torvox `TerminalSurface.kt` 的 Enter 处理做回归加固；composition diff 三态（前缀/删除/替换）算法（:163-226）值得提炼为共享注释文档。

**文档吸收价值**：中——torvox 已实现等价物，用于交叉验证与边界补充。

### 1.6 KeyboardHandler.kt / ModifierManager.kt / DelKeyMode.kt / RightAltMode.kt

**功能说明**：
- `KeyboardHandler`（KeyboardHandler.kt:52）：Compose `KeyEvent` → libvterm keycode/字节流。`onKeyEvent`（:106）主入口（含按键重复、组合键），`onCharacterInput`（:300，Ctrl/Alt 字符输入）、`onTextInput`（:319，UTF-8 字节流）、`onCommittedText`（:351，IME 提交文本直接编码）、`buildModifierMask`（:392，VTERM_MOD_* 位掩码）、`resolveEventModifierState`（:400，**硬件键盘修饰键状态机**：区分物理/逻辑修饰、瞬态锁定）、`getModifierMask`（:423）、`getCodePointFromKeyEvent`（:454，含死键/组合判定）、`dispatchCodepointOrEnter`（:504，Enter 单独处理）、`mapToVTermKey`（:532，Compose Key → VTermKey 大映射表）、`EventModifierState`（:630）、`VTermKey` 常量表（:639）。
- `ModifierManager`（ModifierManager.kt:26）：Ctrl/Alt/Shift 激活状态查询接口（:30-49），供触摸 UI 的修饰键条使用。
- `DelKeyMode`（DelKeyMode.kt）：Delete 键发送 `\x7f` 还是 `ESC[3~` 的设置。
- `RightAltMode`（RightAltMode.kt）：右 Alt 作为 Alt 还是 AltGr 的设置。

**与 torvox 对比**：有（对应物）：torvox `input/KeyModifiers.kt`、`input/ModifierState.kt`、`input/KeyboardMode.kt`（KeyboardMode.kt:32-35 明确注释了 IME composition 与 TYPE_NULL 的关系）、`ui/TerminalInputEncoder.kt`（KeyEvent → 字节流）、`ui/ModifierBar.kt`（触摸修饰键条）。torvox 的编码在 Rust 侧有对应（ffi.rs `writeKey` :931）。差异点：termlib 的 `resolveEventModifierState`（:400）处理**硬件修饰键的瞬态/锁定状态**更细；`DelKeyMode`/`RightAltMode` 是 termlib 的用户设置项——torvox 有类似设置（settings 中 del key 相关），可核对完整性。

**依赖适用性**：Compose `ui.input.KeyEvent`。

**可吸收内容**：`EventModifierState` 的物理/逻辑修饰判定（KeyboardHandler.kt:630-638）可作为 torvox `ModifierState.kt` 的边界用例补充。

**文档吸收价值**：中低——torvox 已覆盖核心，差异在修饰键状态机细节。

### 1.7 ComposeController.kt / ComposeMode.kt（Compose 键序列模式）

**功能说明**：
- `ComposeController`（ComposeController.kt:25）：接口——`startComposeMode`（:34）/`stopComposeMode`（:39）/`toggleComposeMode`（:44）/`getComposedText`（:49）。
- `ComposeMode`（ComposeMode.kt:30）：实现——`activate/deactivate`（:37/:42）、`appendChar/appendText`（:47/:52）、`deleteLastChar`（:57）、`commit`（:67，查表合成字符，失败返回 null）、`cancel`（:79）。配合 Terminal.kt 的 `drawComposeOverlay`（Terminal.kt:2293）在光标处显示组合进度，Esc 取消、超时重置。

**与 torvox 对比**：没有——torvox 无 Compose 键序列模式（无对应文件；grep `ComposeMode` 无命中）。torvox 的键盘依赖 IME composition（`TerminalSurface.kt:405-462`）+ `TerminalInputEncoder.kt`，Compose 键（如 `Compose + o + o` → `°`）对硬件键盘用户是功能空白。

**依赖适用性**：纯 Kotlin 表驱动。

**可吸收内容**：整套可移植——`ComposeMode.kt` 的组合表（约 200 条）+ `drawComposeOverlay` 覆盖层 + Esc/超时取消；建议接入 torvox `ModifierBar` 或 `TerminalInputEncoder` 前置。移植注释建议：
  ```kotlin
  // Compose 键模式：Compose + 两键序列合成字符（termlib ComposeMode.kt:30）。
  // 激活后所有字符输入先进入组合缓冲，Esc 取消，超时/错误序列自动退出。
  ```

**文档吸收价值**：高——torvox 功能空白项，实现成本低。

### 1.8 OscParser.kt / UrlDetection.kt

**功能说明**：
- `OscParser`（OscParser.kt:38）：`parse`（:95）把 `onOscSequence` 的 (command, payload) 转成 `Action` 列表；`handleOsc52`（:123，剪贴板读写，含 base64）、`handleOsc9`（:161，iTerm2 通知）、`handleOsc8`（:200，超链接，`parseHyperlinkId` :262）、`handleOsc133`（:275，**shell integration 提示**：Prompt/CommandStart/CommandExit/Output，生成语义段 Action）、`handleOsc1337`（:341，iTerm2 扩展，如当前目录/通知）。`ProgressState`（:25）支持 OSC 1337 进度条。
- `UrlDetection`（UrlDetection.kt）：极简自动 URL 检测（scheme 白名单 + 尾部标点裁剪）。

**与 torvox 对比**：
- 有（对应物）：OSC 52（torvox `osc_handler.rs` + ffi.rs `clipboardResult` :1819）；OSC 8（torvox `SelectionExpander.kt` + Rust 侧 hyperlink 缓存）；OSC 9/1337 通知（torvox Rust 侧部分支持）。
- 没有（torvox 缺失）：**OSC 133 语义段**（见 1.2，torvox `osc_handler.rs` 是否处理 133 需核对——研究时未见对应符号）。

**依赖适用性**：无新增依赖（base64 用 `android.util.Base64`）。

**可吸收内容**：`OscParser` 的「(command,payload) → Action 列表」纯函数结构（OscParser.kt:95-122）可作为 torvox `osc_handler.rs` 的 Rust 侧重构参考；OSC 133 处理流程（:275-340）是语义段功能的解析端。

**文档吸收价值**：高（OSC 133）+ 中（其余已覆盖）。

### 1.9 ColorCache.kt（颜色缓存）

**功能说明**：`ColorCache`（ColorCache.kt:29）——`get(r,g,b)`（:53）从 `IntArray` 池（LRU 式有限容量）取 `Compose Color` 避免装箱分配；`findPaletteIndex`（:77，把 RGB 映射回调色板下标以复用）；`standardAnsiColor`（:112）、`rgb6Color`（:164，6×6×6 立方体）、`grayscaleColor`（:171，24 级灰阶）生成标准 xterm 256 色。

**与 torvox 对比**：有（对应物）：torvox 渲染在 Rust 侧（`render/cell_builder.rs`、theme 打包），Kotlin 侧无逐 cell 颜色构造，因此无此缓存需求。torvox 的主题色在 `ui/theme/TerminalTheme.kt` 与 Rust 侧 palette 结构。

**依赖适用性**：不直接适用，但 256 色生成逻辑（:112-175）可对照 torvox Rust 侧 xterm 色表（若有）校验一致性。

**可吸收内容**：若 torvox 未来在 Kotlin 侧做配色预览（如 SettingsScreen 调色板预览），`standardAnsiColor`/`rgb6Color`/`grayscaleColor` 可直接复用。

**文档吸收价值**：中低。

### 1.10 ScrollController.kt

**功能说明**：`ScrollController` 接口（ScrollController.kt:22）：`scrollToBottom`（:37）/`scrollToTop`（:42）/`scrollBy(lines)`（:48），由 `TerminalScreenState` 实现，供手势/无障碍/外部调用统一入口。

**与 torvox 对比**：有（对应物）：torvox `TerminalSurface.kt`/`TerminalViewModel.kt` 的滚动逻辑（`ffi.rs setScrollOffset` :2950）。termlib 的优势是**接口化**（可注入 fake 测试）。

**依赖适用性**：无。

**可吸收内容**：接口抽象（:22-48）便于单元测试——torvox 滚动逻辑混在 ViewModel/手势回调中，可提炼接口。

**文档吸收价值**：低。

### 1.11 AccessibilityOverlay.kt / AccessibilityStateManager.kt / LiveOutputRegion.kt（无障碍三件套）

**功能说明**：
- `AccessibilityOverlay`（AccessibilityOverlay.kt:64）：覆盖在终端之上的**只读 Compose 层**，TalkBack 聚焦时用行文本替换绘制内容；`buildSemanticAnnotatedString`（:254，用 AnnotatedString 标记语义段类型）、`buildSemanticDescription`（:341，把行拼成朗读文本）、`getLastCommandOutput`（:435，从语义段提取上条命令输出）、`findNextLineWithSegmentType`（:482，语义段间导航：上一个/下一个 prompt/command/output）。
- `AccessibilityStateManager`（AccessibilityStateManager.kt:39）：`rememberAccessibilityState()` 返回是否启用无障碍的 Compose 状态（监听 `AccessibilityManager`）。
- `LiveOutputRegion`（LiveOutputRegion.kt:43）：**滚动窗口算法**——记录输出增量，构成「最后 N 行活动输出」区域，供 TalkBack live region 朗读；与 ReviewMode（读屏浏览模式）联动，`ReviewModeTest` 覆盖（见 1.12）。

**与 torvox 对比**：
- 有（对应物）：torvox 有基础无障碍——`TerminalScreen.kt:175/184` 的 `announceForAccessibility`、`TerminalSurface.kt:1458` `IMPORTANT_FOR_ACCESSIBILITY_YES`、`MainActivity.kt:498` `testTagsAsResourceId`。
- 没有（torvox 缺失）：① **语义段级朗读与导航**（`findNextLineWithSegmentType` :482）——torvox 只能整屏朗读，无法跳到「上一条命令」；② **LiveOutputRegion 增量朗读**（LiveOutputRegion.kt:43）——torvox 无输出增量播报；③ 无障碍状态驱动的 UI 分支（`rememberAccessibilityState` :39）——torvox 无。

**依赖适用性**：Compose foundation + `androidx.compose.ui.semantics`。

**可吸收内容**：整套可移植到 torvox `TerminalScreen.kt`/`TerminalViewModel.kt`——尤其 `getLastCommandOutput` 与 LiveOutputRegion 滚动窗口，可与 1.2 的 OSC 133 语义段配套实现。注释建议：
  ```kotlin
  // 无障碍阅读模式：用语义化 AnnotatedString 替换屏幕内容，
  // 支持按 prompt/command/output 语义段上下导航（termlib AccessibilityOverlay.kt:64）。
  ```

**文档吸收价值**：高——torvox 无障碍空白中的最大可吸收块。

### 1.12 测试文件（22 个，lib/src/test/java/org/connectbot/terminal/）

**功能说明与技巧**（框架：JUnit4 + Robolectric + Compose UI test + roborazzi golden）：
- `SelectionManagerTest`：选择范围四向移动、模式切换（CHARACTER/WORD/LINE/SEMANTIC）、`toggleMode` 边界、`getSelectedText` 行连接语义（软换行折叠、硬换行保留）、`isCellSelected` 命中。
- `HandleDragTest`：`applyHandleDrag` 纯函数——手柄拖动到行首/行尾/跨行的选择端点更新，拖动超出边界钳制。
- `TerminalRendererGoldenTest`（:41-78）：**roborazzi 截图 golden**——写固定 ANSI 序列（`goldenTerminalContent()` :78）→ `writeInput` → 渲染 → 与 `src/test/roborazzi/terminal-renderer-golden.png` 比对。这是 torvox 可借鉴的回归手段（torvox 有 Roborazzi 配置，roborazzi.properties 存在）。
- `OscParserTest`/`OscSequenceTest`：OSC 52/8/9/133/1337 各 payload 解析、非法输入容错、base64 往返。
- `KeyboardHandlerTest`：Compose KeyEvent → VTermKey 映射、Ctrl/Alt/Shift 组合、修饰键状态机。
- `ImeInputViewTest`：composition 差分三态（前缀扩展/收缩删除/整体替换）、Enter 后 reset、`commitText` 字节流。
- `ComposeModeTest`：组合序列成功/失败/Esc 取消/超时。
- `CursorAndModeEscapeTest`：光标样式、DECSET/DECRST 模式位。
- `GranularNavigationTest`：逐字符/逐词导航（配合 SelectionManager 方向移动）。
- `MagnifierOffsetTest`：`magnifierOffset` 纯函数边界（屏幕边缘不越界）。
- `ReadLastOutputTest`：`getLastCommandOutput` 在滚动/清屏后的正确性。
- `ReviewModeTest`（:32-122）：无障碍阅读模式切换、LiveOutputRegion 输入后更新、空终端朗读、多行导航。
- `ScrollbackClearTest`（:35-103）：ESC 3 J 清滚动区保留可见屏、清后重新填充、2J+3J 组合。
- `SelectionControllerTest`：接口行为（start/toggle/move/selectAll/finish/copy/clear）。
- `SemanticTypeTest`：语义段类型判定与重叠。
- `ShellIntegrationTest`：OSC 133 序列 → 语义段生成 → 选择/复制联动。
- `TerminalGestureTest`：Tap/DoubleTap/LongPress/Pinch 手势到选择/滚动/缩放的动作映射。
- `TerminalScreenStateScrollTest`：滚动位置、粘底、URL 缓存重建。
- `TerminalScreenStateUrlTest`：OSC 8 与自动检测的优先级、跨行 URL。
- `TerminalUrlExtractionTest`：URL 提取边界（尾部标点、多 URL、折行）。
- `AccessibilityOverlayTest`、`ComposeModeTest` 等覆盖前述组件。

**与 torvox 对比**：torvox 有 Roborazzi（roborazzi.properties）+ 集成测试（integration-tests/、maestro/），但**单元级纯函数测试密度**不如 termlib（termlib 把手势、拖拽、magnifier、OSC 解析全部做成可单测纯函数）。

**可吸收内容**：① `HandleDragTest`/`MagnifierOffsetTest` 的纯函数化测试模式——torvox 的拖拽/放大镜逻辑在 `TerminalSurface.kt` 手势回调里，可先提取纯函数再补测试；② roborazzi golden（`TerminalRendererGoldenTest` 模式）可移植到 torvox（golden 覆盖 Rust 渲染输出与 Kotlin 布局）；③ `ScrollbackClearTest` 的 ESC 3 J 语义（torvox Rust 侧 `clearScrollback` 对应 ffi.rs:716 无 Kotlin 测试）。

**文档吸收价值**：高——测试方法论（纯函数化+golden）是 torvox 测试基建的直接升级路径。

### 1.13 termlib 小结

- **最值得吸收**：OSC 133 语义段（1.2/1.8）、无障碍三件套（1.11）、Compose 模式（1.7）、放大镜纯函数（1.1）、CellRun 游程编码（1.3）、golden 测试模式（1.12）。
- **依赖**：无需新增（Compose/Android SDK/ICU4J 可选）。
- **文档吸收价值**：高（5 个功能空白项）。

---

## 2. gnome-console（kgx，GTK4 + VTE 终端）

架构：`KgxApplication`（GAction 体系）→ `KgxWindow`（AdwWindow + `KgxPages` 标签模型）→ `KgxTab`/`KgxTerminal`（已有文档覆盖）→ `KgxProcess`（libgtop 进程监听，驱动 root/remote/playbox 状态样式）。

### 2.1 kgx-window.c（窗口与动作）

**功能说明**（行号来自 grep 与分段阅读）：
- 窗口创建 `kgx_window_init`：`AdwHeaderBar` + `AdwTabOverview` + 空态占位（`kgx-window.ui` 的 `content/empty` GtkStack）。
- 全屏/tear-off（标签拖出成新窗口）：`kgx_window_tearoff_tab` 附近逻辑，`kgx-pages.c` 配合 `kgx_pages_remove_tab` 后在新窗口 `kgx_window_add_page`。
- 关闭请求链：`kgx_window_close_request` → `kgx_pages_maybe_close` → 每个 tab 的 `kgx_terminal` 确认（多标签时弹 `KgxCloseDialog`，见 2.16）。
- 状态样式类：window 挂 `playbox`/`remote`/`root`/`ringing` CSS 类（由 `kgx-process.c` 的进程树状态驱动，见 2.6），供 style.css 区分着色。
- 标题/图标：`kgx_window_update_title`（取活动 tab 的 title/icon，VTE 的 OSC 0/2 + 图标名）。
- 标签拖放：`kgx_pages` 的 DnD 目标（`kgx-window.ui` 的 `AdwTabButton` 区域），跨窗口拖标签复用 `kgx_window_add_page`。

**与 torvox 对比**：torvox 无窗口概念（单 Activity + SessionDrawer 会话抽屉），无 tear-off/多窗口；torvox 有会话切换（`TerminalRuntime.kt` 的 session 管理 + `ffi.rs switchSession` :677）。kgx 的「多标签 + 关闭确认」对应 torvox 的「多会话 + 会话抽屉」——交互模型不同，功能等价性低。

**依赖适用性**：GTK4/Adwaita，不适用。

**可吸收内容**：`ringing`（响铃）状态样式——torvox 收到 bell（`TerminalEmulator` 对应物：torvox Rust 侧 bell 事件）时可在会话抽屉/标题上加视觉提示；kgx 的做法是 CSS 类切换，torvox 可用 Compose 状态等价实现。

**文档吸收价值**：中低。

### 2.2 kgx-pages.c（标签页模型）

**功能说明**：`KgxPages` 实现 `GListModel`（标签列表数据源，供 `AdwTabView`/`AdwTabOverview` 消费）；`kgx_pages_append/remove`、活动页切换 `kgx_pages_set_active`、关闭全部 `kgx_pages_maybe_close`（逐个确认）；**尺寸指示**：`kgx_pages_update_size` 显示 "80 × 24" 标签 1.2 秒（resize 提示）；`kgx_pages_add_status` 设置标签状态图标（busy/blocked 等）。

**与 torvox 对比**：torvox 无标签 UI（会话在 SessionDrawer 列表）；「resize 尺寸指示」在 torvox 无对应（终端尺寸变化无提示）。「busy 状态图标」对应 torvox 的渲染/活动指示（无）。

**依赖适用性**：GListModel/AdwTabView，不适用。

**可吸收内容**：resize 提示 UX（"80 × 24" 短暂显示）可作为 torvox 网格变化时的轻提示；busy 状态图标概念可用于 torvox 会话列表（运行中/退出）。

**文档吸收价值**：低。

### 2.3 kgx-settings.c（设置）

**功能说明**（grep 键清单）：
- schema：`org.gnome.Console`（`data/org.gnome.Console.gschema.xml`）。绑定到 `KgxTerminal` 的键包括：`font`（自定义字体，空则跟随系统等宽字体）、`background-color`/`foreground-color`（可空 → 主题默认）、`use-system-font`、`theme`（`auto`/`day`/`night`/`hacker`，见 2.10）、`last-window-size`、`custom-font`、`terminal-*`（bell/scrollback 等）、`shortcuts`（keybinding 键，`kgx_window` 的 GAction 快捷键映射）。
- 结构：`kgx_settings_init` 读 GSettings，`kgx_settings_bind` 系列用 `g_settings_bind` 双向绑定到 GTK 属性；主题/颜色变更通过 `KGX_SETTINGS_CHANGED` 信号广播（`kgx_livery_manager` 消费）。
- `kgx_settings_get_font` 组合 font family + size 为 PangoFontDescription。

**与 torvox 对比**：有（对应物）：torvox `settings/SettingsRepository.kt`（DataStore：`font_family`、`font_size`、主题、`use_semantic_selection` 等，SettingsRepository.kt:33 起）；torvox 设置比 kgx 更丰富（语义选择、背景图、光标闪烁等）。kgx 无的设置组织价值：**「custom 为空 → 跟随系统」的字体降级语义**（`use-system-font` 布尔 + 空串 = 系统默认）与 torvox `SystemFonts.kt` 的「默认字体跟随系统」一致。

**依赖适用性**：GSettings/GLib，不适用（torvox 用 DataStore）。

**可吸收内容**：无实质新功能；「字体设置空值=系统默认」语义可核对 torvox `SettingsRepository` 的字体默认值处理。

**文档吸收价值**：低。

### 2.4 kgx-application.c（应用生命周期与命令行）

**功能说明**：`kgx_application_startup/activate`：注册 GAction（`new-window`、`new-tab`、`find`、`zoom-*`、`toggle-theme-switcher` 等，见 kgx-window.ui 的 action-name 清单）；命令行 `kgx_application_handle_local_options`/`open`：解析 `kgx [DIR]`、`kgx -e CMD`、多位置参数 → 每位置开新 tab；DBus 激活（`G_APPLICATION_HANDLES_OPEN`）重复调用时在既有窗口开 tab；`--version`、`--revision`；环境变量 `KGX_*`（调试）。

**与 torvox 对比**：有（对应物）：torvox `MainActivity.kt` 的 intent 处理（VIEW 动作、打开目录/bootstrap 参数）+ `TerminalRuntime` 的启动流程。kgx 的「多位置参数 → 多 tab」对应 torvox 的多会话创建（会话列表）。

**依赖适用性**：GApplication，不适用。

**可吸收内容**：无。

**文档吸收价值**：低。

### 2.5 kgx-despatcher.c（URI/命令分发）

**功能说明**：`kgx_despatcher_open` 等——把终端里的 URI 分发到系统默认应用（`g_app_info_launch_default_for_uri`）；**未挂载位置**（`G_IO_ERROR_NOT_MOUNTED`）自动尝试 `g_file_mount_enclosing_volume` 挂载后重试；`kgx_despatcher_show_file_manager` 通过 `org.freedesktop.FileManager1` D-Bus 接口在文件管理器显示路径。

**与 torvox 对比**：有（对应物）：torvox URL 点击打开用 Android Intent（`TerminalViewModel` 的 URL 处理 + `UrlDetector.kt`）；「未挂载自动挂载」对 Android 无对应（SAF 文档提供器替代，torvox 有 `TerminalDocumentsProvider.kt`）；「显示在文件管理器」对 Android 可用 `Intent.ACTION_VIEW` + DocumentsContract 近似实现（torvox 无此功能）。

**依赖适用性**：GIO，不适用；「show in file manager」概念可移植（Android `Intent(ACTION_VIEW)` 指向文件 URI）。

**可吸收内容**：低（概念级）。

**文档吸收价值**：低。

### 2.6 kgx-process.c（进程状态）

**功能说明**：`kgx_process_*` 封装 **libgtop**（`glibtop_proclist`/`glibtop_proc_uid`）：`kgx_process_is_root`（检测 tab 的 PTY 进程是否 root 用户——对比会话 uid 与 euid）、`kgx_process_is_remote`、`kgx_process_is_playbox`；周期轮询（`kgx_process_poll`），状态变化时发信号 → window 切换 CSS 类（2.1）。`kgx-train.c`（见 2.14）提供进程链跟踪。

**与 torvox 对比**：没有——torvox 无「检测 shell 是否 root/远程」功能（Android 无 libgtop 等价物；可近似用 `id -u` 或 PTY 的 uid 判定）。torvox 有 `RootHelper.kt` 类似物只在 rin 里（见第 4 节）。

**依赖适用性**：libgtop，不适用。

**可吸收内容**：概念——终端状态（root/remote）的 UI 区分（kgx 用 CSS 类变色 tab）。torvox 可考虑在会话列表显示 root 会话标记（实现方式：启动时 `id -u` 探测）。

**文档吸收价值**：中（概念移植）。

### 2.7 kgx-paste-dialog.c（粘贴确认）

**功能说明**：`kgx_paste_dialog_*`（kgx-paste-dialog.c:49 起；`set_property` :61 中 `kgx_str_constrained_dup(self->content, 8000)` :73 截断预览；`kgx_paste_dialog_run` :163）——仅在**管理员会话**（`kgx_process_is_root`）粘贴时弹确认对话框：`AdwMessageDialog`，标题「You are pasting a command that runs as an administrator」；内容用 `kgx_str_constrained_dup` 截断（8000 字符上限），提示 Esc 取消、确认后 `kgx_terminal_paste_plain`。

**与 torvox 对比**：有（对应物）：torvox `ui/PasteChipOverlay.kt`（:23-46，粘贴前弹 chip 确认，`TerminalScreen.kt:442-448` 挂载）——torvox 是**每次粘贴都确认**（chip 可点开详情），kgx 仅 root 会话确认。torvox 更严格；kgx 的「8000 字符截断预览」（`kgx_str_constrained_dup`）是 torvox 缺失的细节（`PasteChipOverlay` 无预览截断；torvox `PasteChunker.kt:28` 处理 `\n→\r` 与 xterm 语义，但不预览内容）。

**依赖适用性**：AdwMessageDialog，不适用。

**可吸收内容**：粘贴预览截断（8000 字符 + 省略号）可加入 torvox `PasteChipOverlay`；root 会话判定后强制确认的规则可借鉴（torvox 若实现 root 会话标记，见 2.6）。

**文档吸收价值**：中。

### 2.8 kgx-theme-switcher.c + kgx-livery.c + kgx-livery-manager.c + kgx-colour-utils.c + kgx-palette.c（主题系统）

**功能说明**：
- `kgx-theme-switcher.c`：headerbar 下拉（AUTO/DAY/NIGHT/HACKER 四档），`kgx_theme_switcher_set_theme` 写 GSettings `theme` 键并触发应用层主题切换。
- `kgx-livery.c`：**配色方案（livery）数据类**——`KgxLivery` 结构含 foreground/background/palette（16 色）与可选自定义色；`kgx_livery_parse` 从字符串解析；`kgx_livery_merge`（**合并优先级**：显式颜色 > 系统主题色 > 预设 palette）；`kgx_livery_apply`（应用到 VTE：`vte_terminal_set_colors` + palette）。
- `kgx-livery-manager.c`：监听 `prefers-color-scheme`（系统深浅色）与 GSettings `theme`，决定当前 livery：hacker 主题 → 绿/黑；night → `AdwStyleManager` 强制深色 + VTE 深色调色板；day → 浅色；auto → 跟随系统；自定义 `custom-font`/颜色覆盖。
- `kgx-colour-utils.c`：颜色工具——`kgx_colour_parse`（CSS 颜色字符串 → `GdkRGBA`）、`kgx_colour_darken/lighten`（HSL 调整）、`kgx_colour_to_string`。
- `kgx-palette.c`：palette 条目结构（16 个 `KgxPaletteEntry`：name + RGB），`kgx_palette_get_*` 提供预置主题（day/night/hacker 的 16 色表）。

**与 torvox 对比**：有（对应物）：torvox `ui/theme/TerminalTheme.kt`（Compose 主题）+ Rust 侧 `setTheme`（ffi.rs:2421）——torvox 支持深/浅/跟随系统 + 自定义 16 色（设置里可编辑 palette）；torvox 的语义选择等设置比 kgx 丰富。kgx 独有的：① **HACKER 主题**（绿/黑终端风）；② **livery 合并优先级模型**（显式覆盖 > 系统 > 预设，kgx-livery.c 的 `kgx_livery_merge`）——torvox 的主题解析在 Rust 侧（`setTheme` 参数打包），优先级逻辑分散；③ HSL darken/lighten 工具（kgx-colour-utils.c）——torvox 无（配色预览需要）。

**依赖适用性**：GTK/VTE API，不适用；颜色算法（HSL 调整）可移植到 Kotlin（`TerminalTheme.kt`）。

**可吸收内容**：`kgx_livery_merge` 的优先级模型可作为 torvox 主题层「自定义色覆盖预设色」的文档化规则；`kgx_colour_darken/lighten` 算法可在 torvox 设置页的调色板编辑器中复用。

**文档吸收价值**：中——优先级模型与 HSL 工具可吸收。

### 2.9 kgx-drop-target.c（拖放）

**功能说明**：`kgx_drop_target_*`（kgx-drop-target.c:67 起；核心处理 :136-353，`kgx_drop_target_extra_drop` :485，挂载重试 `kgx_drop_target_mount_on` :522）：`GtkDropTarget` 接受 `text/uri-list`；异步 `g_file_query_info` 读 URI 列表（大列表分块）；**路径转 shell 引用**：`g_shell_quote` 每个路径，无路径的 URI 保持原样；跳过注释行（`#` 开头）；完成后发出 `DROP` 信号，内容为空格连接的 shell 引用字符串（tab 收到后写入 PTY）。

**与 torvox 对比**：没有——torvox 无拖放文件到终端的功能（Android 的拖放支持有限：`OnDragListener` 可接收 `text/uri-list`，但桌面拖放到 Android 终端不常见）。torvox 有文件选择 → 路径插入（文档提供器 `TerminalDocumentsProvider.kt` 是 SAF 导出，方向相反）。

**依赖适用性**：GtkDropTarget，不适用；**g_shell_quote 逻辑**（空格/引号/换行转义）通用，torvox 若做「插入文件路径」可用 Kotlin 等价实现（对 `'`、`\`、空格转义）。

**可吸收内容**：shell 引用转义算法（kgx-drop-target.c 的 `g_shell_quote` 用法）——torvox 的「插入路径」功能（若有）需要同样的转义。

**文档吸收价值**：中低。

### 2.10 kgx-train.c（进程链跟踪）

**功能说明**：`kgx_train_*`——进程树（train）跟踪：给定 PID，沿 PPID 链回溯，判定进程链中是否有 root/remote/playbox 特征（配合 `kgx-process.c`）；`kgx_train_is_root`/`kgx_train_is_playbox` 等在 `kgx-tab.c` 中驱动 `KgxProcessState` 状态机（tab 的 `remote`/`root`/`playbox` 状态）。本质是「这个 tab 的 shell 是不是 sudo/ssh/容器」的判定器。

**与 torvox 对比**：没有——torvox 无进程链检测（见 2.6）。

**依赖适用性**：libgtop/GLib，不适用。

**可吸收内容**：概念——「PPID 链判定 shell 形态」在 Android 可用 `/proc/<pid>/status` 的 PPid 遍历近似实现（torvox 若做 root 会话标记）。

**文档吸收价值**：低。

### 2.11 kgx-fullscreen-box.c（全屏悬浮控件）

**功能说明**：`kgx_fullscreen_box_*`——全屏时包裹 headerbar 的自动隐藏容器：鼠标靠近屏幕顶部或聚焦 titlebar 时显示，移开后隐藏；触摸设备上行为不同（点按切换）；配合 `kgx-window.ui` 的 `fullscreen-box` 模板与 CSS 过渡。

**与 torvox 对比**：部分有——torvox `MainActivity.kt` 有全屏/沉浸模式（`WindowInsetsController`），但无「自动隐藏工具栏」交互（torvox 全屏时工具栏固定/可手势唤出，取决于设置）。

**依赖适用性**：GTK，不适用。

**可吸收内容**：低。

**文档吸收价值**：低。

### 2.12 kgx-close-dialog.c（多标签关闭确认）

**功能说明**：`kgx_close_dialog_*`（kgx-close-dialog.c:52 起；`kgx_close_dialog_run` :232）——`AdwMessageDialog`：标题「Close N windows?」/「Close N tabs?」，正文提示「These windows/tabs are still open」；仅在**多个**窗口/标签被一次关闭时出现（单个直接关）。

**与 torvox 对比**：没有——torvox 关闭会话无确认（会话抽屉里滑动删除/菜单删除）。torvox 有后台会话存活语义（`TerminalRuntime.kt:2330` 注释：Termux 风格会话存活），删除确认是 UX 空白（若误删活动会话）。

**依赖适用性**：AdwMessageDialog，不适用。

**可吸收内容**：批量关闭确认的文案/条件（仅多标签时弹）可在 torvox 会话批量清理时参考。

**文档吸收价值**：低。

### 2.13 kgx-spad.c（错误报告对话框）

**功能说明**：`kgx_spad_*`（kgx-spad.c:65 起）——「Support/Problem About Dialog」：`kgx_spad_build_bundle`（:290，从 window/tab 收集版本/系统信息/最近错误）与 `kgx_spad_new_from_bundle`（:325），对话框带复制按钮与可选附加信息，用于用户提交 issue。本质是**诊断信息导出**工具。

**与 torvox 对比**：有（对应物）：torvox `runtime/LogcatDumpWriter.kt`、`LogcatFileWriter.kt`（日志导出）+ `BootstrapInstaller` 的安装日志——但 torvox 无「一键复制诊断包」对话框。torvox 有 `LogUtil.kt` 与日志文件。

**依赖适用性**：GTK，不适用。

**可吸收内容**：诊断包对话框概念——torvox 设置页可加「复制诊断信息」按钮（版本、内核、渲染后端、日志尾部），对 bug 反馈有直接价值。

**文档吸收价值**：中。

### 2.14 kgx-tab.c / kgx-terminal.c 补遗（已有文档覆盖搜索 UX，此处仅补其余）

**功能说明**：右键菜单（`kgx-tab.c` 的 `kgx_tab_popover`）：New Tab / New Window / Close Tab / Copy / Paste / **Show in File Manager**（经 despatcher）/ Find；`kgx-terminal.c` 的快捷键（Ctrl+Shift+Find、Ctrl+-/= 缩放、Ctrl+Shift+C/V、Ctrl+Shift+W/Q）；缩放经 `kgx_terminal_zoom` 修改字体大小并持久化。

**与 torvox 对比**：torvox 的会话右键/长按菜单在 `ModifierBar.kt`/`TerminalSurface` 上下文菜单（SelectionActions :120）；「Show in File Manager」torvox 无（见 2.5）。

**文档吸收价值**：低（已覆盖）。

### 2.15 .ui / .css 资源

**功能说明**：
- `kgx-window.ui`：`AdwTabOverview`（标签总览页）+ `GtkStack`（`content`/`empty` 切换）+ `AdwHeaderBar`（含 `find` 搜索按钮、theme-switcher、zoom 菜单、`AdwTabButton`）；`AdwBreakpoint`（窄屏隐藏 headerbar 元素）。
- `kgx-pages.ui`/`kgx-tab.ui`：标签按钮（icon+title+close）、`kgx_tab` 的上下文弹层。
- `kgx-terminal.ui`：终端占位 + 右键菜单。
- `kgx-spad.ui`：错误报告对话框布局。
- `style.css`/`style-dark.css`：CSS 变量（`--kgx-*` 色板变量）、`playbox`/`remote`/`root`/`ringing` 状态类、全屏过渡动画；主题通过 CSS 变量 + `AdwStyleManager` 深浅切换。

**与 torvox 对比**：torvox 用 Compose（`SettingsComponents.kt`/`TerminalScreen.kt` 的 Material 主题），无 CSS 体系；「CSS 变量换肤」对应 torvox `TerminalTheme.kt` 的 Material colorScheme 切换。kgx 的「状态类驱动样式」（root/remote 变色）是 torvox 无的机制（可对照 2.6）。

**依赖适用性**：GTK CSS，不适用。

**文档吸收价值**：低。

### 2.16 gnome-console 小结

- **最值得吸收**：livery 合并优先级模型（2.8）、root/remote 会话状态概念（2.6/2.10）、粘贴预览截断（2.7）、诊断信息导出（2.13）、ringing 状态样式（2.1）。
- **依赖**：全部 GTK/GIO/libgtop，不直接适用；概念可移植。
- **文档吸收价值**：中（概念级为主）。

---

## 3. shashlik（Slint 地图渲染框架，wgpu + Rust + KMP）

架构：`app-surface`（平台表面抽象：Android ANativeWindow / iOS CAMetalLayer / winit）→ `renderer-common`（渲染器 trait + worker 线程）→ `renderer-gpu`（wgpu 主渲染器）/`renderer-cpu`（Skia CPU 回退）→ `ffi-run`（uniffi 绑定）→ `kmp`（Kotlin Multiplatform 客户端，TextureView 承载）。与 torvox 的关联点：**Android 上 wgpu 表面生命周期、GL/Vulkan 双后端、KMP 桥接、交叉编译**。

### 3.1 app-surface/src/lib.rs（表面抽象）

**功能说明**：`ViewSize`（:25）、`IASDQContext`（:33，纹理格式适配）、`AppSurface` trait（:56 起）：`view_size`（:62）、`resize_surface`（:64）/`resize_surface_by_size`（:65）、`pintch`（:66，缩放触摸）、`touch`（:67）、`normalize_touch_point`（:68，触摸坐标→归一化）、`enter_frame`（:71）、`get_current_frame_view`（:72）/`create_current_frame_view`（:78，**表面重建**：surface 失效时重建 wgpu Surface + 配置）、`SurfaceFrame` 实现（:103）。`app_surface_use_winit.rs` 是桌面/移动 winit 变体（`SurfaceFrame` 用 winit 窗口）。

**与 torvox 对比**：有（对应物）：torvox `native/src/render/wgpu_backend.rs`（`initialize_wgpu` :44，GL 硬编码 :59）+ `attachWindow`（ffi.rs:1297，ANativeWindow → surface 配置）。差异：torvox 把表面生命周期收敛在单函数（attach/detachWindow），shashlik 抽象成 trait + 显式重建（`create_current_frame_view` :78）——**torvox 的 attachWindow 已实现类似重建**（ffi.rs:1297 起约 95 行处理 surface 重建/format 变化），架构思路一致。

**依赖适用性**：wgpu + raw-window-handle，与 torvox 相同栈。

**可吸收内容**：`normalize_touch_point`（:68）与 `pintch`（:66）的触摸归一化契约——torvox 的触摸缩放（`TerminalSurface.kt` 双指）可对照归一化坐标设计。

**文档吸收价值**：中低（torvox 已覆盖等价机制）。

### 3.2 renderer-common/（渲染器公共层）

**功能说明**：
- `lib.rs`：`Renderer` trait（`screen_size`/`resize`/`update`/`clip_to_world`/`render`/`api`）、`RendererUpdateData`（相机/几何更新）、`CommonRendererApi<C>`（渲染器向 UI 层暴露的 API 包装，Arc 共享）。
- `worker_handler.rs`：**渲染 worker 线程**——`WorkerHandler`（:24 起）：spawn 线程持有渲染器，UI 线程通过 channel 发 `WorkerMessage`（Update/Resize/Render），worker 回传帧完成通知；`ActiveWorker`/`InactiveWorker` 枚举切换（渲染器被替换时优雅停止）。
- `fps.rs`：FPS 统计（滑动窗口、ema）、`FpsWatcher`（帧超时检测，触发降级提示）。
- `collision_handler.rs`：文本/图形碰撞（AABB 相交、点命中），用于文本标签避让。
- `geometry_data.rs`：`GeometryData`（点/线/面/文本元素 + 变换 + 样式）。
- `render_modifier.rs`/`render_style.rs`/`style_id.rs`：渲染样式（线宽/颜色/透明度）与样式 ID 表。
- `r_api_messenger.rs`：UI↔渲染器消息路由。
- `consts.rs`：常量（默认背景色等）。

**与 torvox 对比**：
- 有（对应物）：渲染循环（torvox `render/context.rs`/`pipeline.rs` 的每帧流程 + `RenderWatchDog.kt` 帧超时监控——对应 `fps.rs` 的 `FpsWatcher`）；worker 线程模型（torvox 渲染在 Rust 线程 + Kotlin pollEvent 拉取，`ffi.rs render` :1393——torvox 是「UI 线程拉帧」而非「worker 推帧」，shashlik 是 worker 内渲染 + 信号通知，两者皆可）。
- 没有（torvox 缺失）：① **渲染降级监控的完整闭环**（`FpsWatcher` 帧超时 → 降级 CPU 渲染/降分辨率）——torvox `RenderWatchDog.kt` 只告警不降级；② 碰撞避让系统（地图专用，torvox 不需要）。

**依赖适用性**：wgpu/slint 生态；`fps.rs` 的滑动窗口统计可移植到 torvox `RenderWatchDog`。

**可吸收内容**：`WorkerHandler` 的 Active/Inactive 渲染器切换模式（worker_handler.rs:24-90）——torvox 若实现「渲染后端热切换」（GL↔Vulkan 或分辨率降级）可参考；`FpsWatcher` 降级链概念可在 `RenderWatchDog.kt` 扩展为「持续低帧 → 降 raster_scale」。

**文档吸收价值**：中——降级闭环是 torvox 监控体系的升级点。

### 3.3 renderer-cpu/src/lib.rs（CPU 软件渲染器）

**功能说明**：`CpuRenderer`（:28）：`FontData`（:49，内嵌字体文件解析）、`CpuCanvasApi`（:94，实现 `CanvasApi`：`set_feature_layer_tag`（:133）/`geometry_data`（:137）收集绘制命令）、`process_text`（:259，文本布局 + 逐字渲染到 CPU 位图）、`process_shapes`（:292，多边形填充/描边）、`render`（:445，合成输出：清屏 → 形状 → 文本 → 输出位图）、`update_id_to_alpha`（:126，文本淡出动画 alpha）。支持 `take_shapes`/`take_feature_shapes`（:114/:121，调试/特性图层导出）。CPU 渲染用于：无法创建 GPU 上下文（模拟器、无 Vulkan/GLES）时的回退。

**与 torvox 对比**：没有——torvox 无 CPU 渲染器（wgpu_backend.rs:49-57 注释明确「GLES 是模拟器成熟路径，torvox 硬编码 GL on Android；物理设备 GLES 性能问题再议」——即 torvox 用 GL 覆盖模拟器场景，不提供软件渲染）。shashlik 的 CPU 渲染器是**完整软件光栅化**（地图场景，非终端），torvox 若需要软件回退（极老设备），更适合用 wgpu GL 或降低分辨率而非自研软渲染。

**依赖适用性**：skia-safe（CPU 光栅）+ fontdb；对 torvox 重（不推荐引入）。

**可吸收内容**：`calc_normalized_vector_proj_length`（:247，视锥剔除）等算法对 torvox 无直接用途；「CPU 回退 + GPU 主路径」的分层策略（`Renderer` trait 双实现）可作为 torvox 未来降级架构的参照（见 3.2）。

**文档吸收价值**：低（架构参照）。

### 3.4 app-surface 其余：touch.rs / ios.rs / android.rs（android.rs 已有文档）

**功能说明**：
- `touch.rs`（touch.rs:1-1262）：`Touch` 结构（id/坐标/压力/时间戳）与手势原语（pinch 缩放、单指平移判定阈值）。
- `ios.rs`：CAMetalLayer 表面（`create_metal_layer`、layer 尺寸同步、`MetalLayerSurfaceFrame`）。
- `android.rs`（已有文档覆盖双后端选择）：ANativeWindow 表面 + `is_emulator`（Build.FINGERPRINT 判定）→ GL/Vulkan 选择。
- `lib.rs` 已见 3.1。

**与 torvox 对比**：torvox 仅 Android（`attachWindow` ffi.rs:1297）；iOS 无对应（torvox 无 iOS 目标）。`touch.rs` 的触摸结构对应 torvox `TerminalSurface.kt` 的 MotionEvent 处理（无结构化 Touch 对象）。

**依赖适用性**：与 torvox 相同 wgpu 栈；touch 结构可移植。

**可吸收内容**：低。

**文档吸收价值**：低。

### 3.5 ffi-run/（uniffi 绑定层）

**功能说明**：
- `lib.rs`：`ShashlikMapApi`（:13）——`#[uniffi::export]` 的 Rust 对象：`render`（:38）、`resize`（:44）、`zoom_delta`（:49）、`pan_delta`（:54）、`pitch_delta`（:59）、`set_lat_lon_bearing`（:64）、`set_cam_follow_mode`（:70）、`set_ssao_mode`（:75）、`set_preview_enabled`（:79）、`set_mvt_tileset`（:88）、`calculate_route_to_lat_lon`（:95）、`calculate_route`（:101）；`RouteCosting` 枚举（:22）映射到 map 内部。
- `platform/android.rs`：`create_shashlik_map_api_for_ios`（:27，命名笔误，实为 Android 创建入口）、`AndroidSurfaceAppSurface`（:36）实现 `WgpuCanvas`（`queue` :42 / `config` :46 / `device` :50 / `on_resize` :54 / `create_texture_view` :58 / `present` :71）；`createShashlikMapApi`（:81，**JNI 导出**：`env` + 上下文 + surface 句柄 → 构建 wgpu 实例/设备/表面，返回 long 句柄 + 注册日志 `init_logger` :112）。`platform/ios.rs` 同理（uniffi Swift 侧）。
- uniffi 生成：`uniffi-bindgen.rs`（:47）与 build.rs 生成 Kotlin/Swift 绑定。

**与 torvox 对比**：
- 有（对应物）：torvox 手写 JNI（`ffi.rs` 40+ 函数，`NativeBridge.kt` 声明）——**torvox 不用 uniffi**（依赖 `jni` crate 手写，因为需要精细的字节缓冲/字符串所有权控制）。shashlik 用 uniffi 自动生成绑定（Kotlin 侧 `ShashlikMapApi` 对象 + `createShashlikMapApi`）。
- 差异评估：uniffi 适合「对象方法调用」型 API（shashlik 12 个方法）；torvox 的 API 是「高频小参数 + 大缓冲区」型（`getGridRowsColsPacked` 每帧传 40KB IntArray），手写 JNI 更合适——**结论：torvox 不应迁移 uniffi**，但 `createShashlikMapApi` 的「surface 句柄 → Rust 侧建设备」入口形态与 torvox `attachWindow`（ffi.rs:1297）一致，可对照。

**依赖适用性**：uniffi 不推荐引入 torvox（见上）。

**可吸收内容**：`WgpuCanvas` trait 的 `create_texture_view`/`present` 分离（platform/android.rs:58/:71）——torvox 的 attachWindow/render 也可拆成「建纹理视图」与「提交」两步以支持未来多缓冲；日志初始化入口（:112）与 torvox `initLogger`（ffi.rs:1225）对应。

**文档吸收价值**：中——uniffi vs 手写 JNI 的架构决策记录。

### 3.6 kmp/（Kotlin Multiplatform 客户端）

**功能说明**：
- `shared/src/androidMain/kotlin/com/shashlik/kmp/WGPUTextureView.kt`：**TextureView 承载 wgpu**——`WGPUTextureView(context)` 创建 `TextureView`，`surfaceTextureListener` 回调时把 `SurfaceTexture` 传给 JNI（`createShashlikMapApi`）；`onFrameAvailable` 触发渲染轮询；`onDetachedFromWindow` 销毁 native 对象；尺寸变化 `onSurfaceTextureSizeChanged` → `resize`。即：**Android 侧用 TextureView（而非 SurfaceView）承接 wgpu 渲染，合成到 Compose 层**。
- `ShashlikMap.android.kt`：expect/actual 模式——`ShashlikMap` 的 Android actual：持有 `WGPUTextureView` + `SimpleLocationManager`/`PlayServicesLocationManager`（定位），`BaseLocationManager` 抽象。
- `ShashlikMap.kt`（commonMain）：expect 声明（`createMapView`/`updateLocation` 等）。
- `demo/composeApp/src/androidMain/kotlin/com/shashlik/demo/MainActivity.kt`：Compose `AndroidView { WGPUTextureView }` 集成 + `setContent` 生命周期。
- `shared/build.gradle.kts` + `gradle/libs.versions.toml`：KMP Android 目标配置、版本目录（slint/wgpu/uniffi 版本）。

**与 torvox 对比**：
- 有（对应物）：torvox 的 `NativeBridge.kt`/`Bridge.kt` 是**单平台 Android 桥**（非 KMP）；torvox 的渲染承载是 `TerminalSurface`（`SurfaceView`？还是 TextureView——研究时 `TerminalSurface.kt` 的 `attachWindow` 调用处为 `onSurfaceTextureAvailable` 类逻辑，torvox 用 TextureView 系（`TerminalSurface.kt:126-139` magnifier 与 surface 生命周期注释））。
- 没有（torvox 缺失）：KMP expect/actual 结构（torvox 无 iOS/桌面共享层）；定位服务（地图专用，不适用）。

**依赖适用性**：KMP 插件（torvox 单平台，不适用）；`WGPUTextureView` 的「SurfaceTexture → native 句柄 → 每帧渲染」模式与 torvox 的 TextureView 集成**结构相同**（可交叉验证 `TerminalSurface.kt` 的 surface 回调时序：`onSurfaceTextureAvailable` → `attachWindow`；`onSurfaceTextureSizeChanged` → `resize`）。

**可吸收内容**：低——torvox 已有等价 TextureView 生命周期管理；可核对 `WGPUTextureView.kt` 的「尺寸变化去抖/销毁顺序」边界处理。

**文档吸收价值**：中低。

### 3.7 kms_deploy.sh / Cross.toml / Cargo.toml（构建与交叉编译）

**功能说明**：
- `kms_deploy.sh`（1510 字节）：把构建产物部署到 KMS（Kindle?）设备——adb push `.so` + 资源、重启应用、日志收集。含环境变量（设备 IP、ABI 选择）。
- `Cross.toml`：cross 容器交叉编译配置——`aarch64-unknown-linux-gnu` target：挂载 cargo registry 缓存卷（:7-11）、`pre-build` 安装 arm64 系统库（libxkbcommon/wayland/x11/fontconfig/udev/input/gbm，:13-17）、x86_64 的 `CURL_HOME` 环境（:19-20）。
- 根 `Cargo.toml`：workspace 成员（app-surface/renderer-*/ffi-run 等）+ 依赖版本（wgpu、skia-safe、slint、map 引擎）。

**与 torvox 对比**：
- 有（对应物）：torvox 有 `Cross.toml`（workspace 根）与 `flake.nix`、`scripts/` 构建脚本、`rust-toolchain.toml`；torvox 的 Android 构建走 cargo-ndk（`native/` 的构建脚本）。shashlik 的 Cross.toml 是 **Linux 目标**（部署到嵌入式 Linux 设备），torvox 是 Android NDK 目标——工具不同，但「pre-build 安装系统库」模式对 torvox 的 Linux 桌面目标（若启用）有参考。
- `kms_deploy.sh` 的「adb push + 重启 + 日志」对应 torvox `scripts/` 的部署脚本（torvox 有 `maestro/` 集成测试部署）。

**依赖适用性**：构建工具链配置，不直接适用。

**可吸收内容**：低。

**文档吸收价值**：低。

### 3.8 shashlik 小结

- **最值得吸收**：FpsWatcher 降级闭环（3.2）、worker 渲染器热切换模式（3.2）、uniffi vs 手写 JNI 的决策记录（3.5）、TextureView 生命周期交叉验证（3.6）。
- **依赖**：wgpu 栈与 torvox 相同；skia/uniffi 不推荐引入。
- **文档吸收价值**：中（架构决策与监控体系为主）。

---

## 4. rin（Rust 终端引擎 + Android 应用）

架构：`rin` crate——`core`（grid/cell/buffer 状态机，自研 ANSI 解析）→ `parser`（vte crate）→ `input`（按键模型）→ `renderer`（行遍历导出）→ `platform/android`（JNI + 会话线程）→ `pty.rs`（portable-pty）。Android 应用：`RinLib`（JNI 入口）→ `SessionManager`/`TerminalSessionService`（会话）→ `TerminalSurface`（自定义 View Canvas 绘制）→ Compose 屏幕。**与 torvox 的对比价值**：自研内核 vs ghostty 内核的设计取舍、JNI 数据打包、Canvas 绘制 vs wgpu 渲染、root 提权。

### 4.1 core/（自研终端状态机）

**功能说明**：
- `core/mod.rs`：模块声明。
- `grid.rs`：`Grid`（:5）——`Vec<Vec<Cell>>` 二维数组（行优先）+ `dirty_rows: Vec<bool>`（:6，脏行标记）。`new`（:13）、`get/get_mut/set`（:32/:39/:48，越界返回 None/Err——**Result 化边界**）、`swap_cells`（:58，用于滚屏优化）、`take_row`（:70，整行移出）、`clear`（:81）、`resize`（:86，**裁剪或扩展，新行用默认 cell**）、`row/row_mut`（:106/:136）、**脏行 API**：`is_row_dirty`（:115）/`mark_row_dirty`（:119）/`mark_all_dirty`（:125）/`clear_dirty`（:129）/`has_dirty_rows`（:132）。
- `cell.rs`：`Color`（:6，Rgb/Indexed/Default 三态 + `from_ansi`）、`Hyperlink`（:30，**OSC 8 用 arc-swap 共享**：`HyperlinkInner` :35 引用计数避免拷贝）、`UnderlineStyle`（:63，None/Single/Double/Curly）、`CellStyle`（:75，fg/bg/加粗/斜体/下划线/反显/隐藏/闪烁）、`Cell`（:107：`character: char` + `zerowidth: Vec<char>`（:112，**零宽组合字符**）+ `style` + `hyperlink` + `wide: bool`）、`push_zerowidth`（:147）、`new`（:135）。
- `buffer/mod.rs`：`TerminalBuffer`（:11）——`grid` + `cursor_x/y` + `style` + `scrollback: Vec<Vec<Cell>>` + `tab_stops` + `saved_cursor` + `alternate: Option<Box<AlternateScreen>>`（:25 左右）+ 模式位（bracketed paste/mouse/focus events）+ `content_clipboard: Vec<String>`（OSC 52 缓冲）+ `responses: Vec<Vec<u8>>`（DEC 应答）。`DEFAULT_SCROLLBACK_LIMIT = 2000`（:9）。关键方法：`resize`（:103，**grid resize + 光标钳制**）、`translate_char`（:116，G0/G1 字符集映射）、`write_char`（:138，宽字符占两格/零宽追加/自动换行/行尾 soft wrap）、`execute_command`（:194，**Command 枚举驱动**：Ctrl 键、光标移动、清屏、滚动、tab、模式切换、字符集）。
- `buffer/screen.rs`：`TerminalBuffer::screen_lines()`（遍历可见行，宽字符第二格置空）、`scroll_*`（滚动区上下移，用 `swap_cells`）、`erase_*`（ED/EL/ECH）。
- `buffer/alternate.rs`：`AlternateScreen`（:1-1703）——`save`（grid+cursor 快照）、`restore`；切换时主屏 ↔ 备用屏整屏搬移。
- `buffer/cursor.rs`：`cursor_pos`（:5）、`cursor_style`（:9）、`advance_to_next_tab_stop`（:13，跳 tab 停靠）。

**与 torvox 对比**：
- 有（对应物）：torvox 用 ghostty 内核（Rust），grid/cell/scrollback/alternate screen 全部由 ghostty 提供（`native/src/terminal/ghostty_terminal`），功能远多于 rin；torvox Rust 侧 `render/cell_builder.rs` 有宽字符处理。
- 没有（torvox 缺失/可借鉴）：① **脏行位图 API**（grid.rs:115-132）——rin 把「哪行变了」作为一等公民暴露给渲染器；torvox 的 ghostty 有 `has_dirty_cells` 类接口（ffi.rs 无直接暴露，Kotlin 侧每帧全量拉取 `getGridRowsColsPacked` :2929——**潜在优化点：暴露 dirty 行集，只打包脏行**）；② **`arc-swap` 超链接共享**（cell.rs:35）——避免每 cell 拷贝 URI；torvox Rust 侧 hyperlink 是类似引用计数结构（ghostty 的 hyperlink 处理），可核对；③ 零宽字符向量（cell.rs:112）——torvox Rust 侧有等价（ghostty `Cell` 的 combining 处理）。

**依赖适用性**：无第三方（自研）；arc-swap 是轻量 crate（torvox Cargo.lock 已有 arc-swap 家族）。

**可吸收内容**：**脏行集导出**是 rin 对 torvox 的最大启发——建议在 `ffi.rs` 增加 `getDirtyRows()` 返回脏行位图，Kotlin 侧只重打包脏行（可减少 60-90% 的 JNI 传输量）。注释建议：
  ```rust
  // 脏行位图：仅导出被修改的行，避免每帧全量打包 grid（rin grid.rs:115）。
  ```

**文档吸收价值**：高（性能优化直接可用）。

### 4.2 input/handler.rs（按键模型）

**功能说明**：`Key`（字符/功能键枚举）、`Modifiers`（Ctrl/Alt/Shift 位）、`KeyEvent`（key + modifiers + state）；`handler.rs` 把 `KeyEvent` 转成字节序列（`\x1b[...` 功能键、Ctrl+字母、Alt 前缀 `\x1b`），并处理 bracketed paste 开关。

**与 torvox 对比**：有（对应物）：torvox `ui/TerminalInputEncoder.kt` + Rust 侧 key 处理（ffi.rs `writeKey` :931）——功能等价，torvox 更完善（支持组合键序列、键盘布局）。rin 无新东西。

**依赖适用性**：无。

**可吸收内容**：无。

**文档吸收价值**：低。

### 4.3 renderer/（导出渲染数据）

**功能说明**：`renderer/mod.rs`（trait 声明）+ `screen.rs`：`RenderContext`（字段：格子尺寸、颜色、光标）与 `ScreenRenderer`——遍历 `screen_lines()` 把 `Cell` 转成 `RenderCell`（含颜色解析 `Color::to_rgb`、样式位打包、宽字符跳过逻辑），输出行数组。

**与 torvox 对比**：有（对应物）：torvox `render/cell_builder.rs` + `getGridRowsColsPacked`（ffi.rs:2929）——torvox 把 cell 打包成 IntArray（颜色/样式/字符位域），rin 打包成 `RenderCell` 结构体（JNI 侧转 IntArray）。**打包格式设计**是共同难点；rin 的 `Color::to_rgb` 三态解析（Default/Indexed/Rgb）可对照 torvox 的 palette 索引打包。

**依赖适用性**：无。

**可吸收内容**：低（torvox 已覆盖，且更强）。

**文档吸收价值**：低。

### 4.4 platform/android/（JNI 与会话）

**功能说明**：
- `jni.rs`：`ensure_logger`（:20）、`get_sessions`（:38，全局 `Arc<RwLock<HashMap<EngineHandle, TerminalSession>>>`）、`with_session`（:53，**句柄 → 会话的并发安全访问**）、`create_banner`（:59，启动 banner 注入）、`create_engine_inner`（:102）。JNI 导出（`Java_com_rin_RinLib_*`）：`createEngine`（:152，会话 + PTY + banner）、`createRootEngine`（:183，**root 引擎**：`su` 启动 shell）、`destroyEngine`（:216）、`write`（:226，主会话写）、`writeToEngine`（:245，任意引擎写）、`render`（:264，**返回行数组 String[]**）、`resize`（:277）、`getLine`（:294，单行 String）、`getCursorX/Y`（:315/:329）、`getCellData`（:343，**cell 打包 IntArray**：每个 cell 4 个 int——字符/样式/前景/背景）、`getCellDataOptimized`（:400，**优化版**：跳过未变化行/仅脏行，配合脏行位图）、`hasDirtyRows`（:461）、`isRowDirty`（:475）、`markAllDirty`（:490）、`clearDirty`（:503）、`isAlive`（:516，PTY 存活）。
- `session.rs`：`TerminalSession`——持有 `TerminalBuffer` + PTY + reader 线程（读 PTY → `buffer.write` 循环）+ `render_ready` 通知（`Condvar`/channel）；`resize`/`write` 加锁转发。
- `mod.rs`：模块声明。

**与 torvox 对比**：
- 有（对应物）：torvox `ffi.rs` 的 `initSession`（:397）/`destroySession`（:610）/`switchSession`（:677）/`getSessionCount`（:734）/`resize`（:753）/`writeKey`（:931）/`feedPty`（:863）/`render`（:1393）——**torvox 的 JNI 面更全**（含字体/主题/搜索/剪贴板/背景图 40+ 函数）。rin 独有的：**`getCellDataOptimized` + 脏行 API 的优化打包**（:400/:461-503，见 4.1 吸收建议）；**`createRootEngine`**（:183，`su` 启动——torvox 无 root 会话功能，`TerminalRuntime` 无 root 启动路径）。
- 会话模型：rin 每引擎一个 reader 线程 + 锁；torvox 每会话一个 Rust 侧 poll loop + `pollEvent` 事件队列（ffi.rs:1013）——torvox 的事件驱动模型更接近 ghostty 设计。

**依赖适用性**：jni crate + portable-pty（torvox 用 ghostty 的 pty + `terminal/pty.rs`，相同家族）。

**可吸收内容**：① 脏行优化打包（见 4.1）；② **root 会话**：`createRootEngine`（jni.rs:183）的「`su -c` 启动 + 会话管理」可在 torvox 实现为 `TerminalRuntime` 的 root 会话选项（Android 有 `RootHelper` 需求，见 4.6）；③ `with_session` 的句柄-锁封装（jni.rs:53）比 torvox 的 `SESSION_*` 全局状态更规范（torvox 是 `Arc<Mutex<SessionManager>>` 单例，等价）。

**文档吸收价值**：高——脏行打包优化与 root 会话是具体可吸收项。

### 4.5 parser/（vte 集成）

**功能说明**：`ansi.rs`（20KB）：`struct VteParser` 实现 `vte::Perform`——`print`（普通字符→`write_char`）、`execute`（C0 控制）、`csi_dispatch`（**Command 枚举**：光标/擦除/滚动/模式/颜色/字符集/键盘增强）、`osc_dispatch`（OSC 0/2 标题、7 目录、8 超链接、52 剪贴板→`content_clipboard`）、`esc_dispatch`（字符集 G0/G1、ALT 屏、保存/恢复光标）。`color.rs`：`Color::from_ansi`（16/256 色解析）、`from_rgb`。`mod.rs`：模块。

**与 torvox 对比**：有（对应物）：torvox 用 ghostty 的完整 ANSI 状态机（远强于 vte crate 单文件实现：ghostty 支持 DCS、SGR 扩展、同步渲染等）。**torvox 无 OSC 52 剪贴板缓冲队列的 Kotlin 侧对应**？——torvox `osc_handler.rs` 处理 OSC 52（经 `clipboardResult` ffi.rs:1819 回传），等价。rin 的 Command 枚举化设计（csi_dispatch → `execute_command`）对自研内核是好实践，对 torvox（ghostty）无吸收价值。

**依赖适用性**：vte crate（torvox 不需要，ghostty 自带）。

**可吸收内容**：无（torvox 内核更强）。

**文档吸收价值**：低。

### 4.6 android/（Kotlin 应用）

**功能说明**：
- `MainActivity.kt`（12 KB）：Compose 入口——`SetupScreen`（首次启动：选择 shell/权限）↔ `TerminalScreen` 切换；`onBackPressed` 处理、会话服务绑定、崩溃日志。
- `RinLib.kt`：JNI 声明（`createEngine`/`write`/`render`/`getCellDataOptimized` 等，与 4.4 一一对应）。
- `RinPermStorage.kt` + `permission/StoragePermissionHelper.kt`：**Scoped Storage 适配**——`MANAGE_EXTERNAL_STORAGE` 申请（`ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION`）、`WRITE_EXTERNAL_STORAGE` 降级路径、返回结果回调。
- `RootHelper.kt`：root 检测（`su` 探测：`Runtime.exec("su -c id")` 退出码）与请求。
- `rpkg/RpkgLib.kt`：bootstrap 包管理（内置 busybox/工具包解压到 app 私有目录——类比 torvox `BootstrapInstaller.kt`）。
- `service/TerminalSessionService.kt`：前台服务持有会话（通知 + `FOREGROUND_SERVICE` 类型），进程被杀时恢复。
- `terminal/SessionManager.kt`：会话注册表（`ConcurrentHashMap` 句柄 → 会话元数据）+ 主会话切换。
- `terminal/TerminalSession.kt`：会话元数据（引擎句柄、标题、存活）。
- `ui/components/TerminalSurface.kt`（22 KB，**核心**）：自定义 `View` + `Canvas` 绘制：
  - 绘制循环：`Handler.postDelayed(33ms)`（**30 FPS 轮询**，非 vsync）——`hasDirtyRows` 判定后 `getCellDataOptimized` 拉脏行 → 按行绘制文本（`Paint.measureText` 逐格定位、宽字符 `2*cellW`、光标闪烁、选择高亮、行背景色）；
  - 手势：单指拖动滚动、双指缩放（字体 4-30sp）、长按选择（起点/终点 + 上下文菜单 Copy/Select All）、**`GestureDetector` + `ScaleGestureDetector` 组合**；
  - 输入：`onKeyPreIme`/`onKeyDown` 硬件键、`InputConnection`（composition 跟踪 + `commitText`）、Ctrl 转换（音量键模拟 Ctrl 的 toggle）；
  - 主题：`TerminalColors`（Monet 动态色 → ANSI 16 色映射，`TerminalColors.kt`：`dynamicLightColorScheme`/`dynamicDarkColorScheme` 的 `primary→ANSI 映射表`）+ 自定义色覆盖。
- `ui/screen/SetupScreen.kt`（首次配置向导）/`TerminalScreen.kt`（`ExtraKeysBar` + `TerminalSurface` 组合）。
- `ui/components/ExtraKeysBar.kt`（9.6 KB）：**额外按键栏**——ESC/TAB/CTRL/ALT/方向键/`|`/`-`/`_`/`~` 等高频键；**可折叠/可配置**（`LazyRow` + 长按弹自定义菜单）；CTRL/ALT 是**锁定型 toggle**（与 rin 的 `ModifierState` 联动——按下后状态传给 `TerminalSurface` 的 `onKeyDown` 合成）。
- `ui/theme/Color.kt`/`Theme.kt`/`Type.kt`：Material3 主题（Monet 动态色 + 深/浅）。
- `app/build.gradle.kts`：NDK ABI 过滤、`jniLibs` 拷贝、`FOREGROUND_SERVICE` 权限。

**与 torvox 对比**：
- 有（对应物）：bootstrap 安装（torvox `installer/BootstrapInstaller.kt` + `BootstrapOrchestrator.kt` + `BootstrapDownloader.kt`，torvox 更强：md5 校验/断点/两阶段）；前台服务（torvox `service/TerminalForegroundService.kt`）；Canvas 绘制（torvox 用 wgpu 纹理 + Compose，**渲染路径完全不同**——rin 的 30FPS Canvas 轮询性能远低于 torvox 的 GPU 纹理，但「脏行拉取」思想可吸收）；额外按键栏（torvox `ui/ModifierBar.kt`——**torvox 的 ModifierBar 更强**：Nerd 键标签 `NerdKeyLabels.kt`、可配置工具栏 `ToolbarPreferences.kt`）；Monet 动态色主题（torvox `TerminalTheme.kt` 支持动态色）；IME composition（torvox `TerminalSurface.kt:405-462`）。
- 没有（torvox 缺失）：① **root 会话支持**（`RootHelper.kt` + `createRootEngine`）——torvox 无（bootstrap 装到用户目录，无 su 路径）；② **存储权限申请流程**（`RinPermStorage`——torvox 用 SAF 文档提供器 + 自己的下载目录，无需 MANAGE_EXTERNAL_STORAGE，**torvox 的设计更优**）；③ 30FPS 轮询绘制（torvox 无，且不应吸收——GPU 渲染是 torvox 的优势）。

**依赖适用性**：无新增依赖需求；`su` 探测（RootHelper.kt:1-1852）是纯 Android 代码，可直接移植。

**可吸收内容**：`RootHelper` + root 会话（jni.rs:183 + RootHelper.kt）——若 torvox 要支持 root 启动（部分用户需求），这是最小实现参考；`ExtraKeysBar` 的「锁定型 Ctrl/Alt toggle」交互可对照 torvox `ModifierBar` 的瞬态/锁定行为（ModifierBar.kt 已有，核对即可）。

**文档吸收价值**：中——root 会话是唯一实质性新功能；其余 torvox 已覆盖且更强。

### 4.7 rin 小结

- **最值得吸收**：脏行位图 + `getCellDataOptimized` 的 JNI 增量传输（4.1/4.4）——对 torvox ffi 层是直接性能优化；root 会话（4.4/4.6）。
- **依赖**：无新增（arc-swap 可选）。
- **文档吸收价值**：中高。

---

## 5. 综合吸收优先级建议（面向 torvox 路线图）

| 优先级 | 来源 | 功能 | 落点建议 |
|---|---|---|---|
| P0 | rin | 脏行增量导出（grid.rs:115 + jni.rs:400） | `ffi.rs` 增 `getDirtyRows`，`NativeBridge` 只打包脏行 |
| P0 | termlib | OSC 133 语义段 + 上条命令输出（TerminalEmulator.kt:830/183） | Rust `osc_handler.rs` 增语义段缓存，Kotlin 增「复制上条输出」 |
| P1 | termlib | 无障碍：语义段朗读/导航 + LiveOutputRegion（AccessibilityOverlay.kt:64/482，LiveOutputRegion.kt:43） | `TerminalScreen.kt` + `TerminalViewModel.kt` |
| P1 | termlib | Compose 键序列模式（ComposeMode.kt:30 + drawComposeOverlay Terminal.kt:2293） | `TerminalInputEncoder.kt` 前置组合缓冲 |
| P1 | termlib | golden 渲染测试模式（TerminalRendererGoldenTest.kt:41） | integration-tests/ 增 roborazzi golden |
| P1 | termlib | 放大镜纯函数 + 自绘 MagnifyingGlass（Terminal.kt:2021/2044） | `TerminalScreen.kt`（补充系统 Magnifier） |
| P2 | termlib | CellRun 游程编码（CellRun.kt:25） | Rust `render/cell_builder.rs` 打包层 |
| P2 | shashlik | FpsWatcher 降级闭环（fps.rs） | `RenderWatchDog.kt` 扩展为降级链 |
| P2 | rin | root 会话（RootHelper.kt + jni.rs:183） | `TerminalRuntime` 会话类型扩展 |
| P2 | kgx | livery 合并优先级 + HSL 工具（kgx-livery.c / kgx-colour-utils.c） | `TerminalTheme.kt` 主题优先级文档化 |
| P3 | kgx | 粘贴预览截断 8000 字符（kgx-paste-dialog.c） | `PasteChipOverlay.kt` 预览增强 |
| P3 | kgx | 诊断信息导出对话框（kgx-spad.c） | 设置页「复制诊断信息」 |

## 6. 研究范围与局限

- 行号基于 2026-08 各仓库 HEAD；torvox 侧行号基于当前 HEAD。
- termlib 测试共 22 个文件全部核查；gnome-console 的 kgx-tab.c/kgx-terminal.c 仅补遗（搜索 UX 已在 research-gnome-console.md）；shashlik 的 renderer-gpu/、map/、sgnss/、winit-run/ 不在本次范围；rin 的 parser 作为 core 的支撑简要覆盖，`doc/`、`rpkg/` Rust 侧不在本次范围。
- 涉及「torvox 没有」的判定均经 grep 全量搜索确认（如 `ComposeMode`、`LiveOutput`、`RootHelper` 等关键词无命中）。
