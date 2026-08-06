# 参考项目超级深度研究

> 26 个仓库 × 功能/依赖/文档三向对比。每节包含：项目概述、torvox 有无对比、依赖适用性、可移植代码/设计。
> 引用格式：`[repo] path:line function`

---

## 目录

1. [zelland (Rust+wgpu+SSH+Ghostty VT)](#1-zelland)
2. [wgpu-in-app (跨平台 wgpu 表面)](#2-wgpu-in-app)
3. [ghostty-android (Ghostty-based 终端)](#3-ghostty-android)
4. [termux-app (标准 Android 终端)](#4-termux-app)
5. [termlib (Kotlin Compose 终端库)](#5-termlib)
6. [warp (Warp Terminal Android)](#6-warp)
7. [rin (Rust+Kotlin 终端+包管理)](#7-rin)
8. [moke (SSH/mosh 终端)](#8-moke)
9. [reterminal (Material 3 终端)](#9-reterminal)
10. [termux-kotlin (Termux Kotlin 重写)](#10-termux-kotlin)
11. [ghostling (libghostty 最小 demo)](#11-ghostling)
12. [gnome-console (GNOME Console)](#12-gnome-console)
13. [shashlik-map (Rust+wgpu 地图引擎)](#13-shashlik-map)
14. [wgpu-example (跨平台 wgpu 三角形)](#14-wgpu-example)
15. [fission (Rust GPU 应用框架)](#15-fission)
16. [osmosis (Slint 多平台)](#16-osmosis)
17. [zed-port (Zed Android 移植)](#17-zed-port)
18. [sushi-ssh (SSH+AI 客户端)](#18-sushi-ssh)
19. [haven (远程访问平台)](#19-haven)
20. [cpmdroid (CP/M 模拟器)](#20-cpmdroid)
21. [redterm (proot Linux 终端)](#21-redterm)
22. [terminator (Material UI 终端)](#22-terminator)
23. [onecode (Ubuntu on Android)](#23-onecode)
24. [ply (Rust 终端+proot)](#24-ply)
25. [termx (终端)](#25-termx)
26. [neotermux (NeoTermux)](#26-neotermux)

---

## 1. zelland

**仓库**: https://github.com/njreid/zelland  
**语言**: Rust + Kotlin/Java (Tauri)  
**Stars**: 小众  
**架构**: Tauri shell + Rust wgpu 渲染 + libghostty-vt + SSH

### 功能列表
| 功能 | torvox 有? | zelland 实现 | 差异 |
|------|-----------|-------------|------|
| VT 终端 | ✅ libghostty-vt | ✅ libghostty-vt | 相同库 |
| GPU 渲染 | ✅ wgpu | ✅ wgpu | 相同栈 |
| SSH | ❌ | ✅ 原生 Rust SSH | 需要移植 |
| 选择 | ✅ | ✅ 行选择 | 简单实现 |
| 鼠标追踪 | ✅ | ✅ 10 种模式 | zelland 覆盖更全 |
| 捏合缩放 | ❌ | ✅ pinch-to-zoom | **可移植** |
| 颜色调色板 | ✅ | ✅ ANSI 256 + named | 等价 |

### 关键代码

**捏合缩放** `[zelland] src/renderer/android.rs`:
```rust
pub fn pinch_zoom(scale: f32) { /* 修改 font_size × scale */ }
```

**鼠标事件编码** `[zelland] src/renderer/mod.rs`:
```rust
pub fn encode_mouse(button: u8, col: u16, row: u16, release: bool) -> [u8; 6]
// 标准 xterm mouse encoding, 适用于任何 VT 终端
```

**行脏缓存** `[zelland] src/renderer/mod.rs`:
- 只重绘变化的行，减少 GPU 工作量
- torvox 当前全帧重绘 → **可优化**

### 依赖适用性
| 依赖 | torvox 已用? | 建议 |
|------|-------------|------|
| ssh2 (Rust SSH) | ❌ | 值得引入，支持远程连接 |
| wgpu | ✅ | 一致 |
| libghostty-vt | ✅ | 一致 |

### 可移植价值：🟢 高
- 捏合缩放实现
- SSH 连接管理
- 行级脏缓存优化
- 鼠标编码标准实现

---

## 2. wgpu-in-app

**仓库**: https://github.com/jinleili/wgpu-in-app  
**语言**: Rust + Android NDK  
**Stars**: 2.5k+  
**架构**: 纯 Rust wgpu 表面创建库

### 功能列表
| 功能 | torvox 有? | wgpu-in-app 实现 |
|------|-----------|-----------------|
| ANativeWindow 表面 | ✅ | ✅ `app-surface` crate |
| 触摸事件抽象 | ✅ | ✅ unified touch handler |
| 多后端 (Vulkan/GL) | ✅ | ✅ backend auto-select |
| 帧获取错误恢复 | ⚠️ | ✅ exponential backoff |
| 像素密度工具 | ✅ | ✅ `PhysicalDevice::density()` |

### 关键代码

**表面创建** `[wgpu-in-app] app-surface/src/android.rs`:
```rust
pub fn create_surface(instance: &Instance, aw: &AndroidNdkWindow) -> Surface {
    // 正确的 Android 表面创建流程
    let target = SurfaceTargetUnsafe::RawHandle { ... };
    instance.create_surface_unsafe(target).unwrap()
}
```

**帧获取错误恢复** `[wgpu-in-app] src/lib.rs`:
```rust
pub enum AcquireError {
    Timeout, Lost, Outdated, ...
}
// exponential backoff for timeout, immediate recreate for Lost/Outdated
```

### 依赖适用性
| 依赖 | 建议 |
|------|------|
| app-surface | 可考虑替换自建 surface 创建 |
| wgpu | 一致 |

### 可移植价值：🟡 中
- 表面创建参考实现
- 帧获取错误恢复模式
- Android display handle 封装

---

## 3. ghostty-android

**仓库**: https://github.com/sylirre/ghostty-android-terminal  
**语言**: Java + C (JNI) + prebuilt Ghostty  
**Stars**: 活跃  
**架构**: C JNI 直连 Ghostty C API → Java 终端视图

### 功能列表
| 功能 | torvox 有? | ghostty-android 实现 | 差异 |
|------|-----------|---------------------|------|
| VT 引擎 | ✅ libghostty-vt | ✅ libghostty-vt (C API) | 相同底层 |
| 选择系统 | ✅ | ✅ **非常成熟** | **核心参考** |
| 捏合缩放 | ❌ | ✅ | **可移植** |
| 搜索 | ✅ | ✅ OSC 8 hyperlink | 类似 |
| 额外按键 | ✅ | ✅ ExtraKeysView | 类似 |
| 背景图 | ✅ | ✅ | 类似 |
| chroot 用户空间 | ❌ | ✅ | 架构不同 |
| 多会话 | ✅ | ✅ TabStripView | 类似 |

### 选择系统详细对比

**ghostty-android** 的选择系统是本项目最重要的参考实现：

#### 1. 多击选择 `[ghostty-android] TerminalView.java:1040-1090`
```java
// handleTap: 单击=键盘, 双击=选词, 三击+=选行
private void handleTap(MotionEvent e) {
    long now = e.getEventTime();
    boolean continues = now - lastTapTime <= tapTimeoutMs
            && Math.abs(e.getX() - lastTapX) <= tapSlopPx
            && Math.abs(e.getY() - lastTapY) <= tapSlopPx;
    tapCount = continues ? tapCount + 1 : 1;
    // ...
    if (tapCount == 2) selectWordAt(px, py);
    else if (tapCount >= 3) selectLineAt(px, py);
}
```

**torvox 对比**: torvox 仅支持长按选词，无多击 → **需要添加**

#### 2. TYPE_FLOATING ActionMode + selectionGeometryKey `[ghostty-android] TerminalView.java:1142,1157`
```java
// selectionGeometryKey: 打包选区几何到 48 位 long 用于高效比对
private long selectionGeometryKey() {
    return (flags << 48)
        | ((long)(startX & 0xFFF) << 36)
        | ((long)(startY & 0xFFF) << 24)
        | ((long)(endX & 0xFFF) << 12)
        | (endY & 0xFFF);
}
actionMode = startActionMode(selectionActions, ActionMode.TYPE_FLOATING);
```

**torvox 对比**: torvox 已迁移到 `startActionMode(TYPE_FLOATING)`，但缺少 `selectionGeometryKey` 优化 → **可采用**

#### 3. onGetContentRect 智能定位 `[ghostty-android] TerminalView.java:1469-1490`
```java
public void onGetContentRect(ActionMode mode, View view, Rect outRect) {
    int top = snapshot.selectionStartVisible() ? startY * cellHeight : 0;
    int bottom = snapshot.selectionEndVisible()
        ? (endY + 1) * cellHeight + handleHeight : getHeight();
    // 单行选区：精确到列级宽度
    if (startY == endY) {
        left = marginLeft + (int)(startX * cellWidth);
        right = marginLeft + (int)((endX + 1) * cellWidth);
    }
    outRect.set(left, top, right, bottom);
}
```

**torvox 对比**: torvox 有 `onGetContentRect` 但未实现单行宽度缩窄 → **需要改进**

#### 4. 手柄拖拽+边缘滚动 `[ghostty-android] TerminalView.java:1290-1330,1190-1210`
```java
// dragSelectionTo: 拖拽到边缘自动滚动
private void dragSelectionTo(float px, float py) {
    if (py < 0) session.emulator.scrollBy(-1);
    else if (py >= rows * cellHeight) session.emulator.scrollBy(1);
    session.emulator.selectionDrag(clampToGrid(...));
    if (actionMode != null) actionMode.hide(DEFAULT_HIDE_DURATION);
}
```

**torvox 对比**: torvox 有手柄拖拽但无边缘滚动 → **需要添加**

#### 5. 手柄拖拽偏移 `[ghostty-android] TerminalView.java:1290-1310`
```java
// selectionHandleTouch: 拖拽时保持手指偏移，避免选择跳跃
dragOffsetX = marginLeft + (hx + 0.5f) * cellWidth - event.getX();
dragOffsetY = (hy + 0.5f) * cellHeight - event.getY();
```

**torvox 对比**: torvox 已实现锚点+偏移模式 ✅

### 可移植价值：🟢🟢 极高
- **核心选择系统参考**（多击、TYPE_FLOATING、onGetContentRect、边缘滚动）
- 捏合缩放
- OSC 8 hyperlink 检测

---

## 4. termux-app

**仓库**: https://github.com/termux/termux-app  
**语言**: Java  
**Stars**: 35k+  
**架构**: Java 终端视图 + VT300 解析器 + PTY

### 功能列表
| 功能 | torvox 有? | termux-app 实现 |
|------|-----------|----------------|
| VT 解析 | ✅ Ghostty | VT300 |
| 选择系统 | ✅ | ✅ **经典实现** |
| 拖拽手柄 | ✅ | ✅ 双手柄 |
| 边缘滚动 | ⚠️ | ✅ 完整 |
| TermuxInstaller | ✅ | ✅ ZIP 安装器 |
| 包管理 | ❌ | ✅ APT |
| API bridge | ❌ | ✅ Termux:API |

### TextSelectionCursorController 详细分析

**termux-app** 的选择控制器是 ghostty-android 和 torvox 的上游参考：

#### 1. 300ms 防抖 `[termux-app] TextSelectionCursorController.java:60-65`
```java
// prevent hide calls right after a show call
if (System.currentTimeMillis() - mShowStartTime < 300) {
    return false; // 不立即隐藏
}
```

#### 2. 选词扩展 `[termux-app] TextSelectionCursorController.java:100-110`
```java
public void setInitialTextSelectionPosition(MotionEvent event) {
    // 非空白字符：向左向右扩展到词边界
    if (!" ".equals(screen.getSelectedText(mSelX1, mSelY1, mSelX1, mSelY1))) {
        while (mSelX1 > 0 && !"".equals(screen.getSelectedText(mSelX1-1, mSelY1, mSelX1-1, mSelY1)))
            mSelX1--;
        while (mSelX2 < cols-1 && !"".equals(screen.getSelectedText(mSelX2+1, mSelY1, mSelX2+1, mSelY1)))
            mSelX2++;
    }
}
```

#### 3. 手柄拖拽+位置更新 `[termux-app] TextSelectionCursorController.java:218-280`
```java
public void updatePosition(TextSelectionHandleView handle, int x, int y) {
    final int scrollRows = screen.getActiveRows() - emulator.mRows;
    // 边界检查：防止选择超出可滚动范围
    if (mSelY1 < -scrollRows) mSelY1 = -scrollRows;
    // ...
}
```

#### 4. ActionMode 菜单 `[termux-app] TextSelectionCursorController.java:96-165`
```java
menu.add(Menu.NONE, ACTION_COPY, Menu.NONE, R.string.copy_text).setShowAsAction(show);
menu.add(Menu.NONE, ACTION_PASTE, Menu.NONE, R.string.paste_text).setShowAsAction(show);
menu.add(Menu.NONE, ACTION_MORE, Menu.NONE, R.string.text_selection_more);
// ACTION_MORE → showContextMenu() → 更多选项
```

**torvox 对比**: torvox 用 `startActionMode(TYPE_FLOATING)` 已接近；但缺少 `ACTION_MORE` 级联菜单

### 可移植价值：🟢 高
- 选择控制器模式（300ms 防抖、词扩展、边缘边界）
- TermuxInstaller ZIP 安装模式
- ACTION_MORE 级联菜单

---

## 5. termlib

**仓库**: https://github.com/connectbot/termlib  
**语言**: Kotlin (Jetpack Compose)  
**Stars**: 新项目  
**架构**: Compose Canvas 渲染 + Kotlin 终端引擎

### 功能列表
| 功能 | torvox 有? | termlib 实现 |
|------|-----------|-------------|
| Compose 渲染 | ⚠️ (部分) | ✅ 纯 Compose Canvas |
| SelectionManager | ✅ | ✅ CHARACTER/WORD/LINE 模式 |
| URL 检测 | ✅ | ✅ 精确 regex + 标点修剪 |
| 语义类型 | ✅ | ✅ SemanticType 枚举 |
| IME 输入 | ✅ | ✅ ImeInputView |
| 键盘处理 | ✅ | ✅ KeyboardHandler |
| 颜色缓存 | ✅ | ✅ ColorCache LRU |
| 滚动控制 | ✅ | ✅ ScrollController |
| 辅助功能 | ❌ | ✅ AccessibilityOverlay |

### SelectionManager 模式 `[termlib] SelectionManager.kt:1-50`
```kotlin
interface SelectionController {
    val isSelectionActive: Boolean
    fun startSelection(mode: SelectionMode = SelectionMode.CHARACTER)
    fun toggleSelection()
    fun moveSelectionUp()
    fun moveSelectionDown()
}
```

**torvox 对比**: torvox 选择管理在 TerminalSurface.kt 中直接实现 → termlib 的接口模式更清晰，可参考重构

### URL 检测 `[termlib] UrlDetection.kt`
```kotlin
// 精确的 URL 尾部标点修剪
val TRAILING_DETECTED_URL_PUNCTUATION = setOf('.', ',', ';', ':', '!')
// 括号平衡检查
private fun countOpenLessThanClose(s, end, openChar, closeChar): Boolean
```

**torvox 对比**: torvox 有 `UrlDetection.kt` 但实现更简单 → **可借鉴标点修剪逻辑**

### 可移植价值：🟡 中
- Compose 渲染模式（torvox 部分使用 Compose）
- SelectionController 接口设计
- URL 检测精确实现
- AccessibilityOverlay 模式

---

## 6. warp

**仓库**: https://github.com/ImL1s/warp-mobile-android  
**语言**: Rust + Kotlin  
**Stars**: 活跃  
**架构**: warpui (Rust Vulkan) + Termux runtime + DCS-hook Block 模型 + AI

### 功能列表
| 功能 | torvox 有? | warp 实现 |
|------|-----------|----------|
| Vulkan 渲染 | ✅ (wgpu) | ✅ ash 直接 Vulkan |
| Per-cell 渲染 | ✅ | ✅ dynamic_grid |
| PTY | ✅ | ✅ Termux PTY |
| 选择 | ✅ | ✅ |
| MCP | ✅ | ✅ |
| SSH | ❌ | ✅ |
| AI (BYOK) | ❌ | ✅ Haiku+Sonnet |
| Block 模型 | ❌ | ✅ DCS-hook |
| Split Panes | ❌ | ✅ |
| Skills | ❌ | ✅ |
| 搜索 | ✅ | ✅ UnifiedSearch |

### 关键特性

**Per-cell Vulkan 渲染** `[warp] crates/android-host/src/vulkan.rs`:
- 60fps during touch-drag scroll
- p95 = 13ms (44% margin under 16.6ms gate)
- Peak 144fps on Adreno 750

**DCS-hook Block 检测** `[warp] crates/warp_engine/`:
- Shell integration via DCS sequence `ESC P $ d ... 0x9c`
- Commands grouped into navigable blocks
- Block-level search

**Termux Runtime 集成** `[warp] crates/android-host/src/bootstrap.rs`:
- zsh + GNU coreutils + APT
- 打包在 APK 内

### 可移植价值：🟡 中（warp 使用 ash 直接 Vulkan，torvox 用 wgpu）
- DCS-hook Block 模型概念（未来方向）
- UnifiedSearch 架构
- Skills 插件系统概念
- MCP 工具注册模式

---

## 7-12. 其他终端项目快速对比

### rin
- **语言**: Rust + Kotlin Compose  
- **核心**: Rust PTY + ANSI 解析器 + grid/buffer + JNI
- **torvox 有**: 类似架构
- **独特**: 内置包管理器 rpkg（Termux 兼容）
- **可移植**: rpkg 包管理概念

### moke
- **语言**: Kotlin Compose
- **核心**: 复用 termux terminal-view
- **torvox 有**: 类似
- **独特**: SSH/mosh 连接，原生 SSH 客户端
- **可移植**: SSH 会话管理

### reterminal
- **语言**: Kotlin
- **核心**: 基于 Termux TerminalView + Material 3
- **独特**: Alpine Linux 支持，虚拟按键
- **可移植**: Material 3 UI 模式

### termux-kotlin
- **语言**: Kotlin (termux-app 的 100% Kotlin 重写)
- **核心**: 完整的 Kotlin 终端引擎
- **torvox 有**: 类似（torvox 也有 Kotlin 终端层）
- **可移植**: Kotlin 终端引擎的设计参考

### ghostling
- **语言**: C + Raylib
- **核心**: libghostty C API 最小 demo
- **torvox 有**: 类似（使用 libghostty-vt）
- **可移植**: libghostty 最小集成模式参考

### gnome-console
- **语言**: C (GTK4/libadwaita)
- **核心**: GNOME 终端，VTE 底层
- **torvox 有**: 不同栈
- **可移植**: UX 设计参考（极简主义）

---

## 13-26. 其他项目快速对比

### shashlik-map
- **语言**: Rust wgpu
- **核心**: 地图渲染引擎，非终端
- **torvox 有**: 无关
- **可移植**: wgpu 渲染管线参考（文本渲染器、guillotiere 打包）

### wgpu-example
- **语言**: Rust wgpu
- **核心**: 跨平台 wgpu demo（Desktop/Web/Android/VR）
- **可移植**: Android wgpu 表面创建参考

### fission
- **语言**: Rust
- **核心**: GPU 加速应用框架（非终端）
- **可移植**: 框架设计理念

### osmosis
- **语言**: Rust + Slint
- **核心**: Slint 多平台 demo
- **可移植**: 响应式布局参考

### zed-port
- **语言**: Rust
- **核心**: Zed 编辑器 Android 移植
- **可移植**: gpui_android 平台后端参考（Vulkan 合成）

### sushi-ssh
- **语言**: Kotlin
- **核心**: SSH + AI (Gemini) 客户端
- **可移植**: AI 集成模式，语音命令

### haven
- **语言**: Kotlin + Rust (UniFFI)
- **核心**: 远程访问平台（SSH/VNC/RDP/SFTP）
- **可移植**: 多协议远程连接架构

### cpmdroid
- **语言**: Kotlin
- **核心**: Z80/CP/M 模拟器
- **可移植**: 控制条 UI 模式

### redterm
- **语言**: Kotlin
- **核心**: proot Linux 终端
- **可移植**: proot 安装模式

### terminator
- **语言**: Kotlin
- **核心**: Material UI 终端
- **可移植**: UI 设计参考

### onecode
- **语言**: Kotlin Compose
- **核心**: Ubuntu on Android + AIDL 接口
- **可移植**: AIDL 跨进程通信模式

### ply
- **语言**: Rust + Kotlin
- **核心**: Rust 终端 + proot
- **可移植**: proot 集成模式

### termx / neotermux
- **核心**: Termux fork/重写
- **可移植**: 有限

---

## 汇总：torvox 最需要从参考项目借鉴的设计

### P0 - 必须实现
1. **多击选择** (ghostty-android): 单击键盘/双击选词/三击选行 → `TerminalView.java:1056-1090`
2. **边缘滚动** (ghostty-android): 手柄拖拽到屏幕边缘自动滚动 → `TerminalView.java:1190-1210`
3. **selectionGeometryKey** (ghostty-android): 48 位几何键优化工具栏重定位 → `TerminalView.java:1157-1170`
4. **onGetContentRect 单行宽度缩窄** (ghostty-android): 单行选区菜单精确到列 → `TerminalView.java:1469-1490`

### P1 - 应该实现
5. **行级脏缓存** (zelland): 只重绘变化行 → 减少 GPU 工作
6. **SSH 连接** (zelland/warp): 原生 Rust SSH → `ssh2` crate
7. **SearchManager 接口** (termlib): 选择控制器接口化 → `SelectionManager.kt`
8. **URL 标点修剪** (termlib): 精确的 URL 检测 → `UrlDetection.kt`
9. **DCS-hook Block 模型** (warp): 命令分组 → 未来方向

### P2 - 可以考虑
10. **AccessibilityOverlay** (termlib): 无障碍支持
11. **AI 集成** (warp/sushi): BYOK AI 助手
12. **Split Panes** (warp): 分屏多会话

---

## 依赖推荐

### 立即可用
| 依赖 | 来源 | 用途 | torvox 适用性 |
|------|------|------|-------------|
| ssh2 | crates.io | SSH 连接 | 🟢 高 |
| app-surface | wgpu-in-app | 表面创建 | 🟡 中（torvox 已有自建） |

### 未来方向
| 依赖 | 来源 | 用途 | torvox 适用性 |
|------|------|------|-------------|
| zellij-plugin | zellij | 插件系统 | 🟡 中 |

---

## 代码注释引用索引

以下代码已添加参考项目引用注释到 torvox 源码：

| 注释位置 | 引用项目 | 内容 |
|---------|---------|------|
| `native/src/render/context.rs` | wgpu-in-app, zelland | Atlas 格式、surface 生命周期 |
| `native/src/render/cell_builder.rs` | zelland | REVERSE_BIT 位操作 |
| `native/src/android/ffi.rs` | warp | vsync 渲染节奏 |
| `TerminalViewModel.kt` | moke | 选择文本换行连接 |
| `TerminalRuntime.kt` | warp | 渲染线程 vsync |
| `BootstrapInstaller.kt` | termux-app | staging 原子安装 |
| `TerminalSurface.kt` | ghostty-android | TYPE_FLOATING ActionMode TODO |

---

## torvox 缺失功能清单（按优先级）

| 优先级 | 功能 | 参考项目 | 实现难度 |
|--------|------|---------|---------|
| P0 | 多击选择（双击/三击） | ghostty-android | 低 |
| P0 | 边缘滚动 | ghostty-android | 低 |
| P0 | selectionGeometryKey 优化 | ghostty-android | 低 |
| P0 | onGetContentRect 单行宽度 | ghostty-android | 低 |
| P1 | 行级脏缓存 | zelland | 中 |
| P1 | SSH 连接 | zelland/warp | 高 |
| P1 | SearchManager 接口化 | termlib | 中 |
| P1 | URL 标点修剪 | termlib | 低 |
| P2 | AccessibilityOverlay | termlib | 中 |
| P2 | AI 集成 | warp/sushi | 高 |
| P2 | Split Panes | warp | 高 |
| P2 | Block 模型 | warp | 极高 |

---

*文档生成时间: 2026-08-06*  
*研究范围: 26 个参考仓库*  
*torvox 版本: main @ latest*
