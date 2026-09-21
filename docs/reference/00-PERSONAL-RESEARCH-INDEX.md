# 亲自逐文件研究进度索引

> 更新：2026-08-06 | 研究方式：主代理亲自 `read_file` 逐文件阅读（非子代理）
> 子代理版本文档保留于同目录（research-*.md），personal/补充文档为亲自阅读结果

## 已亲自逐文件阅读的仓库（按完成顺序）

| 仓库 | 亲自阅读文件 | 文档 |
|------|-------------|------|
| **wgpu-in-app** | app-surface/（lib.rs 491 全、android.rs 123 全、ios.rs 89 全、touch.rs 54 全、app_surface_use_winit.rs 99 全、unsupported.rs 20 全）；wgpu-in-app/（lib.rs 39、wgpu_canvas.rs 57、ffi/android.rs 40、ffi/ios.rs、examples/mod.rs、boids.rs 前 80）；Kotlin 4 文件全；Swift 2 文件全；构建脚本/gradle/cargo-so 前 60 | `research-wgpu-in-app.md`（重写为亲自版，17KB） |
| **zelland** | renderer/mod.rs 1123 全、renderer/android.rs 216 全、terminal.rs 252 全、ghostty.rs 335 全、lib.rs 266 全、MainActivity.kt 1047（前后关键）、KeySeqs.kt 38 全、KeybarPlugin.kt 前 150、WGPU_FIXES.md 全、WGPU_GHOSTTY_PLAN.md 前 60；ssh/daemon/helper/keystore/network 结构扫描 | `research-zelland-personal.md`（21.9KB） |
| **termux-app** | TerminalBuffer.java 497 全、TerminalRow.java 283 全、TerminalSession.java 373 全、ByteQueue.java 108 全、JNI.java 41 全、TextStyle.java 结构、TerminalEmulator.java 结构；TextSelectionCursorController 前轮已亲自精读 | `research-termux-app-personal.md`（14.5KB） |
| **ghostty-android** | TerminalView.java 选择系统（前轮亲自精读 showSelectionUi/selectionGeometryKey/reshowToolbar/dragSelectionTo/placeHandle/tapCount 等）+ 渲染层（onDraw 1509-1600、drawRowText 1800-1870、kitty graphics、壁纸） | `research-ghostty-android.md` + `research-ghostty-android-render.md` |
| **Haven** | SelectionToolbar.kt（前轮亲自精读 expandSelectionToWord/expandAcrossUrlWrap/smartCopy/SmartTerminalClipboard）+ FloatingTextInputDialog.kt（核心 189-350） | `research-haven.md` + `research-haven-floating-input.md` |
| **termlib** | SelectionManager.kt（前轮亲自精读）+ SemanticType.kt 全 + ImeInputView.kt（核心 40-320）+ ComposeController.kt 57 全 | `research-termlib.md` + `research-termlib-ime.md` |

## 仍为子代理研究（待亲自阅读，按优先级）

| 仓库 | 现状 | 优先级 |
|------|------|--------|
| warp | pty.rs/bootstrap.rs 亲自读（research-warp.md）；lib.rs JNI 全集/IME 为子代理（research-warp-extra.md） | 高 |
| gnome-console | kgx-tab 搜索亲自读；其余子代理 | 中 |
| fission / zed-port | 全子代理（2484/1340 文件，核心需抽读） | 中 |
| osmosis / shashlik / rin | 子代理（小仓库可快速亲自补） | 低 |
| sushi-ssh / moke / neotermux / reterminal / terminator / termx / redterm / ply / onecode / cpmdroid / termux-kotlin / ghostling / wgpu-example | 子代理 | 低 |

## 亲自阅读确认的关键发现（P0 级）

1. **termux wrap 感知 getSelectedText**（TerminalBuffer.java:60-106）：torvox extractSelectedText 每行硬插 `\n`——复制长输出/软换行拼接错误
2. **termux findStartOfColumn 宽字符换算**（TerminalRow.java:92-128）：torvox substring(col) 会切错 CJK 宽字符
3. **Popup 内 startActionMode(TYPE_FLOATING) 静默 no-op**（Haven FloatingTextInputDialog.kt:202-226）：torvox 菜单在 View 层级无此问题，但必须记录防误用
4. **zelland SGR 鼠标编码**（terminal.rs:96-175，ghostty_mouse_encoder）：torvox 唯一真缺口
5. **atlas 格式必须匹配 surface 格式**（zelland WGPU_FIXES.md Fix 1）：torvox 已规避（swash 自研）
6. **IME 普通模式 PASSWORD 键盘变体**（termlib ImeInputView.kt:91-121）：torvox 键盘是否带数字行待核查
7. **/proc/pid/cwd 查询 + "[Process completed]" 提示**（TerminalSession.java:297-315, :353-364）：MCP terminal_info 增强候选

## 第二轮补充（2026-08-06 晚）

| 仓库 | 亲自阅读文件 | 文档 |
|------|-------------|------|
| **warp** | lib.rs PTY 段（:743-958）+ IME 段（:1343-1428）、ime.rs 159 全、input.rs 506 全、terminal_model.rs 302 全 | `research-warp-jni-ime.md` |
| **gnome-console** | kgx-process.c 268 全、kgx-paste-dialog.c 199 全、kgx-settings.c 结构 | `research-gnome-console-extra.md` |
| **fission** | examples/terminal/src/lib.rs 296 全、fission-widgets/src/terminal.rs 核心（TerminalSessionInner/View/RenderNode） | `research-fission-terminal.md` |
| **shashlik** | fps.rs 全、app-surface/src/lib.rs 前 120、renderer-cpu 结构 | `research-shashlik-extra.md` |
| **termux-kotlin** | ArgumentTokenizer.kt 208 全、AmSocketServer.kt 核心 | `research-termux-kotlin-shell.md` |

## 第二轮新确认的 P0/P1 发现

1. **ArgumentTokenizer（P0）**：BSD 四态机分词器 → MCP run_command 的 command_string→argv
2. **stats_string 驱动契约（P1）**：JNI 诊断字符串 + 测试锁定 schema → torvox 集成测试模式
3. **SO_PEERCRED（P1）**：MCP socket 纵深防御
4. **fps.rs O(1) 滑动窗口 FPS（P2）**：性能测试工具
5. **IME 空 finish 防双提交（P1 记录）**：Gboard setComposing→finish→commit
6. **TouchCancel 完整性（P1 记录）**：torvox selection 拖拽需查 ACTION_CANCEL
7. **root 进程 UI 提示 / 粘贴确认（P2）**：gnome-console 安全 UX

## 剩余待亲自阅读（第三轮）

- zed-port（2484 文件，抽读 crates/terminal/ + gpui_android 核心）
- moke（122 文件，SSH 传输层）
- terminator / termx / sushi-ssh / neotermux / reterminal（Kotlin 终端）
- osmosis / rin / wgpu-example / ghostling（小仓库）
- ply / onecode / cpmdroid / redterm（低价值，可快速扫）
