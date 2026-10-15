# kemokempo/iced_term 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/iced-term`
- 技术栈：Rust + ICED (wgpu graphics) + alacritty_terminal (VT)
- 定位：iced terminal widget for desktop, PTY, keyboard/mouse, fonts/colors.
- 许可证：MIT
- 版本 0.8.0 (crate), examples full_screen, split_view.

## 全仓库结构与关键代码
- `backend.rs`, `terminal.rs`, `view.rs`, `font.rs`, `theme.rs`, `bindings.rs`, `settings.rs`.
- backend command handling and PTY.
- View renders cell grid via iced (wgpu).
- Uses alacritty_terminal as backend parser.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- iced_term renders cells using Rust wgpu/iced; useful rendering patterns but UI framework differs (Compose Android vs iced).
- VT core conflict (alacritty vs Ghostty), so parser backend not adopt.

## 可直接使用的库/工具
- iced_term is desktop framework code; iced GUI rendering is not an Android UI target and no build artifact for Android is present in this report.
- alacritty_terminal Cell/ansi types not.
- Renderer cell-drawing logic port snippets.

## 可间接借鉴
- Iced/WGPU-based cell backdrop, font metrics, and color palette themes.
- Command + event communication pattern (UI <-> backend).
- PTY shell setup via settings.
- Keybinding mapping.

## 可稍微修改使用的代码/设计
- MIT; borrow code for Rust rendering adapter.
- But need avoid AL-style semantics. Adapting to Ghostty is feasible cell renderer.

## 测试数据/测试用例
- Examples and `#[cfg(test)]` tests are included in repository wiring; module list was not expanded here.
- leverage for rendering snapshot/interaction.

## 设计模式
- Clean separation commands/actions/events.
- Terminal backend abstraction by alacritty.
- View module.

## 其他结论
- Desktop UI framework not Android. Potential cell rendering code port.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
