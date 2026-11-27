# zelland 深度审查（0.2.3 / HEAD `8bf9cf55911588451804a39526f8ae869da021b6`）

## 范围与基线

- 上游：`https://github.com/njreid/zelland`，审查时 HEAD 为 `8bf9cf55911588451804a39526f8ae869da021b6`，`src-tauri/Cargo.toml` 版本 `0.2.3`、edition 2024。
- 关键依赖：`wgpu 23`、`glyphon 0.7`、`cosmic-text 0.12`、自绑 `libghostty-vt`（`libghostty/` 与 `libghostty-source/`）、Tauri 2 + Svelte 前端。
- 实读文件：`README.md`、`WGPU_FIXES.md`、`WGPU_GHOSTTY_PLAN.md`、`docs/features/WGPU_GHOSTTY_REMEDIATION_DESIGN.md`、`CLAUDE_NOTES.md`、`src-tauri/Cargo.toml`、`src-tauri/src/lib.rs`、`src-tauri/src/ghostty.rs`、`src-tauri/src/terminal.rs`、`src-tauri/src/renderer/mod.rs`、`src-tauri/src/renderer/android.rs`、`tests/ghostty_vt_test.rs`，并对照 glyphon 0.7.0 源码。
- 本仓定位：同引擎（libghostty-vt）+ wgpu 的第二个完整 Android 终端实现，参照价值集中在 JNI 表面生命周期与 glyphon 集成坑；不引入其代码、依赖或 UI 栈。

## 1. 分层与输入顺序

| 层 | 载体 | 职责 |
| --- | --- | --- |
| Android 壳 | `gen/android/.../MainActivity.kt`、`src-tauri/src/renderer/android.rs` | SurfaceView/WebView 双层、`GestureDetectorCompat` 触摸、`passSurfaceToRust` / `passResizeToRust` / `passTouchToRust` JNI |
| Rust 渲染 | `src-tauri/src/renderer/mod.rs::Renderer` | 全局 `Mutex<Option<Renderer>>`、surface 配置、图集/管线重建、三趟 render pass、`row_cache` |
| 终端会话 | `src-tauri/src/terminal.rs::TerminalSession` | VT 字节写入、render state 更新、鼠标事件编码、`render_native()` |
| VT 引擎 | `src-tauri/src/ghostty.rs` | `GhosttyTerminalWrapper`、`GhosttyRenderStateWrapper`、cell/style/grapheme 读取、指针与行脏标志 |
| 前端/远端 | Svelte + `ssh.rs` | 会话列表、SSH 通道、触摸转 SGR 鼠标序列 |

一帧的输入顺序：`SurfaceView` 事件 → JNI → `Renderer::set_surface` / `resize`（含 `pending_size` 竞态兜底）→ `TerminalSession::process_bytes` → `render_state.update(&term)` → `Renderer::draw_ghostty_state`（行缓存 + `set_rich_text` + `shape_until_scroll` + `glyphon::prepare`）→ `Renderer::render`（清屏/光标 → 文本 → 选区）→ `atlas.trim()`。

## 2. 关键机制

### 2.1 表面与管线

- `set_surface` 取 `caps.formats[0]` 配置 surface，格式变化时 `rebuild_text_pipeline` 重建 `TextAtlas` + `TextRenderer` 并清空 `row_cache`；随后消费本实例或全局 `PENDING_SIZE`，补一次 `resize` + `render`，覆盖"resize 早于 wgpu 初始化"的竞态。
- `resize` 把 `device.limits().max_texture_dimension_2d` 作为上限，同步 `viewport.update` 与游标/选区资源；尺寸钳制对应用 `caps` 的首选格式。
- `render()` 开三趟 pass：`clear_cursor_pass`（黑色清屏 + 一个 6 顶点矩形）、`text_pass`（Load + glyphon）、`selection_pass`（Load + 半透明蓝三角）；提交后 `atlas.trim()`。

### 2.2 行缓存与文本重建

- `with_rows` 只把脏行重新 `build_row_runs`，与 `row_cache` 中旧值比较；没有变化时直接返回，跳过 `set_rich_text`/整形/`prepare`。
- 一旦有变化，就把所有行拼成单个带样式的字符串（行间插 `"\n"`）整体交给 `set_rich_text`，再 `shape_until_scroll` 全量整形。对照文档 `docs/features/WGPU_GHOSTTY_REMEDIATION_DESIGN.md` 明写 "Non-goal: true per-row GPU updates"：所谓 damage tracking 只是 CPU 侧避免重复构串，不是字形级局部更新。
- `build_row_runs` 按 cell 循环，取 `get_cell_style` / `get_cell_graphemes`，把 grapheme 里的 codepoint 用 `char::from_u32` 拼接；`inverse` 只交换前景色；粗体/斜体映射到 `Weight` / `Style`；颜色走 `ghostty_color_to_rgb` + 完整的 16/216/24 级 xterm 色表。
- 256 色映射 `ansi_palette_color` 覆盖 0-15、16-231 立方体与 232-255 灰阶，无越界；但其取值是近似值（如红 `170`），非主题真值来源。

### 2.3 输入与状态

- `get_mouse_tracking()` 门控触摸转发；`encode_mouse_event` 经 libghostty 的鼠标编码器生成 SGR 序列，cell 尺寸取 `renderer::get_cell_size()` 的运行时值而非常量。
- `GhosttyRenderStateWrapper::with_rows` 读取行 dirty 后顺手清掉上游标志；`terminal.rs::render_native` 在确有 renderer 时清本会话 dirty 并 `reset_dirty()`，没有 surface 时保留脏标记，避免丢帧。

## 3. 缺陷、风险与文档claim核对

| 严重度 | 结论 | 依据 |
| --- | --- | --- |
| 阻断 | **当前检出无法编译**：`src-tauri/src/lib.rs` 写 `pub mod renderer;`（无 `#[path]`），同时存在 `src-tauri/src/renderer.rs` 与 `src-tauri/src/renderer/mod.rs`。用 rustc 1.100.0 对同构最小工程实测报 `E0761: file for module renderer found at both ...`。完整 crate 未跑 `cargo check`（需 Android NDK/Tauri 环境），但该错误与依赖无关 | `src-tauri/src/lib.rs`、`src-tauri/src/renderer.rs`、`src-tauri/src/renderer/mod.rs`；复现见第 5 节 |
| 高 | **"每帧更新光标"实际不可达**：`draw_ghostty_state` 在 `dirty == FALSE` 时首行 `return`，光标矩形更新写在 return 之后；注释与行为矛盾。纯光标移动帧若上游未置脏，光标不会跟到新位置 | `src-tauri/src/renderer/mod.rs::draw_ghostty_state` |
| 高 | 文本管线不是行级增量：任一脏行都会 `set_rich_text` 全量重建 + `shape_until_scroll` 全量整形 + `prepare` 全量顶点；行缓存只减少构串次数 | `src-tauri/src/renderer/mod.rs::draw_ghostty_state` 与 `docs/features/WGPU_GHOSTTY_REMEDIATION_DESIGN.md` 的 Non-goal 段 |
| 高 | cell 语义不完整：`build_row_runs` 丢弃非法 codepoint，不读 cell 宽度（宽字符/emoji 仍按一个 cell 的文本入列），不处理下划线、删除线、背景色；`inverse` 只换前景不画反显背景，底层清屏恒为黑色 | `build_row_runs`、`Renderer::render` 的 `LoadOp::Clear` |
| 中 | glyphon 钉在 0.7.0：0.7 的 `is_run_visible` 未把 `run.line_top/line_height` 乘 `scale`，且缺少后续版本的 Y 轴裁剪 clamp、生成式在用量跟踪与 wgpu/cosmic-text 升级 | 对照 `glyphon` 提交 `fa9a3f8`、`5698d9e`、`49dc8f7`；本仓留档 `docs/persistence/glyphon.md` 第 4 节 |
| 中 | 选区顶点缓冲固定 240 个 float（约 40 行），跨行选择超过该行数时后半段被静默截断 | `Renderer::selection_vertex_buf` / `selection_vertex_count` |
| 中 | 前 3 帧强制写调试 PPM：`save_debug_frame` 新建 capture 纹理、`device.poll(Maintain::Wait)` 同步等待并写 `/sdcard/Android/data/com.njr.zelland/files/frame_N.ppm`；启动阶段产生阻塞与存储权限依赖，Bgra/Rgba 还靠硬编码交换通道 | `Renderer::render`、`Renderer::save_debug_frame` |
| 中 | `WGPU_FIXES.md` 的因果表述有误：图集格式是 glyphon 采样纹理与管线 target 的格式，并非 render pass attachment 格式本身；格式不匹配在 wgpu 是校验错误而非"静默丢弃 draw"。修复动作（图集格式跟随 surface）本身正确 | `WGPU_FIXES.md` Fix 1 与 `src-tauri/src/renderer/mod.rs::rebuild_text_pipeline`、`glyphon/src/text_atlas.rs::get_or_create_pipeline` |
| 低 | `caps.formats[0]` / `caps.alpha_modes[0]` 无空数组检查；`WGPU_FIXES.md` 记录的 `CELL_WIDTH=17/CELL_HEIGHT=38` 已与现行运行时尺寸不符，文档滞后 | `Renderer::set_surface`、`WGPU_FIXES.md`「Cell Size」段 |
| 低 | `lib.rs` 的 `test_android_connectivity` 只有 `assert!(true)`，零判别力；`tests/ghostty_vt_test.rs` 存在但需上游 VT 环境才能判读 | `src-tauri/src/lib.rs` 的 `mod tests` |
| 边界 | `prepare` 失败只写 error 日志并沿用上一帧顶点；glyphon 的 `RenderError` 两个变体在 `render()` 中永不返回，分辨率/图集错配不会报错 | `src-tauri/src/renderer/mod.rs::draw_ghostty_state`、`glyphon/src/error.rs` |

## 4. 基准体系与复算方式

- 上游无 GPU 帧时间基准；`tests/ghostty_vt_test.rs`、`src-tauri/tests/ghostty_vt_test.rs` 是 VT 侧用例，`lib.rs` 的 `assert!(true)` 不构成性能或行为证据。`WGPU_FIXES.md` 的"~200 ms wgpu init"是经验数值，无基准脚本支撑。
- 复算命令：
  - `git clone https://github.com/njreid/zelland /tmp/zelland && git -C /tmp/zelland rev-parse HEAD`
  - `grep -n 'pub mod renderer;' /tmp/zelland/src-tauri/src/lib.rs && ls /tmp/zelland/src-tauri/src/renderer*`
  - 最小 E0761 复现：建目录含 `src/lib.rs` 写 `pub mod renderer;`，同时放 `src/renderer.rs` 与 `src/renderer/mod.rs`，执行 `rustc --crate-type lib --edition 2021 src/lib.rs`。
- 生成物复现校验：本留档以固定 commit 为准；上游修复双模块冲突后需重新核对第 3 节首行结论。

## 5. 处置结论

| 项 | 判定 | 依据 |
| --- | --- | --- |
| 引入 zelland 的渲染/UI 分层 | 不采纳：Tauri + Svelte + SurfaceView 双层与本仓 Compose + Rust 渲染线程架构不同，且当前检出有编译阻断 | 第 3 节首行；`docs/specification/DESIGN.md` |
| surface 格式驱动图集/管线重建、`pending_size` 竞态兜底 | 仅参照：本仓 `render-stability`、`startup-grid-resize` 已覆盖同类问题，无需新增抽象 | `Renderer::set_surface`、`openspec/specs/render-stability/spec.md`、`openspec/specs/startup-grid-resize/spec.md` |
| 行脏缓存 + 全量 `set_rich_text` 的"伪增量" | 不采纳：本仓以 cell 实例 + 行级脏带为最小单位，整缓冲重建会把 CJK 开销搬回 CPU | `openspec/specs/cjk-rendering/spec.md`；`native/src/render` |
| libghostty 鼠标编码器 + `get_cell_size()` 映射 | 仅参照其接口形状：本仓输入编码归 `key-input-encoding`，指针路径不照搬 | `src-tauri/src/terminal.rs::encode_mouse_event` |
| 调试 PPM、`assert!(true)` 测试、反显只换前景 | 不得反向采纳 | 第 3 节对应行 |

跳过：未执行完整 `cargo check`/`cargo test`（缺 Android NDK、Tauri 与 libghostty 构建环境）；风险是除已用 rustc 复现的 E0761 外，其他编译期问题未覆盖。
