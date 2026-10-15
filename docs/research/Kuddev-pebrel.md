# Kuddev/pebrel 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/pebrel`
- 技术栈：Rust 2024 + GPUI (Zed fork) 桌面应用；`nebula_terminal` crate 使用 `vte` crate 解析 ANSI；Unix PTY via rustix；Windows ConPTY
- 定位：GPU 终端、SSH/SFTP 工作区与 AI CLI 会话桌面应用（Windows/macOS/Linux）
- 许可证：GPL-3.0-or-later
- 工作区：`nebula_app`、`nebula_terminal`、`nebula_config`、`nebula_gpui`、`nebula_settings`、`nebula_split`、`mobile/link`
- 渲染：GPUI/OpenGL ES2+，.profile release 做 size optimization + hot path O3

## 全仓库结构与关键代码
- `nebula_terminal/src/term`：cell/color/clear/damage/input/keyboard/prompt/renderable/search/term 模块，VT 模型。
- `nebula_terminal/src/grid`：grid resize/storage/row tests。
- `nebula_terminal/src/tty`、`event_loop.rs`、`thread.rs`、`sync.rs`：PTY/TUI 线程、事件循环。
- `nebula_terminal/src/osc_cwd.rs`、`vi_mode.rs`、`selection.rs`。
- `nebula_terminal/tests`：connection_transport、pi_redraw_anchor、windows_pty_lifecycle、`ref` 目录的录制回放测试（zerowidth、colored_underline 等）。
- `nebula_app`：GPUI UI、tabs、splits、SSH/SFTP、AI claws、settings。
- `architecture/notes`：设计决策文档。
- `docs/screenshots`：产品截图。

## 对比本项目
- 本项目为 Android Rust + wgpu/Vulkan + Kotlin/Compose，终端状态以 `libghostty-vt` 为唯一来源。
- pebrel 的 `nebula_terminal` 使用 `vte` 自研终端模型，虽然 Rust，但违反本仓“终端状态单一来源”；不能 adopt 为 VT 核。
- pebrel 目标桌面 GPUI，不支持 Android；GPUI 依赖 Zed fork + heavy deps，不符合 Android JNI/wgpu 架构。
- OpenGL 而非 Vulkan；Android 不支持其渲染栈。

## 可直接使用的库/工具
- 无 Android/Kotlin/F JNI 依赖可直接使用。
- `nebula_terminal` 作为 desktop Rust VT 核，license GPL-3.0 且架构不贴合（无 Android、自研 VT），不可直接嵌入。
- `vte` crate 本身是 Rust VT 解析器building block；本仓已用 libghostty-vt，不适用。

## 可间接借鉴
- PTY 事件循环、线程与 redraw anchor（`pi_redraw_anchor` 测试）在 Android 需重写但可作 Rust 参考。
- damage tracking、cell/grid storage、resize handling。
- search model、vi mode、selection、OSC cwd handling。
- 录制回放测试方法（`.recording` + expected grid JSON）可作本仓 integration test 设计参考，但期望值必须外部真相。
- profile release optimization strategy（size LTO + hot path O3）在 mobile Rust 可借鉴。

## 可稍微修改使用的代码/设计
- Rust term 模块不可直接嵌入 Android（desktop-only APIs）；只能 copy algorithms，且 license GPL-3.0 注意。
- VT 录制测试数据可作测试 fixture 参考，但不能整包 adopt（ST 三条准入）。
- 其 damage/render model 仅作 GPU 一个 renderer 的参考，本仓已用 Glyph atlas + wgpu。

## 测试数据/测试用例
- `ref.rs` 测试回放录制与 expected grid。
- `tests/pi_redraw_anchor.rs`、`connection_transport.rs`、`windows_pty_lifecycle.rs`。
- 单元测试：grid tests、keyboard contract tests、emoji input tests。
- 测试数据 verbose（vttest refs）可作回归对照，但整包有自验证风险。

## 设计模式
- 模块化 terminal crate + app crate 分离。
- vte parser callbacks -> Terminal model -> grid cells -> renderer damage.
- event loop thread + sync.
- profile-based optimization.

## 其他结论
- GPL-3.0 许可证限制直接复用；本仓避免引入许可文件。
- 与本仓 Android/Compose/wgpu/Ghostty 方向差距大；主要作为 Rust GPUI 桌面终端的高压场景对照。
- pebrel 强调 AI CLI 会话，产品定位与本仓常规终端差异。

## 四次审阅记录
- 准确性：基于 Cargo workspace、terminal crate、tests。
- 完整性：涵盖仓库、架构、对比、库/工具、借鉴、可修改、测试、模式、结论。
- 对比正确性：未推荐自研 VT；明确 GPUI/desktop/Android 不兼容。
- 文档一致性：仅新增 Markdown，未改受保护文件。
