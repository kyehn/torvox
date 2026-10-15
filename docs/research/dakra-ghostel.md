# dakra/ghostel 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/ghostel`
- 技术栈：Zig 0.16 + libghostty-vt (vendor 0.1.3.2 dev) + Emacs 动态模块 (C ABI)
- 定位：基于 libghostty-vt 的 Emacs 终端模拟器
- 许可证：GPL-3.0-or-later
- 支持：Linux/macOS/FreeBSD/Windows/Android/Termux，Emacs 28.1+ 动态模块；提供预编译 native module 下载
- 主要模块：`GhostelTerm.zig`、`Renderer.zig`、`PosixPtyProcess.zig`/`ConPtyProcess.zig`/`NativeProcess.zig`、`kitty_graphics.zig`、Emacs Lisp UI

## 全仓库结构与关键代码
- `src/*.zig`：native module 实现，VT state、流、渲染、pty、输入、kitty graphics、图像解码 (png/stb)。
- `src/tests.zig`：Zig tests (renderer, pty, utils, kitty)。
- `lisp/*.el`：Emacs UI、shell integration、comint/eshell integrations、org link、desktop/bookmark。
- `test/hypothesis`：假设/渲染 property tests。
- `build.zig`：Zig 构建与 vendored ghostty。
- `vendor/ghostty`：Ghostty 源码 tarball metadata.

## 对比本项目
- 本项目 Android Rust + wgpu/Vulkan + Compose + libghostty-vt.
- Ghostel 也直接使用 libghostty-vt， является strongest compatibility peer for FFI/状态处理。
- 差异：Emacs native module vs Android JNI/Rust；Emacs UI vs Compose; 渲染 projection to Emacs buffers vs GPU raster.
- Ghostel 已支持 Termux/Android，故 Android PTY/WIN_SIZE/resize 经验尤其 valuable.

## 可直接使用的库/工具
- 无法直接作为 Android/Kotlin 依赖；Zig libc dynamic module architecture不 matching.
- Zig FFI patterns to libghostty-vt strongly useful for Rust side (allocations, Stream, terminal options, screen pages, pin tracking, kitty graphics emission).
- build/download prebuilt module flow useful for CI/distribution paradigm.

## 可间接借鉴
- `GhostelTerm.redraw()` 和 renderer dirtiness/pin tracking/resize commitment.
- PTY resize (cols/rows/xpixel/ypixel) semantics and native PIDs.
- Ghostty Kitty graphics emission (`ghostel--kitty-clear`, placements)。
- input handling and key encoders, hyperlink management, mouse, style face mapping.
- test strategy (Zig unit tests, hypothesis property tests).

## 可稍微修改使用的代码/设计
- Zig renderer cannot directly run in Android, but its projection algorithm and damage logic may inform.
- lisp integration patterns (comint/eshell) not directly relevant to Android UI.
- Prebuilt install flow (`ghostel-module-install.el`) may be source of trust UX baseline but Android uses APK native libs.

## 测试数据/测试用例
- `src/tests.zig`, test/hypothesis render JSON corpora.
- Property tests can inform visual regression.
- Tests target Emacs environment; cannot directly reuse but design can be adapted.

## 设计模式
- Native module boundary wrapping upstream VT.
- Renderer owns grid → buffer projection.
- GhostelTerm wraps terminal/stream/renderer/process.
- recursive mutex, pinned rows, dirty region aggregation.
- extensions for Emacs integrations.

## 其他结论
- Ghostel 是本仓最 relevant external reference because identical VT engine.
- GPL license and Emacs-specific UI prevent direct reuse, but FFI/runtime patterns (especially Android/Termux) highly useful.
- Upstream ghostty version tracking matters; Ghostel depends on ghostty tarball rev;本仓 required tracks master.

## 四次审阅记录
- 准确性：基于 README, src, tests, build zig.
- 完整性：涵盖 repo architecture, compatibility, tests, patterns.
- 对比正确性： stresses Ghostty VT same-engine relevance and Android/Emacs divergence.
- 文档一致性：仅 md.
