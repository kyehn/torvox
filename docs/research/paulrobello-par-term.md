# paulrobello/par-term 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/par-term`
- 技术栈：Rust workspace + wgpu 30 + egui; core engine external `par-term-emu-core-rust`.
- 定位：GPU terminal with split panes, MCP/plugins, agent integration, live graphics.
- 许可证：MIT
- Modules: config, fonts, input, keybindings, render, scripting, mux, mcp.

## 全仓库结构与关键代码
- `par-term-render`: GPU renderer.
- `par-term-terminal`: inline-image payload/bounds.
- `par-term-config` rendering types.
- Docs features, mux extensions.
- Configuration 200+ options.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- Par-term architecture (modules + egui overlay) not mobile, but rendering and graphics policy relevant.
- Core engine is another Rust VT lib.
- MIT.

## 可直接使用的库/工具
- par-term-config and par-term-keybindings are MIT-licensed subcrates; Android dependency compatibility and maintenance requirements were not verified.
- external VT lib is used by core; compare.

## 可间接借鉴
- GPU surface clamping, no-op configure to avoid swapchain teardown.
- Kitty graphics geometry (`c`, `r`, offsets)
- Terminal agent API, command palette.
- ACP/crash triage concepts.
- Plugin triggers.

## 可稍微修改使用的代码/设计
- Rendering uses wgpu/egui and is not the Compose-based Android UI required by this project.
- Core engine external adds size.

## 测试数据/测试用例
- `AUDIT.md`, tests, render tests.
- Input test suite and config validation.

## 设计模式
- Workspace crate composition.
- Agent tool surface.
- Feature flags.

## 其他结论
- The new-product Rust terminal ecosystem; valuable.
- VT source still external, and Ghostty remains canonical for Torvox.
- MIT.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
