# 组合提交直发

## 上下文

- `InputBatchBuffer.kt` 直发上限 8B 只覆盖单码点；多码点 `commitText`（编码后 >8B）仍帧同步等待，无帧时兜底 50ms，中文输入事件多、延迟按次累加。
- 退格路径直连 `writeToPty`；慢感来自组合提交被批缓冲钳制。

## 任务

1. `write` 直发上限提升至组合提交量级并更新注释：单次组合提交不等待帧回调，仍保序（先排空驻留再直发）；粘贴、大批量、编程性写入仍批缓冲。
2. 更新 `InputBatchBufferTest`：原边界用例改用超过新上限的字节块；新增多码点组合提交直发、退格后紧接提交不累积延迟用例；现有 7 例行为保持。
3. 单测验证：`./gradlew ':app:testDebugUnitTest' --tests 'terminal.emulator.runtime.InputBatchBufferTest'` 全绿。
