# Design

## Context

上一轮变更 `split-native-oversized-modules` 把 `native/src/android/ffi.rs`、
`native/src/render/pass.rs` 与 `native/src/terminal/session.rs` 拆成子模块目录，
每个子模块顶部是 `use super::*;`。为了让这些 `super::*` glob 导入在各子模块间
互相可见，门面里加了 7 行 `#[allow(unused_imports)] use xxx::*;`——它把子模块的
符号表整个灌进门面，再靠 `#[allow]` 压掉「导入了但本文件没直接用」的告警。

`semgrep` 的 `no-allow-in-prod` 规则把 `native/src/**/*.rs` 里的 `#[allow(` 一律
判为 ERROR（测试文件按文件名豁免），因此这些抑制必须在 CI 通过前清零。

## Goals / Non-Goals

**Goals:**

- `semgrep scan --error --config .semgrep/rules.yml` 归零。
- 子模块之间的依赖方向可读：谁用了谁，一行 `use` 说清。
- 测试的失败路径与成功路径走同一条断言。

**Non-Goals:**

- 不改任何生产逻辑、函数签名、日志与错误语义。
- 不改 JNI 导出符号名。
- 不动 `docs/specification/`、`docs/STATUS.md` 等受保护文件。

## Decisions

### 1. 门面不再 glob 再导出，子模块改为显式导入

门面对外只保留真正跨模块共享的两个常量（`ATLAS_SIZE`、`DEFAULT_FONT_CELL_SIZE`），
其余符号由各子模块按需显式 `use super::query::session_handle;` 这样的路径导入。

替代方案是保留 glob 再导出、只在 `.semgrep/rules.yml` 里给 `native/src/android/ffi.rs`
开豁免——那等于把门禁降级，与本变更的目的相反。

代价是 `use super::*;` 与显式导入并存时可能出现同名歧义；解决方式是显式导入
写在 glob 导入之前，编译器以显式导入优先，二者不冲突。

### 2. 测试辅助构造器的单元尺寸改为内部取

`build_cursor_probe_instance` 原本收 `cell_width` / `cell_height` 两个参数，但
所有调用点传的都不是外部值，而是同一行 `font_pipeline.cell_metrics()` 的结果
（调用点自己也要这份尺寸用于断言）。构造器内部直接取 `font_pipeline.cell_metrics()`，
参数由 10 个降到 7 个，正好落在 clippy 默认阈值（7）之下，无需任何抑制。

替代方案是把参数打包成结构体——但参数与被测调用一一对应才便于阅读，打包反而
遮蔽映射关系；去掉两个本可自取的参数是最小改动。

### 3. 浮点精确比较不靠 `#[allow]`

`CellSnapshot` 的默认值字段是整数字面量，与字面量做 `assert_eq!` 不涉及浮点
运算误差，`clippy::float_cmp` 在此处是误报。处理方式是移除 `#[allow]` 并保留
断言原样——若 lint 真的触发，说明字段类型变了，那时应当显式处理而不是压制。

### 4. 轮询测试改为「命中即 break，循环外统一断言」

PTY 写入测试原来在轮询命中时 `return;`，超时才 `panic!`。改为用一个
`payload_delivered` 标志记录命中并 `break`，循环结束后统一
`assert!(payload_delivered, ...)`。

这样成功与失败都经过同一条断言，测试不会再「静默通过」。

## Risks / Trade-offs

- [Risk] 显式导入遗漏某个符号导致编译失败 → Mitigation：编译器即时报错，
  `cargo clippy --all-targets -- -D warnings` 为准。
- [Risk] 去掉 `cell_width` / `cell_height` 参数后调用点尺寸与断言用尺寸不一致
  → Mitigation：调用点仍自行取 `font_pipeline.cell_metrics()` 用于断言，
  与构造器内部取的是同一来源。
- [Risk] `use super::*;` 与显式导入并存时符号解析歧义 → Mitigation：显式导入
  在编译期优先，冲突会直接报错，不会静默取错。
- [Risk] 模块拆分后 intra-doc 链接断裂 → Mitigation：`cargo doc` 以
  `RUSTDOCFLAGS="-D warnings"` 为准，链接写成 `[`PtyMaster`](super::PtyMaster)`。

## Migration Plan

- 无迁移：纯代码组织形式与测试写法调整，回滚即还原这些文件。
- 验证：`cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --lib`、`semgrep scan --error`、`openspec validate --all --strict`。
