# pg83/shitty 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/shitty`
- 技术栈：C++ (lib/vterm + terminal app), Vulkan compute (Linux), Metal (macOS), Python build system, no generic windowing toolkit; fonts embedded
- 定位：极快桌面终端，黑盒测试 >5000 cases; self-contained binary, MIT/GPL dual license
- VT: self-developed full parser/state machine (lib/vterm), not Ghostty.
- Features: advanced VT52-VT5xx, Unicode 17 graphemes, Sixel, Kitty keyboard, OSC, mouse, history reflow, themes, PTY tabs.

## 全仓库结构与关键代码
- `lib/vterm`：parser, screen, cell extra store, unicode width, test harness (`vt_test.h`, `vt_headless.cpp`).
- `lib/shitty`：app/render (Vulkan/Metal), font/resolver/glyph cache, composer, input router, PTY, sessions.
- `tst/`：black-box integration test harness and corpus from kitty, esctest, xterm vttests, vttest, tack, libvterm, libtsm, alacritty, ghostty, contour, konsole, mosh.
- `build.py`, `ext/`: embedded fonts and Unicode data.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- Shitty uses self-dev VTerm and Vulkan VK on Linux, Metal on macOS; not Android.
- black-box test corpora and rendering architecture highly useful; no direct dependency.
- license MIT/GPL.

## 可直接使用的库/工具
- None for Android (C++ and custom build).
- `lib/vterm` internal; not public library; license unclear? MIT main but lib/vterm may contain BSD? Need inspect.
- Build Python-based generator inform but policy.

## 可间接借鉴
- Damage-driven GPU rendering and resize transaction.
- Black-box test methodology (PTY socket, snapshots) ideal for Torvox VT regression.
- Harvested corpus list as external test assets.
- Font fallback and glyph cache approaches.
- Roundtrip test partition infrastructure.

## 可稍微修改使用的代码/设计
- C++ parser cannot be adopted; violating Ghostty single-source.
- Black-box corpus be run if externally obtainable; expected outputs need audit.

## 测试数据/测试用例
- `tst/DIFF.md`, `tst/COVERAGE.md`; esctest/xterm vttest/corpus; extensive >5000 cases.
- Black-box PTY socket protocol; not use directly but informs test design.

## 设计模式
- separation parser / screen / host; damage-driven render.
- unit + integration test harness.
- self-contained binary.

## 其他结论
- Most valuable as VT correctness corpus reference and rendering performance benchmark.
- Self-dev VT conflicts single-source policy; no adoption.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
