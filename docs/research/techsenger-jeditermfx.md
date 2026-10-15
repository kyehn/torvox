# techsenger/jeditermfx 研究报告

## 仓库概况
- 源克隆：`/tmp/opencode/jeditermfx`
- 技术栈：JavaFX + Maven 多模块：`jeditermfx-core`, `jeditermfx-ui`, `jeditermfx-app`
- 定位：JetBrains JediTerm (port of Swing terminal)移植到 JavaFX，支持 Linux/macOS/Windows PTY (Pty4J), Unix terminals
- 许可证：Apache-2.0 and LGPL-3
- Features: xterm emulation (passed much vttest), 256 colors, scrolling, copy/paste, mouse, tabs, hyperlinks.
- Dependencies: pty4j 0.12.25, JetBrains core import, JavaFX.

## Code reading highlights
- `jeditermfx-core`: `Terminal`, `DataStreamIteratingEmulator`, `TerminalTextBuffer`, charset/mouse/prompt/typeahead packages.
- Emulator/mouse handling and text buffer width changes.
- Tests: 23 unit tests in core, plus extensive golden test data:
  - before/after opaque screens
  - vttest `Test2_Screen`, `Test3_Characters` drift tests.
- UI module renders and user settings, dark theme API `getDefaultForeground/getDefaultBackground`.
- Documentation compares JediTerm vs TerminalFX.

## Directly usable libraries/tools
- `jeditermfx-core` depends on Java/Jediterm and can be a JVM dependency only if Android runtime compatibility is proven; UI modue is JavaFX but core separation needs separate SDK validation.
- pty4j is JVM PTY library, transitively via JavaFX (be useful for JVM/desktop, not Android JNI).
- Golden VT test data can become Torvox corpus candidates only after license and representation review.

## Indirectly useful
- Emulator/buffer interface designs (`Terminal`, `DataStreamIteratingEmulator`) good for JNI/VM boundaries.
- Mouse hyperlink classes and typeahead design patterns.
- Test orchestration with plain input/grid state before/after can build regression harness.
- Dark theme defaults and UI patterns for Compose not directly.

## Slightly modifiable
- `jeditermfx-core` VT processing implementation be extracted/repackaged to JVM test bed; not direct native Rust.
- Java Golden fixtures can be wrapped as Kotlin resources and compare screen state via libghostty-vt APIs.
- UI (JavaFX) irrelevant to Android Compose; use ideas for tabs/selection/hyperlinks only.

## Test data / test cases
- core has around 23 Java unit tests and huge `src/test/resources/testData` (before.txt/after.txt pairs).
- vttest screen and character test data; good corpus.
- these can become cross-references for torvox test suite, though torvox's Ghostty VT is reference and doesn't need JediTerm exact.
- no Android instrumentation tests in repo.

## Design patterns
- Emulator stream pattern data stream -> emulator command dispatch -> text buffer updates.
- Buffer separate from rendering.
- UserSettingsProvider overrides.
- Hyperlink highlighting modes enum (ALWAYS/HOVER/NEVER) UI accessibility patterns.
- Typeahead package for latency reduction (important Android input).
- Java module JPMS.

## Compare torvox
- torvox is Kotlin/Compose Android uses libghostty-vt; jeditermfx is Java module based xterm emulator + JavaFX UI.
- Java core serve as JVM emulator surrogate but Android demands JNI/shared Rust easier e.g. portable-pty.
- VT semantics differ (Ghostty vs JediTerm); not use direct.
- Visual test data can supplement.
- Common AWT/JavaFX dependencies not target.
- Portability of test fixtures to Kotlin is straightforward through gradle test resources.

## Conclusion
- Not a direct library for Android, but provides valuable VT fixtures/design, especially typeahead architecture and screen before/after golden tests.
- Evaluate Kotlin JVM tests importing golden data via resource paths.

## 四次审阅记录
- 准确性：README, pom, module layout, 23 core tests, resources golden data.
- 完整性：覆盖 core/UI/app/tests data; not every source.
- 对比正确性： clarifies JVM not Android, Ghostty vs xterm.
- 文档一致性： filename correct.
