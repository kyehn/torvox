# Reference Project Analysis

Comparative architecture analysis with peer terminal emulator projects,
conducted August 2026 to validate torvox design decisions.

---

## 1. Peer Projects

| Project | Language | Render | JNI Bridge | SSH | MCP | Notes |
|---------|----------|--------|------------|-----|-----|-------|
| **torvox** (us) | Rust + Kotlin | wgpu (Vulkan) | Direct JNI | External exec-bin | ✅ 8 tools | Ghostty VT engine |
| **chuchu/Ghossh** | Zig + Kotlin | None (Canvas) | Zig JNI | ✅ libssh2 | ❌ | Ghostty VT; 400+ themes; Tailscale |
| **mightty** | Rust (Windows) | GPUI | N/A | ❌ | ❌ | Ghostty VT; build.rs bindgen |
| **Haven** | Kotlin + Rust | None | UniFFI | ✅ JSch | ✅ 130+ tools | PRoot Linux; multi-protocol |
| **Termux** | Kotlin | None (Canvas) | N/A | External ssh | ❌ | apt/pkg; 6 plugins |
| **ghostling** | Zig+Swift | Metal | Zig | ❌ | ❌ | iOS/macOS Ghostty |
| **ghostty-android-terminal** | Kotlin | None | N/A | ❌ | ❌ | arm64chroot; rootfs tarball |

---

## 2. Architecture Comparison

### 2.1 VT Engine

All modern projects use **Ghostty** (`libghostty-vt`):
- **torvox** and **mightty**: Rust wrapper via `libghostty-rs` git dependency
- **chuchu/Ghossh**: Zig wrapper (direct C ABI)
- **ghostling**: Swift wrapper (iOS)

**torvox advantage**: Single source of truth — no parallel data model.
All terminal state comes from Ghostty C API; CellData is extracted for rendering.

### 2.2 GPU Rendering

**torvox** is unique among Android terminals in using full GPU rendering (wgpu/Vulkan).
Others use `Canvas.drawText` per cell (CPU-bound, poor CJK performance).

| Project | Renderer | Performance | CJK Support |
|---------|----------|-------------|-------------|
| **torvox** | wgpu (Vulkan) | 0.2ms typing, 5107 fps | ✅ Font fallback + glyph cache |
| **chuchu** | Canvas.drawText | CPU-bound, ~5 fps scrolling | ❌ Basic |
| **Haven** | Canvas.drawText | CPU-bound | ✅ Through PRoot |
| **Termux** | Canvas.drawText | CPU-bound | ✅ Through Android |

### 2.3 JNI Bridge

| Project | Method | Safety | Complexity |
|---------|--------|--------|------------|
| **torvox** | `jni` crate (Rust) | ✅ SAFETY comments on all unsafe | Low — direct JNI |
| **chuchu** | Zig `export fn` | ✅ Zig ABI compatible | Low — Zig exports C ABI |
| **Haven** | UniFFI | ✅ Auto-generated bindings | Medium — codegen |
| **Termux** | N/A (pure Kotlin) | N/A | N/A |

**torvox choice (direct JNI via `jni` crate)**: Good tradeoff. The `jni` crate
handles JNI environment management, type conversion, and exception throwing.
The only unsafe code is for NDK functions (ANativeWindow, logging).

### 2.4 Thread Model

| Project | Threads per Session | Notes |
|---------|--------------------|-------|
| **torvox** | 4 (Reader, Writer, Waiter, Renderer) + 1 shared MCP | CellData channel decouples VT from render |
| **chuchu** | 2 (VT + UI) | Simpler but UI blocks on VT |
| **mightty** | PtyWorker (Input/Output/Control parts) | Clean worker separation |

### 2.5 Build System

| Project | Build | Deterministic | Complexity |
|---------|-------|--------------|------------|
| **torvox** | Nix + Cargo + Gradle | ✅ | High (Nix learning curve) |
| **chuchu/Ghossh** | Gradle + NDK + Zig | ⚠️ | Medium |
| **mightty** | Cargo (build.rs compiles Zig) | ✅ | Medium |
| **Termux** | Gradle | ⚠️ | Low |

---

## 3. Features to Consider

### 3.1 High Priority

- **Native SSH integration** (libssh2): Replace external exec-bin binary.
  chuchu/Ghossh show it's feasible; would enable session management,
  key storage, Tailscale integration.

- **Tailscale SSH**: chuchu/Ghossh native support via Tailscale API socket.

### 3.2 Medium Priority

- **Extended MCP toolset**: Haven has 130+ tools. File browsing,
  session control, configuration management.

- **PRoot / user-space Linux**: arm64chroot from ghostty-android-terminal
  or PRoot from Haven. Enables apt/dpkg in Termux-compatible userland.

- **Theme system expansion**: chuchu syncs 400+ Ghostty themes.

- **Backup/restore**: chuchu AES-256-GCM encrypted export.

### 3.3 Low Priority (Future)

- **Plugin architecture**: Termux-style Intent-based plugins.
- **Multi-channel distribution**: F-Droid, GitHub Releases, Google Play.
- **Binding generator**: mightty-style automation for Ghostty C API.

---

## 4. Design Validation

| torvox Decision | Validated By | Result |
|----------------|--------------|--------|
| Ghostty as single source of truth | chuchu, mightty, ghostling all use Ghostty | ✅ Correct |
| wgpu GPU rendering | Unique among Android terminals; necessary for performance | ✅ Correct |
| Direct JNI (no boltffi/JNA) | chuchu uses Zig JNI; both avoid JNA | ✅ Correct |
| embedded MCP | Haven proves MCP value for terminal apps | ✅ Correct |
| Single crate (no multi-crate) | No peer project uses multi-crate for terminal engine | ✅ Correct |
| Nix build | Unique but proven correct | ⚠️ Acceptable |

---

## 5. wgpu Android 先行者（2026-08 新增深度研究）

### 5.1 zelland（njreid/zelland）— **本项目最直接的同类**

Rust + wgpu + glyphon + libghostty-vt + Android 的 SSH 终端客户端，Android 为主平台
（API 7.0+），Linux 桌面半成品。研究价值：**唯一一个"Rust wgpu 渲染 + libghostty-vt
引擎 + Android SurfaceView"的完整先例**，证明该架构可行且踩坑记录完整。

**架构**：
```
Android: DrawerLayout + SurfaceView(wgpu) + WebView(Svelte5 透明 UI) + 原生侧栏
Rust:    renderer/android.rs (JNI) → renderer/mod.rs (wgpu+glyphon 全局单例)
         terminal.rs (TerminalSession{term, render_state})
         ghostty.rs (C FFI bindgen + 手工包装) + ssh.rs (russh, 16ms 节拍)
```

**JNI 面**（renderer/android.rs:18-215）：passSurfaceToRust / passSurfaceDestroyed /
passSurfaceResize / passTouchToRust / getSelectionText / setSelectionHighlight /
passPasteToRust / getCellDimensions / updateFontSizeToRust —— 与 torvox 的 JNI
面惊人相似，可逐函数对照。

**数据流**：SSH 字节 → process_bytes → ghostty_terminal_vt_write → 16ms 节拍
render_native → render_state.update() → draw_ghostty_state（行级 dirty + row_cache）
→ render()（3 pass：clear+cursor → glyphon 文本 → selection overlay）→ present。

**libghostty 用法**（ghostty.rs:15-287，完整 API 清单）：
- `ghostty_terminal_new` / `vt_write` / `resize`（像素=cols×CELL_WIDTH 估算）/
  `get`（COLS/ROWS/CURSOR_X/Y/MOUSE_TRACKING）/ `free`
- `ghostty_render_state_new/update/get/set`（DIRTY、COLS、ROWS、ROW_ITERATOR）
- `ghostty_render_state_row_iterator_new/next/get/set/free`（行级 DIRTY）
- `ghostty_render_state_row_cells_new/get/next/free`（RAW、STYLE、GRAPHEMES_LEN/BUF）
- `ghostty_mouse_encoder_new/setopt_from_terminal/setopt/free` + `ghostty_mouse_event_new`
- 构建：`zig build -Demit-lib-vt=true -Dapp-runtime=none -Doptimize=ReleaseFast`
  静态链 `libghostty-vt` + bindgen（blocklist 不透明类型后手工 `*mut c_void`）；
  **Android 必须处理 Bionic TLS 64 字节对齐**（android_fix.zig / android_tls_fix.c）

**文本选择（zelland 版，比 termux 简化）**：
- 长按（MainActivity.kt:117-125）→ `pixelToCell(x,y)`（JNI getCellDimensions 实时
  cell 尺寸，默认 17×38 兜底）→ selStart 记录，默认终点 = 起点右 12 列
  （`col+12` coerce 255）→ JNI setSelectionHighlight → `startActionMode(TYPE_FLOATING)`
- 菜单：Copy/Paste（android.R.string.copy/paste，SHOW_AS_ACTION_ALWAYS），
  onActionItemClicked → doCopy/doPaste → mode.finish()；onDestroyActionMode →
  `setSelectionHighlight(0,0,0,0,false)` 清除
- 拖拽扩展：SurfaceView OnTouchListener 中 selectionActive 时 ACTION_MOVE 更新
  selEnd 并重新高亮（**无手柄**、矩形选择、只扩不缩、无双向端点）
- doCopy：JNI `getSelectionText(sc,sr,ec,er)` → ClipboardManager
- doPaste：剪贴板 → `"\u001b[200~$text\u001b[201~"` 括号粘贴 → passPasteToRust
- 高亮渲染：第 3 个 render pass（selection overlay），CPU 生成 NDC 顶点矩形
  （update_selection_vertices:510-544，逐行 6 顶点三角形，上限 50 行截断），
  半透明蓝 `vec4(0.3, 0.5, 1.0, 0.35)`，LoadOp::Load 叠加在文本之上
- extract_text（:546-567）：直接从 row_cache（按行 CellRun 列表）按列区间拼字符，
  行间 `\n`；**宽字符/组合字符按 chars() 计数，与 Ghostty 列模型有偏差**（已知）
- 单元格反色：**未实现**（zelland 用 overlay 而非 reverse video；torvox 的
  fg↔bg swap 更接近 termux/VTE）

**WGPU_FIXES.md 全部坑（逐条对照 torvox）**：

| # | zelland 坑 | torvox 对照 |
|---|---|---|
| Fix 1 | **Atlas 格式必须等于 surface 格式**：硬编码 Rgba8UnormSrgb 遇 Android Vulkan 常见 Bgra8Unorm 时 text_renderer 静默丢弃，文字完全不显示且无报错；修法 `caps.formats[0]` 变化时重建 atlas | ⚠️ torvox atlas 固定 Rgba8Unorm（TEXTURE_BINDING\|COPY_DST），shader 直接采样，不经过 glyphon 的 render-to-atlas，故不受此坑影响；但 **cell pipeline 的 render target 格式 = surface 格式（Bgra8Unorm），atlas 采样格式与 render target 格式不同是合法的**（纹理采样无格式一致性要求）。若未来用 glyphon 需重查 |
| Fix 2 | **set_surface 后必须立即 render**：只 resize 不 render 黑屏到首个数据 | torvox attach_surface 后 render_inner 每帧 render，无此问题 |
| Fix 3 | SurfaceView 启动期遮挡 WebView：GONE 起步 + JS 双守卫 | torvox 无 WebView，不适用 |
| Fix 4 | **addView 顺序 = 触摸优先级**：后 add 的先收触摸；setZOrderMediaOverlay 只管合成不管触摸 | ⚠️ torvox Compose 层叠需注意（TerminalSurface 与其他 overlay 的触摸顺序） |
| Fix 5 | 滚动方向：distanceY>0（手指上移）→ scroll_down | torvox round-211 已修（fling 方向） |
| Fix 6 | DrawerLayout 边滑被 SurfaceView 吞：onFling(vx<-600) 编程式 openDrawer | torvox drawer 边缘用 `event.x < drawerEdgePixels` 让出，异曲同工 |
| 7 | viewport.update 必须在 surface.configure() 之后 | torvox 同理 |
| 8 | **SELinux 挡 fontdb 自动发现**：需手动试 /system/fonts/… | ✅ torvox font_db.rs 已手动扫描系统字体 |
| 9 | **resize 要 clamp 到 max_texture_dimension_2d**（有的 GPU 只有 2048） | ⚠️ 未验证 torvox 是否 clamp |
| 10 | surface 三回调 + pending_size 兜底（resize 先于 surface 到达） | torvox attachPendingSurface 已处理 |
| 11 | 鼠标事件必须在 `\x1b[?1000h`（mouse tracking）后才转发 | torvox mouse mode 处理需对照 |
| 12 | 鼠标坐标映射用 renderer::get_cell_size() 实时值而非常量 | torvox 用 font metrics，一致 |
| 13 | Bionic TLS 64 字节对齐（zig 静态库链接 Android） | ⚠️ torvox 用 dylib 避开，若改静态需处理 |

**代码量**：Rust src-tauri ≈4.6k 行（renderer 1.1k）+ daemon-rs 3.5k + Kotlin 1.4k +
Svelte 4.5k。**测试**：Rust ~1k 行 + Kotlin 25 个测试（含 SelectionActionModeTest）。

### 5.2 wgpu-in-app（jinleili）— Android/iOS/Web 集成样板

**定位**：把 wgpu 集成进既有 App（不用 winit 接管窗口）的官方示范。crates.io 发布
`app-surface` 1.13.0。**集成 wgpu 所需样板仅 ~220 行 Kotlin / ~220 行 Swift**。

**Android 关键模式（全部值得抄）**：
1. **Surface 生命周期用 `SurfaceHolder.Callback2` 驱动，不碰 Activity 生命周期**：
   surfaceCreated → `createWgpuCanvas(h.surface, idx)`（JNI 返回 jlong 装箱指针）；
   surfaceDestroyed → `dropWgpuCanvas`（Box::from_raw）；旋转/后台自动重建。
   `surfaceChanged` 空实现。
2. **尺寸不缓存、每帧查询**（ANativeWindow_getWidth/Height，android.rs:77-87），
   天然自适应旋转，无需 resize 通知。
3. **帧循环复用 UI 绘制循环**：`onDraw { enterFrame(obj); invalidate() }` 自驱动；
   要固定帧率换 Choreographer 或独立线程 + surface.get_current_texture。
4. **取帧失败恢复**（lib.rs:222-237）：Timeout/Outdated/Lost → 重新
   surface.configure → 重试；Occluded → 跳过本帧。
5. ANativeWindow 包装：`ANativeWindow_fromSurface`（+1 引用计数）→
   `AndroidNdkWindowHandle::new(NonNull)` → `WindowHandle::borrow_raw`，
   Drop 时 ANativeWindow_release（android.rs:60-96）。
6. **Android 固定 `Backends::VULKAN`**（android.rs:20）——GLES 需另走代码路径；
   README 明说"模拟器/虚拟设备只有 GLES，故不建 x86_64/i686 target"。
   ⚠️ torvox 用 GL 后端（SwiftShader Vulkan swapchain dequeueBuffer 失败），
   与 wgpu-in-app 的结论相反——差异在模拟器 GPU 栈，真机应为 Vulkan。
7. `setZOrderMediaOverlay(true)`（或 OnTop）+ `PixelFormat.TRANSPARENT` 才是
   SurfaceView 透明背景唯一办法；**SurfaceView 默认不绘制，必须
   `setWillNotDraw(false)`**。
8. Android 不支持 view_formats（DownlevelFlags 报错）；webgl 后端同样。
9. wgpu v24+ Instance/Adapter/Device/Queue 可 Clone，Surface 用 Arc 包。

**代码量**：app-surface 1.2k 行 + wgpu-in-app 壳 324 行 + Kotlin 366 行。

---

## 6. 综合结论（2026-08 更新）

### 6.1 选择系统三档实现（从简到全）

| 档位 | 代表 | 做法 | 复杂度 |
|---|---|---|---|
| 1 | zelland | 长按→pixelToCell→默认 12 列→半透明 overlay→FloatingActionMode(Copy/Paste)→ACTION_MOVE 拖拽扩展（无手柄） | 低（~200 行） |
| 2 | termux | 长按→智能词选→reverse video→自绘手柄 PopupWindow×2→FloatingActionMode+onGetContentRect 锚定→拖拽约束/边缘滚动/翻转/延迟显隐 | 高（~800 行） |
| 3 | torvox（当前） | GPU fg↔bg swap + 双手柄 + 智能扩展（URL/词）+ 系统 ActionMode | 中高，已实现大部分 |

**推荐**：torvox 保持档位 3 的 GPU 反色与双手柄，但菜单定位从顶部 ActionMode
改为 `TYPE_FLOATING` + `Callback2.onGetContentRect`（termux 的
TextSelectionCursorController.java:194-213 做法），这才能满足"菜单智能靠近选区、
不遮挡文本"的用户要求——zelland 档位 1 无法满足该要求。

### 6.2 渲染链路对照表（wgpu 坑自查清单）

- [x] atlas 采样格式与 render target 格式不同合法（无需修改）
- [x] set_surface 后立即 render（attach_surface → render_inner 每帧）
- [ ] resize clamp 到 max_texture_dimension_2d（待验证）
- [x] SELinux 字体扫描（手动 /system/fonts）
- [x] surface 三回调 + pending_size（attachPendingSurface）
- [ ] mouse tracking 门控（对照 zelland Fix 11）
- [x] 每帧查询尺寸（ANativeWindow_getWidth 或 Kotlin onSizeChanged）
- [ ] Android 真机 Vulkan vs 模拟器 GL 双路径验证（wgpu-in-app 建议 Vulkan）
- [x] Bionic TLS 对齐（dylib 方案避开，若改静态需 android_fix.zig）

---

## 7. termux-app v0.119.0-beta.3 深度参考（行为正确性最高优先）

**定位**：行为/交互正确性的第一参照（用户指定优先级 1）。Java 双模块：`terminal-view/`（渲染+交互）、`terminal-emulator/`（纯 Java VT + JNI PTY）、`termux-shared/`（ExtraKeys/设置）、`app/`（Activity/Service/Installer）。

### 7.1 渲染（TerminalView.onDraw + TerminalRenderer）
- `TerminalView.java:1006-1022` onDraw：从 `TextSelectionCursorController.getSelectors()` 取 `int[4]` 选区 → `mRenderer.render(emulator, canvas, mTopRow, sel)` → `renderTextSelection()` 刷新手柄 PopupWindow
- `TerminalRenderer.java:36-54` 构造：缓存 `mFontWidth`（测量 "X"）、`mFontLineSpacing`、`mFontAscent`、ASCII 0-126 逐字符宽度表 `asciiMeasures[127]`（避免每帧 measureText）
- `render()` :57-157：reverseVideo 先整屏 `drawColor(FG, SRC)`；**连续同风格字符聚合成 run 一次 drawTextRun**（:96-145）；字体非等宽失配用 `canvas.scale()` 缩放到 wcwidth 期望宽度（:192-200）；光标块/下划线/竖条 drawRect 画在 run 上（:208-214），块状光标文字反色（invertCursorTextColor :122-127、:150-153）；选择区与 reverseVideo 都折算进 reverseVideo 标志 fg/bg 互换（:182-187）；bold 前 8 色自动加亮（`foreColor += 8` :171-175）；dim 按 xterm 2/3 衰减（:217-227）；italic `setTextSkewX(-0.35f)`（:229-232）；`canvas.drawTextRun()` API 23+（:236）
- 调色板 `TerminalColors.java:15`：`mCurrentColors[259]` = 16 基础 + 216 立方体 + 24 灰阶 + 3 特殊（FG/BG/CURSOR）

### 7.2 文本选择（TextSelectionCursorController.java — torvox 菜单/手柄主蓝本）
- **浮动菜单锚定** `:193-213`：`Callback2.onGetContentRect` + `ActionMode.TYPE_FLOATING`，选择矩形→菜单定位，含手柄高度偏移与越界 clamp
- **选择边缘滚动** `:243-259`、`:283-299`：拖到屏幕边缘自动滚动
- **宽字符吸附** `getValidCurX` :307-336：跨 transcript 选择必备
- **手柄 PopupWindow** `TextSelectionHandleView.java:68-86`：TYPE_APPLICATION_SUB_PANEL、splitTouch 多窗口拖拽、翻转朝向（:205-242）
- 手柄跟随：onDraw 后 `renderTextSelection()` 刷新位置

### 7.3 ExtraKeys（ExtraKeysView.java）
- 长按重复 + 修饰锁定状态机 `:535-585`：ScheduledExecutor 重复发送 + 特殊键锁存

### 7.4 Bootstrap（TermuxInstaller.java — torvox bootstrap 主蓝本）
- **原子安装** `:137-257`：staging 目录 + SYMLINKS.txt 统一建链 + `renameTo` 原子切换 + 二阶段脚本执行 + 失败回滚删 prefix

### 7.5 PTY（termux.c）
- `:36-114`：IUTF8、关 IXON/IXOFF、fork 后清信号掩码/关多余 fd/clearenv——Android 上比 Java ProcessBuilder 可靠（torvox 已用 Rust nix 实现同款）

### 7.6 其他可抄点
- 刷新链 `TerminalSession.java:342-347` → `TerminalView.java:457-499`：批级 invalidate、选择中滚动跟随、awakenScrollBars 节流
- 光标闪烁无线程化 `TerminalView.java:1322-1338`：Handler postDelayed 自循环
- IME 输入适配 `TerminalView.java:379-433`：\n→\r、控制码→Ctrl 字母、三星键盘 inputType 兼容
- UTF-8 严谨性 `TerminalEmulator.java:505-568`：C1 拒绝、overlong 拒绝
- 设置双层架构：UI 用 SharedPreferences 常量表，用户可编辑用 properties 文件 + 类型化解析 + 热重载

---

## 8. Haven（GlassHaven/Haven）深度参考

**定位**：Android 终端（Compose Canvas/Skia 逐格绘制）+ 最强 MCP 实现（202 工具）+ PRoot bootstrap + 智能复制管线。

### 8.1 终端渲染（termlib fork + Compose Canvas）
- 三层：C++ `lib/src/main/cpp/Terminal.cpp` + vendored libvterm（screen/state/parser）→ JNI `TerminalNative.kt` → Kotlin `Terminal.kt`（189KB）Compose `Canvas` + `drawContext.canvas.nativeCanvas`（Skia）
- **逐格绘制** `Terminal.kt:2957-2983`：先 drawRect 背景/选中高亮，再 nativeCanvas.drawText；Paint 逐格设 isFakeBoldText/textSkewX/isUnderlineText/isStrikeThruText；双下划线/波浪下划线
- 背景透明度 `:2511-2525`：半透明 screen-fill + 默认色空 cell 跳过绘制，露出墙纸
- URL 跨行下划线预计算 `urlContinuationRows` :2535-2545
- 键盘遮挡平移 `translate(-keyboardCoveredPx)` + clipToBounds :2501-2512
- `ScrollbackRing.kt:15` 容量环形缓冲；边缘加速 `edgeScrollDirection` :3108
- 配色推送 `emu.applyColorScheme(ansiPalette, fg, bg)`；**默认不推 ANSI 调色板**（#407：覆盖会重映射 mutt 等 TUI 依赖的颜色）

### 8.2 文本选择（SelectionToolbar.kt — torvox 智能选择参考）
- 长按 → 自动词扩展 `:75-127` + **跨行 URL 展开** `expandAcrossUrlWrap` :137-217（回退行悬挂缩进识别 + looksLikeFullUrl 校验）
- 工具栏 Copy/Paste（bracket-paste 2004 包裹）/ Open URL / **锚点 Start/End 切换 + 四方向键微调**（AnchorMover :52-70）
- **SmartTerminalClipboard** `:357-420`：拦截所有复制，TUI 边框剥离 `findConsistentBorderColumns` :299（≥60% 行的 │┃║ 竖边框列）+ `extractPanelContent` :325；软换行解包（libvterm softWrapped 权威标志）；跨行 URL 重建 `rebuildWrappedUrl` :253
- 粘贴 bracket-paste `TerminalScreen.kt:1240-1247`；OSC 52 `:1002-1007`

### 8.3 MCP（core/mcp + app/agent — torvox MCP 扩展参考）
- **Streamable HTTP 单端点** `POST /mcp`（McpServer.kt:124-128, 1044-1079），自研裸 HTTP/1.1 解析（HttpFraming.kt:64-115，头 64KiB/体 8MiB 防 DoS）
- 4 carrier（DEVICE/TUNNELED/LAN/WIREGUARD）绑定时刻打信任标签 McpOrigin :82-99
- `ToolProvider` 接口拆分巨型工具文件（ToolProvider.kt:16-18，绕 JVM 64KB 方法上限）+ `SchemaDsl.kt:19-77` 声明式 JSON-Schema
- consent 三级（NEVER/ONCE_PER_SESSION/EVERY_CALL）+ 后台 fail-closed + 通知 120s 重试窗（AgentConsentManager.kt:32, 270-364）
- 审计脱敏 `AgentAuditRecorder.kt:44-106`（redactJson password/token → `<redacted>`）
- guest 工具聚合 `guest_<ns>_<tool>`（McpTools.kt:1745-1780，8s 缓存）

### 8.4 PRoot bootstrap（torvox bootstrap 对照）
- 二进制命名 `libproot.so`/`libproot_loader.so` 走 jniLibs（Android 14+ 禁 exec app data，nativeLibraryDir 是唯一可 exec 路径，ProotManager.kt:769-774）
- 纯 Java tar 流式解压（GNU long name/link、strip-components、zip-slip 防护、.l2s. 硬链接 flatten，:1774-1900）+ `.haven-rootfs-ready` 完成标记 + Room 阶段化日志
- 启动命令 `proot -0 --link2symlink --sysvipc -r rootfs` + SELinux 屏蔽（空目录盖 /sys/fs/selinux :2250-2253）+ devShm 覆盖 + fake /proc（.loadavg/.uptime/.stat 文件 bind :2279-2295）+ resolv.conf 重写
- 杀 proot 连 ptrace tracee（ProotProcessTree.kt:14-53，BFS /proc 后代 + SIGTERM→SIGKILL）

---

## 9. ghostty-android-terminal 深度参考

**定位**：Zig libghostty-vt 预编译 + C JNI + 自绘 Canvas 的 Android 终端；**arm64chroot（AArch64 用户态模拟器）** 是其独有方案。

### 9.1 架构
- Java 层约 20 类自定义 View（无 appcompat/material）+ JNI 层 `libterm.so`（pty_jni.c / terminal_jni.c / kitty_unicode.c / png_decode.c(stb_image) / chroot_ng_embed.c）+ 静态链接层（libghostty-vt.a 预编译每 ABI 提交 + arm64chroot + chroot-ng）
- 生产依赖仅 `org.tukaani:xz:1.10`（纯 Java XZ 解码）

### 9.2 bootstrap（arm64chroot）
- 不是 chroot(2)：**从零写的 AArch64 Linux 用户态模拟器**（解释器 + 可选 --jit），静态链接进 libterm.so，fork 后子进程进程内调用 `arm64chroot_main()`（pty_jni.c:126-140）；guest execve 是进程内 ELF 重载——不 exec 宿主机二进制、无 loader，天然规避 Android W^X（targetSdk≥29 禁 execve app 数据）
- rootfs：Alpine/Debian aarch64 tarball + XZ 纯 Java 解压

### 9.3 可抄点（30 项清单精选）
- winsize 像素字段 `pty_jni.c:84-88`：ws_xpixel/ws_ypixel 让 icat 等 Kitty 工具读像素尺寸；初始即最终尺寸
- OSC 侧扫描器 `OscSideScanner.java`：libghostty 不暴露的序列（OSC 52 剪贴板、OSC 9;4 进度）Java 侧被动状态机补全，跨 read 边界携带状态、1MiB 上限
- mksh SIGWINCH 陷阱 `TerminalSession.java:331-342`：首布局后再 spawn + 跳过 no-op resize
- 会话裁剪：无前台服务，进程死即会话亡；`userInteracted` 区分"用户用过才退出"与"启动即失败"（:252-260）
- Kitty 图形两遍绘制 `TerminalView.java:1544-1554`：z<0 文本下、z≥0 文本上
- **反模式提醒**：libghostty-vt 非线程安全必须单锁；minSdk 29 受 bionic ELF TLS 硬限制；ambitious-width 与 RTL 是引擎层限制

---

## 10. ghostling + ghostty 本体深度参考

**定位**：ghostling = 纯 C 单文件（main.c 67KB）+ Raylib + libghostty-vt C API 最小终端 demo（无 Swift/Metal）；**ghostty 本体在 vendor/ghostty**（Zig 全核心 + Metal 渲染器 + Selection.zig 完整选择状态机）。

### 10.1 ghostling（最小可用终端清单）
- 分层：pty_spawn（main.c:43,103,132）、输入映射（:164 key、:211 mods、:290 mouse、:449 handle_input）、渲染状态消费（:798 render_terminal、:662 kitty 图片、:572 scrollbar）、终端回调 effects（:1104 write_pty、:1114 size、:1129 device_attributes、:1154 xtversion、:1163 title、:1182 color_scheme）、主循环（:1195 每帧：滚动条→输入→ghostty_render_state_update 快照→绘制）
- **effect 回调集是"最小可用终端"必须实现的完整清单**——torvox Rust 引擎对照查漏（vim/tmux 兼容）
- 键盘编码三件套：unshifted_codepoint + consumed_mods + encoder 从终端同步模式（main.c:454, 520-530）
- HiDPI 字体：按物理像素光栅化 + bilinear、cell 尺寸从字形实测推导（main.c:1213-1234）

### 10.2 ghostty 本体（vendor/ghostty，Zig）
- 渲染器：`src/renderer/generic.zig` + Metal（Zig 直连 objc，非 MTKView：IOSurfaceLayer.zig:53-122 非 MTKView 图层接入、IOSurface 呈现、尺寸不符丢弃）
- **选择**：`Selection.zig:42-52` tracked Pin（滚动不失效）+ `SelectionGesture.zig:88-123` 手势状态机（双击/三击/拖动 autoscroll 15ms）——**torvox 若做选择，这是最完整的实现范式**
- GPU 资源上传模式 `generic.zig:1575-1587`：bg_cells/fg_rows 按数组列表批量同步、font atlas 变更才重传——wgpu buffer 同步可直接对齐
- 主题：light/dark 对 + PaletteMask（只重传被改的调色板项，Config.zig:9906-9946）
- Kitty 图片三层 z 排序（below-bg/below-text/above，ghostling main.c:827-953）
- 每 10 万帧（~12 分钟）重置渲染器持有的终端状态防内存滞留（generic.zig:1154-1160）
- iOS sim 强制 apple_a17 CPU model（GhosttyXCFramework.zig:47-53）

---

## 11. GNOME Console（kgx）深度参考

**定位**：纯 C17 + GTK4 + libadwaita + VTE（vendor subprojects/vte.wrap）的终端。**策略：子类化 VteTerminal 而非封装**——渲染/滚动/选择/搜索全部委托 VTE。

### 11.1 架构
- 层：KgxApplication → KgxWindow → KgxPages（AdwTabView）→ KgxTab → KgxSimpleTab → **KgxTerminal（直接继承 VteTerminal，kgx-terminal.h:43）**
- 进程层：KgxDepot（spawn 中心）、KgxTrain（会话/进程树）、KgxWatcher、pids/（libgtop 枚举子进程）
- 设置：KgxSettings 封装 GSettings + `g_settings_bind_with_mapping` 类型转换（kgx-settings.c:468-573）

### 11.2 可抄点
- 选择：重写 selection_changed vfunc → 动作使能（copy）；VTE 内置 regex match + hyperlink 双通道
- 当前目录（OSC 7 termprop）驱动 UI：kgx-terminal.c:188-239
- font-scale 统一缩放 0.5-4.0（kgx-settings.c:54-61）；Ctrl+滚轮 GtkEventControllerScroll（kgx-terminal.c:810-825）
- 退出保护 kgx-window.c:199-231（有子进程时关闭确认 + close_anyway 二次确认）；标签级 kgx-pages.c:450-486
- 后台完成通知 + needs-attention（kgx-tab.c:903-943）
- 空态/加载态 GtkStack closure 而非命令式切页（kgx-window.c:243-251）
- 严格编译告警集 meson.build:122-143；每组件独立测试；KGX_DEFINE_DATA 宏异步回调上下文弱引用防悬垂（kgx-shared-closures.h）

---

## 12. 更多 wgpu/rust 终端参考（GitHub 搜索发现）

| 项目 | 星 | 定位 | 与 torvox 相关点 |
|---|---|---|---|
| **raphamorim/rio** | 7.2k | wgpu 30 GPU 终端（桌面+wasm） | wgpu 终端渲染标杆；渲染/后端分层 |
| **raphamorim/sugarloaf** | 49 | rio 渲染引擎：glyph 缓存 + cosmic-text + 网格/光标 + 背景模糊 | 可单独拆用（已并入 rio workspace） |
| **gold-silver-copper/CuTTY** | 29 | wgpu powered terminal（新） | wgpu 渲染循环对照 |
| **ghreprimand/odytty** | 29 | Linux wgpu 终端 | 较新，渲染循环对照 |
| **pmqueiroz/nova** | — | wgpu + vte 解析跨平台终端，多标签 + AI | 发行体系成熟 |
| **kanata9819/solito** | 0 | winit + wgpu + glyphon + portable-pty 桌面最小实现 | 与 torvox 桌面路径架构几乎一致 |
| **ShashlikMap/WgpuKmp-Template** | 10 | Rust wgpu + KMP + Compose UI 共存模板 | **Compose 与 wgpu 集成的最直接参考** |
| **jinleili/bevy-in-app** | 237 | bevy 版 wgpu-in-app | 同作者同思路 |
| **rust-mobile/cargo-apk** | — | 传统 Rust→APK 构建链 | egui/gpui-mobile/vello 路线 |
| **jackpal/Android-Terminal-Emulator** | 3.2k | Java VT-100（已归档） | 历史对照 |

**关键结论**：完整 "Android + wgpu + 终端" 项目目前只有 zelland 一个；文本渲染可行组合已验证 = cosmic-text（排版）+ glyphon（绘制）+ wgpu Vulkan（Android）；alacritty 放弃 wgpu 后端、rio 是唯一活跃 wgpu 终端——移植渲染层首选蓝本 rio/sugarloaf，文本层首选 glyphon，Android 端集成实例 zelland。

---

## 13. 参考引用位置索引（代码注释）

| 引用位置 | 内容 |
|---|---|
| `native/src/render/context.rs` create_atlas_texture | zelland WGPU_FIXES.md pitfall #9：atlas 尺寸 clamp 到 max_texture_dimension_2d |
| `native/src/render/context.rs` bg_image 纹理 | zelland Fix 1：glyphon atlas 格式须匹配 surface 格式；直接 shader 采样无此约束 |
| `native/src/render/context.rs` attach_surface | zelland + wgpu-in-app：SurfaceHolder.Callback 驱动生命周期、每帧查尺寸、attach 后立即 render、acquire 失败 reconfigure+retry |
| `android/.../ui/TerminalSurface.kt` showSelectionMenu | TODO(selection)：迁移 TYPE_FLOATING + Callback2.onGetContentRect（termux TextSelectionCursorController 模式） |

---

## 14. 第二批参考仓库深度扫描（2026-08，19 个）

### 14.1 Android 终端类（直接相关）

**NeoTermux**（0★，2860 行，Kotlin+Compose+C JNI）：Termux 仿制原型。
- C 端 `termlib` PTY 层完整且正确（pty.c:21-31 PtySession、posix_openpt→fork 流程）——可对照 torvox Rust PTY 核对
- **架构教训**：termlib 的 JNI 类从未接线（Kotlin 侧无 System.loadLibrary）——"写完未接线"的死代码教训
- 实际渲染是 LazyColumn+Text 假输出（TerminalScreen.kt:181-195）——反面教材

**OnecodeTerminal**（0★，276 行）：全屏终端系统栏手势/排除区交互（MainActivity.kt:43-175）+ AIDL 会话服务模型（README.md:47-69）——可照搬；核心实现不在检出内。

**Ply**（0★，321 行）：无 PTY 直接管道反面教材（MainActivity.kt:45-69 ProcessBuilder 管道，vi/top 必然失效）；proot 参数组合（proot.rs:28-41 `--link2symlink + /dev /proc /sys 绑定`）可参考。

**RedTerm**（11★，6196 行）：**复用 Termux 库**（terminal-emulator/view v0.118.3，build.gradle.kts:88-89）+ 自研 proot 链。
- proot 参数全集 `ProotRunner.kt:62-70`：`-0 -L -r -w --link2symlink --sysvipc --kill-on-exit`；环境变量 :29-43（PROOT_LOADER/libfakeuid/TERM=xterm-256color）；writeResolvConf :221；补 /etc/group :239
- rootfs 安装流水线 `DistroInstaller.kt:44-67`：下载→SHA-256 校验(:194)→解压(native xz :242 / Java xz :252 双路径)→权限修复→注入 passwd/hosts/resolv.conf(:448)
- 前台服务 CPU 统计 `TerminalService.kt:91-138`（/proc/stat 递归子进程）
- 可点链接 URL/路径正则 `TerminalBackend.kt:144-148`

**terminator**（0★，5632 行）：自研 Kotlin 终端状态机全链路 + Compose Canvas。
- **seccomp_safe_fork**（规避 SELinux exec 限制的 fork 方案）
- **$TERMINFO 方案**（自带 terminfo 目录让 ncurses 应用工作）
- 若自研仿真器首选参照

**cpmdroid**（10★，4442 行）：Z80/CP/M 模拟器，主线程批量执行指令 + 定时持久化（NVRAM 5s/脏磁盘 20s，MainActivity.kt:100-136）——与终端无关，参考价值低。

**TermX**（0★，40465 行）：经典 View 终端 + 70% 代码是 API 层（SSH/cron/tunnel/X11）。
- properties 双文件配置 `TermXProperties.kt:94-176`（Termux 兼容 + 生成模板）——无监听需重启生效
- 单 TerminalView + attach/detach 多会话（避免 ViewPager 复用复杂性）
- `TerminalColors.kt:54-156` 现成 5 色板
- `BellHandler.kt:17-33` 铃声 100ms 节流
- 悬浮终端 `FloatingTerminal.kt:43-68` TYPE_APPLICATION_OVERLAY
- 渲染 run 合并条件 + ASCII 宽度缓存（朴素实现，torvox 已更优）

**termux-kotlin-app**（48★，71385 行）：termux Java→Kotlin 机械转换 fork + AI Agent 框架。
- 文本选择（Kotlin 版）：`TextSelectionCursorController.kt:38-98` 长按选区初始化+单词展开；`:190-250` 把手拖动/坐标钳制/自动滚动；`TextSelectionHandleView.kt:54-79` PopupWindow 把手（**避免 ActionMode 兼容问题**）
- `GestureAndScaleRecognizer.kt:75-84` 长按后抑制 onUp 细节
- bootstrap 嵌入方案：`TermuxInstaller.kt:56-272` + `termux-bootstrap-zip.S`（汇编 .incbin 把 bootstrap 嵌入 so、JNI 直读，免网络 30MB 冷启动）；`app/build.gradle:228-309` 构建期下载+SHA-256 可复现流程
- `TerminalViewClient.kt`/`TerminalSessionClient.kt` 薄接口解耦（View↔App、Session↔App）
- `ExtraKeysView.kt`/`ExtraKeysInfo.kt:39-71` JSON 矩阵式可配置键盘行
- `TermuxActivity.kt:528-556` 长按上下文菜单全集（URL 选择/分享/杀进程）
- 前台服务持有会话 `TermuxService.kt:541-609`（进程外存活）

**ReTerminal**（618★，36874 行）：Material 3 + 复用 Termux 库 + **完整 PRoot C 移植**。
- PRoot 完整源码 core/proot/（~2 万行 C，编译 libproot.so/libloader.so）——Android 上 PRoot 全参考
- `SessionService.kt:24-72` FOREGROUND_SERVICE_TYPE_SPECIAL_USE 前台服务 + Binder + Compose 可观察会话列表
- 可配置快捷键 `ShortcutBinding.kt:10-140`：序列化 `"CTRL\|SHIFT\|54"`、保留键集合、冲突检测、分发管道——**完整可复用方案**
- 伪 /proc `TerminalUtils.kt:37-234`（伪造 stat/vmstat bind 进沙箱）
- `AlpineDocumentProvider.kt:25-80` 应用目录暴露给系统文件管理器（torvox DocumentsProvider 对照）
- 防泄漏 `TerminalViewModel.kt:22-29` WeakReference<TerminalView>
- 壁纸/模糊背景 `TerminalScreen.kt:152-171` 透明终端+壁纸
- 终端嵌入 Compose `TerminalViewLayout.kt:37-92` AndroidView factory/update 双块模式

### 14.2 SSH/远程终端类

**sushi-ssh-client**（0★，12361 行）：JSch SSH 客户端 + TextView 轻量终端。
- `TerminalBackend.kt:3` 传输抽象：connect/sendText/sendCommand/sendCtrlC/sendCtrlD/resizePty/disconnect/**execCommand**（独立 exec channel 与交互 PTY 分离，供 AI/脚本用）
- `SshClient.kt:97` ConnectFailure 失败分类（:62）+ AuthPlan 密码/密钥偏好（:105）+ jump host（:241）
- 反面教材：TextView setTextIsSelectable 无光标自绘

**Rin**（20★，6932 行）：**与 torvox 相似度最高**（Rust vte 引擎 + JNI 分层）。
- **脏行 + JNI int 打包刷新通道**（Rust 算脏行 → int 数组过 JNI → Kotlin Canvas 画）——torvox 可对照 CellData 通道
- 选择：拖选→网格坐标（简单版）
- 前台服务 + 会话生命周期

**moke**（66★，23012 行）：Termux 移植 + SSH/mosh。
- **选择实现标杆**：双手柄 + ActionMode（最完整）；`getSelectedText` 行连接语义（back 行是否拼一行）；词级选择 `getWordAtLocation`——**torvox 选择语义应抄这套**
- `TerminalTransport` 传输抽象（SSH/mosh 可插拔，模拟器与传输解耦）
- 滚动：transcript 行 + mTopRow + scrollCounter + 自动跟随判定（:453-486）
- `WcWidth.java` 39K 宽字符宽度表；`TerminalRow` 稀疏行分配 :447
- mosh 子进程隔离 + terminfo 部署边界策略

### 14.3 wgpu/GPU Android 类

**wgpu-example**（123★，3797 行）：winit + egui + wgpu 跨平台样板。
- Android 走 `android_activity::AndroidApp` + winit ApplicationHandler（src/lib.rs:35-50 android_main）——纯 Rust 全栈，与 Compose 无交集
- `suspended()/resumed()` 显式销毁/重建 renderer 生命周期样板（:90-93, 181-187）
- **第三方模拟器坑**：MuMu/BlueStacks 遗留 libEGL_emulation.so 绑定 ANativeWindow 导致 GLES 回退失败 `Surface::configure: Invalid surface`（README:170）；官方模拟器 `-gpu host` 可行

**shashlik-map**（9★，12116 行）：Rust wgpu + KMP + Compose 地图引擎。
- **Compose 与 wgpu 集成**：TextureView 承载 + uniffi+gobley API 自动绑定
- 每帧全量 render（简单、延迟高）——对比 warp 的脏渲染

**warp-mobile-android**（7★，39284 行）：warp 终端移动版（direct ash 而非 wgpu，85KB）。
- **终端渲染正解**：SurfaceView + **Choreographer vsync** + 脏单元格增量 push（比每帧全量更适合高频终端刷新）
- DECSET 1049 轮询 `WarpTerminalService.kt:879-900`：terminalIsAltScreen() → Compose 卡片 ⇄ 全屏 Vulkan 网格自动切换——**torvox「Compose+wgpu 终端」要解决的问题**
- 帧捕获 capture_to_png（:1289 读回 swapchain 验证渲染）+ ADB 脚本化测试（tools/scripts/test-render-scene.sh）——渲染正确性验证现成套路
- `font_render.rs:216` cosmic-text 字体系统；`font_picker.rs` OEM 字体名差异处理
- swapchain out-of-date/suboptimal 重建（:23-26）；Adreno p95 18ms

### 14.4 框架/移植方法论类

**fission**（16★，291617 行）：生产级 Rust 多端 GUI 框架（保留式 UI）。
- 单向管道 `Authoring → Core Runtime → Rendering Backend → Platform Shell` 无反馈环（docs/02-high-level-architecture-overview.md:11-15）
- **确定性第一**：离散帧模型、每帧可产 Core IR/layout/semantic/display list 快照 + 事件 trace、完整 headless 路径
- "shell 是适配器，不是解释器"（docs/02-3-platform-shells-vs-core-runtime.md:68）
- 后端：fission-render + vello + wgpu2d

**osmosis**（0★，13657 行）：Slint+Bevy 应用骨架。
- 单向依赖纪律 + 共享 wgpu 合成 + ADR 治理——小而美活样板

**zed-android-port**（0★，136 万行）：Zed 的 Android 移植 fork——**移植战争战地手册**。
- **零 fork 移植策略**：平台层做成 gpui 纯实现、应用层零改动（README.md:234-235）
- workaround 文档模板（workarounds/README.md:128-153）：Problem → Constraint → Solution → Why this works（承重不变量）→ Failure mode if regressed——平台移植知识管理最佳实践
- 具体移植技术点（workarounds/README.md 索引）：**Choreographer vsync**、**wgpu device-lost 恢复**、RefCell 不 drain 模式（refcell-drain-platform-bug.md）、open_window 阻塞到 ANativeWindow 就绪、activity 重建幂等、**JNI 异常清除**、android_main 小栈 JNI 堆栈溢出规避
- 双存储域 exec/noexec（README.md:91-101）+ targetSdk 28 SELinux 约束（:38）
- BACKLOG.md bug 记录格式：repro 步骤 + 代码路径 + 疑似原因 + logcat 定位信号（BACKLOG.md:5-47）
- 上游合并治理：文件归属三分类 + merge=ours + 每补丁文档（UPSTREAM_MERGE.md:13-46）

---

## 15. 综合交叉结论（第二批）

1. **终端选择语义标杆**：moke（双手柄+行连接语义）> termux-kotlin-app（Kotlin 版 TextSelectionCursorController）> TermX（长按选词 MVP）——torvox 菜单迁移按 termux 原版（§7.2），语义细节对照 moke `getSelectedText`/`getWordAtLocation`
2. **Android 终端渲染架构演进**：朴素 Canvas 逐格（TermX/NeoTermux）→ 复用 Termux 库（RedTerm/ReTerminal）→ Rust 引擎 + JNI 脏刷新（Rin）→ **SurfaceView + Choreographer vsync + 脏单元格增量 push（warp，终端场景正解）**——torvox 已走最后一条路线，warp 的 vsync 细节可补强
3. **Compose+wgpu 集成两范式**：shashlik TextureView 每帧全量（简单）vs warp SurfaceView+vsync+脏 push（性能）——torvox 用 SurfaceView 正确
4. **bootstrap 三方案**：termux 网络下载+staging 原子安装（§7.4，torvox 当前）→ termux-kotlin-app .incbin 嵌入 so 免网络 → ReTerminal/RedTerm PRoot 发行版（-0 --link2symlink --sysvipc --kill-on-exit + SHA-256 + xz 双路径）
5. **配置可复用**：ReTerminal ShortcutBinding 快捷键序列化方案、TermX properties 双文件、termux-kotlin-app JSON 矩阵键盘行
6. **移植方法论**：zed-android-port workaround 文档模板 + BACKLOG 格式可直接引入 torvox 文档实践
7. **Proot 参考**：ReTerminal core/proot 完整 C 源码 + RedTerm 启动链——若 torvox 未来支持发行版
8. **构建**：wgpu-example xbuild 管线 + 官方模拟器 -gpu host 可用性确认
