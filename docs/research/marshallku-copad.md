# marshallku/copad 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/copad`
- 技术栈：Rust (edition 2024, alacritty_terminal) + platform native UIs (GTK4 Linux, Metal macOS, SwiftUI iOS), FFI shared core.
- 定位：AI-agent orchestration terminal across cross-platform with plugins/mux comux.
- 许可证：MIT
- 主要 crates: copad-core, copad-term (C ABI via alacritty), comux, mux daemon.

## 全仓库结构与关键コード
- `copad-term/src/lib.rs` C ABI bridge over alacritty event loop.
- copad-linux uses GTK/VTE stack on desktop; binding inventory is not complete in this report.
- web API and sockets, telemetry.
- Documentation plugin services.
- copad-mux provides agent orchestration.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- Copad quite mature Linux/macOS/iOS, not Android UI.
- Complementary for agent features, IPC control and split/comux inspiration.
- VT core conflict as usage (alacritty).

## 可直接使用的库/工具
- Rust `copad-term` bridge, if interface stable; useful Native VT runtime integration.
- `coctl`/JSON-RPC CLI patterns and comux multiplexer.
- Event Bus/triggers architecture.
- Plugins.

## 可间接借鉴
- C ABI bridge pattern with native app UI.
- Detach/resume AI sessions, terminal output event stream.
- Plugin trigger config.
- Worktree integration.

## 可稍微修改使用的代码/设计
- Rust FFI integration with Ghostty? Different VT (alacritty) requiring C bindings that currently conceal event semantics.
- any Ghostty FFI layer mimic? uncertain.
- Native GTK UI not Android, but Rust state can drive Compose.

## 测试数据/测试用例
- Monorepo run at various tests and E2E.
- alacritty cell tests.

## 设计模式
- Rust core + native UI.
- C ABI snapshots.
- Event bus, triggers.
- Multiplexer.

## 其他结论
- Strong product, MIT license.
- Not direct VT source due alacritty but valuable architecture.
 - copad-mobile uses a WebView-based remote terminal; that dependency prevents direct mobile adoption under Torvox constraints.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
