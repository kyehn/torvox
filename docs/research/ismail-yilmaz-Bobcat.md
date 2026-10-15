# ismail-yilmaz/Bobcat 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/bobcat`
- 技术栈：C++17 + Ultimate++ (U++), TerminalCtrl widget + Conway (self-dev VT)
- 定位：跨平台 desktop terminal (Linux/Windows/macOS/BSD), feature-rich, Protocols Sixel/Kitty/iTerm2/Jexer, web/headless backends.
- 许可证：GPL v3

## 全仓库结构与关键代码
- `Bobcat/*.cpp/h` UI and terminal application, profiles/split.
- VT implementation delegated to `TerminalCtrl` external widget (not in this repo).
- Linkifier, Finder, QuickText.
- Makefile + web/headless build.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- Bobcat is desktop and relies on external TerminalCtrl; U++ framework not Android.
- Eligible feature set useful as UX/reference.

## 可直接使用的库/工具
- TerminalCtrl at https://github.com/ismail-yilmaz/Terminal is an external dependency not bundled; its VT implementation must be inspected separately and is not analyzed here.
- U++ not Android.

## 可间接借鉴
- Vi-like selector/key bindings, image formats, hyperlink.
- Web/HTML5 Canvas terminal rendering ex<rfl:parameter name="mple (Java?).
- Search regex patterns.
- Headless backend approach.

## 可稍微修改使用的代码/设计
- External C++ terminal widget not Android portable.
- VT self-dev not Ghostty.

## 测试数据/测试用例
- None specific.

## 设计模式
- App + profiles + plugins.
- Linkifier/diagnostics UI.

## 其他结论
- Requires external TerminalCtrl; C-grade desktop.
- GPL v3.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
