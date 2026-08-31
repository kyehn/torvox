## Why

基于 `docs/reference/` 26 个参考项目的深度研究（zelland、ghostty-android、termlib、termux、Haven、warp、shashlik、zed-port、wgpu-in-app 等 14886 行研究）与全库扫描，torvox 在 mouse/无障碍/OSC133/CellRun 等 4 项 P0/P1 能力已部分落地但存在语义缺口、测试缺口与工程债；同时 PTY winsize 竞态、Bootstrap sha 校验、MCP 参逃逸与越权、行级脏缓存等 5 项细节硬化未闭环。需在不新增重抽象前提下，保守、可验证地完成全量硬化，使模拟器 90fps+、后端确定性 1016+ 测例零 flaky、UI 可自动化验证。

## What Changes

- **硬化 4 项已落地能力**（行为不变，补测试与文档）：`mouse-encoding`（SGR/1006 门控 + 实时 cell 尺寸 + 拖拽/滚轮全覆盖）、`a11y-overlay`（visibleLines/LineNavigator 包裹 + 500ms debounce）、`osc133-semantic`（ST/BEL 双终结 + 跨 chunk + A 重置 + D;exit_code）、`cell-run-cache`（同格式游程合并）
- **新增 5 项细节硬化**：`terminal-winsize-sync`（spawn 前预计算 pixel 尺寸同步 winsize）、`bootstrap-sha-verification`（sha256 sidecar best-effort 校验）、`mcp-argument-tokenization`（`shell-words` 替代手写分词）、`mcp-peer-credential`（`SO_PEERCRED` uid 校验）、`render-dirty-cache`（`CachedInstances` 行级脏带固化 + zelland 等价性文档）
- **工程债收敛**：`render/tests.rs` `_gpu` 重命名、magic number 常量化、TerminalSurface 2700 行局部注释、Settings 重复 theme 去重；依赖滚动更新（cargo/gradle/flake）+ 体积守卫（`libnative.so` 16M 静联、无 NEEDED ghostty、APK 上限）与零新增告警（clippy/machete/detekt/markdownlint）
- **BREAKING** 无。所有 FFI/Kotlin ABI 保持兼容，仅新增可选 `shell-words` 依赖与轻量 `SemanticSegment` 列范围字段。

## Capabilities

### New Capabilities

- `terminal-winsize-sync`: 初始 winsize 在 spawn 前基于像素尺寸计算并同步，避免首帧折行
- `bootstrap-sha-verification`: Bootstrap 下载后若同 URL 存在 `.sha256` 则校验，失败删除 staging 并重试
- `mcp-argument-tokenization`: MCP `run_command` 使用成熟 crate 拆分 argv，零手写分词
- `mcp-peer-credential`: MCP Unix socket accept 后校验 `SO_PEERCRED` uid == app uid
- `render-dirty-cache`: 行级脏带缓存与 `CachedInstances` 等价性测试锁定

### Modified Capabilities

- `mouse-encoding`: 补 bounds clamp 与 drag 序列单测，文档化 zelland ghostty_mouse_encoder 调用链
- `a11y-overlay`: 补 Robolectric 截断/debounce 去重与包裹行为测试，限频 announce
- `osc133-semantic`: 扩展 `SemanticSegment` 列范围与 `exit_code`，稳定跨 chunk 状态机
- `cell-run-cache`: 增量式 `CellRun` 合并相邻同样式 cell，保持 CellData 兼容

## Impact

- **Rust**: `native/src/terminal/output_processor.rs`（OSC133）、`cell_builder.rs`（CellRun）、`pty.rs`/`session.rs`（winsize）、`mcp/`（tokenizer/peercred）、`render/`（dirty cache 文档）
- **Kotlin**: `TerminalSurface`（mouse 透传 live cellW/H、contentDescription debounce）、`TerminalAccessibility`、`BootstrapDownloader`、`Bridge`
- **构建**: `flake.nix`/`Cargo.lock`/`gradle` 依赖滚动，`scripts/build-android-libs.nu` 60MB 上限守卫
- **测试**: Rust 1016+16 新增 + Kotlin Robolectric/Instrumented + 模拟器 gfxinfo 90fps 验证；现有测试保持绿
