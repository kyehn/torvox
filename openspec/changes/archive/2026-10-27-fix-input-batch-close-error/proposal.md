# Proposal

## Why

`InputBatchBuffer.close()` 在 detach 时把驻留字节交给发送线程，但它自建了一份
`sender.execute { flushSink(...) }`，没有沿用 `sendLocked` 的异常处理。执行器把任务
包装成 FutureTask，Runnable 抛出的异常只进 Future，**既不落日志也不影响进程**。

后果：detach 前最后一击键（或粘贴尾部）写入失败时完全无症状——输入静默消失，
日志里片痕不留。这与 DESIGN 的「异常应输出错误日志」相悖，也让「粘贴没到 shell」
这类问题无法从日志判定。

## What Changes

- `close()` 的最终刷写改走 `sendLocked`：写入异常记错误日志，入队被拒记警告。
- 顺带消除重复：`sendLocked` 与 `close()` 不再各写一份 `sender.execute`。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `ime-small-write-direct`: 补上批缓冲关闭路径的失败上报要求。

## Impact

- `android/app/src/main/java/terminal/emulator/runtime/InputBatchBuffer.kt`：`close()`
  复用 `sendLocked`。
- `android/app/src/test/java/terminal/emulator/runtime/InputBatchBufferCloseTest.kt`：
  新增回归用例（Robolectric 捕获 logcat）。
