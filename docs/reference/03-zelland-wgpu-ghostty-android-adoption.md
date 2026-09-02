# 深度研究：zelland / wgpu-in-app / ghostty-android 特性对比与采纳计划

> 研究日期：2026-08-07
> 参考仓库：`njreid/zelland`、`jinleili/wgpu-in-app`、`sylirre/ghostty-android-terminal`（深度克隆于 `/tmp/refs3/`）

---

## 1. zelland — 行级脏缓存（src-tauri/src/renderer/mod.rs:843-890）

### 参考实现

```rust
pub fn draw_ghostty_state(&mut self, state: &mut GhosttyRenderStateWrapper, cursor_pos: (u16, u16)) {
 let dirty = state.get_dirty();
 if dirty == GHOSTTY_RENDER_STATE_DIRTY_FALSE { return; }
 let mut changed = false;
 state.with_rows(|line_idx, is_dirty, cells| {
 if is_dirty || line_idx as usize >= self.row_cache.len() {
 let runs = build_row_runs(cells);
 let row_idx = line_idx as usize;
 if row_idx >= self.row_cache.len() { self.row_cache.resize(row_idx + 1, Vec::new()); }
 if self.row_cache[row_idx] != runs { // 行级去重
 self.row_cache[row_idx] = runs;
 changed = true;
 }
 }
 });
 // 终端变小时收缩缓存（防止幽灵行）
 let (_num_cols, num_rows) = state.get_size();
 if self.row_cache.len() > num_rows as usize {
 self.row_cache.truncate(num_rows as usize);
 changed = true;
 }
 // 光标位置每帧更新，与文本变化解耦
 ...
 if !changed { return; } // 跳过昂贵的 text pipeline（shape + prepare）
}
```

### 核心价值

1. **行级粒度**：`with_rows(|line_idx, is_dirty, cells|)` 只重建脏行，而非整屏。
2. **行缓存 + 内容比较**：`row_cache[row_idx] != runs` 去重，即使行标脏但内容没变也跳过。
3. **整体 dirty 标志**：`get_dirty() == FALSE` 直接整帧跳过（我们的 `cell_data_dirty` 类似）。
4. **光标独立于文本更新**：光标每帧更新，文本只有 changed 才走 text pipeline。

### 本地对比

- 本地 `build_cell_data()` 每帧全量重建全部 cell 的 CellData（`render_inner` → `receive_cell_data` → `build_instances_from_cell_data` 每帧全量）。
- 本地已有 `cell_data_dirty` 全局脏标志（internal.rs），但没有**行级**脏跟踪。
- Ghostty C API 提供 `with_rows` 行级访问（`libghostty-vt` bindings 中 `terminal_cell_iterator` / `render_state` 系列）。需确认 `with_rows` 是否暴露。

### 采纳评估

- **中价值/中风险**：改造成本高（需在 render_state 暴露行级脏 API），当前 SwiftShader 模拟器 ~1.8fps 已受 GPU 限制，行级缓存收益主要在真机。
- **可做简化版**：`build_cell_data` 中跳过 `cell_data_dirty == false` 时的重建已存在；补「行缓存比较」到 `build_instances_from_cell_data` 不现实（实例是平的）。
- **决定**：记录为参考，暂不实施全量行级脏缓存（与 Ghostty 上游架构耦合过深）；在 cell_builder 处补注释。

---

## 2. zelland — 捏合缩放 + get_cell_size() 动态更新

### 参考实现（src-tauri/src/renderer/mod.rs:601-611, 1094-1096）

```rust
pub fn update_font_size(&mut self, css_px: f32, dpr: f32) {
 let physical = css_px * dpr;
 self.cell_height = physical;
 self.cell_width = physical * 0.45;
 let font_size = physical * 0.75;
 self.text_buffer.set_metrics(...);
 self.row_cache.clear(); // 字体变化必须清行缓存
 self.render();
}
pub fn get_cell_size() -> (f32, f32) {
 with_renderer(|r| (r.cell_width, r.cell_height))
}
```

### 关键点

1. 字体缩放 → 更新 cell 尺寸 → **清空行缓存**（我们的 glyph_id_cache/cjk_glyph_cache 已做类似）。
2. `get_cell_size()` 动态查询而非编译期常量——鼠标编码/像素映射全部走实时 cell 尺寸。
3. 捏合缩放在前端监听 ScaleGestureDetector，Rust 侧只需 `update_font_size` 幂等接口。

### 本地对比

- 本地 `setFontSizeInPlace` / `setRasterScale` 已有（JNI `setFontSizeInPlace` → cell 6.0x12.0 逻辑 × raster_scale）。
- 本地 `applyGridResize: 480 x 746 ime=0 cell=(13.502197,27.0) -> 20x35` 表明 Kotlin 端实时计算 cell 尺寸并重算 grid —— **已实现动态 cell 尺寸**。
- 本地 `ScaleGestureDetector` 已有（TerminalSurface.kt），缩放改变 font size 时走 `onZoomChanged`。
- **差异**：本地没有"缩放→清字体缓存"的显式钩子？需确认 `setFontSizeInPlace` 是否清 glyph cache（之前 round 已修 `set_font_size_in_place` 清 shape_cache）。

### 采纳评估

- **已基本实现**。补：确认 `set_font_size_in_place` 清 `glyph_id_cache` + `cjk_glyph_cache`（与 zelland `row_cache.clear()` 对齐）。

---

## 3. zelland — 鼠标编码标准实现（ghostty_mouse_encoder）

### 参考实现（src-tauri/src/terminal.rs:41-90+）

```rust
pub fn process_mouse(&self, x: f32, y: f32, action: &str) -> Vec<Vec<u8>> {
 let mouse_mode = self.get_mouse_mode();
 if !mouse_mode { log::warn!("process_mouse: mouse_mode=false, event dropped"); return vec![]; }
 ...
}
pub fn encode_mouse_event(&self, x_px: f32, y_px: f32, action: &str) -> Option<Vec<u8>> {
 // 使用 Ghostty 官方 C API 编码：
 ghostty_mouse_encoder_new(...);
 ghostty_mouse_encoder_setopt_from_terminal(encoder, self.term.inner);
 // 用渲染器实时 cell 尺寸做像素→cell 映射
 let (cell_w, cell_h) = crate::renderer::get_cell_size();
 let size = GhosttyMouseEncoderSize { screen_width, screen_height, cell_width, cell_height, ... };
 ghostty_mouse_encoder_setopt(encoder, OPT_SIZE, &size);
 ghostty_mouse_event_set_position(event, { x, y });
 ghostty_mouse_event_set_action(event, PRESS/RELEASE);
 ghostty_mouse_event_set_button(event, LEFT/RIGHT/FOUR/FIVE);
 ghostty_mouse_encoder_encode(encoder, event, buf, &mut out_len);
}
```

### 核心价值

1. **标准实现**：用 Ghostty 官方 mouse encoder（SGR/1006/X10 编码全部由上游处理），不做手写。
2. **mouse_mode 门控**：非 1000/1002/1003 模式时丢弃事件（防止误发序列）。
3. **实时 cell 尺寸**：像素→cell 用渲染器当前值。

### 本地对比

- 本地有 `MouseModeTracker.kt`（Kotlin 追踪 1000/1002/1003 模式）+ `MouseModeTrackerTest`（8 测试）。
- 本地 TerminalSurface 只有右击（expandToWord）和中击（paste）——**没有标准鼠标事件编码发送到 PTY**！滚动、左键拖动没有 SGR 序列。
- bindings.rs 已有 `MouseEncoder` / `MouseEncoderSize` 类型（3632-3666 行）——**API 已暴露，未使用**。
- libghostty-vt 的 terminal 是否暴露 `setopt_from_terminal`？需查 bindings。

### 采纳评估

- **高价值/低成本**：实现 `native` 侧 `encode_mouse_event(x, y, action, cell_w, cell_h)`，Kotlin 侧在 mouse mode 激活时把滚动/点击发送。真机 + 桌面 SSH 场景（vim/htop 鼠标支持）是实际功能缺口。
- 需要：bindings 确认 `ghostty_mouse_encoder_*` 全套 + `setopt_from_terminal`。

---

## 4. wgpu-in-app — jni_fn 宏（ffi/android.rs:10-19）

### 参考实现

```rust
use jni_fn::jni_fn;
#[unsafe(no_mangle)]
#[jni_fn("name.jinleili.wgpu.RustBridge")]
pub fn createWgpuCanvas(env: *mut JNIEnv, _: JClass, surface: jobject, idx: jint) -> jlong { ... }
```

### 核心价值

1. 自动生成 `Java_包名_类名_函数名` 符号，避免手写 17 个函数名的拼写错误（我们曾因 `Java_io_bridge_NativeWindow` 死代码吃过亏）。
2. 编译期检查函数名与类名一致。

### 本地对比

- 本地手写 `Java_terminal_emulator_bridge_NativeBridge_*`（17 个，ffi.rs）。
- 需要加 `jni_fn` crate 依赖（轻量，零传递依赖？需确认）。

### 采纳评估

- **中价值/低成本**：加 `jni_fn = "0.6"` 依赖，17 个函数加 `#[jni_fn("terminal.emulator.bridge.NativeBridge")]`。风险：宏生成的符号名必须与 Kotlin `external fun` 完全一致——正因如此值得用（编译期强制）。
- **激进选择**：采纳。这是减少人为错误的防御。

---

## 5. wgpu-in-app — view_formats / acquire 差异对照

### 参考实现（app-surface/src/lib.rs:315-350）

```rust
let view_formats = if cfg!(feature = "webgl") {
 vec![] // WebGL 不支持 view_formats
} else if cfg!(target_os = "android") {
 // Android 不支持 view_formats:
 // Downlevel flags DownlevelFlags(SURFACE_VIEW_FORMATS) are required
 // but not supported on the device.
 vec![format] // format 与 view_formats 相同 → configure 自动忽略
} else if format.is_srgb() {
 vec![format, format.remove_srgb_suffix()]
} else {
 vec![format.add_srgb_suffix(), format.remove_srgb_suffix()]
};
```

### 本地对比

- 本地 context.rs:461 用 `view_formats: vec![]`（管线配置）——与 wgpu-in-app Android 分支**方向一致**（Android 不支持，给空/相同值）。
- pass.rs:109 已有 `SURFACE_VIEW_FORMATS` 注释。
- **差异**：本地没有把 `view_formats` 与 `format` 的关系写成注释对照。

### 采纳评估

- **低价值/极低成本**：在 context.rs attach_surface 处补注释（引用 wgpu-in-app app-surface/src/lib.rs:315-350 的对照表 + 我们的模拟器实证）。

---

## 6. ghostty-android — 选择系统 UX 最佳参考

### 6.1 选择状态由模拟器拥有（TerminalView.java:298-314）

```
// --- Selection. The emulator owns the selection itself (it tracks its
// text across scrolling and new output); this view only mirrors it:
// `selecting` spans the ActionMode lifecycle, the handle rects are
// recomputed from each snapshot in onDraw and hit-tested on touch.
private boolean selecting;
private int draggingHandle = -1;
private boolean longPressDragging;
private float dragOffsetX, dragOffsetY; // grabbed cell center − touch point
private ActionMode actionMode;
private long toolbarSelGeom = Long.MIN_VALUE;
```

**本地**：选择状态在 ViewModel（`state.value.selection`），View 只 mirror —— **架构一致**（甚至更彻底，Kotlin 侧选中状态在 VM）。

### 6.2 tapCount 多击选择（TerminalView.java:1051-1083）

```java
boolean continues = now - lastTapTime <= tapTimeoutMs
 && abs(e.getX() - lastTapX) <= tapSlopPx && abs(e.getY() - lastTapY) <= tapSlopPx;
tapCount = continues ? tapCount + 1 : 1;
if (tapCount == 1) { finishSelection(); return; } // 单击 → 取消选择
else if (tapCount == 2) { selectWordAt(x, y); } // 双击 → 词
else { selectLineAt(x, y); } // 三击 → 行
```

**本地**：`onDoubleTap`（TerminalSurface.kt:1350）已有词选择（快速连点 = 行）；三击→行在快速连点分支。本地注释已记录 ghostty-android 的 tapCount 模式差异。
**差异**：本地单击不清选择（ghostty-android 单击 dismiss 选择 + 打开键盘）。这是我们缺失的 UX：**单击已激活选择时应 dismiss**。

### 6.3 Callback2 + onGetContentRect 菜单锚定（TerminalView.java:1469-1492）

```java
@Override
public void onGetContentRect(ActionMode mode, View view, Rect outRect) {
 int top = snapshot.selectionStartVisible() ? snapshot.selectionStartY() * cellHeight : 0;
 int bottom = snapshot.selectionEndVisible()
 ? (snapshot.selectionEndY() + 1) * cellHeight + handleHeight
 : getHeight();
 int left = 0, right = getWidth();
 if (同一行) {
 left = textMarginLeft + (int)(snapshot.selectionStartX() * cellWidth);
 right = textMarginLeft + (int)((snapshot.selectionEndX() + 1) * cellWidth);
 }
 outRect.set(left, top, right, bottom);
}
```

**本地**：用默认顶部 ActionMode（TYPE_FLOATING 在 API-35 SwiftShader 模拟器不渲染——已实证，见 提交信息）。
**决定**：保留顶部 ActionMode（模拟器可见性实证优先），但把 `onGetContentRect` 的实现代码保留/补注释——真机上 TYPE_FLOATING 可用时切换。

### 6.4 selectionGeometryKey 工具栏重定位（TerminalView.java:1157）

```java
private long selectionGeometryKey() {
 if (!snapshot.hasSelection()) return Long.MIN_VALUE;
 long flags = (startVisible ? 1 : 0) | (endVisible ? 2 : 0);
 return (flags << 48) | (startX & 0xFFF) << 36 | (startY & 0xFFF) << 24
 | (endX & 0xFFF) << 12 | (endY & 0xFFF);
}
```

在 onDraw 里比较 `toolbarSelGeom != selectionGeometryKey()` 时才 `actionMode.invalidateContentRect()` —— **避免每帧重定位**。

**本地**：TerminalRuntime 每帧 poll 事件时检查选择变化触发重绘；菜单定位逻辑在 Kotlin `SelectionMenuOverlay`/handle 窗口。**无 GeometryKey 模式**——本地菜单是顶部栏无需定位；手柄窗口位置随选择更新。

### 6.5 边缘滚动（TerminalView.java:1190）

```java
if (py < 0) { session.emulator.scrollBy(-1); }
else if (py >= rows * cellHeight) { session.emulator.scrollBy(1); }
```

**本地**：已有 `edgeScrollHandler` + `pendingEdgeScroll`（TerminalSurface.kt ACTION_MOVE 分支）——**已实现**。

### 6.6 reshowToolbar 的 hide(0) 技巧（TerminalView.java:1247）

```java
private void reshowToolbar() {
 toolbarSelGeom = selectionGeometryKey();
 actionMode.invalidateContentRect();
 actionMode.hide(0); // 立即重显，避免 ~2s hide duration
}
```

**关键技巧**：拖拽结束重新显示菜单时，`invalidateContentRect()` 单独用会保留框架的 hide-requested 标志（~2s 延迟）。`hide(0)` 立即重显。

**本地**：`endSelection` 后 `menuDismissed = false` 触发 Compose 菜单重显——无 2s 延迟问题（Compose 状态驱动）。

---

## 7. ghostty-android — UI 层"零依赖自绘"范本

### 7.1 ThemeStore / 256 色生成（ThemeStore.java:27）

- 用户自定义主题链路：主题预设 + 自定义前景/背景/调色板 → 256 色扩展生成。
- **本地**：`resolvedTerminalTheme` + `BridgeTerminalColorScheme`（含 selectionBg 等）已有主题链路；**无 256 色生成**（我们从 Ghostty 拿 palette，16 色 + 扩展色由 Ghostty 处理）。

### 7.2 BackgroundImageStore（BackgroundImageStore.java:27-90）

```java
static File file(Context context) { return new File(context.getFilesDir(), FILE); }
static String importFrom(Context context, Uri src) {
 // 复制到私有存储（文件持久 + 无 SAF 权限失效问题）
 try (InputStream in = resolver.openInputStream(src); OutputStream out = ...) { ... }
}
static Bitmap decode(String path, int reqW, int reqH, int blurPercent) {
 // 降采样（power-of-two）+ 可选高斯模糊（BLUR_MAX_DIM=1080 上限）
}
```

**核心模式**：

1. **复制私有存储**：选图后立即复制到 `filesDir`，后续读取不依赖 SAF URI。
2. **自愈**：文件缺失/不可解码 → 返回 null → UI 回退纯色背景（不崩溃）。
3. **模糊预算**：blur 在 ≤1080px 副本上跑，半径 = 短边 5%，2 次 box-blur ≈ 高斯。

**本地**：背景图链路存在（`setBackgroundImage` JNI + 双字节序修复）；需检查是否复制到私有存储 + 自愈。

### 7.3 TabStripView 原地调和

ghostty-android 的标签栏在 onDraw 中自绘（无 Compose/RecyclerView 依赖）。本地用 Compose（SessionDrawer）——不同技术栈，模式不适用。

---

## 8. ghostty-android — native 层三知识

1. **scrollback 字节预算**：scrollback 行数据按字节预算限制，防止无限增长。

- 本地：`DEFAULT_SCROLLBACK_LINES=50_000`（行数预算）——方向一致，行数 vs 字节。
1. **NUL 剥离**：从终端读文本时剥离 NUL 字节。

- 本地：`read_line_text` 返回 `Option<String>`；**未显式剥离 NUL**。需补（`cell_to_text`/`read_line_text` 处）。
1. **winsize 像素字段**：TIOCSWINSZ 的 ws_xpixel/ws_ypixel 填像素值。

- 本地：`PtyPair` 是否填像素字段？需查 pty.rs。

---

## 9. 采纳优先级总表

| # | 特性 | 来源 | 价值 | 成本 | 决定 |
|---|------|------|------|------|------|
| A | 标准鼠标编码（ghostty_mouse_encoder） | zelland | 高（vim/htop 鼠标支持是实际缺口） | 中 | **实施** |
| B | 单击 dismiss 选择 | ghostty-android | 中（UX 一致性） | 低 | **实施** |
| C | NUL 剥离 | ghostty-android | 中（数据正确性） | 低 | **实施** |
| D | jni_fn 宏 | wgpu-in-app | 中（防拼写错误） | 低 | **实施** |
| E | view_formats 对照注释 | wgpu-in-app | 低 | 极低 | **实施**（注释） |
| F | 行级脏缓存 | zelland | 中（性能） | 高 | **暂缓**（记录） |
| G | 背景图复制私有存储 + 自愈 | ghostty-android | 中 | 低 | **检查/实施** |
| H | winsize 像素字段 | ghostty-android | 低 | 低 | **检查** |
| I | reshowToolbar hide(0) | ghostty-android | 低（Compose 无此问题） | 低 | 记录 |
| J | 行缓存字体变化清空 | zelland | 低（已实现） | 低 | 验证 |

---

## 10. 测试策略

- **Rust 单测**：mouse encoder（mode 门控、SGR 序列、越界 clamp）、NUL 剥离、read_line_text。
- **Kotlin JVM 单测**：MouseModeTracker 已有；补单击 dismiss 逻辑（SelectionState 变换）。
- **模拟器实测**：鼠标模式（`adb shell input mouse` 或 motion 注入）滚动/点击发送验证；单击 dismiss；背景图选图后重启（自愈）。
- **OCR**：菜单按钮、选择文本。
