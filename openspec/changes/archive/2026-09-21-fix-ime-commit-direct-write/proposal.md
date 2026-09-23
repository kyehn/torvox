## Why

CJK 输入中"退格后重追加拼音"、拼音增量以及一次 `commitText` 提交多个汉字（编码后 >8B）仍走帧同步批缓冲：每次等待下一帧（0-16.7ms，无帧时最差 50ms 兜底），中文输入事件数为英文数倍，延迟按次累加，体感退格慢。上一刀只放行单码点量级（≤8B）的直发，多码点组合提交仍被钳制。

## What Changes

- 直发上限从"单字符量级"（8B）扩展为"组合提交量级"（64B，约 21 个汉字）：编码后不超过上限的 `write` 跳过帧同步立即发送；粘贴、大批量、编程性写入（通常数百字节以上）仍走批缓冲合并写。保序语义不变（先排空驻留再直发）。
- 同步更新 `InputBatchBufferTest`：原边界用例改用超过新上限的字节块以维持原语义；新增多码点组合提交直发、退格后紧接提交不累积延迟用例。现有 7 例行为保持。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `ime-small-write-direct`: 小提交直发的字节上限由单码点量级扩展为组合提交量级；超过上限仍维持批缓冲语义。

## Impact

- `android/app/src/main/java/terminal/emulator/runtime/InputBatchBuffer.kt`：`write` 直发阈值与注释。
- `android/app/src/test/java/terminal/emulator/runtime/InputBatchBufferTest.kt`：边界用例调整 + 新增用例。
