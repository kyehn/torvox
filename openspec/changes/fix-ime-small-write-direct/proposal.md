## Why

中文输入每个字符走多次帧同步批缓冲（拼音增量、候选提交、全角标点各一次），每次正常等 0-16.7ms、无帧时等足 50ms，延迟按次累加；组合内退格后重追加同样被钳制，体感为退格慢且顿挫。英文单事件/字符故不明显。

## What Changes

- 编码后小字节提交（单个字符量级）跳过帧同步立即发送；粘贴与大批量仍走批缓冲。
- 同步更新 `InputBatchBufferTest` 第一用例。

## Capabilities

### New Capabilities

- `ime-small-write-direct`: 小提交直发，大批量批缓冲。

### Modified Capabilities

无。

## Impact

- `InputBatchBuffer.kt` 的 `write` 分支；`InputBatchBufferTest`。
