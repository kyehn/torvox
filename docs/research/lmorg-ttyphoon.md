# lmorg/ttyphoon 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/ttyphoon`
- 技术栈：Go + Wails (WebView-based UI), typescript frontend in `frontend/src`; self-developed `virtualterm` package for VT.
- 定位：多媒体终端 (output blocks, structured text/tables/images, tmux control plane, AI).
- 许可证：GPL-2.0-only（根 LICENSE 明示）。
- 主要 modules: backend.go, virtualterm, window, timezone etc.

## 全仓库结构与关键代码
- `virtualterm/` stores terminal emulator code separated by package; this report does not claim a clean portable boundary.
- `frontend/` TypeScript UI + rich overlays.
- `tmux/` tmux control plane integration.
- `tests` test cases, lint.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- TTYphoon uses Go/Wails webview rendering; VT self-dev.
- Features like output blocks and structured text inform terminal UI overlay architecture and UX.
- Not Android.

## 可直接使用的库/工具
- None.

## 可间接借鉴
- Output block grouping and command exit status coloring.
- Structured text and clickable tree widgets in terminal stream.
- Tmux control plane integration patterns.
- AI tracing non-intrusive arrangement.
- Go VT implementation and test out (unit tests).

## 可稍微修改使用的代码/设计
- Webview UI is desktop not mobile.
- VT self-dev not needed.

## 测试数据/测试用例
- `test/`, `references_context_test.go`, and escape-key tests exist; assertion granularity was not expanded.
- inspect VT conformance expected behavior.

## 设计模式
- Frontend/backend separation via Go/Wails.
- `virtualterm` as terminal state lib.

## 其他结论
- Web-heavy terminal; Android Compose offers rich widgets natively.
- Not direct.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
