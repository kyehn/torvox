# tuzig/terminal7 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/terminal7`
- 技术栈：TypeScript + xterm.js + Capacitor (Hybrid app), web goal, tests Vitest/Playwright
- 定位：WebRTC/Ssh-based remote terminal multiplexer (screen/tmux reincarnation) for web/mobile/tablet
- 许可证：GPL
- 构建：Vite + WebRTC Go agent (webexec).

## 全仓库结构与关键代码
- `src/` TypeScript client app; xterm.js terminal.
- `android/`, `ios/` Capacitor native wrappers.
- `tests/`, `aatp/`, docs.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- Terminal7 web-first; VT handled by xterm.js; WebRTC networking layer.
- Not direct; web approach less performance/native.

## 可直接使用的库/工具
- None.

## 可间接借鉴
- Remote multiplexer/session UX and WebRTC design.
- Mobile hybrid capacitor app architecture (not needed Torvox).
- Clipboard integration cross devices (Sstyled scheme).

## 可稍微修改使用的代码/设计
- TypeScript code not Android native.

## 测试数据/测试用例
- Vitest + E2E Playwright, useful web integration but not VT conformance.

## 设计模式
- Capacitor native wrappers + web UI.

## 其他结论
- Web-based remote terminal, C-grade mobile product. Not direct.
- Terminal7's webexec network agent is outside Torvox's declared local-only scope and is not imported.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
