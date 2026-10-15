# ghreprimand/odytty 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/odytty`
- 技术栈：Rust + wgpu + winit + DRM/Wayland etc; from-scratch terminal VT (no external Ghostty integration) + accessibility, tabs/panes, theming.
- 定位：Odyssey visual identity terminal, Linux-first, desktop releases.
- 许可证：GPL-3.0-only（根 LICENSE 明示）。
- Uses wgpu 29.0; test infrastructure (fuzz, benches).

## 全仓库结构与关键代码
- `Cargo.toml`, src: boxdraw, native gpu tests, paths separate from wgpu.
- SPEC.md, docs (compat/fuzz/tests).
- VT parser 采用仓库自编的 parser 和 screen buffer；仓库没有 libghostty-vt 依赖，具体 sequence coverage 需按源码 matrix 测。

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- Very relevant for wgpu rendering cell grids, text layout, themes, accessibility, box drawing, fuzz tests.
- Self-dev VT competitive benchmark; cannot replace Ghostty.

## 可直接使用的库/工具
- Not a library; crate.
- Native GPU test suite useful for sanitization.
- Box-drawing algorithms are implemented in Odytty and require separate license and implementation review before reuse.

## 可间接借鉴
- `src/paths` purity rule (std only) aligns Torvox strong architecture (Rust core isolated from host callback).
- wgpu terminal text rendering and box drawing.
- Fuzz/bench methodology.
- Accessibility controls.

## 可稍微修改使用的代码/设计
- Rust rendering app code can be adapted partially; dependencies may differ (wgpu 25? Torvox version).
- Some custom theme can inspire.

## 测试数据/测试用例
- `fuzz/`, `benches`, `tests` sequences.
- Release notes.

## 设计模式
- Architecture purity.
- config/keybinding.

## 其他结论
- “from scratch” VT means source comparison but not recommended.
- Rust+wgpu highly comparable to Android Terra? Useful rendering reference.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
