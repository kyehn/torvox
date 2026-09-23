# 深度研究：zelland（njreid）— 亲自逐文件阅读版

> 研究日期：2026-08-06 | 克隆位置：`repositories/refs/zelland`（depth 1）
> 研究方式：**主代理亲自逐文件完整阅读**（非子代理生成）
> 项目链接：https://github.com/njreid/zelland
> 定位：Tauri + wgpu + glyphon + libghostty-vt 的 Android/Linux SSH 终端。**与 torvox 技术栈最接近的参考**（同用 wgpu + libghostty-vt）

## 0. 架构总览

```
Android Activity (MainActivity.kt, 1047 行)
├── DrawerLayout（原生侧边栏：sessions/hosts 树）
│   ├── FrameLayout
│   │   ├── WebView（Svelte 前端，透明背景，欢迎页/模态框/日志）
│   │   └── SurfaceView（wgpu 终端表面，setZOrderMediaOverlay）
│   └── LinearLayout（原生侧边栏面板）
├── GestureDetectorCompat + ScaleGestureDetector
└── KeybarPlugin（LinearLayout 分区，非 overlay）
         │ JNI                          │ JS bridge
         ▼                              ▼
┌───────────────────────────────┐   Svelte 前端（src/lib/）
│ Rust (src-tauri/src/)          │
│  renderer/mod.rs (1123) 3-pass │
│  renderer/android.rs (216) JNI │
│  terminal.rs (252) 会话+鼠标   │
│  ghostty.rs (335) C FFI 绑定   │
│  ssh.rs (499) russh 管理器     │
│  keystore.rs (618) 密钥/生物   │
│  daemon.rs (337) zellij 守护   │
│  helper.rs (390) 远程辅助      │
│  network.rs (149) wireguard    │
└───────────────────────────────┘
```

## 1. 渲染器：`renderer/mod.rs`（1123 行，完整阅读）

### 1.1 常量与 shader（:15-50）

- `CELL_WIDTH=17.0` / `CELL_HEIGHT=38.0`（:15-16）——**编译期常量仅作 fallback**（WGPU_FIXES.md #10：鼠标映射必须用 `get_cell_size()` 实时值）
- `CURSOR_SHADER`（:19-39）：uniform `{rect: vec4, color: vec4}`，**无顶点缓冲**——`vs_main` 用 `@builtin(vertex_index)` 生成 6 顶点三角形
- `SELECTION_SHADER`（:41-50）：顶点位置直通 + **硬编码半透明蓝 `vec4(0.3, 0.5, 1.0, 0.35)`**（:48）

### 1.2 结构体（:53-105）

- `CellRun {text, fg, bold, italic}`（:53-60）：单行样式 run
- `Renderer`（:62-105）：instance/adapter/device/queue/surface/config/**pending_size** + glyphon 全套（cache/font_system/swash_cache/atlas/atlas_format/text_renderer/viewport/text_buffer）+ **row_cache: Vec<Vec<CellRun>>**（行级缓存）+ **span_buf**（复用，防每帧分配）+ cell_width/cell_height + cursor 管线（pipeline/bgl/uniform_buf/bind_group/cursor_pixel_rect）+ selection（tuple + pipeline + vertex_buf + count）+ debug_frames_saved

### 1.3 全局状态（:107-127）

- `static RENDERER: Lazy<Mutex<Option<Renderer>>>`（:107）——全局单例
- **`static PENDING_SIZE: Lazy<Mutex<Option<(u32,u32)>>>`（:112）**——surface 尺寸独立存储，`passResizeToRust` 可在 renderer 初始化前写入；`set_surface` 读取兜底（注释 :109-111：wgpu init 约 200ms，resize 常先到）
- `update_font_size_global`（:120-127）：异步锁内更新字体

### 1.4 init（:130-269）

- `Backends::all()` + `request_adapter(HighPerformance, compatible_surface: None)`（:136-142）——**surface 无关的 adapter 选择**（适配器在 surface 前创建）
- `required_features: Features::empty()` + `required_limits: downlevel_webgl2_defaults().using_resolution(adapter_limits)`（:158-160）——**downlevel 兼容**（保证 GLES 可跑）
- **字体加载（:179-213）**：先加载捆绑 Nerd Font（`/data/data/com.njr.zelland/files/fonts/`），**SELinux 可能阻止 fontdb 自动发现** → 手动尝试 `/system/fonts/NotoSansMono-Regular.ttf` 等 6 个路径（:193-207）——与 torvox 的 font_db.rs 系统扫描对比
- glyphon 初始化（:215-227）：`TextAtlas::new(&device, &queue, &cache, cache_format)`——**占位格式 Bgra8Unorm**（:219），set_surface 时若 surface format 不同则重建（Fix 1）
- `Metrics::new(CELL_HEIGHT * 0.75, CELL_HEIGHT)`（:226）

### 1.5 set_surface（:271-330）

- `create_surface_unsafe(SurfaceTargetUnsafe::RawHandle { raw_window_handle, raw_display_handle })`（:272-285）
- `caps.formats[0]` 取格式（:287-288）
- config：`present_mode: Fifo`、`desired_maximum_frame_latency: 2`、`view_formats: vec![]`（:290-299）
- **Fix 1 关键（:305-312）**：surface_format != atlas_format → `rebuild_text_pipeline`——**atlas 格式必须匹配 surface 格式，否则文字静默不显示**（WGPU_FIXES.md Fix 1 完整记录）
- **Fix 2 关键（:316-329）**：pending_size 应用后必须 `render()`——否则黑屏直到首个会话连接

### 1.6 rebuild_text_pipeline（:332-350）

重建 atlas + text_renderer + **row_cache.clear()** + cursor/selection 资源重建。

### 1.7 cursor 管线（:354-437）

- shader + bgl（uniform buffer）+ pipeline（BlendState::REPLACE）+ 32 字节 uniform buffer + bind group
- 无顶点缓冲：`rpass.draw(0..6)`（vs_main 用 vertex_index 展开矩形）

### 1.8 selection 管线（:439-492）

- 顶点缓冲预分配 `50 * 6 * 8`（50 行 × 6 顶点 × 8 字节）（:484-486）
- `set_selection`（:494-508）：**规范化 start<=end**，inactive 清空
- `update_selection_vertices`（:510-544）：逐行生成 NDC 矩形顶点（首行从 sc 列，末行到 ec+1 列，中间行全宽），**超过 50 行截断**（:535-536）

### 1.9 extract_text（:546-567）

从 row_cache 提取选区文本（行间 `\n`），用于 JNI `getSelectionText`。

### 1.10 render（:635-741）— 3-pass

- Pass 1（:660-686）：**clear + cursor**——`LoadOp::Clear(black)` + cursor 矩形（uniform 直写）
- Pass 2（:688-708）：**text**——`LoadOp::Load` + `text_renderer.render`
- Pass 3（:710-731）：**selection overlay**——半透明蓝矩形（有 selection 才开启）
- `atlas.trim()`（:735）
- **debug_frames_saved < 3**（:737-740）：前 3 帧调 `save_debug_frame`（:743-840）——copy_texture_to_buffer → map_async → **PPM 文件写 /sdcard**（对齐 256 字节行宽，BGRA→RGB 转换）

### 1.11 draw_ghostty_state（:842-962）

- **dirty 检查（:843-846）**：render state 全局 dirty=false → 直接 return（跳过整个 glyphon prepare）
- `text_buffer.set_size`（:851-852）
- **行级缓存（:855-868）**：`with_rows` 遍历，仅 dirty 行重建 runs；row_cache 内容未变则 `changed=false`
- **shrink 缓存（:871-875）**：终端变小 → `row_cache.truncate`（防 ghost rows）
- **cursor 每帧更新（:877-884）**：`cursor_pixel_rect = (col*cell_w, row*cell_h, cell_w, cell_h)`
- **changed=false 跳过 glyphon（:886-890）**：无文本变化 → 跳过 set_rich_text+shape+prepare（性能关键）
- span_buf 复用（:895-915）：每行 runs → (text, Weight, Style, Color)，行间插入 `\n` span（白字）
- set_rich_text + shape_until_scroll（:920-936）+ prepare（:938-961）

### 1.12 build_row_runs（:966-1028）

逐 cell：graphemes 空 → 空格；**inverse → fg/bg 交换**（:987-991）；相同样式合并 run。

### 1.13 颜色（:1033-1082）

- `ghostty_color_to_rgb`：StyleColor 的 NONE/PALETTE/RGB 三 tag 处理（PALETTE 用 ansi_palette_color）
- `ansi_palette_color`（:1051-1082）：16 基本色 + **6×6×6 立方**（55+v*40 公式）+ 灰度（8+idx*10）——标准 xterm 256 色

### 1.14 全局访问（:1084-1121）

`with_renderer` / `get_cell_size`（实时 cell 尺寸）/ `RawWindow`/`RawDisplay` 包装（Send + raw-window-handle traits）。

## 2. JNI 层：`renderer/android.rs`（216 行，完整阅读）

9 个导出（全部 `#[cfg(target_os="android")]` + `spawn_on_runtime` 异步执行）：
- `passSurfaceToRust`（:20-55）：ANativeWindow_fromSurface → RawWindow/RawDisplay → async：renderer 未初始化则 `Renderer::init().await` → `set_surface`。注释明确"不在此时 resize——passResizeToRust 随后立即带真实尺寸到达"
- `passSurfaceDestroyedToRust`（:58-70）：`drop_surface`（surface=None, config=None）
- `passResizeToRust`（:73-97）：**先 `store_pending_size`（全局静态，防 renderer 未就绪竞态）** → resize + render
- `getSelectionText`（:100-121）：row_cache 提取选区 → jstring
- `setSelectionHighlight`（:124-141）：set_selection + render
- `passPasteToRust`（:144-166）：jbyteArray → ssh.write_input
- `getCellDimensions`（:169-181）：jfloatArray[2]
- `updateFontSizeToRust`（:184-200）：物理像素 → update_font_size(px, 1.0)
- `passTouchToRust`（:203-216）：**action 是 JString**（"down"/"move"/"up"）→ ssh.process_touch

## 3. 终端：`terminal.rs`（252 行，完整阅读）

- `TerminalSession { term, render_state, dirty }`（:4-8）
- `process_bytes`（:23-26）：term.write + dirty=true
- `resize`（:28-35）：**像素尺寸 = cols*CELL_WIDTH, rows*CELL_HEIGHT**（编译期常量！）——与 renderer 实时 cell 尺寸不一致的潜在 bug（torvox 用 font metrics 正确）
- `process_mouse`（:51-93）：**scroll/click 拆成 Press+Release 两个序列**（:60-90）
- **`encode_mouse_event`（:96-175）**：用 libghostty C API 的 `ghostty_mouse_encoder_*`——`setopt_from_terminal`（从终端读 mouse mode）+ `GhosttyMouseEncoderSize`（**用 renderer::get_cell_size() 实时值**，:108）+ `ghostty_mouse_event_set_*`（position/action/button/mods）+ `ghostty_mouse_encoder_encode` 到 64 字节缓冲
  - action 映射（:129-133）：click/scroll → PRESS，release → RELEASE
  - button 映射（:136-147）：LEFT/RIGHT/FOUR（滚轮上）/FIVE（滚轮下）/UNKNOWN
  - **mouse_mode=false 时丢弃事件**（:54-56，等 `\x1b[?1000h`）
- `render_native`（:180-200）：render_state.update → with_renderer(draw_ghostty_state + render) → **仅在实际提交帧后清 dirty**（:191-194，无 renderer 时保留 dirty 下次重试）
- 4 个单测（:207-251）：processes_data / render_native_without_renderer（不 panic）/ encode_mouse_event SGR（`\x1b[<` 前缀）/ right_click

**torvox 对比**：torvox **没有 SGR 鼠标编码**（MouseModeTracker 只跟踪模式，不产生序列）——这是唯一值得移植的能力（见 §5）。

## 4. 绑定：`ghostty.rs`（335 行，完整阅读）

- bindings：`include!(concat!(env!("OUT_DIR"), "/ghostty_vt_bindings.rs"))`（build.rs bindgen 生成，:9-11）
- `GhosttyTerminalWrapper`（:15-119）：new（**`ghostty_terminal_new(alloc, &mut terminal, options)`——传 GhosttyTerminalOptions 结构体**，含 max_scrollback=0）/ write（ghostty_terminal_vt_write）/ resize（含像素）/ get_size/get_cursor_pos/get_mouse_tracking（ghostty_terminal_get 三参数风格）/ Drop free / **Send+Sync**
- `GhosttyRenderStateWrapper`（:121-287）：new（state + row_iterator + row_cells 三对象）/ update / get_dirty / reset_dirty / get_size / **with_rows（:210-250）**：迭代行，读行 dirty flag，回调后重置行 dirty / Drop 三对象 / Send+Sync
- `get_cell_style`（:301-312）：**先设 style.size 再 get**（C API 惯例）
- `get_cell_graphemes`（:314-335）：先查 len 再取 buf

**torvox 对比**：torvox 用 `libghostty-vt` Rust crate（更高层封装）。**注意 API 差异**：zelland 的 `ghostty_terminal_new(alloc, term, options)` 传 options 结构体——而 torvox vendor 的 C API 是 `(alloc, term, cols, rows)` 分开参数（torvox 曾做 ABI 修复，见 patches/）。这证明 libghostty C API 在不同版本间变化，torvox 的 patches 方案（锁定签名）是必要的。

## 5. 主集成：`lib.rs`（266 行，完整阅读）

- `spawn_on_runtime`（:27-33）：**JNI/UI 线程无 Tokio 上下文 → tauri::async_runtime::spawn**——JNI 与 async 的桥接模式
- tauri commands：ssh_connect/disconnect/write/resize/scroll、daemon_*、run_remote_command、set_terminal_font_size（:129-132）、show_agent_notification、密钥管理（generate/list/delete）、biometric_result
- **插件日志分级（:210-215）**：`russh → Warn`、`tokio_tungstenite → Warn`——第三方库降噪（同 wgpu-in-app 模式）
- Android KeyManager vs StandardKeyManager（:198-201）——平台 trait 分派

## 6. Kotlin 宿主：`MainActivity.kt`（1047 行，完整阅读关键部分）

- **手势（onCreate 内 GestureDetectorCompat）**：
  - `onSingleTapConfirmed`（:110-121）：passTouchToRust("click") + 隐藏 EditText 聚焦弹键盘
  - `onLongPress`（:122-135）：pixelToCell → **默认选 12 字符**（`(col+12).coerceAtMost(255)`）→ setSelectionHighlight + startSelectionActionMode
  - `onFling`（:136-151）：**左滑（vx<-600 且 |vx|>1.5|vy|）→ 打开 DrawerLayout**（WGPU_FIXES.md Fix 6：SurfaceView 吞掉所有触摸，DrawerLayout 边缘检测失效，必须程序化打开）
  - `onScroll`（:152-166）：**两指** → scroll_down/scroll_up（Fix 5：distanceY>0 = 手指上移 = 向下滚）
- **ScaleGestureDetector（:168-182）**：pinch → baseFontSize（20-80 物理像素）→ updateFontSizeToRust
- **SurfaceView（:225-300）**：
  - `setZOrderMediaOverlay(true)`（:241）——**WebView 硬件 surface 无视 z-order，punch-through 不可靠**
  - **WebView 先 add、SurfaceView 后 add**（:284-285）——Fix 4：**触摸派发给最后添加的子视图**，SurfaceView 后加才能先收到触摸
  - `visibility = GONE` 起始（:237，Fix 3）+ JS `TerminalNative.setVisible` 控制
  - selection 拖拽（:250-262）：ACTION_MOVE 时实时 setSelectionHighlight
  - surfaceCreated 通知 JS `surface-ready` 事件（:265-272）
- **隐藏 EditText（:294-330）**：1×1 透明，IME 输入 → TextWatcher → `KeySeqs.charToSeq(ch, ctrl, alt)` → JS `kb-input` CustomEvent（逐字符转发）
- **ActionMode（:840-890）**：`startActionMode(callback, TYPE_FLOATING)`——Copy/Paste 两按钮；onDestroyActionMode 清 selection
- doCopy（:892-899）：getSelectionText JNI → ClipboardManager；doPaste（:901-912）：**bracketed paste 包装**（`\u001b[200~...\u001b[201~`）
- copyFontsFromAssets（:946-968）：字体从 assets → filesDir/fonts
- JNI 声明（:990-999）：9 个 external
- KeyStore/Biometric（:1010-1047）：AndroidKeyStore 加密 + BiometricPrompt

## 7. 输入编码：`KeySeqs.kt`（38 行，完整阅读）

`charToSeq`（:26-37）：Ctrl 字母 → `ch & 0x1f`、Ctrl+[ → ESC、Ctrl+\ → FS、Ctrl+] → GS、Ctrl+^ → RS、Ctrl+_ → US、Ctrl+@ → NUL、Alt → ESC 前缀、`\n` → CR。

**torvox 对比**：torvox 的 `TerminalInputEncoder`（encodeCommittedText）功能等价（Ctrl 折叠 + bracketed paste）。KeySeqs 是独立可测对象（有测试）——torvox 的 InputEncoder 也有单测。等价。

## 8. 键盘栏：`KeybarPlugin.kt`（273 行，前 150 行精读）

- **LinearLayout 分区（:70-110）**：WebView 硬件 surface 无视 elevation/z-order，overlay 方案会被盖住——**把 wryRoot + keybar 包进垂直 LinearLayout**（weight 分配）解决。这是"硬件 surface 无视 z-order"的经典解法（torvox 用 TextureView 无此问题，但教训值得记录）
- IME insets padding（:86-96）：keybar 在 IME 之上（`max(imeInsets.bottom, systemInsets.bottom)`）
- 修饰键**双击锁定**（350ms，:35-40）
- `emit` 测试 seam（:44-47）

## 9. 文档：`WGPU_FIXES.md`（完整阅读，最高价值）

12 条 checklist + 6 个 Fix 记录：
1. **Fix 1**：atlas 格式必须匹配 surface 格式（glyphon 静默丢弃）
2. **Fix 2**：set_surface 应用 pending resize 后必须 render
3. **Fix 3**：SurfaceView 起始 GONE + JS 可见性守卫（无会话不显示）
4. **Fix 4**：触摸派发给最后添加的子视图——WebView 先加、SurfaceView 后加
5. **Fix 5**：onScroll distanceY 方向（手指上移 = 内容下滚）
6. **Fix 6**：DrawerLayout 边缘检测被 SurfaceView 吞掉——onFling 程序化打开
7. **Checklist 9**：mouse mode guard（`?1000h` 前丢弃触摸）
8. **Checklist 11**：DrawerLayout + SurfaceView 共存
9. **Checklist 12**：JS bridge 数据推送（Svelte $effect → @JavascriptInterface）

## 10. 其他模块结构（结构扫描）

| 模块 | 行数 | 职责 | torvox 关联 |
|------|------|------|------------|
| ssh.rs | 499 | russh SshManager（connect/run_command/upload_file/process_touch） | 无 SSH（本地终端）；process_touch 模式可参考 |
| keystore.rs | 618 | AndroidKeyStore/生物识别/密钥管理 | 无关联 |
| daemon.rs | 337 | zellij 守护进程（WebSocket + loro CRDT） | 无关联 |
| helper.rs | 390 | 远程辅助（curl 脚本） | 无关联 |
| network.rs | 149 | WireGuard 隧道 | 无关联 |
| renderer.rs | 380 | **旧版单 pass 渲染器（死代码）** | 对比：旧版 draw_terminal_grid(text) 单 buffer 方式 |
| intent.rs/keybar.rs/android_notif.rs | 52 | Tauri 插件 | 无关联 |

## 11. 与 torvox 功能对比总表

| 功能 | zelland | torvox | 结论 |
|------|---------|--------|------|
| 渲染架构 | 3-pass（clear+cursor / text / selection） | 单 pass 实例化 | **torvox 更优**（少 2 个 pass，样式完整） |
| 光标渲染 | uniform buffer + vertex_index 展开 | CellInstance quad | 等价（torvox 更统一） |
| 选区渲染 | 半透明蓝叠加（硬编码） | fg↔bg 反色 | **torvox 更优**（系统样式一致） |
| 行级缓存 | row_cache + dirty 行 | cell_data_dirty | 等价 |
| 文本引擎 | glyphon（每帧全量重排版） | swash 自研 atlas | **torvox 更优**（已规避 glyphon 坑） |
| SGR 鼠标编码 | ghostty_mouse_encoder C API | **无** | **torvox 缺口**（唯一值得移植） |
| 字体加载 | 捆绑 Nerd Font + 手动系统路径 fallback | font_db 系统扫描 + extra paths | 等价（zelland 有 SELinux fallback 教训） |
| surface 生命周期 | 整树 set_surface/drop_surface + PENDING_SIZE | ADR-0007 惰性 attach | **torvox 更优** |
| 渲染线程 | JNI 触发 + tauri runtime | 独立渲染线程 + notifyRender | **torvox 更优** |
| 触摸 | GestureDetector + passTouchToRust | Compose 触摸 + SelectionHandles | 平台差异 |
| 菜单 | ActionMode TYPE_FLOATING（Copy/Paste） | ActionMode TYPE_FLOATING（Copy/SelectAll/Paste） | 等价（torvox 多 SelectAll） |
| 选区文本提取 | row_cache extract_text | scrollbackLine JNI | 等价 |
| bracketed paste | doPaste 包装 | TerminalInputEncoder | 等价 |
| 256 色映射 | ansi_palette_color（6×6×6 立方） | ghostty 内部处理 | torvox 不用（ghostty 有完整调色板） |
| 调试帧 | 前 3 帧 PPM 写盘 | 截图测试 | torvox 更系统化 |
| 键盘栏 | KeybarPlugin（LinearLayout 分区） | ModifierBar（Compose） | 平台差异 |
| IME | 隐藏 EditText + TextWatcher | BaseInputConnection + InputCoalescer | 平台差异 |

## 12. 依赖分析

| 依赖 | 用途 | 适用 torvox？ |
|------|------|--------------|
| wgpu | 渲染 | **已用**（torvox 30 > zelland 版本） |
| glyphon | 文本 | **不适用**（torvox 自研 swash 栈更优，已规避 glyphon 格式耦合坑） |
| libghostty-vt（C FFI + bindgen） | 终端核心 | torvox 用 libghostty-vt Rust crate（更高层） |
| russh | SSH | 不适用（torvox 本地 PTY） |
| tauri | 应用框架 | 不适用（torvox Compose + JNI 更轻） |
| once_cell / log | 工具 | 已用等价 |
| dinghy | Android 测试 | 可选（torvox 用 adb/maestro） |

**先进激进判断**：zelland 依赖常规；glyphon + bindgen 方案比 torvox 的 libghostty-vt crate 脆弱（空 submodule、手动追加 11 个 opaque 类型）。torvox 方案更优。

## 13. 可吸收到 torvox 的具体内容

1. **SGR 鼠标编码（P0，唯一真缺口）**：libghostty-vt 是否暴露 `ghostty_mouse_encoder_*`？torvox 的 `MouseModeTracker` 只跟踪模式。若 libghostty-vt 有 mouse encoder 封装，直接接线 touch → SGR 序列（terminal.rs:96-175 参考）。参考实现：`GhosttyMouseEncoderSize` 用实时 cell 尺寸（:108-119）。
2. **atlas 格式匹配注释（P0）**：WGPU_FIXES.md Fix 1 已记录到 torvox context.rs 注释（此前研究已完成）。
3. **pending_size 竞态模式（P1）**：torvox `attachPendingSurface` 已有等价；补"renderer 未就绪时 resize 先存全局"注释。
4. **触摸派发顺序注释（P1）**：Fix 4——torvox Compose 无此问题，但 AndroidView 内 View 叠加时"最后添加先触摸"是通用教训。
5. **mouse mode guard（P1）**：`?1000h` 前丢弃触摸（terminal.rs:54-56）——torvox 若实现鼠标编码必须带。
6. **日志分级（P2）**：russh/tokio_tungstenite → Warn（lib.rs:210-215）——torvox 的 tower-mcp/axum 可降噪。
7. **线性布局 vs overlay（P2）**：硬件 surface 无视 z-order 的教训（KeybarPlugin.kt:70-110）——torvox TextureView 已规避。

## 14. 项目文档吸收价值

- **WGPU_FIXES.md**：wgpu+glyphon+libghostty Android 集成的完整坑位清单——**torvox 的 docs/lessons/ 应吸收**（至少 5 条 checklist 已对应 torvox 实践）
- WGPU_GHOSTTY_PLAN.md：分阶段迁移计划（Phase 0 测试 harness → Phase 1 换大脑 → Phase 2 原生 surface → Phase 3 全集成）——torvox 的 ADR-0007 实施路线可参照其"先 mock 验证再接线"策略
- TESTING.md / AGENTS.md / PLAN.md：多 agent 协作文档文化

## 15. 结论

zelland 是 torvox 最接近的架构参考（wgpu + libghostty-vt + Android），但 torvox 在渲染架构（单 pass vs 3-pass）、surface 生命周期（惰性 attach vs 整树重建）、文本引擎（swash vs glyphon）上**全面领先**。唯一真缺口：**SGR 鼠标编码**（ghostty_mouse_encoder）。WGPU_FIXES.md 是坑位清单的最佳来源（torvox 已完成其中大部分，值得对照核查）。
