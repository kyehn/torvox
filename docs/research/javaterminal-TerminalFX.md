# javaterminal/TerminalFX 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/terminalfx`
- 技术栈：Java + JavaFX + Maven multi-module, java 25, javafx 26
- 定位：JavaFX terminal widget, core (screen/session contract) and local (PTY) separation
- 许可证：MIT
- 模块：core, local, example; depends pty4j, JNA etc.

## 全仓库结构与关键代码
- core: Terminal screen/session abstr. local: shell PTY backend.
- example usage.
- JavaFX rendering UI.

## 对比本项目
- 本项目 Android Rust+wgpu+Compose + Ghostty VT.
- TerminalFX is not an Android renderer; its terminal emulation is self-contained Java and does not integrate with Torvox VM immediately.
- Java MIT but still desktop JVM and performance limits.

## 可直接使用的库/工具
- core abstraction conceptually embed in Android via RoboVM/none; not recommended.
- JavaFX not Android.

## 可间接借鉴
- 分离 core screen/session vs pty backend architecture (clean decoupling).
- API design (no pty dependency in core).
- MIT license friendly.

## 可稍微修改使用的代码/设计
- Java terminal not portable; JavaFX absent Android.

## 测试数据/测试用例
- core tests; limited VT compliance.

## 设计模式
- Modular core/local split.
- JavaFX controls.

## 其他结论
- Not applicable as direct dependency.
- Modular design lesson (UI vs process) aligns Torvox JNI boundary.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档一致性.
