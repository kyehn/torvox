# 任务

- [x] 定位编译失败的根因：`build_instances_cached` 已删除而 `pass/frame.rs`、基准与
      单元测试仍在调用（`origin/main` 上 `cargo check --all-targets` 即报错）
- [x] 实例构建收敛为单一入口 `CachedInstances::rebuild`，并回报本帧走的是增量还是全量
- [x] `Renderer` 删去 `cpu_instances` 与 `cell_full_mask_cache`，缓存成为实例唯一副本
- [x] `partial_frame_bands` 改用构建过程回报的路径判定，调用方不再自行推断
- [x] 删除辅助入口 `build_instances_from_cell_data`，基准与分配预算用例改测生产路径
- [x] 单测钉住路径判定：增量帧断言 `Some(true)`，降级帧断言 `Some(false)`
- [x] 复验 `cargo check --all-targets`、`cargo clippy --all-targets --all-features
      -- -D warnings`、`cargo fmt --check` 通过
- [x] 复验 `cargo test --all-targets` 全绿（1002 个库用例 + 集成用例 + 基准下限断言）
- [x] 复验 Android 目标 `cargo ndk -t arm64-v8a clippy --all-targets -- --deny warnings`
      通过
- [x] 规范补上「实例缓存是唯一副本」与「路径判定唯一且由构建过程回报」，并更新 CI
      下限断言引用的基准名
