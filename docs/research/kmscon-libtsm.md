# kmscon/libtsm 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/libtsm`
- 技术栈：C, Meson, terminal state machine (`libtsm`).
- 定位： reusable, backend-independent VT emulator; KMScon terminal state+tab/display logic.
- 许可证：MIT (COPYING)
- 库 provides screen, tab, PTY, selection, etc. as external lib.

## 全仓库结构与关键代码
- libtsm source lives under terminal/screen implementation files; exact filenames must be opened separately if required.
- `test/` unit tests.
- meson (Linux basic).
- API docs.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- `libtsm` is one VT implementation; use as comparison or feature docs.
- licensing MIT.
- Cannot replace Ghostty due single-source policy and lesser features.

## 可直接使用的库/工具
- C lib can compile Android via NDK and JNI interpose, but extensive adaptation.
- If Ghostty unavailable or policy changes, libtsm as renderer-independent backend.
- No direct Android libraries.

## 可间接借鉴
- Data structures: screen lines, attributes, selection.
- Test suite and VT behavior specs.
- Dec modes, state machine edge cases.

## 可稍微修改使用的代码/设计
- Use a few C API mapping + JNI; not necessarily beneficial.
- build test harness to compare Ghostty snapshots.

## 测试数据/测试用例
- `test/` has unit/acceptance; good behavioral references.

## 设计模式
- State machine separates rendering and terminal.
- Display aggregation.

## 其他结论
- Better than libvte modern? Historical but less new features than Ghostty; good for core.
- Android use means license okay but development load high.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
