## MODIFIED Requirements

### Requirement: OSC133 语义段列范围与退出码

系统 SHALL 在 `output_processor` 中维护 `SemanticSegment {start_col,end_col,kind,exit_code}`，解析 `A/B/C/D` 序列时使用 ST/BEL 双终结、跨 chunk 状态机、A 重置与 `D;exit_code` 解析，记录列范围。

#### Scenario: ST 与 BEL 均可终结

- **WHEN** 输入 `"\x1b]133;B\x07echo hi"` 与 `"\x1b]133;B\x1b\\echo hi"`
- **THEN** 两种终结均生成 `PromptEnd` 段

#### Scenario: 跨 chunk

- **WHEN** `"\x1b]133;B"` 与 `"\x07"` 分两次 `process_output` 到达
- **THEN** 段正确合并，不丢失

#### Scenario: A 重置

- **WHEN** 收到 `A` 且已有未完成的 B/C 段
- **THEN** 清空 `semantic_segments` 与 `capture_buf`

#### Scenario: D 携带退出码

- **WHEN** 输入 `"\x1b]133;D;42\x07"`
- **THEN** 最近 `CommandFinished` 段的 `exit_code == Some(42)` 且 `getLastCommandOutput` 可取

### Requirement: getLastCommandOutput 列精确

系统 SHALL 使 `getLastCommandOutput` 返回最近 `CommandFinished` 前的 `CommandOutput` 文本，列范围基于 `ghostty` 游标列精确记录。

#### Scenario: 多行命令

- **WHEN** 输入包含换行的命令 `echo -e "a\nb"` 的 133 序列
- **THEN** `getLastCommandOutput` 返回含换行的完整输出
