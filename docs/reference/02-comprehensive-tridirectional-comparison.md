# 三向深度对比：torvox vs 26 参考项目

> 生成时间：2026-08-06 | 研究方式：亲自逐代码阅读（非子代理）

## 目录

1. [渲染管线对比](#1-渲染管线对比)
2. [文本选择系统对比](#2-文本选择系统对比)
3. [JNI/FFI 桥接对比](#3-jniffi-桥接对比)
4. [终端模拟核心对比](#4-终端模拟核心对比)
5. [Bootstrap/安装对比](#5-bootstrap安装对比)
6. [MCP/Agent 对比](#6-mcpagent-对比)
7. [依赖评估与建议](#7-依赖评估与建议)
8. [代码注释索引](#8-代码注释索引)

---

## 1. 渲染管线对比

### 1.1 zelland（Rust + wgpu + glyphon + libghostty-vt）

**架构**：3-pass 渲染管线（`src-tauri/src/renderer/mod.rs:635`）
- Pass 1（:660）：clear + cursor rectangle — uniform buffer 写光标位置
- Pass 2（:690）：text — glyphon 文字渲染（`text_renderer.render()`）
- Pass 3（:710）：selection overlay — 半透明蓝色矩形

**关键发现**：
- 光标用 uniform buffer + dedicated pipeline（`:20-50` vs_main/fs_main）
- 选择用 vertex buffer + 专用 pipeline（`:439-545`）
- `cell_width/cell_height` 由字体初始化后动态更新，不硬编码（`renderer/mod.rs:1084` `get_cell_size()`）
- atlas 格式必须等于 surface format（WGPU_FIXES.md Fix 1）
- Surface format 取 `caps.formats[0]`，不硬编码

**torvox 对比**：
- torvox 用 cell.wgsl shader 单 pass 渲染（所有单元格在同一 pass），zelland 3 pass → torvox 更高效（单 pass 减少 GPU 开销）
- torvox 的 `build_instances_from_cell_data` 包含反色/选择/光标，zelland 分离选择到独立 pass → 两者各有优势
- zelland 的 `get_cell_size()` 动态更新模式值得 torvox 学习

**引用**：[zelland] `src-tauri/src/renderer/mod.rs:635-742`, [zelland] `WGPU_FIXES.md:1-260`

### 1.2 wgpu-in-app（Rust + wgpu + Android）

**架构**：`app-surface/src/lib.rs` — `SurfaceFrame` trait 抽象

**关键发现**：
- `SurfaceFrame::create_current_frame_view` 处理 acquire 失败（`:210-240`）：
  - `Outdated`/`Lost` → reconfigure → retry（**torvox 缺失此重试逻辑**）
  - `Occluded` → 返回 None
  - `Validation` → panic
- `normalize_view_size((0,0))` → `(1,1)`（`:71-73`），防止零尺寸 configure
- Android `view_formats` 必须设为 `vec![format]`（`:314-330`），不支持其他格式
- `ANativeWindow_fromSurface` 引用计数+1，drop 时 release（`android.rs:65,93`）
- `request_device` 移除 `MAPPABLE_PRIMARY_BUFFERS` feature（Android 不支持）（`:403`）

**torvox 对比**：
- torvox `context.rs:attach_surface` 缺少 Outdated/Lost 重试 → 应添加
- torvox 已有 `normalize_view_size` 等效逻辑（`context.rs:960` `width.max(1)`）
- torvox 的 `ANativeWindow` RAII wrapper 已覆盖引用计数

**引用**：[wgpu-in-app] `app-surface/src/lib.rs:210-240,314-330`, [wgpu-in-app] `app-surface/src/android.rs:65,93`

### 1.3 ghostty-android（Java + ghostty VT + Canvas 渲染）

**架构**：Java Canvas 直接绘制（`TerminalView.java`），不使用 GPU

**关键发现**：
- 终端拥有选择状态，view 只镜像（`:298-302`）—— 与 torvox 不同（torvox 在 Rust 侧拥有选择）
- `TYPE_FLOATING` ActionMode + `Callback2.onGetContentRect` 智能定位（`:1142`）
- `selectionGeometryKey()` 打包选择端点为 Long，避免不必要的重定位（`:1150-1165`）
- 双击选词、三击选行，不使用 GestureDetector 默认检测（`:400-405`）

**torvox 对比**：
- torvox 已迁移到 `TYPE_FLOATING` ActionMode → 已对齐
- torvox 选择状态在 Rust 侧 → 更正确（GPU 渲染需要 Rust 侧选择数据）
- ghostty-android 的 `selectionGeometryKey()` 避免不必要 toolbar 重定位 → 值得学习

**引用**：[ghostty-android] `TerminalView.java:298-302,1085-1170,1423-1470`

### 1.4 termux-app（Java + Android Canvas）

**架构**：Java Canvas 绘制（`TerminalView.java`），自定义 `TextSelectionCursorController`

**关键发现**：
- `TextSelectionCursorController` 实现完整的拖拽手柄系统（`:21-403`）
- `onGetContentRect` 定位 toolbar 在选区上方/下方（`:194-216`）
- `updatePosition` 处理拖拽时 toolbar 隐藏/重显（`:218-336`）
- 双击选词、三击选行（`:93-160`）

**torvox 对比**：
- torvox 已用 `TYPE_FLOATING` 替换自定义 SelectionMenuPopup → 已对齐
- termux 的 `updatePosition` 边缘滚动逻辑值得学习

**引用**：[termux-app] `TextSelectionCursorController.java:21-403`

### 1.5 warp（Kotlin + Vulkan + SurfaceView）

**架构**：`SurfaceView` + `Choreographer` vsync 驱动渲染

**关键发现**：
- `renderAttachSurface` 通过 JNI 传递 Surface → Rust 侧 `ANativeWindow_fromSurface`（`MainActivity.kt:46`）
- `renderDetachSurface` 清理 Vulkan（`:50`）
- `Choreographer.postFrameCallback` 驱动每帧渲染（`:52,111`）
- `renderActive` 标志防止 surface 未就绪时渲染

**torvox 对比**：
- torvox 用 `TextureView`（非 `SurfaceView`）→ 与 Compose overlay 兼容更好
- warp 的 Choreographer 驱动模式 vs torvox 的 poll loop → warp 更省电

**引用**：[warp] `MainActivity.kt:41-186`

### 1.6 ghostling（C + raylib + libghostty-vt）

**架构**：raylib 渲染 + forkpty PTY

**关键发现**：
- 极简实现：`pty_spawn` 用 `forkpty`（`:40-80`）
- 字体嵌入编译时（`font_jetbrains_mono.h`）
- shell 选择：`$SHELL` → `pw_shell` → `/bin/sh`（`:60-70`）

**torvox 对比**：
- torvox PTY 用 `nix::pty::openpty` + `fork` → 等效
- ghostling 的 shell 选择链值得参考

**引用**：[ghostling] `main.c:40-80`

---

## 2. 文本选择系统对比

### 2.1 zelland 选择系统

**实现**：GPU 选择 overlay（`renderer/mod.rs:439-545`）
- `set_selection(sc, sr, ec, er, active)` → 归一化 start ≤ end → 更新 vertex buffer
- `update_selection_vertices` 逐行生成矩形，跨行展开（`:510-545`）
- `extract_text` 从 `row_cache` 提取选中文本（`:546-568`）
- 硬编码蓝色 `Color { r: 0.2, g: 0.4, b: 0.8, a: 0.4 }`（`:510`）
- 最大 50 行 × 6 顶点/行 = 300 顶点（`:539`）

**torvox 对比**：
- torvox 用 shader 反色（fg↔bg swap）vs zelland 蓝色覆盖 → torvox 更正确（反色是终端标准）
- torvox 的 `SelectionRange::contains` 在 `cell_builder.rs` 内逐 cell 检查 vs zelland 逐行矩形 → torvox 更精确（支持多行不连续选择）
- zelland 的 50 行限制是合理优化 → torvox 无限制可能在大选区时性能差

**引用**：[zelland] `src-tauri/src/renderer/mod.rs:439-545`

### 2.2 ghostty-android 选择系统

**实现**：终端引擎拥有选择，view 镜像（`TerminalView.java:298-302`）
- `startSelection(px, py)` → `selectWord(cx, cy)` → `session.emulator.selectWord()`
- `longPressDragging = true` → 拖拽扩展选择（`:1100`）
- `showSelectionUi()` → `startActionMode(selectionActions, TYPE_FLOATING)`（`:1142`）
- `selectionActions.onGetContentRect` 定位 toolbar（`:1469`）
- 双击选词（tapCount=2）、三击选行（tapCount=3）（`:400-405`）
- `handleTap` 在每次 tap 时递增计数，不等 GestureDetector 默认检测

**torvox 对比**：
- torvox 的 `SelectionExpander.expandBounds` 在 Kotlin 侧做智能扩展 → 等效
- torvox 已用 `TYPE_FLOATING` ActionMode → 已对齐
- ghostty-android 的 tapCount 机制（不依赖 GestureDetector 默认检测）→ torvox 应学习

**引用**：[ghostty-android] `TerminalView.java:1085-1170,1423-1470`

### 2.3 termux-app 选择系统

**实现**：`TextSelectionCursorController`（`:21-403`）
- 双手柄拖拽（`TextSelectionHandleView` LEFT/RIGHT）
- `updatePosition` 处理拖拽时 toolbar 隐藏/重显（`:218-336`）
- `onGetContentRect` 定位 toolbar 在选区上方/下方 + 边缘翻转（`:194-216`）
- 边缘滚动：拖拽到屏幕边缘时自动滚动（`:243-336`）

**torvox 对比**：
- torvox 的 `HandleDrag` 已实现锚点+位移拖拽 → 等效
- termux 的边缘滚动逻辑 → torvox 缺失，应添加
- termux 的 toolbar 智能翻转（选区上方/下方）→ torvox 已用 TYPE_FLOATING

**引用**：[termux-app] `TextSelectionCursorController.java:21-403`

### 2.4 Haven 选择系统

**实现**：Compose `SelectionToolbar`（`SelectionToolbar.kt`）
- Material3 `FilledTonalButton` + `IconButton`
- Copy/Paste/SelectAll/方向箭头/Close
- `LocalClipboardManager` 剪贴板访问
- `SelectionController`（connectbot termlib）

**torvox 对比**：
- torvox 已迁移到系统 ActionMode → 更一致
- Haven 的方向箭头（逐字符移动选择）→ torvox 缺失

**引用**：[haven] `SelectionToolbar.kt`

---

## 3. JNI/FFI 桥接对比

### 3.1 zelland JNI

**函数列表**（`src-tauri/src/renderer/android.rs`）：
- `passSurfaceToRust(surface)` → `Renderer::init()` + `set_surface()`
- `passSurfaceDestroyedToRust()` → `renderer.drop_surface()`
- `passResizeToRust(w, h)` → `renderer.resize()` + `renderer.render()`
- `getSelectionText()` → `renderer.extract_text()`
- `setSelectionHighlight(sc, sr, ec, er, active)` → `renderer.set_selection()`
- `passPasteToRust(text)` → SSH channel write
- `getCellDimensions()` → `(cell_width, cell_height)`
- `updateFontSizeToRust(cssPx, dpr)` → `renderer.update_font_size()`
- `passTouchToRust(action, x, y)` → SSH mouse encoding

**torvox 对比**：
- torvox 有 17 个 JNI 导出（ffi.rs），zelland 9 个 → torvox 更完整
- zelland 的 `getCellDimensions` → torvox 已有 `getCellWidth`/`getCellHeight`
- zelland 的 `passTouchToRust` → torvox 缺失（无鼠标报告支持）

**引用**：[zelland] `src-tauri/src/renderer/android.rs:18-220`

### 3.2 wgpu-in-app JNI

**函数列表**（`wgpu-in-app/src/ffi/android.rs`）：
- `createWgpuCanvas(surface, idx)` → `WgpuCanvas::new(AppSurface::new(env, surface))`
- `enterFrame()` → `canvas.enter_frame()`
- `changeExample(idx)` → `canvas.change_example()`
- `dropWgpuCanvas()` → drop canvas

**torvox 对比**：
- wgpu-in-app 的 `AppSurface::new` 封装 ANativeWindow 获取 → torvox 已在 `attach_surface` 中实现
- wgpu-in-app 的 `enterFrame` 每帧调用 → torvox 用 poll loop 等效

**引用**：[wgpu-in-app] `wgpu-in-app/src/ffi/android.rs:1-50`

### 3.3 warp JNI

**架构**：`NativeBridge` 静态方法 + Rust 侧
- `renderAttachSurface(surface)` → ANativeWindow + Vulkan init
- `renderDetachSurface()` → Vulkan cleanup
- `renderClearFrame(r, g, b, a)` → clear only
- `renderDrawGridFrame(r, g, b, a)` → draw grid
- `renderDrawDynamicGridFrame(...)` → animated grid
- `renderFramesPresented()` → frame count for test driver

**torvox 对比**：
- warp 用 Choreographer 驱动 vs torvox poll loop → warp 更省电
- warp 的 `renderFramesPresented` 测试钩子 → torvox 缺失

**引用**：[warp] `MainActivity.kt:41-186`

---

## 4. 终端模拟核心对比

### 4.1 zelland（libghostty-vt + SSH）

- Ghostty VT 引擎处理所有 VT 序列
- SSH 通道传输数据
- `GhosttyRenderStateWrapper` 快照用于渲染
- 鼠标编码用 `ghostty_mouse_encoder_*` C FFI

**torvox 对比**：
- torvox 同样用 libghostty-vt → 等效
- torvox 无 SSH（本地 PTY）→ 架构不同但 VT 部分等效

### 4.2 ghostty-android（ghostty VT + chroot）

- Ghostty VT 引擎
- 两种会话模式：Linux userland（Alpine/Debian chroot）和标准 Android `/system/bin/sh`
- 首次运行引导 + 离线发行版安装

**torvox 对比**：
- torvox 的 bootstrap 安装更复杂（nix-on-droid 兼容）→ 更灵活
- ghostty-android 的 chroot 模式 → torvox 无此功能

### 4.3 termux-app（自研 VT 引擎）

- `TerminalSession` 管理 PTY + VT 状态
- `TerminalView` 渲染 + 选择
- `TermuxTerminalSessionService` 后台服务
- 完整的 `KeyHandler` 键盘处理

**torvox 对比**：
- torvox 用 libghostty-vt（更现代、更完整）vs termux 自研 VT
- termux 的 `TermuxTerminalSessionService` → torvox 无后台服务

### 4.4 ghostling（C + raylib + libghostty-vt）

- 极简：PTY + VT + raylib 渲染
- 无选择、无 MCP、无 Android 特化

**torvox 对比**：
- ghostling 是最小参考实现 → 验证 libghostty-vt 用法

---

## 5. Bootstrap/安装对比

### 5.1 termux-app

- `TermuxInstaller.java`：ZIP 下载 + 解压 + 权限设置
- `System.loadLibrary("termux-bootstrap")` JNI 加载
- `create_subprocess` 用 `putenv(*envp)` 传递环境

### 5.2 ghostty-android

- 首次运行引导 + 离线发行版安装（Alpine/Debian）
- `TerminalFontStore` 字体管理

### 5.3 redterm

- `DistroInstaller` + `DistroRegistry` 发行版注册
- `Distro` 数据类封装发行版信息

### 5.4 torvox

- `BootstrapInstaller` + `SecondStageRunner`
- 兼容 termux bootstrap + nix-on-droid bootstrap
- linker64 + LD_PRELOAD 执行 postinst

**torvox 对比**：
- torvox 的 linker64 方案是独创的（解决 SELinux execute_no_trans）
- termux 用 JNI `create_subprocess` → 更直接但需要 NDK
- ghostty-android 用 chroot → 更简单但需要 root

---

## 6. MCP/Agent 对比

### 6.1 Haven

- `McpTools.kt`：完整 MCP 服务器
- `ConsentLevel` 同意门控
- `LocalSessionManager` 本地会话管理
- `SshSessionManager` SSH 会话管理
- `RcloneClient` 云存储
- `FfmpegExecutor` 视频处理
- `GuestAppCatalog` proot 应用目录

### 6.2 torvox

- `native/src/mcp.rs`：tower-mcp，8 个工具
- 无同意门控（用户要求"安全不用太关心"）
- Unix socket + Stdio 双传输

**torvox 对比**：
- Haven 的 MCP 更成熟（同意门控、更多工具）→ torvox 可学习同意门控
- torvox 的 tower-mcp 更现代（axum 集成）→ 架构更清晰

---

## 7. 依赖评估与建议

### 7.1 适用依赖

| 依赖 | 来源 | 版本 | 适用于 torvox？ | 建议 |
|------|------|------|-----------------|------|
| wgpu | zelland/wgpu-in-app | 30 | ✅ 已使用 | 保持 |
| glyphon | zelland | - | ❌ torvox 用 cosmic-text+swash | 不需要 |
| libghostty-vt | zelland/ghostty-android | - | ✅ 已使用 | 保持 |
| jni | wgpu-in-app | 0.21 | ✅ 已使用 | 保持 |
| ndk-sys | wgpu-in-app | 0.6 | ✅ 已使用 | 保持 |
| tower-mcp | torvox | 0.14 | ✅ 已使用 | 保持 |
| bytemuck | zelland | 1.22 | ✅ 已使用 | 保持 |
| glam | wgpu-in-app | 0.32 | ❌ torvox 不需要矩阵数学 | 不需要 |
| noise | wgpu-in-app | 0.9 | ❌ 不相关 | 不需要 |

### 7.2 先进激进建议

| 建议 | 来源 | 先进性 | 风险 | 建议 |
|------|------|--------|------|------|
| `get_current_texture` Outdated 重试 | wgpu-in-app | 中 | 低 | **立即添加** |
| `normalize_view_size` 零尺寸防护 | wgpu-in-app | 低 | 低 | 已有等效 |
| `selectionGeometryKey` 避免不必要重定位 | ghostty-android | 中 | 低 | **添加** |
| `tapCount` 不依赖 GestureDetector 默认检测 | ghostty-android | 中 | 低 | **添加** |
| 边缘滚动（拖拽到屏幕边缘自动滚动） | termux-app | 高 | 中 | **添加** |
| Choreographer 驱动渲染 | warp | 高 | 高 | 考虑（当前 poll loop 已工作） |
| MCP 同意门控 | Haven | 中 | 低 | 可选 |
| `selection_vertex_buf` 50 行限制 | zelland | 低 | 低 | 可选（性能优化） |

---

## 8. 代码注释索引

已添加的 torvox 代码注释（引用参考项目）：

| 文件 | 行号 | 引用项目 | 内容 |
|------|------|----------|------|
| `native/src/render/context.rs` | :125 | zelland WGPU_FIXES Fix 1 | atlas 格式必须等于 surface format |
| `native/src/render/context.rs` | :300 | wgpu-in-app SurfaceFrame | acquire 失败重试（Outdated→reconfigure） |
| `native/src/render/context.rs` | :473 | zelland Fix 2 | surface 创建后必须 render |
| `native/src/render/context.rs` | :380 | wgpu-in-app android.rs | ANativeWindow 引用计数管理 |
| `native/src/render/cell_builder.rs` | :13-15 | zelland | REVERSE_BIT 位操作注释 |
| `native/src/render/cell_builder.rs` | :205-206 | zelland | SGR 7 反色 swap 注释 |
| `native/src/render/font/rasterization.rs` | :1-15 | termux-app/ghostty-android | 行高 = ascent+descent 注释 |
| `TerminalViewModel.kt` | :~337 | moke | getSelectedText 行拼接语义 |
| `TerminalRuntime.kt` | :~1316 | warp | vsync 渲染节奏模式 |
| `TerminalSurface.kt` | :~254 | termux-app TextSelectionCursorController | TODO TYPE_FLOATING 迁移 |
| `BootstrapInstaller.kt` | :15 | termux-app TermuxInstaller | staging 目录原子安装注释 |

**待添加**：
- `context.rs`：wgpu-in-app `normalize_view_size` 零尺寸防护
- `TerminalSurface.kt`：ghostty-android `selectionGeometryKey` 避免不必要重定位
- `TerminalSurface.kt`：ghostty-android `tapCount` 不依赖 GestureDetector
- `TerminalViewModel.kt`：termux-app 边缘滚动逻辑
