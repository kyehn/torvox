# iewnfod/lumina-terminal 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/lumina-terminal`
- 技术栈：Tauri + Rust backend (portable-pty) + React frontend xterm.js.
- 定位：cross-platform terminal emulator with profiles and MCP server.
- 许可证：MPL-2.0（根 LICENSE 明示）。
- Wrapper project.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- Lumina terminal hybrid web UI; VT emulation by xterm.js.
- Not native Android nor Rust VT.

## 可直接使用的库/工具
- None.

## 可间接借鉴
- Tab/profile management.
- Universal engine boot process via Tauri Rust backend.
- Rich xterm.js addons (unicode/graphemes, etc).
- Autoproxy sync, MCP server experimental.
- Command palette and shell-completion overlay.

## 可稍微修改使用的代码/设计
- Frontend is web; not Android native.
- Portable PTY on Linux.
- `portable-pty` not Android.

## 测试数据/测试用例
- `tests/` holds frontend/app tests; assertions were not expanded here.

## 设计模式
- Rust backend + webview frontend.
- Event command palette.

## 其他结论
- xterm.js is a web-first frontend and would require WebView, which Torvox does not use; performance must be measured by web standards separately.
- VT provided by xterm.js.
- Tauri 2 may mobile support but Rust+WebView differs.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
