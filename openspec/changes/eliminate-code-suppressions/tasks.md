# Tasks

## 1. JNI 门面导入

- [x] 1.1 `native/src/android/ffi.rs`：删掉 7 处 `#[allow(unused_imports)] use xxx::*;`
      glob 再导出，只保留门面真正对外共享的 `ATLAS_SIZE`、`DEFAULT_FONT_CELL_SIZE`
      → 子模块改为显式 `use super::xxx::符号`，
      验证：`cargo clippy --all-targets -- -D warnings` 零告警且 `semgrep` 零 findings
- [x] 1.2 `native/src/android/ffi/{font,input,query,session,state,wake}.rs` 与
      `ffi/render/{clipboard,frame,overlay,session_ops}.rs`：补齐跨子模块的显式导入
      → 验证：同上
- [x] 1.3 `native/src/android/ffi/*_tests.rs`：测试导入指向真实定义模块，
      不再经门面 glob 中转
      → 验证：`cargo test --lib` 全绿

## 2. 测试抑制

- [x] 2.1 `native/src/render/tests/mod.rs`：去掉
      `#[allow(clippy::too_many_arguments)]`，单元尺寸改由构造器内部
      `font_pipeline.cell_metrics()` 取得，参数由 10 个降到 7 个
      → 验证：`cargo clippy --all-targets -- -D warnings` 零告警，
      渲染测试全绿
- [x] 2.2 `native/src/terminal/ghostty_terminal/tests/{mod,mouse_osc}.rs`：去掉
      `#[allow(clippy::float_cmp)]`，断言与字面量精确比较
      → 验证：`cargo clippy --all-targets -- -D warnings` 零告警
- [x] 2.3 `native/src/terminal/pty/tests.rs`：去掉测试内裸 `return;`，
      轮询命中即 break，循环外统一 `assert!`
      → 验证：`cargo test --lib` 全绿且 `semgrep` 零 findings

## 3. 文档链接

- [x] 3.1 `native/src/terminal/session/output.rs`：修复模块拆分后失效的
      intra-doc 链接 `[`PtyMaster`]` → `[`PtyMaster`](super::PtyMaster)`
      → 验证：`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` 通过

## 4. 集成验证

- [x] 4.1 `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
      `cargo ndk -t arm64-v8a clippy --all-targets -- -D warnings`、
      `cargo machete --with-metadata` 全绿
- [x] 4.2 `cargo test --lib` 全绿（726 passed）、`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` 通过
- [x] 4.3 `semgrep scan --error --config .semgrep/rules.yml` 零 findings
- [x] 4.4 `openspec validate --all --strict` 全绿

## Workflow follow-up

- 归档变更。
