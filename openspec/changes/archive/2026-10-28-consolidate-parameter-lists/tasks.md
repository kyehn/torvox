# 任务

- [x] 建 change（proposal/design/tasks，skip_specs：纯重构无行为变化）
- [x] 查询应答路径改传 `&mut CommandState`（`internal/dispatch.rs`）
- [x] VT 线程主循环去掉查询通道句柄克隆与逐层传递（`internal/run.rs`）
- [x] 新帧数据成组为 `NewFrame`（`android/ffi/render/frame.rs`）
- [x] 原生侧门禁：`cargo fmt --all -- --check`、`cargo clippy --all-targets -- --deny warnings`、`cargo test --all-targets --no-fail-fast`
- [x] `openspec validate --all --strict` 通过
