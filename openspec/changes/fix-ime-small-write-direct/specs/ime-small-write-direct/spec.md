# ime-small-write-direct Specification

## Purpose

小字符提交与退格同延迟，大批量输入仍合并写 PTY。

## ADDED Requirements

### Requirement: 小提交直发

编码后字节数不超过单字符上限的 `write` MUST 跳过帧同步立即发送；
超过上限的 MUST 维持批缓冲语义（驻留至帧回调或兜底冲刷）。

#### Scenario: 单汉字提交直发

- **WHEN** 写入 3 字节单汉字
- **THEN** 不等待帧回调即发送

#### Scenario: 大批量仍批缓冲

- **WHEN** 写入超过上限的字节块
- **THEN** 驻留缓冲直至冲刷
