# ketraterm/KetraTerm 研究报告

## 仓库概况
- 源克隆：`/tmp/opencode/ketraterm`
- 技术栈：Kotlin/JVM 25 (`Kotlin 2.4+`) Gradle 多模块； parser/core/session/render/host/input/pty/transport/ui; JMH; Swing 桌面 app + IntelliJ plugin.
- 定位：Kotlin/Java 终端库 + 桌面 product; headless session/custom transport; 适用于 JV VM,  not Android.
- 许可证：Apache-2.0.
- Modules: protocol parser core host input render-api render-cache transport-api session shell-integration pty completion * plus products.
- Docs: architecture, feature/gap maps, embedding, rendering, concurrency docs.
- Tests significantly rich: 402 test `.kt` + testkit + JMH benchmarks + conformance tool xterm oracle.

## Code reading highlights
- `ARCHITECTURE.md`: lifecycle and concurrency model: caller serializes parser/core; session synchronization; outbound ordered writer; conflated render worker and leased copied frame; Swing owns UI.
- Parser + core separated, host-command adaptation, Render API frames, transport API raw bytes.
- Storage uses parallel primitive arrays and cluster arena; incremental scrollback rows recycled; core owns reflow.
- Swing renderer reuses copied cell planes and layout caches; font selection Java2D.
- Features: Unicode clusters/marks/width/emoji; scrollback resize reflow; wrap-aware selection; 256 true color; sync output; Kitty keyboard, bracketed paste, pixel mouse; OSC hyperlinks; async search; shell integration; command completion/ranking/learning; host-controlled permissions; PTY via Pty4J; headless sessions/custom transports.
- Conformance testing docs and xterm oracle external tool.

## Directly usable library/tools
- `ketraterm-kotlin` 是 Kotlin/JVM 模块，编译到 JDK 25/Kotlin 2.4 bytecode 并依赖 Swing/JVM tooling；报告未发现 Android Runtime 目标或 APK 入口，因此不能直接进入 Torvox Android runtime。
- Parser and command vocabulary definitions may still inform JNI/Kotlin parser wrappers if core Scala APIs ported, but no good.
- Render lease/Frame lifecycle design immediately reusable in torvox's RenderDriver to avoid stale frame retention.
- JMH benchmark suite and xterm oracle can define performance and terminal baseline test.
- Testkit provides connector fakes for session tests.
- Kil xterm oracle differential-testing methodology valuable.
- Docs on Unicode grapheme storage and reflow provide insight.

## Indirect use
- API boundary patterns (Transport-API, session coordination) to align Rust <-> Kotlin JNI boundary and avoid shared process state.
- Storage layout docs define primitive arrays; Rust backend should reuse.
- Concurrency contract (render leased frame callback) as torvox invariant.
- Host-controlled permissions and action policy architecture transfer naturally to Compose UI because torvox separates keyboard input, clipping/alternatives.
- Shell integration metadata producers; torvox can later support OSC cwd/command status in a fragmentary manner.
- Input encoder architecture supports mode-aware key sequences; can implement Android IME robustly.
- Knowledge on shell integration, Kitty keyboard, releases forms basis for roadmap.

## Slightly modifiable
- Core/Parser modules don't directly compile to Android; vendor their unit tests as golden coverage.
- JMH scenario metrics should be translated to Kotlin benchmark wrapper if needed, but Android runtime difference.
- `docs/development/conformance-testing.md` 提供特定命令，本报告只记录存在，未重新运行。
- `tools/xterm-oracle` 是基于 xterm reference behavior 与 golden text 的 differential checker；复用需要改写为 Torvox JNI/terminal API 调用。
- BOM dependency model not suitable for Android.

## Tests/test data
- 402 test files, testkit, connector fakes.
- JMH benchmarks around 30 hot paths.
- `tools/xterm-oracle` 目录提供 oracle campaign 与 runner 脚本；未重新执行。
- `docs/development/conformance-testing.md` details.
- No source VT golden fixtures mentioned; exact outbound byte assertions.
- borrow scenarios names for torvox integration tests once JNI boundary stable.

## Design patterns
- Clean module layering: protocol, parser, core, host, input, render, transport, session.
- Unidirectional flow connector -> parser -> host -> core; render frame lease.
- Session is synchronization boundary; conflated render worker; composition over runtime.
- Primitive custom cell arrays rather than object-heavy data structures.
- Host-owned resource policies; clipboard permission checks.
- Completion sources with rank/learning.

## Compare torvox
- KetraTerm is extensive JVM library unlike Android native. But architecture (modular VT engine) provides patterns for Rust -> Kotlin.
- Torvox uses Ghostty VT and wgpu rendering; KetraTerm Java2D Swing rendering irrelevant.
- Direct usage not fundable due JVM 25 and desktop UI; but protocol/concurrency design references.
- Strong Android adaptation candidates: screen storage layout, input encoder, session boundary, shell command contracts, render lease.
- Potential future requirement source docs before matching API names if interoperating.
- Must not import Ghostty-incompatible behavior; KetraTerm independent VT implementation can differ edge cases.

## Conclusion
- Largest direct-product catalog in this Kotlin terminal world; only conceptually usable for Android due platform API/runtime mismatch.
- High value for VT tests, docs, architecture invariants.
- Be selective: no code integration.

## 四次审阅记录
- 准确性: README, ARCHITECTURE, feature map, module inventories.
- 完整性: README core; don't enumerate all docs/tests.
- 对比正确性: Java 25/Kotlin 2.4 and Swing incompatibility explicit.
- 文档一致性: slug correct.
