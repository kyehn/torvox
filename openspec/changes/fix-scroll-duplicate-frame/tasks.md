# Tasks

## 1. 滚动在途标志

- [ ] 1.1 `RenderState` 新增「滚动命令在途」标志（`native/src/android/ffi.rs`），
      验证：字段含义与设计一致，注释写明为何不能在途重绘
- [ ] 1.2 `setScrollOffset` 的重绘触发由 `dirty` 置位改为该标志，
      验证：命令发送失败或偏移无变化时不置标志（失败时宿主会重试）

## 2. 空闲重绘门控

- [ ] 2.1 空闲重绘门控抽成纯函数并纳入滚动在途标志（`render_inner` 空闲路径），
      验证：滚动在途时直接返回、不重绘
- [ ] 2.2 跳过时归还内容脏标志，
      验证：字体/主题等只置该标志的改动不会被跳过的那一帧吞掉
- [ ] 2.3 新帧路径清除滚动在途标志（渲染失败时同样清除，使重试不被挡掉），
      验证：新帧呈现后空闲重绘恢复

## 3. 回归用例

- [ ] 3.1 新增表驱动单测：滚动在途时五个变化信号任一为真都不重绘；
      不在途时任一信号为真即重绘（`native/src/android/ffi.rs` 测试模块），
      验证：`cargo test --lib` 全绿且新用例能判红改坏的门控

## 4. 集成验证

- [ ] 4.1 Rust：`cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、
      `cargo test --lib` 全绿
- [ ] 4.2 JVM：`:app:testDebugUnitTest`、`:app:detekt`、`:app:spotlessCheck` 通过
- [ ] 4.3 规范：`openspec validate --all --strict` 全绿

## Workflow follow-up

- 归档变更并同步 `openspec/specs/render-loop-scroll-cadence/spec.md`。
