# Design

## Context

渲染循环一次迭代的耗时构成（`TerminalRuntime` 渲染线程）：

```text
loopFrameStart
  ├─ 空闲门控驻留（waitOutput，至多 500ms）      → loopGateDoneNanos
  ├─ 渲染前推送（选区/滚动偏移/滚动余量）        → loopPrerenderDoneNanos
  ├─ 渲染 JNI（renderWithNewOutput）             → renderDoneNanos
  ├─ 渲染失败退避（delay(RENDER_ERROR_SLEEP_MS)）  ← 只在 count < 0 时
  ├─ 输出泵（pollAll）与事件派发                 → loopPollDoneNanos
  └─ 帧统计与标题查询
loopDurationNanos = 循环末尾 - loopFrameStart
```

现状判定：`loopDurationNanos - 门控驻留 > 30ms` 即记 warn。门控驻留被排除是因为
它「是正常设计态而非慢」；退避没有被排除，是因为它此前没有被单独计量——它落在
`renderDoneNanos` 与 `loopPollDoneNanos` 之间，被并进了 `poll` 段。

真机日志里 5 次「平台收走窗口」各自产生一条 `poll≈50.5ms` 的 `SLOW_LOOP` warn，
而同一时刻真正的渲染耗时只有 1–5ms。

## Goals / Non-Goals

- Goal：主动等待（门控驻留、失败退避）不计入慢循环判定。
- Goal：`SLOW_LOOP` 行的各段时长自洽，定位信息不失真。
- Goal：正常退化路径不产生 warn；真实慢渲染照常告警。
- Non-goal：不改渲染失败的处理流程（退避时长、错误分级、重试节奏都不动）。
- Non-goal：不改 `loop timing window` 的口径——它量的是循环周期（含等待），
  用于推算帧节奏，与「哪里慢了」是两件事。

## Decisions

### 决策一：退避时长按实测计量，不按配置值折算

`delay(sleepMs)` 前后取 `System.nanoTime()` 得到实际退避时长。取配置值会在
`delay` 被取消（协程中断）时算多，取实测值则与日志里其它段的口径一致（都是实测）。

### 决策二：判定口径抽成纯函数 `slowLoopWorkNanos`

判定要能脱离渲染循环单测：把「整循环时长扣掉两段主动等待」写成纯函数，用例直接
断言「50ms 退避 + 5ms 工作不算慢」「40ms 工作算慢」。函数只在本处调用一次，但它是
被测口径本身（与 `shouldUseIdleLatch`、`transientRenderErrorIsWarning` 同类）。

### 决策三：日志行保留 `total` 原义，另加 `backoff=` 字段

`total` 仍是整循环的真实时长（含等待），否则各段之和与 `total` 对不上。退避单独
成字段后，`total = gate + pre + render + backoff + poll + tail` 恒成立，定位时能
直接看出「这 50ms 是主动等的，不是慢」。

`poll` 段改为真正的轮询与事件派发耗时（退避已扣除）。

## Risks / Trade-offs

- [退避计量遗漏某条退避路径] → 退避只有一处（`count < 0` 分支的 `delay`），计量与
  退避同处一语句，不存在第二处。
- [真实慢渲染被误放行] → 判定只扣主动等待，渲染 JNI 本身的耗时原样计入；真机上
  45ms 的慢渲染仍在阈值之上，照常告警。
- [退避恰好掩盖了别的慢段] → `backoff=` 字段显式给出退避时长，行内各段自洽，
  排障时不会被隐藏。

## Migration Plan

无数据迁移。改动只在渲染循环的计时与一行日志的字段上。

## Open Questions

无。
