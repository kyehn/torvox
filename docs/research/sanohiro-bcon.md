# sanohiro/bcon 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/bcon`
- 技术栈：Rust + OpenGL ES (DRM/KMS/EGL/GBM), evdev/xkbcommon, vte crate parser, Sixel/Kitty, splits/tabs.
- 定位：Linux TTY (bare console) 无X/Wayland GPU 终端，built for AI coding tools; bundled panes.
- 许可证：MIT
- Features: vte parser + grid, GPU rendering, IME, panes, search, notifications.

## 全仓库结构与关键代码
- `src/terminal/parser.rs`: implements vte::Perform and applies to grid.
- `src/terminal/mod.rs`: VT parser + grid integration.
- renderer (GL), pane layout, input, summary.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- bcon is bare Linux TTY; provides relevant GPU rendering and terminal parser designs but Android has Compose.
- vte crate is self-contained, be used for desktop tests but Ghostty single-source prohibits core VT replacement.

## 可直接使用的库/工具
- vte Rust crate (MIT-compatible) be for test/shimmer, but not primary.
- Geometry/OpenGL code not Android (wgpu uses GL/Vulkan backend).
- Panel search and mouse drag are design references only.

## 可间接借鉴
- vte::Perform + Grid state architecture; similar interaction patterns.
- Sixel decoder integration is implemented in bcon; upstream origin was not established.
- Damage/panes/search UX.
- Fast GPU rendering frame loop on Linux DRM.

## 可稍微修改使用的代码/设计
- Renderer code is desktop-specific; not direct.
- Sixel decoder or vte extension may be extracted but not required.

## 测试数据/测试用例
- `tests/` was not categorized by this report.
- inspect Sequence test under `tests`.

## 设计模式
- Parser state machine via vte, CPU grid.
- Panel layout, scrollback.

## 其他结论
- MIT, compelling if Android ever bypasses GPU? But wgpu enough.
- Not direct due single VT.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
