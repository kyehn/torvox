# pop-os/cosmic-term 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/cosmic-term`
- 技术栈：Rust + libcosmic/iced + alacritty_terminal (VT core) + cosmic-text shaping/rendering
- 定位：COSMIC desktop terminal (Wayland/Unix), tabs, color schemes, keybindings
- 许可证：GPL-3.0-only
- 主要模块：src/terminal.rs (alacritty term integration), terminal_box UI, config, DnD, theme, password manager.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- cosmic-term uses alacritty_terminal, another VT core option; conflicts with libghostty-vt single-source.
- libcosmic is heavy desktop GUI framework; not Android.
- alacritty_terminal be alternative VT core but self-dev risks; not recommended adopt.
- Rust terminal integration patterns and PTY management useful.

## 可直接使用的库/工具
- alacritty_terminal Rust crate technically run on Android, but violates Ghostty single-source.
- cosmic-text shaping be useful but Android uses host font system and Ghostty? not direct.
- libcosmic not Android.

## 可间接借鉴
- Config color scheme/Profiles.
- tabs/window model.
- GPU vs font layout choice.
- keybinding/paste interaction.

## 可稍微修改使用的代码/设计
- C++/Rust terminal integration not portable to JNI easily but terminal.rs provides PTY+alacritty usage pattern.

## 测试数据/测试用例
- None major VT tests visible (alacritty has its own). Can cross-check expected.

## 设计模式
- UI front (libcosmic) + terminal backend (alacritty).
- state sync via term event channel.

## 其他结论
- MIT? GPL-3 allowed but direct only concepts.
- Cosmic shell ecosystem terminal is Rust/alacritty based rather than the Rust/C representation used above.
- Good contrast if Torvox considers VT alternatives, but not recommended.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档一致性.
