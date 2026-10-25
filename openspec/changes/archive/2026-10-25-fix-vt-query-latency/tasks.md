# Tasks

## 1. 回归用例（`vt-query-latency`）

- [x] 1.1 新增 `idle_vt_thread_answers_queries_without_waiting_for_tick`：
      空闲 VT 线程上连续 8 次查询的总耗时 < 2 个空闲节拍，
      验证：修复前实测 383.87ms（失败），修复后 < 5ms（通过）

## 2. VT 线程双通道等待

- [x] 2.1 `run_inner` 的等待改为 `flume::Selector` 同时监听命令通道与查询通道，
      查询就绪即应答，验证：1.1 全绿
- [x] 2.2 `drain_queries` 接受本轮已从通道取出的查询（`first`），
      避免已取出的查询被丢弃，验证：查询用例全绿
- [x] 2.3 查询通道断开后不再参与选择（已断开的通道恒就绪会变成忙等），
      验证：会话析构与生命周期用例全绿

## 3. 语义订正

- [x] 3.1 `types.rs` 的 `VT_IDLE_TICK_MS` 注释改回「空闲等待下一条命令的时长」，
      验证：注释与实现一致
- [x] 3.2 订正 `tests.rs` 里「查询延迟上界即节拍」的旧注释，
      验证：注释与实现一致

## 4. 规范增量

- [x] 4.1 规范增量落 `specs/vt-query-latency/spec.md`，
      验证：`openspec validate --strict` 通过

## 5. 集成验证

- [x] 5.1 Rust：`cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、
      `cargo test --lib` 全绿
- [x] 5.2 JVM：`:app:testDebugUnitTest`、`:app:detekt`、`:app:spotlessCheck` 通过
- [x] 5.3 规范：`openspec validate --all --strict` 全绿
