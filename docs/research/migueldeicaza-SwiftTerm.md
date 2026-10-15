# migueldeicaza/SwiftTerm 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/swiftterm`
- 技术栈：Swift 6.2 + SwiftPM；Apple/macOS/iOS/Linux/Wasm 历史；可选 Embedded/WASM traits；含 C? Swift only
- 定位：Swift 终端模拟器库（VT 解析、终端模型、渲染辅助、剪贴板/图形）；不依赖 libghostty
- 许可证：MIT (LICENSE)
- 主要模块：SwiftTerm (Terminal, Buffer, CellStorage, CSI/Escape parser, Graphics, Search, Selection, Kitty, Sixel?); Tests 175 个测试文件
- 目标：Apple 为主，portable graphics trait 支持 Android/Linux/WASM

## 全仓库结构与关键代码
- `Sources/SwiftTerm/*.swift`：Terminal, Buffer/Line/Position, EscapeSequenceParser, CSI parser, CellStorage, CharData, Colors, KittyGraphics, SixelDcsHandler, MouseInput, Selection, Search.
- `Portable/`、`Apple/`、`Mac/`、`iOS/`：平台适配。
- `Tests/SwiftTermTests`：大量终端行为、CSI/DCS、Kitty、buffer、search、selection、bidi、fuzz regression tests.
- `Package.swift` 支持 traits (Embedded/PortableGraphics/Wasm), hosted cross-platform.
- `GhosttyFuzzCorpusTests`、`GhosttyFuzzRegressionTests`：与 Ghostty 的 fuzz 语料回归对比。

## 对比本项目
- 本项目 Android Rust + wgpu/Vulkan + Compose, VT by Ghostty.
- SwiftTerm 自研 VT 与终端模型，与“终端状态单一来源”冲突；不能 adopt.
- Swift 生态/ABI 与 Kotlin JNI 不兼容；Apple-centric; Android Swift runtime 不适合作为本仓库。
- Tests and conformance data high value.

## 可直接使用的库/工具
- 无 Android/Kotlin 依赖可直接使用。
- SwiftTerm 作为库仅 Apple/Swift 原生产品；Android 需 JNI/NDK interop 不值得。

## 可间接借鉴
- Escape sequence parser and CSI parameter parsing design.
- Kitty graphics handling, clipboard, keyboard encoding.
- Search/selection algorithms.
- Test matrices and fuzz corpus regression methodology.

## 可稍微修改使用的代码/设计
- Swift test corpus cannot directly run on JVM/Rust, but can inspire Rust integration tests; need convert and ensure expected values external truths.
- Ghostty fuzz corpus provides upstream fuzz material; adoption criteria require validation by TorVox independent of repository self-check tests.

## 测试数据/测试用例
- 175 Swift test files covering parser, buffer, kitty, DCS, mouse, search, selection, colors, bidi, fuzzer, Ghostty corpus regressions.
- EmbeddedCore tests for minimal build.
- High value reference for expected behavior, but needs porting and expected-truth audit.

## 设计模式
- self-contained terminal core with platform adapters
- service objects (SearchService, SelectionService)
- locks/queue for I/O
- trait-based portability via SwiftPM

## 其他结论
- SwiftTerm 是高质量自研终端库; but not adopt for本仓 due platform and single-source conflict.
- Its Ghostty fuzz corpus comparison may be especially relevant to verify upstream alignment.

## 四次审阅记录
- 准确性：基于 Package.swift and Sources/Tests.
- 完整性：涵盖仓库、架构、对比、库、借鉴、修改、测试、模式.
- 对比正确性：强调 Swift/Apple and VT single-source conflict.
- 文档一致性：仅 md.
