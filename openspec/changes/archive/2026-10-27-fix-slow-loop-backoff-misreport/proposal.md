# Proposal

## Why

真机 logcat（`logcat_2026-10-27_15-34-48.txt`）里 9 条 `SLOW_LOOP` 警告有 5 条形如
`total=55.6ms … render=5.0ms poll=50.5ms`：这 5 条全部紧跟在「平台收走原生窗口 →
渲染失败 → 50ms 退避」之后，是同一个误报。

误报有两层：

1. **归因错了**：那 50ms 是渲染失败后**刻意**等的退避（`RENDER_ERROR_SLEEP_MS`），
   却被记进 `poll` 段。`SLOW_LOOP` 的作用是「慢循环帧的定位仪器」，把主动等待算成
   工作耗时，定位信息就此失真：真正花在轮询上的时间不足 1ms。
2. **级别错了**：平台收走原生窗口是 `logcat-hygiene` 明列的正常退化路径，MUST 记为
   debug 或 info，MUST NOT 记为 warn 或 error。当前实现却在这次退化后立刻记一条
   warn 级 `SLOW_LOOP`——真机上 8.5 分钟里 5 条应用自有警告有 5 条来自这条误报。

空闲门控驻留（`waitOutput` 至多 500ms）早已被排除在判定之外，退避与它是同一类
「设计上主动等的」时间，却漏在了判定之内。

## What Changes

- 渲染失败退避的时长被单独计量，不再计入 `SLOW_LOOP` 的判定与 `poll` 段。
- `SLOW_LOOP` 行新增 `backoff=` 字段显式给出这段主动等待，各段时长之和仍等于
  `total`。
- 判定只看去掉两段主动等待后的实际工作时长；平台收走窗口这条正常退化路径不再产生
  warn，真正的慢渲染仍照常告警。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `logcat-hygiene`：明确「主动等待（空闲门控驻留、失败退避）不计入慢循环判定」，
  且正常退化路径不得触发 warn 级慢循环告警。

## Impact

- `android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt`：退避计量、
  判定口径与日志字段。
- `android/app/src/test/java/terminal/emulator/runtime/`：判定口径的纯函数用例。
