# godot-pty/gpty 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/gpty`
- 技术栈：Rust + Godot GDExtension + alacritty_terminal (VT core) + portable-pty, tokio, sqlite/sqlite FTS
- 定位：AI-oriented PTY foundation: tiling multiplexer + JSON-RPC/MCP control + concept capture + persistence.
- 许可证：GPL-3.0-only（仓库根 LICENSE 明示）。
- 源码 crate: gpty-core, gpty-gdext, CLI, agents.

## 全仓库结构与关键代码
- `crates/gpty-core/src/term.rs` wraps alacritty Terminal.
- `parser.rs`, IPC command dispatch.
- Godot bridge + network agent windows? MCP manifest.
- Portability.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- gpty is another desktop/Windows product; architecture useful for future IPC/MCP-like agent and multiplexer if Android; VT core conflict.
- GPL-3.0-only 不兼容直接作为通用 Android 代码依赖；直接采用不受支持。

## 可直接使用的库/工具
- MIT/Apache 元数据应被删除：仓库许可证为 GPL-3.0-only，未确认上游 compatible relicensing，Gamma 社区慎用。
- `portable-pty` 在 Android NDK 环境中不可直接运行：Android 应用 sandbox 不开放 `/dev/ptmx`，且上游没有官方 Android backend；本项目不建议采用。
- `vte` crate alternative.

## 可间接借鉴
- IPC/MCP JSON-RPC terminal control API.
- Capture lines concept engine.
- PTY environment sanitization rules (see AGENTS.md).
- Portable PTY thread model.

## 可稍微修改使用的代码/设计
- `gpty-core` parser is alacritty wrappers; not Ghostty.
- `portable-pty` on Android needs a native/JNI-specific PTY bridge; openpty/isatty based assumption conflicts with Android app sandbox。
- Clean APIs; can borrow design.

## 测试数据/测试用例
- Docs and crates tests.
- Godot integration tests possible.

## 设计模式
- Core + framework bridge separation.
- Commands, events, actions.

## 其他结论
- LLM-generated code; security rules insightful.
- Not direct.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
