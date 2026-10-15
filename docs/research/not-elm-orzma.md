# not-elm/orzma 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/orzma`
- 技术栈：Rust, custom "orzma_vt" crate, webview-in-terminal protocol; renderer details came from package inventory, exact backend stack not fully enumerated.
- 定位：Hybrid terminal/browser, TypeScript SDK.
- 许可证：MIT
- VT core self-developed, units/tests.

## 全仓库结构与关键代码
- `crates/orzma_vt/` VT parser/interpreter, hyperlink, SGR handling.
- `crates/orzma_terminal/`, `orzma_configs/`.
- `docs/book/` user guide.
- SDK: ratatui widget and web.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- Webviews-in-terminal is desktop-oriented; native Android embedding is not part of that design. Self-contained VT compatibility coverage must be measured separately.
- License MIT.

## 可直接使用的库/工具
- `orzma_vt` cannot become TorVox's VT component because Ghostty is its sole VT source; it is only useful as reference.
- Rust SDK ratatui embedding design.
- Webview protocol JS bridge.

## 可间接借鉴
- Live webview/graphics embedding in this report is retained as desktop design insight; Android View embedding is outside report scope.
- vt test structure in `crates/orzma_vt/tests`.
- OSC hyperlink naming (alacritty).
- Generative AI policy interesting (prohibit AI except listed).

## 可稍微修改使用的代码/设计
- VT self-dev not direct.
- WKWebView-based UI is outside TorVox's Rust/wgpu rendering model, and WebView is not part of current project scope.

## 测试数据/测试用例
- orzma_vt tests.

## 设计模式
- Terminal VT crate separated from renderer.
- Client/server protocol for webview app.

## 其他结论
- Innovative hybrid product; MIT.
- Webview embedding not core Torvox.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
- Dependence on custom OS native backends; webviews not straightforward.
