# sudo-tee/hollow 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/hollow`
- 技术栈：Zig 0.16.0 + LuaJIT runtime + Ghostty VT core + Direct3D/FreeType software rendering; Windows/WSL primary, Linux X11, macOS planned.
- 定位：可配置终端 + Lua 插件/pane/workspace 系统，个人/用户脚本定制。
- 许可证：未确认（仓库 README/build 不含 explicit OSS license field in first read; LICENSE? check not performed due scope).
- build.zig.zon deps: Ghostty 1.3.2-dev (commit 20abdb50), zluajit, nightwatch; Zig build.
- 结构：src/mux, app, ui, render, lua, term, pty, htp; docs extensive; tests (mux).

## Code reading highlights
- `build.zig.zon` pins Ghostty core as dependency with `fonts` lazy dependencies.
- `src/term/ghostty.zig`: 1830 lines runtime interface to Ghostty, callbacks and terminal state; pane integration and render dirty flags.
- `src/render`: atlas, backend, font discovery, shaping, kerning, cache; custom software renderer, kitty graphics, screenshots.
- `src/lua`: Lua API hooks and tests; Plugin system (`hollow.plugins`) for panes/overlays/widgets.
- `src/pty`: PTY implementation, ConPTY and bypass helper for WSL.
- `src/mux`: multiplexer/tests.
- `docs`: configuration, keymap, lua API bindings, actions, platforms.
- Benchmarks as 3MB docs.

## Directly usable
- Zig Ghostty integration pattern (`src/term/ghostty.zig`) can directly inform torvox Rust `libghostty-vt` wrapper. API hooking, callbacks, dirty tracking, update/render state transition patterns.
- ConPTY and WSL bypass technical notes (not Android).
- Atlas/glyph cache and font shaping algorithms if porting rendering. But torvox uses wgpu/Rust.
- `src/lua/tests` contains Lua test material, not a VT conformance fixture set applicable to Ghostty.
- Build dependency methodology with Ghostty direct, avoids duplicated wrappers.

## Indirect useful
- Rendering lifecycle dirty flags (`render_dirty`, idle panes); Android GPU loop can optimize.
- Kitty graphics errors and segmentation if porting Kitty parser. Ghostty already supports image protocol.
- ConPTY race handling in pty (`src/pty`) for Windows desktop; Android does not need.
- Lua config/plugin mechanism as extensibility idea for terminal commands; not Android permission model.
- Docs on Ghostty VT core integration.
- `nightwatch` indicates UI/browser test tooling, not terminal VT assertions.

## Slightly modifiable
- Zig Glyph atlas code port to Rust wgpu if algorithms match target Unicode ranges. But substantial.
- Ghostty runtime callback interface patterns can be copied into Rust wrapper minimal.
- Docs platform specifics guide if adding desktop target.

## Test data/test cases
- `src/lua/tests` for Lua API assertions; not VT semantics.
- `src/mux/tests.zig`.
- benchmark files/bench scripts.
- No pure VT corpus shown.
- No ConPTY integration tests were found in this repository.

## Design patterns
- zig tracking: Ghostty runtime is separate object, callbacks registered before calls.
- pane multiplexer, render cache, Lua dynamic plumbing.
- Config-driven keymap/actions ("actions").
- Render app and terminal render separation including `terminal_render.zig`.
- Error reporting in terminal (bell, title).
- Resource management macros (callbacks null slots).

## Compare torvox
- Hollow shares Ghostty core philosophy, but in Zig desktop emulator; torvox Kotlin+Rust + Android + wgpu.
- torvox can leverage hollow's macro-level architecture and callbacks while maintaining libghostty-vt.
- Both manage render state dirty; Hollow closer to Vim-like panes, whereas torvox mobile UI is single terminal with Compose.
- Hollow's Lua/plugin configurability is overkill for Android.
- Hollow's font and shaping code complements wgpu, but platforms differ (Direct3D software).
- Hollow source is more desktop/Windows-focused; Linux supported Mac planned (*not* Android).
- Important security note: PTY and ConPTY bypass have privileges not target mobile sandbox.

## Conclusion
- Primary reference for proper Ghostty runtime lifecycle and render dirty flags. Direct dependencies not applicable Android (Zig/Native UI).
- Security sensitive ConPTY helper irrelevant.
- port callback and arena structures; license note needed, but hollow license not confirmed.

## 四次审阅记录
- 准确性：README/build.zig.zon/src term/render/pty/mux.
- 完整性：not all 77 source files individually; samples enough plus Ghostty.
- 对比正确性：zig -> Rust/Kotlin difference explicit.
- 文档 consistency: slug correct.

