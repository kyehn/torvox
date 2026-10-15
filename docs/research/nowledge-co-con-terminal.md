# nnowledge-co/con-terminal 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/con-terminal`
- 技术栈：Rust + GPUI (Zed fork) + `con-ghostty` crate; `con-ghostty` 调用 Ghostty 的嵌入 C API（full libghostty, Metal/Windows/Linux renderer）, macOS/Windows/Linux beta
- 定位：AI harness terminal, native fast terminal-first, built-in agent panel
- 许可证：MIT
- 工作区：con-app, con-core, con-terminal, con-ghostty, con-agent, con-process, con-cli, con-paths
- VT emul: Ghostty full (not only libghostty-vt) via FFI; config patch elaborate

## 全仓库结构与关键代码
- `crates/con-ghostty/src/terminal.rs`：Safe wrapper around Ghostty embedded C API, creates native views (Metal/Windows render); config merge, appearance readback.
- `crates/con-ghostty/src/ffi.rs`/`ffi_abi.c`：FFI ABI.
- `con-terminal/src/*`：Terminal model/theme integration.
- `con-app` GPUI UI and agent integration.
- `con-process`, `con-cli`.
- Windows backend includes atlas/render/pipeline/conpty; Linux/macOS backends.
- Tests in crates (con-test?).

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + `libghostty-vt` only.
- con uses **full libghostty** (including renderer), not Android (macOS/Windows/Linux only). It bypasses自研 WGPU and Android compose.
- Android is not supported; Ghostty full C ABI on Android not mature.
- For Torvox, using full Ghostty conflict with Android/GPU but demonstrates config-driven integration.

## 可直接使用的库/工具
- Rust crates `con-ghostty` is not a portable library (tight app coupling).
- Full Ghostty rendering may be unavailable on Android; no direct adoption.
- Config patch/terminal colors patterns useful in Kotlin; but not as Android asset.

## 可间接借鉴
- Ghostty config generation and host-policy merge (colors, font, opacity, blur, clipboard policy).
- safe wrapper patterns around full Ghostty API (once/tick, app callbacks).
- native process/PTY integration per platform (Windows ConPTY, POSIX).
- theme/appearance readback.

## 可稍微修改使用的代码/设计
- `con-ghostty` is Rust integration of full Ghostty; cannot serve Android JNI/Compose due platform dependencies.
- Config string patch and sanitized font family design can be adapted to Torvox font theme settings (but theme fixed).
- Windows ConPTY code may inform future Windows host, not Android.

## 测试数据/测试用例
- Tests directory and smoke/integration scripts have not been fully enumerated in this report.
- `con-test` crate appears to contain conformance/terminal harness tests; invocation details not re-run.

## 设计模式
- FFI safe wrapper with config patches.
- multi-platform backend modules.
- app-integrated terminal objects.

## 其他结论
- High-value Rust example of Ghostty full API use, but platform/API scope excludes Android. Persistent ecosystem rated web/WASM/desktop C; con fits desktop Rust full-Ghostty (not libghostty-vt only). Useful compare if Torvox considers full Ghostty desktop.
- GPL? MIT so more permissive; but upstream dependency Ghostty's license unknown here.

## 四次审阅记录
- 准确性: Cargo + terminal.rs.
- 完整性.
- 对比正确性: Android excluded noted.
- 文档一致性.
