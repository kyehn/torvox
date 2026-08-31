## MODIFIED Requirements

### Requirement: 同格式游程合并

系统 SHALL 在 `cell_builder` 中实现 `build_row_runs`，将连续且 `fg/bg/flags` 字节等价的 `CellData` 合并为单个 `CellRun {start_col,length}`，保持 `CellData` FFI 兼容（仍展开为实例以 reuse 现有管线）。

#### Scenario: 同格式单 run

- **WHEN** 一行含 80 个同色同背景同 flags 的 cell
- **THEN** `build_row_runs` 返回长度 1，且 `length == 80`

#### Scenario: 混合格式分段

- **WHEN** 一行前 40 列红、后 40 列蓝
- **THEN** 返回 2 runs，`[0,40)` 与 `[40,80)`

#### Scenario: 空行

- **WHEN** 输入空切片
- **THEN** 返回空 `Vec`，不 panic

#### Scenario: 增量路径复用

- **WHEN** 帧间仅部分行 dirty
- **THEN** clean 行复用 `CachedInstances`，dirty 行按 run 合并后展开，实例数与全量路径一致

### Requirement: 合并率可验证

系统 SHALL 使同格式文本的合并率 >50% 且提供 `cargo bench cell_builder` 可观测。

#### Scenario: Benchmark 可重复

- **WHEN** 执行 `cargo bench cell_builder` 两次
- **THEN** 同格式长行 `runs == 1` 稳定，且耗时波动 <10%
