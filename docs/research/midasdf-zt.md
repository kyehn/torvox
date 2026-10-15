# midasdf/zt 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/zt`
- 技术栈：Zig + native platform backends (Cocoa / X11 / Wayland / framebuffer), comptime config, embedded font.
- 定位：experimental minimal terminal emulator (no full features).
- 许可证：MIT
- Features: xterm-256color + truecolor, SGR, alt screen, bracketed paste, CJK, OSC8, OSC52, damage, SIMD.

## 全仓库结构与关键代码
- Zig `src/` files implement zt; VT parser crate relationship should be confirmed from build files, not from broad comment.
- tests and benchmarks.
- Embedded 59k glyphs Nerd Fonts.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- zt Zig no Android; self-dev VT.
- SIMD fast paths and efficient terminal cell state valuable reference.

## 可直接使用的库/工具
- None.

## 可间接借鉴
- Bulk parser fast paths and damage tracking.
- Row-map scroll O(1).
- Terminal test method and config comptime design (not Zig itself).
- Font embedding approach.

## 可稍微修改使用的代码/设计
- Zig parser not portable directly but portable approaches.

## 测试数据/测试用例
- `/tests` contains sequence-oriented test inputs; assertions and runners were not catalogued.
- benchmark scripts.

## 设计模式
- Terminal emulation without libghostty.
- Backend abstraction at compile time.

## 其他结论
- Minimal and pragmatic; MIT.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
