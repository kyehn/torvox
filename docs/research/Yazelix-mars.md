# Yazelix/mars 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/mars`
- 技术栈：Rust workspace (Rio-derived) + Sugarloaf WGPU/Swegniog rendering, alacritty_terminal as VT core, Nix packages.
- 定位：Yazelix 默认终端 (成像 text), Ghostty alternative, performance gates, Kitty protocol/graphics.
- 许可证：MIT（根 LICENSE 明示，Raphael Amorim copyright）。此仓库基于 Rio/Sugarloaf 并携带多模块许可证；每个 crate 也有 own LICENSE，需保持分发时声明。
- Features: sugarloaf GPU rendering, cell baseline, cursor config, Kitty graphics, link handling, perf instruments.

## 全仓库结构与关键代码
- `rio-backend/`: grid, vi mode alacritty-derived, event handling, terminal bridge.
- `sugarloaf/`: custom GPU glyph rendering.
- `teletypewriter/`: PTY.
- `specs/`: change scorecard docs, design docs.
- `rio-backend/crosswords` stores the terminal cell grid and buffer implementation in this Rio-derived checkout.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- Mars uses alacritty_terminal but high-quality Rust/wgpu terminal self-reference. Sugarloaf may be adaptable to Android GL/Vulkan via wgpu; but full dependency stack large.
- VT single-source conflict.

## 可直接使用的库/工具
- `sugarloaf` if target-platform support warrants; substitute Torvox rendering backend, but major integration/dependency changes (wgpu features).
- `teletypewriter` is a cross-platform Rust PTY implementation derived from Rio, but Android PTY permissions and sandbox rules mean it still needs a native/JNI bridge rather than direct import.
- Config spec inventory useful UX metadata.

## 可间接借鉴
- Cell baseline glyph placement (Ghostty-style) for bitmap rendering and font metrics.
- Nerd-font status sizing and font fallback.
- Kitty graphics protocol details and derived WGPU source rect.
- Link hit spans / edge punctuation.
- JIT performance gating for renderer.
- Nix package metadata contract.

## 可稍微修改使用的代码/设计
- Sugarloaf renderer not mobile; but algorithms can port.
- `teletypewriter` PTY not Android PTY.
- Docs.

## 测试数据/测试用例
- `specs/`, benchmarks in repo, gated tests.
- Benchmark and spec case files exist; whether they map to Torvox frame timings must be recomputed on target devices.
- alacritty cell grid tests.
- Rio rendering tests.

## 设计模式
- Cargo workspace; backend + sugarloaf + PTY.
- Config-driven appearance.
- Performance gated internal metrics.

## 其他结论
- High-value WGPU renderer and rendering glyph placements; fork telemetry.
- alacritty VT conflict should not replace Ghostty.
- Use performance methodology rather than libs if Ghostty integration is main.
- Mars is product-level Rust terminal and policy transparency.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
