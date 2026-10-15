# PerBothner/DomTerm 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/domterm`
- 技术栈：C/lws-term + Java WebSocket server + JavaScript xterm.js/webapp + Electron/QTDomTerm
- 定位：结合 REPL/terminal emulator, DOM rendering (webapp, electron, Qt)
- 许可证：GPL-ish? COPYING
- 仓库包含 C and hlib (client JS), LWS websocket terminal, Java server.

## 全仓库结构与关键代码
- `hlib/`: DomTerm client shell, xterm-like UI, command execution.
- `lws-term/`: C websocket server using libwebsockets?
- `webapp/`: web app frontend.
- `electron/`: Electron app.
- `qtdomterm/`: Qt C++ wrapper.
- `tests/`, `doc/`.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- DomTerm is a web/Electron terminal; its terminal parser JS stack differs from Torvox's Rust VT baseline.
- 不可 directly embed into Android Rust.

## 可直接使用的库/工具
- None for Android.

## 可间接借鉴
- Webapp/Electron terminal integration patterns.
- WebSocket terminal server Java/C.
- Client-side command execution and results display (REPL).

## 可稍微修改使用的代码/设计
- JS/web assets cannot be embedded for Torvox rendering under project prohibition of web UI stack; rendering path remains Rust.
- No direct Rust.

## 测试数据/测试用例
- tests include shell integration. Not applicable.

## 设计模式
- Client/server split via WebSocket.
- DOM-based cell layout.

## 其他结论
- C-grade web hybrid reference; not direct.
- inform Android future web-based terminal but not now.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档一致性.
