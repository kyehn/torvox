# ii8/havoc 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/havoc`
- 技术栈：C + Wayland, uses `tsm` (KMScon terminal state machine library in `tsm/`)
- 定位：minimal Wayland terminal.
- 许可证：MIT
- 主要模块：main.c, glyph.c, ptyutil.h, tsm local copy.

## 全仓库结构与关键代码
- Repository contains a vendored libtsm/terminal state machine; ownership was not rewritten because the original dependency remains visible.
- Small codebase; Wayland shm (may be CPU).
- Terminal emulation externalized to libtsm.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- Havoc irrelevant as product, but corroborates libtsm desktop ecosystem.

## 可直接使用的库/工具
- libtsm (local copy) not recommended.

## 可间接借鉴
- Minimal terminal architecture and keybinding.

## 可稍微修改使用的代码/设计
- None.

## 测试数据/测试用例
- None.

## 设计模式
- Terminal lib + renderer.

## 其他结论
- Desktop C-grade. Not direct.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
