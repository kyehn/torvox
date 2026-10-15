# mirbehnam/RtlTerminal 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/rtl-terminal`
- 技术栈：C# / WPF (Windows) with ConPTY and custom `TerminalBuffer` / `TerminalView`
- 定位：Right-to-left-aware terminal, WPF window.
- 许可证：MIT（根 LICENSE 明示）。
- 主要 modules: `ConPtySession`, `TerminalBuffer`, `SmartRtl`, font/window settings.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- Win-only WPF; but RTL/bidirectional research relevant for Android UI, especially complex text layout.
- VT is custom TerminalBuffer, not Ghostty.

## 可直接使用的库/工具
- None.

## 可间接借鉴
- ConPty session management.
- RTL text shaping priorities.
- Compose terminal key settings.

## 可稍微修改使用的代码/设计
- C# not Android.

## 测试数据/测试用例
- tests directory; unknown.

## 设计模式
- Direct WPF view, ConPTY.

## 其他结论
- Useful RTL context (Arabic/Hebrew) as a feature; Compose provides RTL support.
- Not direct.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
