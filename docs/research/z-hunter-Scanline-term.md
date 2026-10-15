# z-hunter/Scanline-Term 研究报告

## 仓库概况
- 源克隆：`/tmp/opencode/scanline`
- 技术栈：Tauri 2 + React 19 + TypeScript + xterm.js + Rust ConPTY, Windows 10/11 x64.
- 定位：CRT retro terminal, multi-tab ConPTY, browser tabs, AI assistant, MCP server, session recovery.
- 许可证：MIT.
- 前端 shaders xterm; Rust backend ConPTY + MCP.
- Agent context files (`AGENTS.md`) present though not project-specific.

## Code reading highlights
- Rust dependencies: `conpty-oxide`; bundled Windows ConPTY dynamic libs.
- `main.rs` manages terminal state locking, spawn shell, input/channels, `terminal-output` events, exit.
- `mcp.rs` exposes create/list/close terminals to AI agents.
- `browser.rs`, `codex.rs`, `presets.rs`, `home.rs`.
- Vitest frontend tests; Tauri config; shaders.
- Strong UX but architecture desktop-web terminal, not Android native.

## Directly usable
- No Android JNI relevance:
  - ConPTY only Windows backend.
  - xterm.js / React / WebGL rendering replace Ghostty and wgpu approach.
- Best candidates from docs/code:
  - MCP agent terminal bridge code shows shape of agent-facing handle IDs (`terminal-n`) and lifecycle error messages.
  - Windows terminal launch fallback and process output channels.
  - Search, preset, tabs and CRT effects UX ideas only.
- No performance asset usable.

## Indirectly useful
- `scanline-virtual-screen` host and simulator adds display behavior but web renderer.
- ConPTY state machine writer/reader can be referenced only for additional platform support or labs.
- MCP server implementation is well-separated and can inform optional agent command-request UI if needed.
- Portability and runtime environment distribution of native binaries reference only.
- ` outputs/readme` captures design principles: ultra-lightweight, instant read, integrated browser tabs, AI assistant.

## Slightly modifiable
- Nothing should be copied into Android native production.
- Frontend tests (vitest) can inspire TypeScript test format but torvox Kotlin.
- Bundled ConPTY binaries cannot be ported to Android.

## Tests / test data
- vitest frontend tests cover components and terminal status inputs; exact assertions were not catalogued.
- No VT conformance suite.
- Existing tests exercise row painting, but scenario enumeration was not provided.
- No test matrices for terminal semantics.

## Design patterns
- Web-embedded terminal UI with native PTY.
- Tauri IPC channel vs web frontend.
- AI and MCP model with tool outputs.
- Preset and home page controller.

## Compare torvox
- Deep mismatch (web WebView renderer, xterm.js) for Android target.
- Android project uses Ghostty; Scanline is webterm based.
- Documentation still useful design space.

## Conclusion
- Not a technical resource for core renderer or parser, but good product UX / integration path example.

## 四次审阅记录
- 准确性: README/package/Cargo code.
- 完整性: relevant backend only.
- 对比正确性: platform and renderer.
- 文档 consistency: slug correct.
