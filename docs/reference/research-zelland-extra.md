# Zelland 深度研究补充文档（基于新克隆全量源码）

> 本报告基于 `/home/runner/work/kudzu/kudzu/repositories/refs/zelland`（njreid/zelland，83+ 文件）的新克隆逐文件精读，补充旧版 `research_zelland_wgpu.md`（基于 `/tmp/refs2` 克隆）未覆盖的细节：3-pass 渲染管线的完整实现、glyphon 集成陷阱、russh SSH 会话循环、libghostty-vt C FFI 集成、Android 平台层（SurfaceView/JNI/KeybarPlugin）、Svelte 前端数据流、daemon-rs/zellij-plugin/src-voice 附属组件，以及与 torvox 的功能级对比。
>
> 所有行号均以本克隆实际内容为准（`文件:行号`）。生成时间：2026-08-06。

---

## 目录

1. [项目定位与完整架构](#1-项目定位与完整架构)
2. [核心渲染管线：3-pass（renderer/mod.rs）](#2-核心渲染管线3-passrenderermodrs)
3. [JNI 平台层（renderer/android.rs）](#3-jni-平台层rendererandroidrs)
4. [libghostty-vt 集成（ghostty.rs + build.rs）](#4-libghostty-vt-集成ghosttyrs--buildrs)
5. [终端会话（terminal.rs）与 SSH 管理器（ssh.rs）](#5-终端会话terminalrs-与-ssh-管理器sshrs)
6. [Tauri 命令层（lib.rs）与配套模块](#6-tauri-命令层librs-与配套模块)
7. [Android Kotlin 平台层（gen/android）](#7-android-kotlin-平台层genandroid)
8. [Svelte 5 前端](#8-svelte-5-前端)
9. [daemon-rs（zlnd 守护进程）](#9-daemon-rszlnd-守护进程)
10. [附属组件：zellij-plugin / src-voice / proto / libghostty](#10-附属组件zellij-plugin--src-voice--proto--libghostty)
11. [项目文档资产](#11-项目文档资产)
12. [功能对比：zelland vs torvox（逐项）](#12-功能对比zelland-vs-torvox逐项)
13. [依赖分析（是否适用于 torvox）](#13-依赖分析是否适用于-torvox)
14. [可吸收到 torvox 的具体内容（含代码注释建议）](#14-可吸收到-torvox-的具体内容含代码注释建议)
15. [项目文档吸收价值](#15-项目文档吸收价值)

---

## 1. 项目定位与完整架构

### 1.1 定位

**README 自述**（README.md:1-15）："A native Android + Linux terminal client built on Tauri, wgpu, and SSH"——移动优先的 SSH 终端与命令中心。核心卖点：**通过 SSH 直连远程主机**（russh），在 Android 上用 **wgpu(Vulkan) + glyphon** 原生渲染终端（无 WebView 终端模拟），可选地通过本地 Rust 守护进程 `zellandd`（daemon-rs）同步协作式 markdown 注释。

关键历史：项目经历了从 alacritty_terminal + xterm.js → libghostty-vt + wgpu 原生渲染的迁移（WGPU_GHOSTTY_PLAN.md 标记 COMPLETED，但 PLAN.md:22 自评"partially complete"）。**仓库内同时存在旧版单 pass 渲染器 `src-tauri/src/renderer.rs`（15KB，旧 Phase-2 代码）与新版 3-pass 渲染器 `src-tauri/src/renderer/mod.rs`（42KB）**；`lib.rs:4` 只声明 `pub mod renderer;`，目录优先解析到 `renderer/mod.rs`，因此**生效的是 3-pass 版本**，`renderer.rs` 是未删除的死代码（仅在文档与 git 历史中可见）。

### 1.2 仓库全景（monorepo，5 个组件）

```
zelland/
├── src/                        # Svelte 5 前端（SvelteKit 静态适配）
├── src-tauri/                  # Tauri v2 Rust 客户端（SSH + 终端 + wgpu 渲染 + JNI）
│   └── gen/android/            # Tauri 生成的 Android 工程（Kotlin，仅提交 3 个 .kt）
├── daemon-rs/                  # zlnd 守护进程 + zn CLI（axum + tokio + Loro CRDT）
├── src-voice/                  # 独立实验：语音终端（whisper STT + portable_pty）
├── zellij-plugin/              # Zellij 后台插件（wasm，pipe 协议报 tab 列表）
├── libghostty/                 # libghostty-vt 集成壳（ghostty/ 为空 submodule，src/android_fix.zig）
├── libghostty-source/          # （空/占位）
├── proto/zelland.proto         # daemon 与客户端的 protobuf 契约（单一来源）
└── docs/features/*.md          # 15+ 份设计文档（详见 §11）
```

### 1.3 三端架构总览

```
┌─ Android（MainActivity.kt）────────────────────────────────────┐
│ DrawerLayout                                                    │
│ ├─ FrameLayout：WebView(Svelte，透明、Welcome/模态框)           │
│ │               + SurfaceView(wgpu Vulkan 终端表面，z-order 高)  │
│ └─ LinearLayout：原生侧边栏（sessions + hosts 树）               │
│ GestureDetector：单击→SGR click+弹键盘 / 长按→选择 / 双指滚动    │
│                 / 左滑 fling→开抽屉；ScaleGestureDetector→pinch │
│ KeybarPlugin：原生 IME 工具栏（Ctrl/Alt/Meta 双击锁定 + 方向键） │
└──────────┬ JNI ───────────────────────┬── addJavascriptInterface
           ▼                            ▼
┌─ Rust（src-tauri）─────────────────────────────────────────────┐
│ lib.rs（Tauri 命令）── ssh.rs(SshManager/russh)                │
│   ├─ terminal.rs(TerminalSession) ── ghostty.rs(C FFI 绑定)    │
│   │     └─ renderer/mod.rs（3-pass wgpu 渲染器，全局单例）      │
│   ├─ keystore.rs（Android Keystore/生物识别 + 文件密钥库）      │
│   ├─ helper.rs（远程 zlnd 助手自安装/自升级）                   │
│   ├─ network.rs（gotatun 用户态 WireGuard）                    │
│   └─ daemon.rs（WebSocket protobuf + HTTP 与 zlnd 通信）       │
└──────────┬ SSH(russh, keepalive 30s) ──┬─ HTTP/WS
           ▼                             ▼
   远程主机：zellij attach + zlnd 助手   daemon-rs(zlnd)：axum + Loro CRDT 注解同步
```

**数据流（关键）**：SSH 字节流 → `TerminalSession.process_bytes` → `ghostty_terminal_vt_write`（Zig VT 引擎）→ 16ms 定时器检查 `is_dirty` → `render_native` → `draw_ghostty_state`（读 Ghostty render state 脏行）→ glyphon 文本 + 光标 pass + 选择 overlay pass → Surface present。输入反向：Kotlin 手势 → JNI `passTouchToRust` → `SshManager.process_touch` → `TerminalSession.process_mouse`（SGR 1006 编码，经 ghostty_mouse_encoder）→ SSH channel。

### 1.4 技术栈版本

| 组件 | 版本 | 备注 |
|---|---|---|
| Tauri | 2.x（`tauri = "2"`） | wry/tao Android 支持 |
| Svelte | 5（runes） | SvelteKit 2 + vite 6 |
| wgpu | 23.0 | `raw-window-handle 0.6` |
| glyphon | 0.7 | 文本布局/着色 |
| russh | 0.57.0 | SSH 客户端 |
| libghostty-vt | Ghostty 上游 Zig（submodule） | bindgen 0.71 C 绑定 |
| jni/ndk | 0.21 / 0.9.0 | JNI 桥 |
| prost | 0.14.3 | protobuf |
| daemon-rs | axum 0.8 + tokio + loro(CRDT) + kdl + yrs | AGENTS.md:29-36 约定 |

---

## 2. 核心渲染管线：3-pass（renderer/mod.rs）

文件：`src-tauri/src/renderer/mod.rs`，1123 行。这是整个仓库最值得 torvox 研究的文件。

### 2.1 常量与 WGSL 着色器

- `CELL_WIDTH/CELL_HEIGHT`（:15-16）：`17.0 × 38.0` 物理像素（旧 renderer.rs:18-19 为 `24×32`）。实际运行中由 `update_font_size` 覆盖。
- `CURSOR_SHADER`（:19-39）：**硬编码 6 顶点三角形列表面**（无顶点缓冲），uniform 含 `rect(vec4: x_left,y_top,x_right,y_bottom, NDC)` + `color(vec4)`；`vs_main` 用 `array<f32,6>` 展开矩形。片段直接返回 uniform 颜色。
- `SELECTION_SHADER`（:41-50）：从顶点缓冲读 `pos: vec2`，片段**硬编码半透明蓝 `(0.3, 0.5, 1.0, 0.35)`**——选择色不可配。

### 2.2 数据结构

- `CellRun`（:53-60）：单行内一段样式一致的文本（`text, fg(rgb), bold, italic`）。注意：**不支持 underline/strikethrough/dim 属性**（glyphon 只用了 weight/style/color），也不区分背景色（背景就是 surface clear 黑）。
- `Renderer`（:62-105）字段分组：
  - GPU 核心：`instance/adapter/device/queue` + `surface/config`（`Option`）+ `pending_size`；
  - glyphon：`_cache: Cache`（命名 `_cache` 说明只用于 Viewport 构造）、`font_system/swash_cache/atlas/atlas_format/text_renderer/viewport/text_buffer`；
  - **行级损伤缓存**：`row_cache: Vec<Vec<CellRun>>`（每行一个 run 列表）+ `span_buf`（复用 `(String, Weight, Style, Color)` 元组，避免每帧分配）；
  - 光标：`cursor_pipeline/bind_group_layout/uniform_buf/bind_group`（均 Option）+ `cursor_pixel_rect: Option<(px,py,w,h)>`；
  - 选择：`selection: Option<(sc,sr,ec,er)>`（列/行，归一化）+ `selection_pipeline/vertex_buf/vertex_count`；
  - `debug_frames_saved: u32`（前 3 帧存 PPM）。
- 全局单例：`static RENDERER: Lazy<Mutex<Option<Renderer>>>`（:107）——**进程级单例，只能有一个 tab 的 surface**（CLAUDE_NOTES.md:97 明示此限制）。
- `PENDING_SIZE`（:112）+ `store_pending_size`（:114-118）：尺寸在渲染器初始化（约 200ms）前到达时的暂存区，`set_surface` 时回读（:320-322）。

### 2.3 初始化 `init()`（:130-）

- wgpu Instance `Backends::all()`（:131-134；旧版 renderer.rs:43 是硬编码 VULKAN，CLAUDE_NOTES.md:107-110 批评过）。
- 适配器 `HighPerformance`、`compatible_surface: None`（:136-142）；设备 `required_limits: downlevel_webgl2_defaults().using_resolution(adapter_limits)`（:159-160）——兼容低端 GPU。
- **字体加载**（:175-213，Android 特有）：先尝试从 `copyFontsFromAssets` 拷到 `/data/data/com.njr.zelland/files/fonts/` 的 **NotoSansMNerdFontMono-Regular/Bold.ttf**（:179-189）；若 fontdb 面数为 0（SELinux 阻止系统字体发现），回退硬编码系统路径列表（:195-207）。无字体则记录 error（:209-213，glyphon 后续会 panic）。
- glyphon 初始化（:215-227）：`Cache::new(&device)` → `TextAtlas::new(..., Bgra8Unorm 占位)` → `TextRenderer::new` → `Viewport::new(&device, &cache)`（**新版 API 以 Cache 为参**，见 WGPU_GHOSTTY_REMEDIATION_DESIGN.md:29-30）→ `Buffer::new(font_system, Metrics::new(CELL_HEIGHT*0.75, CELL_HEIGHT))`。

### 2.4 Surface 生命周期与管线重建

- `set_surface`（:317-330 关键段）：创建 surface → 取 capabilities → 配置（Fifo、`desired_maximum_frame_latency: 2`）→ 若 `caps.formats[0] != atlas_format` 则 `rebuild_text_pipeline`（Fix 1）→ 应用 pending_size 并立即 `render()`（Fix 2，避免黑屏）。
- `rebuild_text_pipeline`（:335-350）：**atlas 格式必须等于 surface 格式**，否则 text_renderer.render 在 pass 内被静默丢弃（WGPU_FIXES.md Fix 1 的教训）；重建后清 `row_cache`，并重建 cursor/selection 管线。
- `build_cursor_resources`（:354-437）：cursor 管线（无顶点缓冲，`buffers: &[]`，:393）+ 32 字节 uniform buffer（`rect+color`，:417-422）+ bind group；blend `REPLACE`（:402）。
- `build_selection_resources`（:439-492）：selection 管线（`layout: None` 自动布局，:449；vertex stride 8 = Float32x2，:453-461）+ **预分配顶点缓冲 `50 * 6 * 8` 字节**（50 行 × 6 顶点 × 2×f32，:483-489）。
- `drop_surface`（:591-595）：锁屏时 SurfaceView 销毁调用，置 None；下次 `set_surface` 重挂。

### 2.5 3-pass 渲染 `render()`（:635-741）——本报告核心

```rust
// 1. 先写光标 uniform（pass 外，:656-658）
// Pass 1（:660-686）clear_cursor_pass：
//   LoadOp::Clear(纯黑) —— 同时承担清屏与光标矩形绘制
//   若 cursor_pixel_rect 存在：set_pipeline(cursor) + draw(0..6, 0..1)
// Pass 2（:688-708）text_pass：
//   LoadOp::Load —— 保留光标
//   text_renderer.render(&atlas, &viewport, &mut rpass)
// Pass 3（:710-731）selection_pass（仅当 vertex_count > 0）：
//   LoadOp::Load —— 保留文本
//   set_vertex_buffer(0, buf) + draw(0..vertex_count)
// 提交 + present + atlas.trim()（:733-735）
```

- 每帧无条件 `surface.get_current_texture()`（:641）——**无超时/重试**，Mali 挂起风险（torvox pass.rs 为此造了 acquire worker 线程，见 §12）。
- `save_debug_frame`（:743-840）：前 3 帧把渲染结果 copy 到读回缓冲，写成 **PPM（P6）到 `/sdcard/Android/data/com.njr.zelland/files/frame_N.ppm`**；`bytes_per_row` 按 256 对齐（wgpu 要求，:754-756）；假定 Bgra8 并交换 B/R（:827-830）。

### 2.6 文本更新 `draw_ghostty_state`（:842-962）

- 入口检查 `state.get_dirty()`（:843-846），干净则跳过（**整帧跳过，不重建**）。
- `text_buffer.set_size(Some(w), Some(h))`（:851-852）。
- `state.with_rows(|line_idx, is_dirty, cells| ...)`（:855）：**只有脏行或超出 row_cache 的行**才 `build_row_runs`（:856-857）；未变行用缓存 run 列表。这实现了 WGPU_GHOSTTY_PLAN.md 宣称的"damage-aware row updates"。
- 用 `span_buf` 拼 `(text, Weight, Style, Color)` 元组（:917-936）后 `set_rich_text(font_system, spans, default_attrs, Shaping::Advanced)` + `shape_until_scroll(font_system, false)`（:935）。
- `prepare`（:938-961）：单个 `TextArea { buffer, left/top 0, scale 1.0, bounds=全屏, default_color 白 }`。
- `build_row_runs`（:966-1028）：遍历 row cells，合并相邻同样式 cell 为 run；inverse 时 fg/bg 互换（:987-991）；空 grapheme → 空格（:978-985）。**每行全宽重建（无列级损伤）**。
- `ghostty_color_to_rgb`（:1033-1110）：`GhosttyStyleColorTag` 的 NONE/PALETTE/RGB 三分支（PALETTE 走 `ansi_palette_color`），并 `#[allow(non_upper_case_globals)]` 处理 bindgen 生成的常量。

### 2.7 选择渲染与文本提取

- `set_selection(sc, sr, ec, er, active)`（:494-508）：归一化（保证 start ≤ end，:501-505）→ `update_selection_vertices`。
- `update_selection_vertices`（:510-544）：像素→NDC 闭包（:517-518）；**逐行 push 一个矩形**：首行 x 从 `sc*cw` 起、末行到 `(ec+1)*cw` 止、中间行全宽（:528-534）；**顶点数截断上限 50 行**（:535-536，与缓冲预分配一致）；`queue.write_buffer` 直写（:539-543）。
- `extract_text(sc, sr, ec, er)`（:546-567）：从 row_cache 逐 run 逐字符截取（:559-563），行间插 `\n`——**选择的复制文本完全在 Rust 侧由缓存重建**，不依赖 ghostty。

### 2.8 字体与尺寸

- `update_font_size(css_px, dpr)`（:599-612）：`cell_height = css*dpr`、`cell_width = cell_height*0.45`（**0.45 宽高比**，与 Terminal.svelte:24 一致）、`font_size = physical*0.75`；清 row_cache 并立即 render。
- `resize(w, h)`（:614-633）：clamp 到 `max_texture_dimension_2d`（:615-617）；无 surface 时存入 `pending_size`（:619-622）；有则 `surface.configure` + `viewport.update(Resolution)` + 清 row_cache（:624-632）。
- 全局访问器 `get_cell_size()`/`update_font_size_global`（:120-127）：供 terminal.rs 的鼠标编码使用"活"的 cell 尺寸（terminal.rs:108）。

### 2.9 旧版 `renderer.rs`（死代码，:1-380）

单 pass 版本：无 cursor/selection、`Backends::VULKAN`（:43）、Fifo + 1×1 初始尺寸（:109-118）、`with_renderer` 风格回调。**结论：3-pass 是当前唯一生效路径**；旧文件保留是迁移残留（CLAUDE_NOTES.md 建议清理）。

---

## 3. JNI 平台层（renderer/android.rs）

文件：`src-tauri/src/renderer/android.rs`，216 行。全部函数 `#[cfg(target_os = "android")]` + `#[unsafe(no_mangle)]` + `extern "system"`，命名遵循 `Java_com_njr_zelland_MainActivity_<fn>`（Kotlin 侧 `system.loadLibrary` 后直接 JNI 调用）。

| 函数 | 行号 | 说明 |
|---|---|---|
| `passSurfaceToRust` | :18-59 | `ANativeWindow_fromSurface`（:26-32）→ 构造 `AndroidNdkWindowHandle` + `AndroidDisplayHandle`（:36-38）→ 包成 `RawWindow/RawDisplay` → `spawn_on_runtime` 异步：renderer 未初始化则 `Renderer::init().await`（:43-50）→ `set_surface`（:54）。注释明确"此处不 resize，等 passResizeToRust"（:55-56）。 |
| `passSurfaceDestroyedToRust` | :63-74 | 调 `drop_surface`（锁屏/surface 销毁）。 |
| `passResizeToRust` | :78-99 | **总是先 `store_pending_size`**（:90，防止渲染器未就绪丢失尺寸）→ `resize` + `render`（:95-96）。 |
| `getSelectionText` | :103-118 | 返回 `extract_text` 的 Java String（Kotlin ActionMode 复制用）。 |
| `setSelectionHighlight` | :122-138 | 转发到 `renderer.set_selection` 并立即 `render`（:134-135）。 |
| `passPasteToRust` | :142-161 | `convert_byte_array` → 写到 `focused_session`（:155-157）。 |
| `getCellDimensions` | :165-171 | 返回 `get_cell_size()` 的 float 数组。 |
| `updateFontSize` | :173-191 | `css_px, dpr` → `update_font_size_global`（:188-189）。 |
| `passTouchToRust` | :195-216 | action 字符串（"click"/"scroll_up"/…）+ x/y → `SshManager.process_touch`（:209）。 |

模式要点：**所有 JNI 入口都通过 `crate::spawn_on_runtime` 跳到 Tauri 的 tokio 运行时**（JNI/UI 线程没有运行时上下文）；锁用 `unwrap_or_else(|e| e.into_inner())`（毒锁恢复）。

---

## 4. libghostty-vt 集成（ghostty.rs + build.rs）

### 4.1 ghostty.rs（335 行）

- bindgen 产物以 `include!(concat!(env!("OUT_DIR"), "/ghostty_vt_bindings.rs"))` 内联（:9-13）。
- `GhosttyTerminalWrapper`（:15-119）：
  - `new(cols, rows)`（:20-35）：`ghostty_terminal_new(ptr::null(), &mut terminal, options)`，**`max_scrollback: 0`**（:25，零滚动缓冲设计——本地无 scrollback，滚动靠应用层模拟或远端）；失败返回 `i32` 错误码（非 Result<String>）。
  - `write(&[u8])`（:37-41）：`ghostty_terminal_vt_write`。
  - `resize(cols, rows, width_px, height_px)`（:43-59）。
  - `get_size/get_cursor_pos/get_mouse_tracking`（:61-107）：走 `ghostty_terminal_get` + 枚举常量。
  - `Drop`（:110-116）`ghostty_terminal_free`；`unsafe impl Send/Sync`（:118-119）。
- `GhosttyRenderStateWrapper`（:121-约 240）：
  - `new`（:128-156）：三连创建 `ghostty_render_state_new` + `row_iterator_new` + `row_cells_new`，失败逐级释放。
  - `update(terminal)`（:158-166）：`ghostty_render_state_update`。
  - `get_dirty/reset_dirty`（:168-189）：Dirty 位查询/清零。
  - `with_rows<F>`（:210-约 245）：取 row_iterator → 循环 `row_iterator_next` → `row_get(CELLS)` + `row_get(DIRTY)` → 回调 `(row_idx, is_dirty, &row_cells)` → 脏行 `row_set(DIRTY=false)` 重置（:238-240）。**这就是渲染器行级损伤的来源**。
- 辅助函数：`get_cell_style`、`get_cell_graphemes`（:326-335，先 `ghostty_render_state_row_cells_get_len` 再取 `GRAPHEMES_BUF` 的 `Vec<u32>`）。

### 4.2 build.rs（130 行）

- prost 编译 `../proto/zelland.proto`，自动加 `serde` derive（:7-9）。
- `build_libghostty`（:18-74）：
  - Rust target → Zig target 映射表（:25-39）：Android 四 ABI 全部 `api 30`；Linux x86_64/aarch64 直通；host 用 `"native"`。
  - `zig build -Demit-lib-vt=true -Dapp-runtime=none -Doptimize=ReleaseFast -Dsimd=false -Dtarget=<t>.<api> -p <out>`（:50-59）——**app-runtime=none 是关键**（只要 VT 库，不要 ghostty 的 app 层）。
  - 链接：`-l static=ghostty-vt`（:66-68）；Android 追加 `c++_shared`、`nativewindow`（:70-72）。
- bindgen 配置（:75-130）：对 `ghostty.h` 生成，`blocklist_type` 掉 SgrParser/KeyEvent/KeyEncoder/MouseEvent/MouseEncoder（:102-106，这些类型在 ghostty.h 中不完整），`allowlist_function("ghostty_.*")` + `allowlist_type("Ghostty.*")`（:107-108）；**手动追加 11 个 opaque 指针类型别名**（:113-126，如 `GhosttyTerminal = *mut c_void`）——这是 C 头文件不完整的 workaround。

### 4.3 libghostty 目录

- `libghostty/ghostty/` 为空（submodule 未拉取）；`libghostty/src/android_fix.zig`（247B）：**强制 TLS 段 64 字节对齐以满足 Android Bionic linker**（`threadlocal var _tls_align_fix: u64 align(64)` + 导出 `ghostty_android_tls_fix()`）。

---

## 5. 终端会话（terminal.rs）与 SSH 管理器（ssh.rs）

### 5.1 terminal.rs（252 行）

`TerminalSession { term: GhosttyTerminalWrapper, render_state: GhosttyRenderStateWrapper, dirty: bool }`（:4-8）。

- `new`（:11-21）、`process_bytes`（:23-26，置 dirty）、`resize`（:28-35，**用 renderer 的 `CELL_WIDTH/HEIGHT` 常量换算像素尺寸传给 ghostty**——CLAUDE_NOTES.md 曾批评传 0,0 导致鼠标坐标不准，此处已修复为编译期常量，且编码时改用活尺寸，见下）、`is_dirty`（:37-39）、`get_mouse_mode`（:41-43）、`get_cursor_pos`（:45-47）。
- `process_mouse(x, y, action) -> Vec<Vec<u8>>`（:51-93）：**mouse mode 未启用（`\x1b[?1000h` 未收到）时丢弃事件**（:53-57）；scroll_up/down 合成 Press+Release 两个序列（:60-75）；click 合成 Left Press+Release（:76-84）。
- `encode_mouse_event`（:96-约 180）：核心——`ghostty_mouse_encoder_new` → `setopt_from_terminal`（继承终端模式，CLAUDE_NOTES.md:83-85 提醒过复用编码器要重置）→ 用 `crate::renderer::get_cell_size()` 的**活 cell 尺寸**构造 `GhosttyMouseEncoderSize`（:108-119）→ 生成 SGR 序列。测试（:242-251）断言输出以 `\x1b[<` 开头（SGR 1006）。

### 5.2 ssh.rs（499 行）

- `SshChannelMsg`（:19-24）：只剩 `Closed` 变体（Viewport 变体已死——CLAUDE_NOTES.md:18-22）。
- `open_session`（:27-47）：`client::Config { keepalive_interval: 30s, keepalive_max: 3 }`（:34-35）——**保活专为 Android 熄屏后会话存活**。
- `AuthMethod`（:49-54）：Password / PrivateKey / Key(Keystore)。
- `SshConfig`（:56-68）：host/port/username/auth_method/密码/密钥路径/密钥口令/key_id/session_name/project_root。
- `SessionMsg`（:70-74）：`Data(Vec<u8>) / Resize{rows,cols} / ProcessMouse{x,y,action}`。
- `Client` + `check_server_key`（:76-87）：**恒返回 `Ok(true)`，不验证主机密钥**（安全缺口，文档化）。
- `load_private_key`（:90-111）：PrivateKey 走文件+passphrase 解码；Key 走 `key_manager.get_russh_key`。
- `authenticate`（:114-133）：password 或 `authenticate_publickey(PrivateKeyWithHashAlg)`。
- `SshManager`（:135-140）：`active_sessions: HashMap<String, mpsc::Sender<SessionMsg>>` + `focused_session: Option<String>`。
- `run_command`（:150-181）：一次性执行命令并收集输出（helper 检测用）。
- `upload_file`（:183-233）：`cat > <shell_quote(path)>` + 32KB 分块 + eof + 检查 ExitStatus（:204-230）。
- **`connect`（:235-354）——核心会话循环**：
  1. 建 channel → `request_pty(true, "xterm-256color", cols, rows, 0, 0, &[])`（:259）；
  2. exec `build_zellij_connect_command`（默认 `zellij attach --create <session> || $SHELL`，:265-272，构造见 :494-497 测试）；
  3. `tokio::spawn` 会话任务：`mpsc::channel(100)` + `TerminalSession::new`（:274-288）；
  4. **`tokio::select!` 三分支**（:294-351）：`rx.recv()`（Data→channel.data；Resize→`channel.window_change` + `ts.resize`；ProcessMouse→`ts.process_mouse` 后逐序列发送）；`channel.wait()`（Data→`ts.process_bytes`；ExitStatus/Eof→break）；**`flush_interval.tick()`（16ms，60FPS，MissedTickBehavior::Skip）**：dirty 则 `ts.render_native()`（:343-349）——渲染与网络同循环，无单独渲染线程；
  5. 结束时 `output.send(SshChannelMsg::Closed)`（:353）；
  6. 注册 sender + 设置 focused（:356-360）。
- `process_touch`（约 :380-410）：focused session 的 tx 发 `ProcessMouse`；无 focused 返回 Err。
- `resize/scroll/write_input/disconnect`：`scroll` 走 `scroll_viewport`（零滚动缓冲设计，注释说明，CLAUDE_NOTES.md:39-41）。

---

## 6. Tauri 命令层（lib.rs）与配套模块

### 6.1 lib.rs（约 256 行）

- 模块声明（:1-12）；`APP_HANDLE` 全局（:20-23）+ **`spawn_on_runtime`**（:27-33，JNI 线程进 tokio 的桥）。
- Tauri 命令：`ssh_connect`（:46-64，带 `Channel<SshChannelMsg>` 事件通道）、`ssh_disconnect`（:66-70）、`ssh_write`（:72-75）、`ssh_resize`（:77-80）、`ssh_scroll`（:82-85）、`daemon_connect`（:87-90，WS 连 zlnd）、`daemon_run_zellij_action`（:92-95）、`ssh_list_zellij_sessions`（:97-109，远端跑 `zellij list-sessions -n -q`）、`run_remote_command`（:111-119）、以及 set_terminal_font_size/updateTerminalFontWeight/密钥管理/侧边栏数据等命令（:120-255 区间）。
- `ManagedKeyManager(pub Arc<dyn KeyManager>)`（:44）。
- Linux 专用：webkit2gtk 导入（:34-35）；dinghy 测试 `test_android_connectivity`（:258-266）。

### 6.2 keystore.rs（约 618 行）

- `KeyIdentity`（:10-16）；`KeyManager` trait（:18-36）：`generate_key/list_identities/delete_identity/sign(生物识别)/get_russh_key`。
- `StandardKeyManager`（:38-91）：文件密钥库；`master_passphrase`（:55-71）——UUID 主口令存 `master.key`（0o600）；`load_decrypted_key`（:74-90）ssh_key crate 解析 + 解密。
- `generate_key`（:95-115）：Ed25519（`ssh_key::PrivateKey::random`），**用主口令加密落盘**（:105-109）。
- Android 实现（约 :200-500）：Keystore 生成 + **生物识别签名流**——Rust 发起 `sign` 后通过 Tauri 事件把请求交给 Kotlin（BiometricPrompt），Kotlin 结果经 JNI 回传（`BiometricResponse` serde，:611-617 测试）。

### 6.3 helper.rs（约 390 行）

`zlnd` 远程助手自安装器：`ensure_remote_helper_inner`（:54-119）——mkdir（:61-66）→ `detect_remote_platform`（uname 探测 4 种 triple，:17-38）→ 版本对比（远端 `zlnd --version` vs 本地 `CARGO_PKG_VERSION`）→ 不一致则 `upload_file` 到 `~/.local/state/zelland/zlnd.upload` + `chmod 755 && mv ~/bin/zlnd`（:80-97）→ `start_command`（nohup + `~/.local/state/zelland/zlnd.port` 端口文件，:111-116 + :384-389 测试）→ `wait_for_helper_with_port` 轮询健康（:118）。

### 6.4 daemon.rs（约 337 行）

- `proto` 内联（:1-3）；`RecentSessionEntry/Project`（:16-30）；`DaemonManager { ws_write: Arc<Mutex<Option<WsSink>>>, http_client }`（:37-40）。
- `connect(url, app_handle)`（:50-118）：`connect_async` → split → spawn 读循环：二进制消息 `Envelope::decode`（protobuf，:70-72）→ `Notification` payload：非 Android 走系统通知（:81-87），所有平台 `app.emit("agent-notification", ...)`（:108）→ 其他 payload `emit("daemon-event")`（:112）。
- 其余命令：`daemon_list_project_files`、`daemon_record_session`、`daemon_mutate_file`（HTTP GET/PUT 到 `http://<host>:<port>`，约 :200-337）。

### 6.5 network.rs（150 行）

**用户态 WireGuard**：`gotatun`（tokio 版 WireGuard 实现）。`TunnelDevice` 类型别名（:12-16）；`WireguardConfig`（:18-25）；`start_tunnel_inner`（:55-114）：base64 解码 32 字节密钥（:57-73）→ endpoint 解析（:76-77）→ allowed IP 解析（:80-85）→ `Peer::new(...).with_endpoint(...).with_allowed_ips(...).keepalive=25`（:88-91）→ `build().with_default_udp().create_tun("zn0")`（:95-99）→ `set_private_key`（:105-107）；`stop_tunnel`（:117-124）。

### 6.6 小模块

- `android_notif.rs`（47 行）：`show(title, body, session_name)`（:7-47）——`ndk_context::android_context()` 取 VM/Context → `JavaVM::from_raw` + `attach_current_thread` → 反射调 Kotlin `AgentNotifHelper.show(activity, title, body, session)`（:33-40）→ 异常检查/清除（:41-44）。
- `intent.rs`（21 行）：通知动作 `handle_notification_action` → `emit("navigate-to-session")`（:7-12）；空插件 `init`（:14-21）。
- `keybar.rs`（10 行）：**空插件**——注释（KeybarPlugin.kt:19-24）说明原生键盘栏不走 Tauri 插件机制，而是 MainActivity 直接实例化。
- `main.rs`（19 行）：Linux 下 `WEBKIT_DISABLE_DMABUF_RENDERER=1` + 条件 `GDK_BACKEND=x11`（:9-16，NVIDIA Wayland 崩溃 workaround）。

---

## 7. Android Kotlin 平台层（gen/android）

> **重要发现**：仓库只提交了 3 个 Kotlin 文件（`MainActivity.kt` 46KB、`KeybarPlugin.kt` 11KB、`KeySeqs.kt` 1.6KB + 测试 `KeySeqsTest.kt`）。README 声称的 `KeybarSeqs.kt`、`TerminalSessionService.kt`、`AgentNotifHelper.kt` 以及 `res/layout/native_keybar.xml`、`res/values/colors.xml` **均不在克隆中**（Tauri 的 `gen/` 目录被 gitignore，仅手动提交了部分）。`MainActivity.kt:82` 引用的 `TerminalSessionService::class.java`、`KeybarPlugin.kt:59` 引用的 `R.layout.native_keybar` 在克隆内无法编译——**说明 gen/android 并非可独立构建的完整工程**，这是与 torvox（自有完整 Gradle 工程）的重大差异。

### 7.1 MainActivity.kt（1047 行）

- 状态字段（:49-75）：selection（:58-62）、pinch 缩放（:64-67，`baseFontSize = 38f` 匹配 CELL_HEIGHT 默认）、原生侧边栏（:69-75，`expandedHostIds`、`lastSidebarJson` 缓存）。
- `onCreate`（:77-103）：KeyStoreManager/BiometricManager（:79-80）→ `copyFontsFromAssets()`（:81，把 Nerd Font 拷到 files/fonts）→ **启动前台服务 TerminalSessionService**（:82，SSH 会话在锁屏后存活）→ Android 13+ 通知权限（:85-89）→ back 键：关抽屉否则发 `kb-sidebar-toggle` 给 JS（:91-103）。
- 手势（:105-170）：
  - `onSingleTapConfirmed`（:106-114）：`passTouchToRust("click", x, y)` + 聚焦隐藏 EditText 弹系统键盘——**用隐藏 EditText 把 IME 输入变成 TextWatcher 流**；
  - `onLongPress`（:117-125）：`pixelToCell` 算格 → 默认选中"从光标到行尾 +12 列"→ `setSelectionHighlight` + `startSelectionActionMode()`（原生复制栏）；
  - `onFling`（:127-139）：左滑（vx < -600 且水平主导）→ `drawerLayout.openDrawer`；
  - `onScroll`（:141-154）：**仅双指** → `passTouchToRust("scroll_down"/"scroll_up")`；
  - `ScaleGestureDetector`（:157-170）：pinch → `baseFontSize` clamp [20,80] → `updateFontSizeToRust`。
- `onNewIntent`（:183-193）：通知动作带 `navigate_session` extra → JS `window.__navigateToSession`。
- `onWebViewCreate`（:195-238）：
  - `TerminalNative` JS 桥（:208-215）：`setVisible(bool)` 控制 **SurfaceView VISIBLE/GONE**（WGPU_FIXES.md Fix 3：初始 GONE + 前端带 activeSession 条件）；
  - `SidebarNative` 桥（:217-230）：`updateData(json)/openDrawer/closeDrawer`；
  - WebView 透明（:232）、`decorView.post` 后建 KeybarPlugin + `setupNativeSurface`（:234-237）。
- `setupNativeSurface`（:240-约 300）：SurfaceView 创建、`setZOrderMediaOverlay(true)`、holder 回调（surfaceCreated→`passSurfaceToRust`、surfaceChanged→`passResizeToRust`、surfaceDestroyed→`passSurfaceDestroyedToRust`）。
- 按键（约 :390-410）：物理键盘 `dispatchKeyEvent` → DPAD → `KeybarSeqs.modifiedArrow`。
- 原生侧边栏（:592-824）：`updateNativeSidebarData` 解析 JS JSON → `addSectionLabel`（:613-624）/`addStatusDot`（:641-649）/`addFavoriteProjectRow`（:651-695）/`addFavoriteSessionRow`（:697-727）/`addProjectHostRow`（:729-797，展开/删除）/`addProjectChildRow`（:799-824，pin 切换）——全部代码构建 View，无 XML 布局；事件经 `dispatchJsEvent`（:626-632）回 JS。
- 生物识别/加密（约 :1030-1046）：`encryptWithBiometricKey(alias, data)`——Keystore AES + Base64(IV+密文)（:1038-1046）。

### 7.2 KeybarPlugin.kt（273 行）

- 设计（:18-25 注释）：**不依赖 Tauri 插件发现**，由 MainActivity 直接 `KeybarPlugin(activity, webView).setup()`；事件用 `webView.evaluateJavascript + CustomEvent` 发 JS（无 IPC）。
- 修饰键状态机（:30-42）：`modCtrl/modAlt/modMeta` + `*Locked` + 双击锁（`doubleTapMs = 350`）。
- `setup`（:51-111）——**核心布局技巧**：把 WRY 根视图从 contentFrame 移除，与键盘栏一起塞进垂直 LinearLayout（:63-97）——注释解释（:64-67）：WebView 硬件表面绕过 z-order 合成，overlay 会被 Surface 盖住，LinearLayout 分区最干净；`OnApplyWindowInsetsListener` 用 IME/systemBars 最大者做 padding（:78-84）；找不到 wryRoot 时 fallback 底部 overlay（:99-106）。
- `KeybarBridge`（:114-120）：`window.KeybarNative.setVisible`。
- 按钮（:141-184）：Ctrl/Alt/Meta（`handleModifierTap` :187-221，单点切换/双击锁/再点解锁）、Esc/Enter/Tab、方向键（`sendArrow` :177-179 → `KeybarSeqs.modifiedArrow`）、Tab 切换（:181-184，发 `kb-go-to-tab` 走 Zellij action）。
- `sendSeq`（:171-175）：发 `kb-input` CustomEvent（`{"seq":...}`），发后 `resetModifiers`（:224-229，未锁的修饰键复位）+ haptic。
- `toJsString`（:262-272）：JS 字符串转义。

### 7.3 KeySeqs.kt（38 行）

纯函数映射：Ctrl+字母→0x01-0x1A、Ctrl+`[`→ESC、Ctrl+`\`→FS、Ctrl+`]`→GS、Ctrl+`^`→RS、Ctrl+`_`→US、Ctrl+`@`→NUL、**Alt+任意→`ESC+ch`**、`\n`→`\r`（:15-37）。`KeySeqsTest.kt` 覆盖这些映射。

---

## 8. Svelte 5 前端

### 8.1 Terminal.svelte（~130 行）——终端粘合层

- `onKbInput`（:13-18）：监听 `kb-input` CustomEvent（KeybarPlugin 发出）→ `TextEncoder` → `invoke('ssh_write')`。
- `getCellDimensions`（:21-26）：`cellH = terminalFontSize * dpr`、`cellW = cellH * 0.45`（与 Rust 端 0.45 一致，**前端先算、Rust 后校**）。
- `doFitAndResize`（:31-40）：ResizeObserver → `appState.resize(tabId, rows, cols)`（前端算 cols/rows，SSH 侧 `window_change`）。
- `connectSsh`（:61-…）：拿 `SshConfig` → `invoke('ssh_connect', { channel })`——**Tauri Channel 接收 `SshChannelMsg`**（`Viewport` 类型仍在 TS 类型里 :57-59，但 Rust 端已不发送）。
- 关键点（:93-108）：`set_terminal_font_size` 先于 resize（保证 cell 布局一致）；容器透明（:117-125）露出下层 SurfaceView。

### 8.2 app.svelte.ts（845 行）——中央状态

- Svelte 5 runes：`$state` 主机/会话/活动 tab/字体/日志/密钥/pinned 项目等（:68-89）；`$derived` 活动会话/项目（:91-100）。
- `PRIORITY_ORDER = ['README.md','DESIGN.md','PLAN.md']`（:109）——md 排序。
- `helperUrl`（:111-114）：`http://<host>:<helperPort ?? 8083>`。
- `onSessionConnected`（:502-541）——连接后编排：状态置 connected → `ensureSessionHelper`（远程装 zlnd）→ `daemon_record_session` → `daemon_connect(ws://...)` → 拉项目 md 文件列表（:532-540）。
- `activateProject/openProjectByName`（:462-486, :560-…）：按项目自动建 session（id = `project-<hostId>-<projectId>`）。
- 前端也带 `buildSshConfig`（:488-500）。

### 8.3 组件与工具

- `+page.svelte`（约 500 行）：app 壳。Android 模态框（addHost/addSession/settings/errors，:22-45）；`onMount` 注册原生事件（:85-121）：`kb-sidebar-toggle`→`SidebarNative.openDrawer()`、`kb-go-to-tab`→Zellij action、`native-reload-terminal/native-add-host/.../native-connect-session`、`native-drawer-closed`→恢复 Keybar 可见性（:113-116）。
- `Sidebar.svelte`（25KB）：Web 端（Linux）侧边栏；Add Host/Session 表单、设置、密钥生成（:5-27 状态，:28-46 表单逻辑）。
- `MarkdownPane.svelte`（33KB）：marked + highlight.js + 懒加载 mermaid（:16-25）；注解扩展（:33-75）；TOC、选区注解表单（:77-100）。
- `annotations.svelte.ts`（3.2KB）：`createAnnotationManager`——**WebSocket 直连 daemon** `ws://<host>:8083/annotations/sync/<filepath>`（:41），JSON 消息（非 protobuf，注解通道是 JSON），指数退避重连（:19-20, :44-47）。
- `marked-annotations.ts`（3.6KB）：markdown 注解锚点扩展；`key-mapper.ts`（1KB）：`getControlSequence`（Ctrl+字母）+ `SPECIAL_KEYS` + `modifiedArrow`（`\x1b[1;{mod+1}{letter}`，:33-38）；`kb-input.ts`（393B）；`time-ago.ts`、`md-ordering.ts`、`markdown-path.ts`。
- 测试：`kb-input.test.ts`、`key-mapper.test.ts`、`md-ordering.test.ts`、`markdown-path.test.ts`、`time-ago.test.ts`（TESTING.md:19-25）。

### 8.4 构建配置

- `package.json`：svelte 5 + vite 6 + vitest 4 + @tauri-apps/*（haptics/notification/os/store/opener）+ marked/mermaid/highlight.js/lucide/pico CSS（:16-31）。
- `vite.config.js`：Tauri 固定端口 1420/1421、忽略 src-tauri 监听（:18-33）。
- `tauri.conf.json`：窗口 800×1200 无边框；Android 仅 aarch64；`csp: null`。

---

## 9. daemon-rs（zlnd 守护进程）

定位：**部署在远程主机上的本地助手**（经 SSH helper 自安装，§6.3），提供项目/文件/注解 API。axum 0.8 + tokio + Loro（CRDT）+ kdl 配置（AGENTS.md:29-36）。

- `main.rs`（36 行）：zlnd 入口——KDL 配置加载 + `server::run`。
- `cli.rs`（105 行）：zn CLI——`show <file>` / `md <file>` / `notify --title --body --question --source`（带 `ZELLIJ_SESSION_NAME/TAB_INDEX/PANE_ID` 环境变量注入，:54-63），POST 到 `127.0.0.1:port/api/v1/trigger/*`。
- `lib.rs`（165B）+ `proto.rs`（75B）：库壳。
- `config.rs`（194 行）：`Config { port(默认 0=OS 分配), cert_file, key_file, projects_path(~/code) }`，KDL 解析（:35-56）、CLI 合并（:59-66）、13 个测试。
- `server.rs`（约 350 行）：
  - `AppState`（:20-27）：config/asset_manager/registry/watcher_tx/loro_manager。
  - `build_router`（:30-71）路由表：`/api/v1/projects`（GET）、`/projects/activate`（POST）、`/projects/{id}/files`、`/meta/version`、`/fs/read`、`/fs/annotate`、`/sessions/recent`（GET/POST）、`/api/v1/trigger/{show,md,notify}`（**loopback-only**，:32-36）、`/assets/{id}`、`/ws`、`/annotations/sync/{*filepath}`（GET）、`/annotations/{*filepath}`（GET/PUT）。
  - `loopback_guard`（:83-97）：非回环 IP 一律 403（安全约定）。
  - `write_port_file`（:99-110）：`~/.local/state/zelland/zlnd.port`——helper 健康探测的锚点。
- `ws.rs`（约 200 行）：`ClientRegistry`（:16-34，broadcast 订阅）+ `handle_ws`：二进制 protobuf `Envelope` 收发（:171-192 有 roundtrip 测试）。
- `loro_manager.rs`（约 340 行）：Loro CRDT 文档管理——`AnnState`（:13）/`LoroManager`（:23）/`DocState`（:27）；`doc_to_json`（:34）、`load_or_create_doc`（:92）、`persist_data`（:317，snapshot+注解 JSON 双写）、`extract_anns`（:329）。
- `store/mod.rs`（约 440 行）：注解存储——`Annotation`（:24，`regular` :35 / `code` :39 代码块注解）；**markdown 注释解析器** `parse_markdown_comments`（:66）把 `[comments](#handle)` 锚点段解析/回写（`flush_annotation` :158、`find_fenced_block_end` :303、`detect_fence` :365、`strip_comments_section` :379）；`loro_cache_path`（:400）。`store_tests.rs` 8.8KB。
- `watcher.rs`（约 210 行）：notify 文件监听 → `WatchCommand`（:11）→ `detect_file_type`（:17，md/pdf/…）→ 广播 OpenViewRequest。
- `handlers/`：`annotations.rs`（3KB）、`assets.rs`（1.1KB，serve_asset）、`fs.rs`（10.8KB，读/注解写文件）、`meta.rs`（393B，version）、`projects.rs`（7.6KB）、`sessions.rs`（6.1KB，recent 记录）、`trigger.rs`（3.6KB，show/md/notify 分发）、`utils.rs`（349B）。
- `assets.rs`（4.2KB）：AssetManager + 清理任务。
- 测试：`tests/watcher_test.rs`（1.9KB）、`store/store_tests.rs`（8.8KB）。

---

## 10. 附属组件：zellij-plugin / src-voice / proto / libghostty

### 10.1 zellij-plugin/src/lib.rs（2.3KB）

**Zellij 后台插件**（wasm32-wasip1）：`ZellandTabs`（:24-64）——无 UI 面板，订阅 `TabUpdate` 事件（:30-47），响应 `list-tabs` pipe 消息（`zellij -s <session> pipe --plugin ... --name list-tabs`，:16-22）返回 `{"v":2,"tabs":[...]}`（:52-57），`unblock_cli_pipe_input`（:57）。协议版本 `VERSION=2` 必须与客户端 `PLUGIN_VERSION` 一致（:5）。

### 10.2 src-voice（语音终端实验，独立 Tauri 应用）

- `src-tauri/src/pty.rs`（172 行）：**portable_pty 本地 PTY**——`pty_spawn`（:26-102）：`openpty(24×80)` → `$SHELL` → reader 线程 `emit("pty-output")`（:62-85）+ writer 任务（:88-95）；`pty_write`（:105-120）。
- `src-tauri/src/speech/engine.rs`（11KB）：`SpeechEngine`（:23）——whisper-rs STT，`warm_up`（:83）/`transcribe`（:92），测试（:284-295）。
- `speech/audio.rs`（5.8KB）：录音采集；`speech/mod.rs`（6.7KB）。
- `src/routes/+page.svelte`（10.7KB）+ `VoiceTerminal.svelte`（3.2KB）；`tests/e2e_speech.rs`（6.4KB）带 WAV fixture。
- 与主 app 关系：**平行实验项目**（语音输入终端），不参与主架构。

### 10.3 proto/zelland.proto

`Envelope`（:10-22）oneof：ping/open_view/annotation/status/notification/list_sessions/create_session/zellij_action；`ZellijAction`（:24-27）；`NavigationTarget`（:39-43）；`NotificationSource` 枚举（:45-51，USER/CLAUDE_CODE/GEMINI_CLI/ZELLIJ_PLUGIN）；`AnnotationAction`（:78-94，带 context_hash 锚定）。**单一 proto 源**，daemon-rs 与 src-tauri 都用 prost 编译（AGENTS.md:31,49-52）。

### 10.4 libghostty

- `src/android_fix.zig`：TLS 64 字节对齐补丁（§4.3）。
- `ghostty/` 空目录（submodule 未拉取）；`libghostty-source/` 占位。

---

## 11. 项目文档资产

| 文档 | 大小 | 价值 |
|---|---|---|
| `WGPU_FIXES.md` | 15KB | **渲染器事故记录（最高价值）**：Fix 1 atlas 格式必须匹配 surface；Fix 2 set_surface 后必须 render；Fix 3 SurfaceView 启动时盖住 WebView（GONE + JS 条件）；字体 SELinux 回退；Vulkan 后端选择；acquire 挂起；DrawerLayout 与 SurfaceView 触摸冲突（WGPU_FIXES.md:329 规则：永远程序化开抽屉） |
| `WGPU_GHOSTTY_PLAN.md` | 2.9KB | 迁移四阶段方法论（Dinghy 测试基建 → 混合模式 → 原生 surface → 全集成 + 零滚动缓冲优化） |
| `docs/features/WGPU_GHOSTTY_REMEDIATION_DESIGN.md` | 1.8KB | 补救清单：host 构建回归、glyphon API 对齐、移除 ANSI viewport 载荷、xterm 依赖清理、SGR mouse 测试 |
| `CLAUDE_NOTES.md` | 9.7KB | 代码审查清单（死代码/panic/单例架构/Vulkan 硬编码等），很多已在 PLAN.md:20-73 标记修复 |
| `PLAN.md` | 14KB | 里程碑追踪（Phase 9 WGPU+Ghostty、Phase 6 daemon、WireGuard 等） |
| `TESTING.md` | 5.9KB | 测试矩阵（TS/Rust/Kotlin 三层）与待补测试建议 |
| `docs/features/NATIVE_UX.md` | 35KB | 原生键盘栏设计深挖（WebView 虚拟键盘的 6 大问题 vs 原生顶栏方案） |
| `docs/features/DAEMON_DESIGN.md` / `ANNOTATION_DESIGN.md` | 5KB+6KB | daemon 迁移 Go→Rust、注解系统设计 |
| `docs/features/FIDO_SSH_DESIGN.md` / `PER_USER_HELPER_DESIGN.md` / `NOTIFY_DESIGN.md` / `SPEECH_DESIGN.md` / `ZAPLET_DESIGN.md` / `ALACRITTY_DESIGN.md` | — | 设计文档（部分为前瞻设计，未全实现） |
| `APP_STORES.md` | 9.3KB | 上架策略（Play Store/F-Droid 等） |
| `AGENTS.md` | 6.2KB | 开发约定（task 命令、Rust/Svelte 风格、proto 流程、context-mode 路由规则） |

---

## 12. 功能对比：zelland vs torvox（逐项）

> torvox 定位：Android 终端（Kotlin Compose + Rust native + wgpu 30 + libghostty-vt（Rust crate）+ MCP）。行号引用 torvox 工作区文件。

### 12.1 渲染管线：3-pass vs 单 pass

| 维度 | zelland | torvox |
|---|---|---|
| Pass 结构 | **3 个 render pass**（renderer/mod.rs:660-731）：①clear+cursor 矩形（硬编码 6 顶点，无顶点缓冲）②glyphon 文本 ③selection 半透明蓝 overlay（顶点缓冲，最多 50 行） | **1 个主 cell pass**（render/pass.rs:389-412 单次 `draw_instanced`）+ bg pass（:327，壁纸/模糊）+ kgp pass（:358，kitty graphics）。光标/选择/搜索高亮**全部并入 cell 实例**（render/cell_builder.rs:224-243），无独立 pass |
| 每帧成本 | 每帧 `get_current_texture` 无超时；脏帧时全量 `set_rich_text` + `shape_until_scroll` + `prepare`（glyphon 重排版全部文本） | acquire 在独立 worker 线程带 2s 超时（pass.rs:12-47，防 Mali-G57 挂起）；idle 时**跳过绘制**，仅 cursor blink/selection/style 变化触发重绘（android/ffi.rs:1565-1572） |
| 损伤追踪 | 行级：ghostty render state dirty 行 → 只重建脏行 run，但**每行仍全宽**（renderer/mod.rs:855-857） | 帧级缓存：`last_frame` 缓存 cells+cursor（ffi.rs:1601），内容不变不重画 |
| 属性支持 | 仅 fg/bold/italic（CellRun :53-60），**无下划线/删除线/背景色/256 色之外的 dim** | 完整：reverse、underline、strikethrough、dim、blink、24-bit、CJK 宽字符、Nerd Font/emoji（cell_builder.rs:205-227） |
| 光标 | 独立 pass 白色实心矩形（`BlendState::REPLACE`，:402），无样式/闪烁 | cell 内 cursor quad：Block/Bar/Underline 样式 + 颜色 + 0.7 alpha + app 层闪烁（ffi.rs:193-230,1533-1555）+ 用户样式覆盖（:2656-2676） |
| 背景 | 每帧 LoadOp::Clear 纯黑（:668） | 独立 bg pipeline：纯色/壁纸图像/模糊（context.rs:531-591） |
| 帧调试 | 前 3 帧存 PPM 到 /sdcard（:743-840） | renderdoc 支持（renderdoc_capture.rs）+ 截图测试（screenshot_tests.rs 35KB） |

**结论**：torvox 的实例化单 pass 在 GPU 效率和表达能力上都优于 zelland 的 3-pass；zelland 的 3-pass 是"glyphon 全家桶 + 简单矩形叠加"的务实路径，光标/选择独立 pass 适合原型，但选择色硬编码、无样式光标是明显短板。

### 12.2 字体：glyphon vs 自研 swash 栈

| 维度 | zelland（glyphon 0.7） | torvox（自研） |
|---|---|---|
| 组件 | `FontSystem/SwashCache/TextAtlas/TextRenderer/Viewport/Buffer` 全托管 | fontdb + cosmic-text（shaping）+ swash（rasterize）+ guillotiere（atlas 分配）+ 自研 GlyphPipeline（font/mod.rs），LRU 驱逐（:619 测试） |
| 关键陷阱 | **TextAtlas 格式必须等于 surface 格式**（WGPU_FIXES.md Fix 1；renderer/mod.rs:335-350 重建）；整 Buffer 全量重排版 | 直接 shader 采样字形 atlas，**无格式约束**（context.rs:559-562 注释明确引用 zelland 教训） |
| 字体加载 | Nerd Font 打包 + SELinux 下系统路径回退（:175-213） | 系统字体发现 + CJK 回退加载（font/mod.rs:1035-1072）+ FreeType 黄金对比测试（:316-432） |
| CJK/emoji | 依赖字体库，无专门处理 | CJK 双宽、emoji、变体选择器专项处理（cjk.rs 12KB） |

**结论**：glyphon 省事但坑多（每帧重排版 + 格式耦合 + 无缓存控制）；torvox 自研栈已超越。zelland 的字体加载回退策略值得 torvox 借鉴（Android SELinux 下 fontdb 可能空库）。

### 12.3 连接模型：SSH vs 本地 PTY

| 维度 | zelland | torvox |
|---|---|---|
| 传输 | **russh 0.57 SSH**（远端 shell/Zellij，ssh.rs:235-354）；keepalive 30s×3（:34-35） | **本地 PTY**（nix `posix_openpt`，pty.rs:81-109；spawn :109；raw mode :523-614；env 构建 :676） |
| 会话循环 | tokio::select 三分支 + 16ms 定时渲染（ssh.rs:294-351） | 独立 ghostty VT 线程 + 命令通道（terminal/ghostty_terminal/internal.rs:62-264）+ PTY 读循环（session.rs:492-563） |
| 主机密钥 | 不验证（ssh.rs:81-86） | 不适用 |
| 滚动 | 零 scrollback 设计，`scroll_viewport` 委托 VT | 滚动缓冲 + `scroll_viewport`（internal.rs:439） |
| 剪贴板 | Kotlin ActionMode + `getSelectionText` JNI（android.rs:103-118） | **OSC 52 双向**（session.rs:573-610，answer_clipboard_read 回写 PTY）+ JNI 异步 answer（ffi.rs:347-374,1834-1891） |
| 附加 | WireGuard 隧道（network.rs:55-114）、SSH helper 自升级（helper.rs）、Zellij 会话编排 | MCP 服务器（mcp.rs 42KB）、shell env 传递、OSC 133 标记解析（session.rs:920-960） |

**结论**：两者产品形态不同（远程 SSH 终端 vs 本地终端），不可直接移植；但 zelland 的 SSH 会话循环结构（mpsc + select + 定时渲染）与 torvox 的线程模型互为参考。

### 12.4 选择（selection）

| 维度 | zelland | torvox |
|---|---|---|
| 渲染 | **独立 overlay pass**，半透明蓝（0.3,0.5,1.0,0.35）盖在文本上（SELECTION_SHADER :41-50） | **单元格背景替换**（inverse video 风格，cell_builder.rs:215-224）+ 可选 selection_bg 主题色（ffi.rs:2384-2389） |
| 模式 | 仅"长按→行尾+12 列"矩形（MainActivity.kt:117-125） | Char/Word/Line/Block/Semantic 五种模式（ffi.rs:2377-2382） |
| 复制 | ActionMode 原生复制栏 → JNI `getSelectionText`（Rust 缓存提取） | OSC52 + 系统剪贴板双通道 |
| 附加 | 无 | **搜索高亮叠加**（cell_builder.rs:227，盖在 selection 之上） |

**结论**：torvox 的 cell 级 selection 更优（模式多、颜色可配、与搜索高亮可叠加）；zelland 的 overlay 方案简单直观（一个 pass + 顶点缓冲），可作为 torvox 若需"选区上叠加其他覆盖层"时的参考实现。

### 12.5 光标渲染

zelland：独立 pass + uniform 矩形（白色实心，REPLACE blend，无闪烁逻辑——闪烁完全缺失）。torvox：cell 实例内 quad，支持样式/颜色/alpha/app 层闪烁（启用、速度、相位重置、样式覆盖四个 JNI 入口：ffi.rs:2594-2676）。**torvox 完胜**；zelland 的 `cursor_pixel_rect` + uniform 更新模式（renderer/mod.rs:570-587）可作为"低成本光标"备选。

### 12.6 touch → SGR mouse

| 维度 | zelland | torvox |
|---|---|---|
| 能力 | **完整**：click/scroll/right_click → SGR 1006 序列（terminal.rs:51-180），依赖 `ghostty_mouse_encoder` C API；mouse mode 未启用时丢弃（:53-57） | **无**：无触摸→SGR 编码路径；`is_mouse_tracking_active()` 仅查询（public_api.rs:560）；触摸只用于光标/选择 |
| 手势来源 | Kotlin GestureDetector 语义化（click/scroll_up/scroll_down/right_click） | 不适用 |

**结论**：若 torvox 未来支持 SSH 或模拟鼠标（如 htop/vim 鼠标模式），zelland 的 process_mouse/encode_mouse_event 是可直接移植的最小实现（~100 行，无 C 依赖也可用纯 Rust 重写 SGR 编码）。

### 12.7 其他功能矩阵（有无对照）

| 功能 | zelland | torvox |
|---|---|---|
| MCP 服务器 | 无（daemon 有 WebSocket protobuf 事件，非 MCP） | **有**（native/src/mcp.rs 42KB，tower-mcp） |
| Kitty graphics | 无 | **有**（kgp pass + kitty_graphics.wgsl） |
| 主题/壁纸 | 无（固定黑底） | **有**（set_bg_color/背景图/模糊，context.rs:512-591） |
| 通知 | 系统通知 + agent-notification 事件（daemon.rs:75-108 + android_notif.rs） | 无（终端通知 → OSC 9/BEL？session.rs:612 poll_notification 有雏形） |
| 语音 | 独立 src-voice（whisper） | 无 |
| WireGuard | 有（gotatun 用户态） | 无 |
| 密钥管理 | Keystore + 生物识别签名 | 无（本地终端不需要） |
| 远程文件/注解 | daemon-rs 全套 | 无 |
| 测试基建 | Dinghy Android 设备测试、~110 个 Rust/TS/Kotlin 测试 | proptest/quickcheck/conformance 98KB + 截图测试 + roborazzi |
| 渲染线程模型 | 全局单例 + tokio 任务内 Mutex（CLAUDE_NOTES.md:89-103 批评） | 专用渲染线程 + 独立 acquire worker |

---

## 13. 依赖分析（是否适用于 torvox）

### 13.1 russh 0.57 —— 不适用（除非加 SSH 功能）

- 成熟度：稳定、异步、纯 Rust；keepalive/密钥认证/上传 API 齐全。zelland 用法正确（open_session + authenticate + channel）。
- torvox 是本地终端，无 SSH 需求；若未来做"远端会话"或 MCP 侧 SSH 执行，russh 是首选（vs thrussh 不活跃、async-ssh2 同步阻塞）。**吸收点**：keepalive 30s×3 配置 + mpsc 会话循环模式。
- 激进程度：中等。注意 zelland 不验证主机密钥（ssh.rs:81-86）——torvox 若采用必须实现 known_hosts。

### 13.2 glyphon 0.7 —— 不适用（torvox 自研栈已超越）

- 优点：15 分钟上手、OpenGL/Vulkan 通吃。
- 缺点（zelland 亲历）：①atlas 格式必须匹配 surface（WGPU_FIXES.md Fix 1）；②Buffer 需 set_size + 全量 set_rich_text + shape，无增量；③每帧 prepare 全量上传字形；④字体发现依赖 fontdb，Android SELinux 下可能空库；⑤样式仅 weight/style/color（CellRun 丢 underline 等）。
- 结论：torvox 的 cosmic-text+swash+guillotiere 自研管线（已含 atlas 驱逐、CJK、FreeType 对照）是正确投资；**不要回退到 glyphon**。

### 13.3 Tauri v2 —— 部分适用

- zelland 用它拿到了：移动端 WebView UI + Rust 后端 + JNI 桥 + 插件生态（store/notification/haptics/shell）。
- 痛点（zelland 暴露）：①`gen/android` 由 CLI 生成且被 gitignore，Kotlin 层提交不全（§7）；②WebView+SurfaceView 叠层 z-order 管理困难（WGPU_FIXES.md Fix 3）；③KeybarPlugin 被迫绕过插件机制直接改 Activity 内容树（KeybarPlugin.kt:63-97 的 LinearLayout 包裹技巧）；④wgpu 23 落后（torvox 已 wgpu 30）。
- torvox 已选 Kotlin Compose + Rust native（无 Tauri），架构更干净；**Tauri 的移动端坑清单值得记录但不应引入**。

### 13.4 Svelte 5 —— 不适用（前端栈不同）

- runes 响应式 + SvelteKit 静态适配在 Tauri 下工作良好，测试完备（vitest）。
- 对 torvox 无吸收价值（torvox 无 WebView 前端）；但 `Terminal.svelte` 的"前端算 cell 尺寸 → 后端校正"模式（§8.1）可抽象为任何平台的 resize 协议。

### 13.5 libghostty-vt 集成方式对比（重要）

| | zelland | torvox |
|---|---|---|
| 获取 | git submodule（幽灵子模块，克隆缺失） | **git 依赖 Uzaaft/libghostty-rs + patch 到 generated-patches**（Cargo.toml:47-56），bootstrap 脚本打 Zig 正确性补丁 |
| 绑定 | bindgen 0.71 生成 C 绑定 + **手动追加 11 个 opaque 类型**（build.rs:113-126）+ zig 静态库交叉编译（api 30） | 官方 Rust crate（libghostty_vt::...），自带 key/render/style/terminal 模块 |
| 稳定性 | 脆弱（头文件不完整 workaround、TLS 对齐补丁 android_fix.zig） | 相对稳定（Cargo.lock 锁定 + 本地 patch 可控） |
| 结论 | **torvox 的 git+crate+patch 方案明显更优，保持现状** | |

### 13.6 激进程度总评

zelland 整体是"**高功能密度原型**"：单作者、快速迭代、文档驱动（15+ 设计文档）、风险点集中在渲染层（wgpu 23 过旧、单例渲染器、glyphon 陷阱已踩遍）。依赖选型（russh/gotatun/loro/axum）不激进但务实；`gotatun`（用户态 WireGuard）是较冷门但合理的选择。**对 torvox 的价值主要在经验（WGPU_FIXES.md）与模式（JNI 生命周期、SSH 循环、SGR 编码），不在依赖本身。**

---

## 14. 可吸收到 torvox 的具体内容（含代码注释建议）

### A. WGPU_FIXES.md 经验（大部分已吸收，补两条）

1. **atlas/纹理格式规则**——torvox 已在 context.rs:559-562 吸收（注释明确引用 zelland Fix 1）。建议在 `context.rs` 的 atlas 创建处补一条注释锚点：
   ```rust
   // 吸收自 zelland WGPU_FIXES.md Fix 1 + pitfall #9：
   // 1) 经 shader 采样的字形 atlas 与 render-target 格式解耦（我们走 direct sampling，
   //    无 glyphon 的格式耦合问题）；2) atlas 尺寸必须 clamp 到 max_texture_dimension_2d
   //    （部分 Mali 只报 2048）。create_atlas_texture 已 clamp（context.rs:655-659），勿移除。
   ```
2. **surface 生命周期竞态**（zelland 的 `PENDING_SIZE`，renderer/mod.rs:112-118 + set_surface:320-322）：torvox 已有 attach_window 处理（ffi.rs:1313），确认尺寸在 renderer 就绪前到达时是否丢失——若无，补一个 `pending_size` 暂存（与 zelland 同款）。

### B. 字体加载回退策略（Android SELinux）

zelland renderer/mod.rs:175-213 的模式：bundled Nerd Font 优先 → fontdb 空则系统路径硬编码回退。torvox font/mod.rs 已处理 CJK 回退；建议补"系统字体发现失败时的路径回退"测试与代码（`try_load_cjk_fonts` :1035 已有雏形）。

### C. touch → SGR mouse 编码（若未来需要）

移植 zelland terminal.rs:51-180 的语义，纯 Rust 重写（~80 行）：
- mouse mode 未启用（无 `\x1b[?1000h`）→ 丢弃；
- scroll 合成 Press+Release 两段（tmux/htop 依赖）；
- cell 尺寸用渲染器活值（zelland 的教训：terminal.rs:108 用 `get_cell_size()` 而非常量）。
建议位置：`native/src/terminal/` 新增 `sgr_mouse.rs`，事件源接 Kotlin GestureDetector（与 ffi.rs 的 input 通道一致）。

### D. SSH 会话循环模式（若未来加远程会话）

zelland ssh.rs:294-351 的 `tokio::select!`（输入 mpsc / channel.wait / 16ms 渲染定时器）结构清晰，可作 torvox 未来 `RemoteSession` 的骨架；keepalive `30s × 3` 配置直接采用（ssh.rs:34-35）。

### E. JNI surface 生命周期命名与竞态处理

zelland 的 `passSurfaceToRust/passSurfaceDestroyedToRust/passResizeToRust` 三段式（android.rs:18-99）+ "尺寸先暂存、初始化后回放"（:90 + mod.rs:320-322）是 Android wgpu 集成的标准范式；torvox 的 `attach_window_inner/detach_window_inner`（ffi.rs:1313,1639）已对应，建议核对 resize 在 attach 前到达时的处理并注释引用 zelland 范式。

### F. 原生键盘栏的 LinearLayout 包裹技巧

KeybarPlugin.kt:63-97：**WebView 硬件 surface 会绕过 z-order 合成**，overlay 方案会被盖住；把 WRY 根视图与工具栏装进垂直 LinearLayout 分区。torvox 若未来做 compose 外挂输入工具栏（如快捷 Ctrl/方向键条），同样会遇到 SurfaceView/Compose 层叠问题——此技巧可直接迁移为 Compose 侧 `Box` + 分区布局。参考注释：
   ```kotlin
   // 吸收自 zelland KeybarPlugin.kt:63-97：硬件加速 surface（WebView/SurfaceView）
   // 不受 elevation/z-order 控制，工具条必须与 surface 平级分区（LinearLayout/Box），
   // 不能做浮层 overlay。
   ```

### G. 选择文本提取的 Rust 侧缓存方案

zelland `extract_text`（renderer/mod.rs:546-567）从 row_cache 提取——torvox 已有 cell 数据缓存（ffi.rs:1601），若要实现"选择→复制纯文本"不经 OSC52，可参考此遍历模式（注意 zelland 按字符计数列，torvox 需按 cell 列处理宽字符）。

### H. 帧调试转储

zelland 前 3 帧 PPM 转储（renderer/mod.rs:743-840）——torvox 有 screenshot_tests + renderdoc，功能覆盖，无需移植；但"前 N 帧自动转储"作为 CI/真机首帧调试手段成本极低，可作备选。

### I. 不吸收清单（明确）

- 3-pass 渲染结构（torvox 单 pass 更优）；
- glyphon（§13.2）；
- gotatun WireGuard（与终端无关，torvox 无 VPN 需求）；
- daemon-rs/注解系统（产品方向不同）；
- 全局单例渲染器（CLAUDE_NOTES.md:89-103 已自证是反模式）；
- 不验证 SSH 主机密钥（安全反模式）。

---

## 15. 项目文档吸收价值

1. **WGPU_FIXES.md**（最高优先）：Android wgpu 集成的实战事故记录——atlas 格式、resize 后首帧、SurfaceView 层级、字体 SELinux、acquire 挂起、触摸与 DrawerLayout 冲突。torvox 已吸收 Fix 1/9（context.rs 注释），建议把"完整阅读 WGPU_FIXES.md"作为 torvox 渲染器维护者 onboarding 必读，并在 torvox 的 render 文档中维护等价事故清单。
2. **WGPU_GHOSTTY_PLAN.md + REMEDIATION_DESIGN.md**：迁移方法论（分阶段验证：Dinghy 设备测试 → 混合模式 → 原生 → 清理）与"迁移后补救清单"思路，可借鉴到 torvox 未来大改（如 wgpu 版本升级、渲染重构）的流程设计。
3. **CLAUDE_NOTES.md**：代码审查发现清单（死代码、panic 路径、单例、硬编码后端）——torvox 的 AGENTS.md/code review 流程可参考其"标记 resolved/open"的追踪格式。
4. **TESTING.md**：三层测试矩阵 + "待补测试"清单——torvox 的测试体系已更完善（conformance/截图/proptest），可对照补"渲染器表面重建"类生命周期测试（zelland 也缺，PLAN.md:71-73 自认）。
5. **NATIVE_UX.md**：移动终端 UX 设计深挖（虚拟键盘 6 大痛点、原生键盘栏权衡）——torvox 的 Compose 输入栏设计可引用其问题清单。
6. **APP_STORES.md / AGENTS.md**：上架策略与开发约定，对 torvox 的发布流程有参考价值。

---

> **文档生成时间**：2026-08-06
> **分析基线**：refs/zelland 克隆（含 src-tauri/src/renderer/mod.rs 3-pass 版、gen/android 仅 3 个 Kotlin 文件、daemon-rs、src-voice、zellij-plugin、proto、libghostty 空 submodule）
> **核心结论一句话**：zelland 是"SSH 远程终端 × Tauri 移动壳 × glyphon 3-pass"的高密度原型，其 wgpu 集成教训（WGPU_FIXES.md）、JNI surface 生命周期范式与 SGR mouse 编码对 torvox 有直接吸收价值；渲染管线本体（3-pass/glyphon/单例）与依赖（russh/tauri）不适用于 torvox 现有架构。

## deep-v3 增量（复核第 1 轮：CLAUDE_NOTES.md / APP_STORES.md）

### CLAUDE_NOTES.md（Claude Code 审查笔记，2026-03-25/27）

- **格式价值**：状态图例（✅ Resolved / ⚠️ Open / 🔲 Won't fix）+ 按主题编号 + "Dead Code from Phase 1" 章节——torvox 可借鉴为"AI 审查笔记模板"（docs/ 下 `review-notes` 式文档），与 docs/review-status.md 互补。
- **实质内容 1（已解决项）**：resize 传 `(0, 0)` 像素尺寸 → Ghostty 无法像素精确鼠标坐标/行高。**torvox 无此问题**（resize 链已传真实 cell 像素尺寸），对照确认。
- **实质内容 2**：`SshChannelMsg::Viewport` 死变体残留——审查应查"有测试覆盖但无生产路径"的死代码，torvox 复核同理。
- 其余（死代码清理、panic→Result、text-only renderer）均与 zelland personal 研究已覆盖内容一致。

### APP_STORES.md（Tauri 应用商店分发指南）

- Android 侧：keytool 生成 keystore → keystore.properties → `tauri android build --bundle aab` → Play Console 上传。torvox 为原生 Android（非 Tauri），且用 aosp-testkey 签名（非 Play 上传 key）——**无适用点**，记录不吸收。

## deep-v1 增量（2026-08-07 全文件精读轮 #2）

### 本次新精读文件（前次未逐行读）
- `daemon-rs/` 全部 24 文件（zlnd 守护进程）：server.rs（axum 路由 + **loopback_guard 中间件**——非 loopback 连接返回 403，:138-152）、ws.rs（protobuf Envelope + broadcast 256 + KeepAlive ping）、loro_manager.rs（Loro CRDT 注解同步，doc_to_json:34-90）、store/mod.rs（**Markdown 注解解析/回写**：parse_markdown_comments:66、reify_markdown_comments:197、code_ann_id 确定性 ID、loro_cache_path:400）、watcher.rs（notify 文件监视 → OpenView 广播，**跳过隐藏文件/目录** :120-125）、handlers/*.rs（fs/trigger/sessions/projects/annotations/assets）、config.rs（KDL 配置 + port 0 = OS 分配）、assets.rs（**AssetManager TTL 30min + 5min 清理循环**）、projects.rs（scan_projects 跳过隐藏目录）
- `src-voice/` 全部（语音终端实验）：pty.rs（**portable_pty** 用法 + spawn_blocking 读循环 + mpsc 写通道）、speech/engine.rs（**ONNX Moonshine STT**：encoder/decoder 双 Session + KV cache 手写 6 层 × 4 张量 + greedy decode）、speech/audio.rs（cpal 录音 + **rubato SincFixedIn 重采样** 48k→16k）、speech/mod.rs（SpeechState + stop_and_transcribe）
- `zellij-plugin/src/lib.rs`（65 行）：wasm 后台插件 + **版本化 pipe 协议**（VERSION=2，Zelland 检查 "v" 决定是否推送更新）
- `src-tauri/src/ssh.rs`（499 行完整）：russh 客户端（keepalive 30s + 密码/私钥/KeyStore 三认证）、**zero-scrollback 设计**（scroll() 直接 Ok）、**16ms flush interval（60 FPS 语义）+ MissedTickBehavior::Skip**、`ts.process_mouse` SGR mouse、shell_quote/bulid_zellij_connect_command（zellij attach 命令构造）
- `src-tauri/src/daemon.rs`（337 行）：Tauri↔zlnd WebSocket 客户端（protobuf 解码 + 通知分发）
- `src-tauri/src/helper.rs`（390 行）：**远程助手自安装**——detect_remote_platform（uname）→ read_remote_helper_version → 版本不匹配则 upload_file + chmod + mv → 启动并等 port
- `src-tauri/src/keystore.rs`（618 行）：StandardKeyManager（文件私钥）+ **AndroidKeyManager（Keystore + Biometric）**：BiometricRequest oneshot 注册表（register_biometric_request:206 / complete_biometric_request:213）
- `src-tauri/src/network.rs`（150 行）：WireguardConfig + start_tunnel（无实质实现，结构骨架）
- `src-tauri/src/android_notif.rs`（47 行）：**Rust→Kotlin 反向 JNI**（find_class AgentNotifHelper + call_static_method + exception_check/describe/clear）
- `src-tauri/src/intent.rs`/`keybar.rs`：Tauri 插件骨架（空实现）
- `src-tauri/build.rs`：**libghostty-vt zig 构建映射表**（Rust target→Zig target，Android API 30）+ prost_build
- `src-tauri/src/main.rs`：WebKitGTK NVIDIA DMA-BUF workaround（桌面，torvox 不相关）
- `src-tauri/src/terminal.rs`（252 行完整）：**SGR mouse 编码器**（process_mouse:34-85，scroll_up/down = Press+Release 两序列、click = Press+Release；mouse_mode=false 时 drop 并 warn）
- `src-tauri/src/renderer.rs`（1-380 死代码）：旧版 glyphon 渲染器（CELL_WIDTH=24/CELL_HEIGHT=32 常量 + Rgba8UnormSrgb atlas）

### 新发现（相对 deep-v0 文档）

| # | 发现 | 级别 | torvox 对照 |
|---|------|------|------------|
| 1 | **loopback_guard 中间件**：zlnd 只服务本机（非 loopback 403）——**torvox MCP Unix socket 天然安全**，若未来加 TCP 通道可参考此模式 | P3 | daemon-rs server.rs:138-152 |
| 2 | **zero-scrollback 设计**：zelland SSH 明确"无本地滚动缓冲"（scroll() 返回 Ok）——**torvox 有 50K 行 scrollback**，设计取向相反，记录对照 | P3 | ssh.rs scroll():476-479 |
| 3 | **16ms flush interval + MissedTickBehavior::Skip**：60 FPS 刷新语义，跳过积压 tick——torvox 渲染线程事件驱动，等价效果 | P3 | ssh.rs:381-386 |
| 4 | **AssetManager TTL + 清理循环**（30min TTL + 5min 清理）——临时资源管理模式，torvox 无对应 | P3 | assets.rs:15-55 |
| 5 | **Rust→Kotlin JNI 通知**（android_notif.rs）：torvox 全部 Kotlin→Rust，反向调用是参考（MCP toast/notify 工具未来可走此路径，但 torvox 已有 EVENT_QUEUE 事件通道——不采纳） | P3 | android_notif.rs:13-45 |
| 6 | **Biometric oneshot 注册表**：keystore.rs 的 register/complete 模式与 torvox dialogResult 的 REQUEST_REGISTRY 同构（oneshot + 全局 map）——**确认 torvox 模式是标准做法** | P3 | keystore.rs:206-218 |
| 7 | **版本化插件协议**（zellij-plugin VERSION=2 + 客户端检查）——协议演进模式，torvox MCP 工具列表由 tower-mcp 管理，不适用 | P3 | zellij-plugin lib.rs:6-9 |
| 8 | **helper 远程自安装**（检测平台→上传→chmod→版本比对）——torvox bootstrap 安装器类似（本地安装而非远程），确认 torvox BootstrapInstaller 思路正确 | P3 | helper.rs:46-140 |
| 9 | **portable_pty 用法**（src-voice pty.rs）：torvox 自研 PtyPair 更底层（fork/execve/linker64），portable_pty 抽象层（Box<dyn MasterPty>）不适用于 Android | P3 | src-voice pty.rs:1-178 |

### 依赖评估（deep-v1 确认）
- `loro`（CRDT）、`prost`（protobuf）、`notify`（文件监视）、`russh`（SSH）、`ort`（ONNX）、`cpal`/`rubato`（录音/重采样）——**全部不适用于 torvox**（无协作/SSH/语音功能）
- `kdl`（配置格式）：torvox 用 DataStore——不换
- 确认：torvox 与 zelland 功能交集仅"终端核心 + wgpu 渲染"，zelland 的 SSH/协作/语音是差异化功能

### 文档吸收
- AGENTS.md 的 daemon-rs 约定（AGENTS.md:29-36，extra 已引）——协作工作流约定，与 torvox 无关
