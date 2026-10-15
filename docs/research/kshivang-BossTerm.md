# kshivang/BossTerm 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/boss-term`
- 技术栈：Kotlin Multiplatform + Android (`bossterm-core-mpp`), JetBrains jEditerm derivative; Compose Desktop UI (`compose-ui`).
- 定位：Embeddable Kotlin terminal emulator, desktop app + core module publishable Maven Central.
- 许可证：reported as LGPL 3.0 core plus Apache-licensed additional modules; exact module boundary must be read from source tree.
- Modules: core-mpp (JVM/Android), compose-ui, app, examples.

## 全仓库结构与关键代码
- `bossterm-core-mpp` gives terminal emulator for Android/Kotlin.
- Core modules include `TerminalStarter` / `ScreenHistoryStorage` placeholders as reported in repository listing; exact paths were not exhaustively transcribed.
- Compose UI (`compose-ui`) renders terminal.
- Grapheme ICU4J.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- BossTerm core exposes a Kotlin terminal engine and Compose widgets and can run without JNI/Rust; nonetheless its VT core is JetBrains-derived and incompatible with Torvox's required Ghostty single-source. Whether it supports every modern Ghostty sequence is outside this report's scope and requires conformance comparison.
- Candidate for quick Kotlin fallback or reference terminal widget integration with standard Compose.
- LGPL 3.0 imposes Android redistribution and source availability obligations, so direct adoption requires license review; Ghostty stays Torvox's primary VT source.

## 可直接使用的库/工具
- Maven module com.risaboss:bossterm-core integrates into Android as a Kotlin/JVM dependency; its use would replace Torvox's Ghostty VT path, which is not permitted by project constraint.
- `compose-ui` terminal widgets target Kotlin/JVM desktop execution; adapting them to Android Compose requires changes not verified here.
- Test data inventory was not expanded in this report because clone is not bundled with the target app build validation.

## 可间接借鉴
- Input handling (keyboard/IME), screen model API.
- Grapheme segmentation ICU4J, split-width issues.
- JNI-free architecture (direct Kotlin PTY because Java/kotlin can call `android.system.Os`?).
- Terminal session/tab architecture and sharing.

## 可稍微修改使用的代码/设计
- Can embed as Android dependency, but VT is JetBrains-derived, not Ghostty. To keep B single-source, must choose either Ghostty or BossTerm; cannot bridge.
- Feature gap cause telnet/ssh mismatch.
- Performance lower than Rust/wgpu but simpler.

## 测试数据/测试用例
- The available source listing does not enumerate Bossterm test methods; test coverage claims come from repository path names only.
- use as additional oracle, but license.

## 设计模式
- Compose terminal widget; it's directly relevant to Compose UI and state.
- Embedded application patterns good.

## 其他结论
- Very high applicability on Android UI/IME/split architecture, but VT core license/quality tradeoff. Primary should remain Ghostty; if Ghostty integration fails, BossTerm is fallback.
- GPL/LGPL LGPL only core.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
- First observed core module supports `minSdk=24`.
