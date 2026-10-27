# Design

## 修法

`close()` 的最终刷写与常规写入走同一条出口 `sendLocked`：

- `flushSink` 抛异常 → 记错误日志（「PTY write failed」）。
- 执行器已关闭（`RejectedExecutionException`）→ 记警告。

两条出口都持 `lock` 调用，`sendLocked` 的「必须在持有 lock 时调用」契约不变；
入队顺序与先前排队的字节保持一致（单线程执行器 FIFO）。

## 判别力

回归用例让 `flushSink` 抛异常，关闭后断言 `InputBatchBuffer` 标签下有错误级日志。
去掉修复后该用例失败（异常被 FutureTask 吞掉，日志为空）——用例不是恒真断言。
