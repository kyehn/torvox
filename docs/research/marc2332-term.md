# marc2332/term 研究报告

## 仓库概况
- 源克隆：`/tmp/opencode/marc2332term`
- 技术栈：Rust + Freya GUI framework (`terminal-pty` feature) + clap, toml, gix.
- 定位： Freya-based Linux/Flatpak terminal with user workflows: tabs, panel split resizing, git worktree sidebar.
- 许可证：MIT.
- 代码 unusually app-level around Freya's `Terminal`; `src/panel.rs`, `state.rs`, `session.rs` home.
- 目标 open source project.

## Code reading highlights
- ` panel.rs` uses `freya::terminal::*` to render terminal area and font family, handles terminal area sized events.
- `state.rs` and session persistence (JSON) for tabs/shells/projects.
- Git worktree features bundled via `gix`: project/worktree groups/live diffs based on status command.
- Config `marcterm.toml` shell/font size.
- `flatpak.rs` handling etc.
- Release uses cargo/flatpak + website/docs.

## Directly usable
- Freya GUI dependencies are not Compose Android; desktop-only.
- `Terminal::new` public API shows how GUI framework exposes terminal handle and callbacks; only if moving torvox to desktop.
- Keyboard configurations and command defaults inform component shortcuts.
- Table state logic for split panels and tabs useful as design reference for future multi-terminal UI.

## Indirectly useful
- Freya's terminal architecture (host GUI uses abstract Terminal child and callback) informs generic terminal view APIs.
- Panel split topology is a compact Compose `SubcomposeLayout` analogy and can be retained as UI design insight; no implementation scheduled.
- `freya` itself run under winit/glutin? Not Android target and no direct wgpu Android use.
- Persistence strategy for tabs/network/history can reference.

## Slightly modifiable
- Nothing relevant to Android JNI or Rust/wgpu at source level; Freya internals untouched.
- Components can be re-implemented in Compose if user asks.

## Test data / test cases
- None visible outside application; no VT tests.
- Repository contains config and test assets; their exact purpose was not individually unpacked.

## Design patterns
- Terminal render as child UI element with sized-event callback to calculate drag.
- `use_state` application context and persisted session architecture.
- Worktree group intuitive application.
- Tab state captured leaf/pane.

## Compare torvox
- marcterm is higher-level app than terminal core; Freya owns VT rendering and PTY unlike torvox.
- Kotlin/Android + wgpu + Ghostty sets differing; no code port.
- Functional product-level architecture can inform split GUI only if using desktop expansion.
- Users' privacy and permissions not affected by term itself.

## Conclusion
- No direct technical integration.
- Useful as an example of modern compact Rust GUI terminal and freya patterns, plus panel-persistence features.

## 四次审阅记录
- 准确性: README, Cargo, src panel/state/session.
- 完整性: app UI source.
- 对比正确性: Android target explicit.
- 文档一致性: slug correct.
