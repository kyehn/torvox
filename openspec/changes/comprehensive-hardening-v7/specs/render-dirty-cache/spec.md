## Purpose

固化 `CachedInstances` 行级脏带缓存的正确性，确保与 zelland 行级去重等价且在 60fps 下跳过未变更行的实例重建。

## ADDED Requirements

### Requirement: 行级脏带

系统 SHALL 维护 `CachedInstances` 与 `compute_dirty_bands`，当且仅当行的 `fg/bg/flags` 字节等价且 `selection/cursor` 未影响该行时，复用缓存实例，否则重建。

#### Scenario: 未变更行复用

- **WHEN** 帧间仅第一行变化，其余 39 行内容与样式不变
- **THEN** `dirty_bands` 长度为 1，且未变更行的 `instance` 指针与上一帧相等

#### Scenario: 选择变化触发重建

- **WHEN** 选择区域从 `cols 0..5` 扩展至 `0..10`
- **THEN** 受影响行的 band 被标记脏，即使文本未变

#### Scenario: 等价性对照

- **WHEN** 构造与 zelland `with_rows` 相同的随机网格变更序列
- **THEN** `compute_dirty_bands` 的 `changed` 判断与 zelland 结果位等价（property test 1000 次随机）

### Requirement: 可观测与文档

系统 SHALL 在 `render/tests.rs` 中以注释引用 zelland 的 `WGPU_FIXES Fix1`，并通过 `bytemuck` 字节等价实现 `rows_equal`，避免字段级遗漏。

#### Scenario: 注释可追溯

- **WHEN** 审查 `render/cell_builder.rs:rows_equal`
- **THEN** 注释含 `// 参考 zelland WGPU_FIXES Fix1` 且实现为 `bytemuck::bytes_of` 比较
