# TragicWarrior/libvtty 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/libvtty`
- 技术栈：C VT terminal library with RGB color mapping, key maps, mouse, CSI buffering; CMake
- 定位： libvtty (former libvterm, fork of libROTE) VT 虚拟化 C 库；KMScon lineage 未在文档确认。
- 许可证：LGPL 2.1 core（COPYING 明示 derivative licensed under LGPL 2.1 except where noted），模块额外许可以源文件头部为准。
- File count ~? there are many vtty_*.c modules.

## 全仓库结构与关键代码
- `vtty.c`, `vtty_buffer.c`, `vtty_csi*` modules.
- Color cache/math, keymaps, mouse drivers.
- `demo/`, tests (`.suite`, `.reference`).
- `TESTING.md`.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- libvtty is C VT library; potential reuse in Rust via FFI but license LGPL plus VT divergence (Ghostty single-source issue).
- Not recommended direct use.

## 可直接使用的库/工具
- C library link Android but license LGPL; API useful.
- 本报告只总结仓库 README/Grep 未给 VT 覆盖矩阵；VT 支持情况需按源码确认。Torvox 以 Ghostty 为唯一 VT 来源。

## 可间接借鉴
- Keymap definitions (vt100, rxvt, xterm).
- Color caching and math (RGB approximation for ANSI 256 etc.).
- Mouse driver implementations.
- CSI extension layer 为独立模块；测试 framework 使用参考输出做 black-box 对照（具体 harness 未逐条展开）。

## 可稍微修改使用的代码/设计
- C source can be compiled into Android NDK but FFI overhead/complexity.
- Potential borrow C keymap, not core VT.

## 测试数据/测试用例
- Tests 与参考输出位于 `test/*.suite/*.reference`；运行方式与版本未在本报告中逐项列出。
- TESTING.md. Valuable if adapting VT tests but partial.

## 设计模式
- Terminal library separated from UI.
- Modular CSI command structs.
- Color cache performance.

## 其他结论
- LGPL core 若直接链接到 Android bundle，将触发 LGPL 分发/链接 obligations，因此不可直接收入 Torvox；另因 Ghostty 单一来源，也不采用。

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
