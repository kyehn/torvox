# omnipty-terminal/omnipty 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/omnipty`
- 技术栈：Rust + GPUI (Zed engine) + alacritty_terminal, custom PTY event loop.
- 定位：macOS/Linux native terminal, file tree/copy mode/workspaces, GPU rendering.
- 许可证：MIT
- Source small (~122 files plus assets).

## 全仓库结构与关键代码
- `src/terminal/*`: images, event loop.
- `Cargo.toml`: alacritty 0.26, GPUI etc.
- Docs Linux port.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- OmniPTY uses GPUI, not available on Android but a high-quality Rust GPUI terminal reference (Zed-like).
- VT self-source external (alacritty).
- Omnipty event loop implementation remains design material; this report does not copy or validate it.

## 可直接使用的库/工具
- GPUI not Android; but library pattern as desktop shell.
- Rust and salacritty dependencies don't help Android directly.

## 可间接借鉴
- Custom byte streaming event loop in replacement of alacritty's.
- Workspace/performance side tree and copy mode.
- TOML hot reload.
- Kitty protocol integration.
- Status bar git detection.

## 可稍微修改使用的代码/设计
- Event loop integration can port concepts.
- Rust GPUI annotations not mobile.

## 测试数据/测试用例
- `tests/` exists as a top-level directory; test frameworks and coverage not detailed in this report.
- Large image tests.

## 设计模式
- GPUI application architecture.
- Custom event loop.
- Source/kitty graphics.

## 其他结论
- MIT and polished.
- GPUI is a desktop renderer and is not an Android UI target; rendering code is reference-only for this project.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
