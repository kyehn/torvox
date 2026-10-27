# Proposal

## Why

`semgrep scan --config .semgrep/rules.yml` 报出 11 条告警，全部落在「代码靠抑制
而非靠修」的形态上：

- 7 处 `#[allow(unused_imports)] use xxx::*;`：JNI 门面用 glob 再导出子模块的
  全部符号，再靠 `#[allow]` 压掉「部分导入未被使用」的噪声。
- 1 处 `#[allow(clippy::too_many_arguments)]`：渲染测试的辅助构造器参数过多，
  直接把 lint 关掉。
- 1 处 `#[allow(clippy::float_cmp)]`：终端快照测试与字面量精确比较，被当成
  浮点误差风险抑制。
- 1 处测试内裸 `return;`：等待子进程读到载荷的轮询循环在成功时提前返回，
  绕过「断言具体行为」的约束。

这些抑制把真实缺陷一起盖住：glob 再导出让子模块之间的依赖方向无法一眼看清，
裸 `return` 让「超时未收到载荷」与「成功收到」走出不同的断言路径。

## What Changes

- JNI 门面 `native/src/android/ffi.rs` 去掉 7 处 `#[allow(unused_imports)]` 的
  glob 再导出，子模块之间改为显式 `use super::xxx::符号`，依赖方向可读。
- 渲染测试辅助构造器 `build_cursor_probe_instance` 去掉
  `#[allow(clippy::too_many_arguments)]`：单元尺寸本就来自字体流水线，改为
  构造器内部取 `font_pipeline.cell_metrics()`，参数由 10 个降到 7 个。
- 终端快照测试去掉 `#[allow(clippy::float_cmp)]`：断言保持与字面量精确比较
  （默认值为整数字面量，无浮点运算误差）。
- PTY 写入测试去掉裸 `return;`：轮询改为「命中即 break，循环外统一断言」，
  成功与超时两条路径都经过同一条断言。
- 顺带修复 `native/src/terminal/session/output.rs` 中因模块拆分而失效的
  intra-doc 链接（`[`PtyMaster`]` → `[`PtyMaster`](super::PtyMaster)`）。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

无：本变更只改代码组织形式与测试写法，不改变任何对外行为，故不放 delta。

## Impact

- `native/src/android/ffi.rs` 与 `native/src/android/ffi/**`：导入方式变化，
  JNI 导出符号名（`#[no_mangle]` 字面量）不变。
- `native/src/render/tests/**`：测试辅助构造器签名变化，仅测试内部使用。
- `native/src/terminal/pty/tests.rs`、`native/src/terminal/ghostty_terminal/tests/**`：
  测试写法变化，断言语义不变。
- 不改 `docs/specification/`、`docs/STATUS.md`、`Cargo.toml` 等受保护文件。
- 行为不变：由 `cargo clippy --all-targets -- -D warnings`、`cargo test --lib`
  与 `semgrep scan --error` 保证。
