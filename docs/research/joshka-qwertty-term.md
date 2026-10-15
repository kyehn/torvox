# joshka/qwertty-term 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/qwertty`
- 技术栈：Rust workspace, VT engine rewritten from Ghostty; Metal renderer (macOS); PTY via rustix.
- 定位：Rust rewrite of Ghostty backend + terminal stack.
- 许可证：MIT (porting upstream Ghostty semantics; attribution)
- Subcrate `qwertty-term-vt`, font, renderer, termio.

## 全仓库结构与关键代码
- `crates/qwertty-term-vt`: page-based scrollback, refcount styles, Kitty graphics and keyboard, differential test corpus.
- `crates/vt-diff`: 176-case differential comparison vs libghostty-vt; throughput and fuzz tests.
- `crates/qwertty-term-font`: CoreText/shaping/sprite glyphs.
- `crates/qwertty-term-renderer`: Metal IOSurface.
- `examples/frame-capture`: deterministic bytes->PNG.
- Docs and analysis.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- This project directly demonstrates embedding Ghostty protocol semantics as Rust (100% Rust integration), extremely relevant. It bypasses external C dependency/JNI and pre-provides embeddable APIs.
- MIT supports inclusion or derived tests.

## 可直接使用的库/工具
- `qwertty-term-vt` Rust crate compile for Android and replace `libghostty-vt` if cross-compilation feasibility and binary size permit. But needs assessment feature integration; UI (Compose) receives pages.
- Differential test corpus (176-case) and fuzz tests are directly valuable for Torvox VT correctness.
- Embeddability design (`examples/frame-capture`) useful Rust core integration.

## 可间接借鉴
- Differential/fuzz methodology vs Ghostty as correctness oracle.
- Refcount styles and page-based scrollback.
- Kitty graphics implementation patterns.
- Native Rust PTY and embedding APIs.
- Rendering run/dirty-proof.

## 可稍微修改使用的代码/设计
- Rust VT core can be incorporated but license ethics: upstream Ghostty code carries MIT and parser implementation. They cite attribution.
- Metal renderer not Android; rendering replaced with wgpu/Compose.
- Shell env tweaks (RUSTIX PTY).
- `qwertty-term` is an independent keyboard terminal prototype; Ghostty remains the sole TorVox VT source and no binding conflict is assessed.

## 测试数据/测试用例
- differential corpus (`vt-diff/tests`) with throughput and fuzz; available.
- miri subset.
- Directly useful to validate Torvox Ghostty core.

## 设计模式
- VT logic fully Rustified and embeddable by API.
- Deterministic frame capture.
- Differential oracle.

## 其他结论
- Highest value repo so far for Android integration possibility. Investigate crate compatibility and size.
- Still Ghostty semantics, no license issue if attribution retained.
- Integration effort high due page model, but payoff high for a pure Rust Android port (no C FFI).

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
