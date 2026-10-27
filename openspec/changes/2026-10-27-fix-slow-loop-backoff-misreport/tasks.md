# Tasks

## 1. 慢循环判定口径修正

- [x] 1.1 渲染失败退避前后取时，单独计量退避时长；`SLOW_LOOP` 行新增 `backoff=`
      字段，`poll` 段改为扣除退避后的真实轮询耗时。
      验证：`total = gate + pre + render + backoff + poll + tail` 恒成立。
- [x] 1.2 判定改用纯函数 `slowLoopWorkNanos`：整循环时长减去门控驻留与退避。
      验证：`cargo`/Gradle 编译通过，detekt 无告警。

## 2. 回归用例

- [x] 2.1 纯函数用例：50ms 退避 + 5ms 工作不算慢；40ms 工作算慢；门控驻留同样
      不计入。
      验证：`testDebugUnitTest` 全绿。

## 3. 全量验证

- [x] 3.1 `openspec validate 2026-10-27-fix-slow-loop-backoff-misreport --strict` 通过。
- [x] 3.2 Gradle 单测与 lint 通过。

## Workflow follow-up

- 归档本变更，把 `logcat-hygiene` 的新增要求并入 `openspec/specs/logcat-hygiene/spec.md`。
