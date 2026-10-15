# JetBrains/jediterm 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/jediterm`
- 技术栈：Java/Kotlin + Gradle + Swing UI，纯 Java 终端模拟器库 (emulator + ui)
- 定位：可嵌入 IDE 的 terminal widget，支持 SSH 与 local PTY (Pty4J)
- 许可证：Apache-2.0 或 LGPLv3（二选一）
- 主要模块：`core` (emulator/model/key/mouse), `ui` (Swing terminal panel), `JediTerm` standalone
- 支持 vttest 大部分用例，xterm emulation

## 全仓库结构与关键代码
- `core/src/com/jediterm/terminal/emulator/*`：VT 解析、CSI/OSC/DCS、鼠标、键盘、同步输出。
- `core/src/com/jediterm/terminal/model/*`：buffer lines, storage, limited size, hyperlinks.
- tests resources: extensive vttest before/after files (`core/tests/resources/testData/vttest`).
- ui: Swing terminal display/paint/input.
- pty: local pty integration via Pty4J.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- JediTerm self-developed VT; conflict with single-source.
- JVM/Swing; Android integration would require embedding a Java library and rendering outside Compose/WGPU; the repository provides no Android target, so Jediterm is documented as a VT baseline only.
- vttest resources can inform integration tests.

## 可直接使用的库/工具
- Android 可使用 Java 库，但 VT emulation self-dev conflicts; also performance/UI mismatch.
- `jediterm-pty`/Pty4J is a JVM PTY abstraction and is not an Android native PTY backend because process spawning and PTY APIs differ.
- No direct renderer.

## 可间接借鉴
- vttest expected-output before/after data as conformance reference.
- terminal model resize, hyperlinks, synchronized output, mouse encoding, keyboard encoding.
- Type-ahead manager design.

## 可稍微修改使用的代码/设计
- Java emulator cannot be ported to Rust without rewrite; port vttest data (license caveat).
- Components may serve as Kotlin-side test reference for expected ANSI behavior, but this risk self-validation; must audit expected truth.

## 测试数据/测试用例
- 81 tests, spanning terminal text buffer, scrolling, modes, synchronized output, rendering surrogate, transfer to alternate buffer, links.
- vttest before/after files; high conformance value if expected truths external.
- Need run as reference (not vendoring).

## 设计模式
- emulator with terminal model/lines storage
- data stream driving interface
- widget and rendering separation
- pty connector abstraction.

## 其他结论
- Apache/LGPL dual license friendlier than GPL, but自研 VT conflicts.
- Useful as reference for test expectations and emulator features; not as dependency for本仓.

## 四次审阅记录
- 准确性：基于 source/tests.
- 完整性：涵盖 repo, compare, libs, indirect, tests.
- 对比正确性: vt single-source conflict noted.
- 文档一致性: only md.
