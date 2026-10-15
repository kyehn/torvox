# benyblack/ntilde 研究报告

## 仓库概况
- 源克隆：`/tmp/opencode/ntilde`
- 技术栈：.NET 10 + Avalonia UI + Skia GPU rendering + Rust PTY backend; Windows/Linux/macOS.
- 定位：Ntilde (renamed from NovaTerminal), modern terminal emulator and extension platform.
- 许可证：MIT（仓库根 LICENSE 明示 MIT，copyright 2026 Behnam Yousefi）。
- Architecture modules: Rendering, Pty, App, Mux, Conformance, McpServer, CommandAssist, VtContract.
- Features: deterministic rendering, replay-driven debugging, persistent sessions, Skia GPU rendering, command assist, screen integration, SSH tabs.
- Tests: MCP, mux, VT tests design, test matrices; release release gated unit tests.

## Code reading highlights
- `src/Ntilde.Rendering`: GlyphAtlas, SharedSKFont/Typeface, GlyphCache, RowCache , SixelDecoder, RenderPerfMetrics, RenderPerfWriter.
- `src/Ntilde.Pty`: ConPtyNative, RustPtySession, session serialization, output interfaces.
- `src/Ntilde.App/native`：Rust FFI crate，源文件声明包含 SSH；PTY 接入状态未逐文件展开。
- `src/Ntilde.Conformance`: VT conformance report tool parses feature matrices and validates capability catalog; compares stored JSON report.
- `tests/Ntilde.VT.Tests`: VT tests.
- `docs/vt_coverage_matrix.md` and `docs/vt_ghostty_gap_matrix.md` maps VT coverage and Ghostty-specific gaps.
- Release tooling and MCP server exposes live sessions to AI agents (instinct scripts).
- Persistent shell-integration files and mux daemon.

## Directly usable
- Ntilde .NET/Avalonia 不能直接用于 Android；.NET 运行库不是本项目目标原生运行时；Rust PTY/BSON FFI 仅具局部可借鉴性，不再作为 VT 来源，因为 Ghostty VT 已覆盖 VT 语义。
- Conformance documentation and matrix tools can be adapted to torvox test PR by matching `VtContract` `docs/vt_ghostty_gap_matrix.md`.
- Ghostty gap matrix directly focuses on Ntilde difference from Ghostty, valuable for torvox expected behavior.
- vt conformance report implementation offers deterministic release gating strategy, useful.
- Shell integration and persistent session behavior patterns may inform Kotlin service.

## Indirectly useful
- Deterministic rendering and replay debug concepts match torvox need for rendering tests:
  - snapshot/render frame inputs and golden output
  - pixel/row comparison
  - replay scripts.
- Glyph cache font fallback layout and row cache offers reference for Blur/scanline/raster batch code.
- Rust PTY FFI structure shows source ordering and safe JNI ABI.
- Command assist provider stack as deferred future.
- MCP AI agent integration marks roadmap if project later wants terminal agent control.
- Persistent mutex sessions with serialization influence service lifecycle.
- Performance metrics output classes can guide RenderPerf within torvox Compose.

## Slightly modifiable
- Rust PTY backend only any FFI extraction not across Kotlin JNI due Android Linux and `portable-pty` availability.
- Skia rendering mode not wgpu.
- VT conformance tool and matrices can be translated into Gradle tests if manually adopted.
- 
- Tests may remain local only.
- User can port Ntilde's test corpora if license.

## Tests / test data
- `.NET VT tests` (in `tests/Ntilde.VT.Tests`)
- MCP server tests (configuration, tool behavior)
- mux daemon tests (`MuxPaths`, daemon process, console text renderer)
- Ntilde.VT.Tests 与 Conformance 相关路径在 clone 中可见；断言细节未展开。
- 测试数据位于仓库 docs/ 与 tests/ 中；具体场景与矩阵条目按 `docs/vt_coverage_matrix.md` 定义，未逐条转写。
- Row/Sixel/render perf 的测试名称与模块路径已核对；测试用例清单未逐条转写。
- Deterministic setup and regression matrix.

## Design patterns
- Clean modules by concern (rendering, pty, mux, conformance, mcp).
- Rust FFI PTY + screen API bound into .NET native.
- Contract enum (`Ntilde.VtContract`) and central capability catalog.
- Conformance report generated at "build" time and compared in CI to prevent drift.
- Daemon separation with mux Unix/NamedPipe and persistence.
- Command assist "providers / storage / ViewModels" layering.
- AI agent control using MCP only when opted in.
- Rendering performance metrics exported externally.

## Compare torvox
- Ntilde uses Skia +.NET while torvox uses Kotlin+Compose + wgpu/Vulkan; no direct replacement.
- Voice VT semantics grounded in conformance and gap against Ghostty; hence most relevant to code architecture even when not building on it.
- Rust PTY approach parallels Android NDK but mobile environment definitely Linux bionic and sandbox restrictions.
- Persistent/mux architecture has more complexity than Android app requirements.
- Ntilde corpora can settle torvox choice details especially where libghostty-vt differs expected.
- Ntilde Ghostty gap matrix may explicitly tell torvox when behavior should preserve host expectation vs potential power feature.
- 本报告只新增 docs/research 文件；未改动被研究仓库或本项目实现代码。

## Conclusion
- Most relevant to test/correction/gap matrix rather than production code.
- Documentation/reference should avoid importing whole .NET platform.
- Suggest studying gap and coverage matrix in detail if future exact matching target.

## 四次审阅记录
- 准确性: source module paths, PTY Rust, conformance docs loaded.
- 完整性: read main concerns but not code every module.
- 对比正确性: Android/native constraints stated.
- 文档 consistency: slug correct.
