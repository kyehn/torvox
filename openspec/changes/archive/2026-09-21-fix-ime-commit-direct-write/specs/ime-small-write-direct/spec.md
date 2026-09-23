# ime-small-write-direct Specification

## MODIFIED Requirements

### Requirement: 小提交直发

编码后字节数不超过直发上限的 `write` MUST 跳过帧同步立即发送；超过上限的 MUST 维持批缓冲语义（驻留至帧回调或兜底冲刷）。直发上限覆盖单次 IME 组合提交量级（UTF-8 多码点短语，含 3 字节/码点汉字与 4 字节 emoji），粘贴、大批量、编程性写入不属于直发范围。

#### Scenario: 单汉字提交直发

- **WHEN** 写入 3 字节单汉字
- **THEN** 不等待帧回调即发送

#### Scenario: 多码点组合提交直发

- **WHEN** 写入编码后 12 字节（4 个汉字）的组合提交
- **THEN** 不等待帧回调即发送

#### Scenario: 退格后紧接提交不累积延迟

- **WHEN** 退格字节直连后紧接写入编码后 12 字节的组合提交
- **THEN** 提交立即发送，不等待帧回调，顺序保持

#### Scenario: 大批量仍批缓冲

- **WHEN** 写入超过直发上限的字节块
- **THEN** 驻留缓冲直至冲刷
