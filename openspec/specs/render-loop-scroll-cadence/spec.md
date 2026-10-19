# render-loop-scroll-cadence Specification

## Purpose

Scrolling was janky: logcat loop-timing windows collapsed to ~2fps during a
scroll gesture (avg≈500ms ≈ the idle-latch timeout), even though the native
render itself was fast (`SLOW_FRAME render=36-48ms`; real device `frame
avg=0-1ms`). The sink was the render loop *parking* at the 500ms idle latch,
not the GPU draw.

## Requirements

### Requirement: 滚动期间渲染循环保持活跃 cadence

渲染循环 MUST 在滚动手势进行期间保持 17ms 活跃 latch + vsync 泵帧，不得因
idle-clock 过期而塌到 500ms idle latch（滚动帧≈2fps）；手势停止
`SCROLL_MOTION_WINDOW_NANOS`（250ms）后 MUST 回落 idle latch 保持省电。

#### Scenario: 手势期间维持帧率

- **WHEN** 用户开始滚动且上一次信号静默已超过 5s
- **THEN** 循环不选择 idle latch，按活跃 cadence 渲染（真机 logcat loop
      ≈1fps → 60fps），滚动帧不掉帧

#### Scenario: 手势停止后回落 idle

- **WHEN** 手势停止移动超过 250ms
- **THEN** 循环回到 500ms idle latch（≈2fps），维持既定省电策略

### Requirement: 滚动运动窗口驱动而不是刷新 idle-clock

滚动运动事件（`setScrollRemainderPx` / `setScrollOffset`）MUST 戳
`lastScrollNanos` 并由 `hasScrollMotion()` 门控活跃 cadence；vsync 回调与
latch 超时 MUST 同时咨询 `hasScrollMotion()`，使 idle-clocks 过期后手势仍能
立即恢复活跃帧率。

#### Scenario: 稀疏事件仍保持活跃

- **WHEN** 手势事件稀疏（主线程忙于手势/组合评分，事件间隙长）
- **THEN** 250ms 运动窗口内的间隙不被 500ms latch 吞掉，帧率不塌陷

### Requirement: 窗口边界诊断查询移出渲染线程

渲染循环 MUST NOT 在 60 帧窗口边界（`RENDER_DIAGNOSTIC_FREQUENCY` 命中）于
渲染线程内发起 VT 通道外置查询（会话标题等）——该查询要等 VT 线程空闲 tick
应答，把窗口边界那一帧拉到 60–115ms（真机实测）；压测环境则踩满 500ms
查询超时（模拟器功耗同族发作落进 CI 三个用例的定时器等待）。窗口边界的
loop 统计日志与滚动上下文诊断 MUST 移出渲染线程执行，而
`FrameTimingTrend` 趋势判定 MUST 保持在渲染线程单线程语义。

#### Scenario: 窗口边界不再拉一帧长时停顿

- **WHEN** 60 帧窗口封尾触发会话标题等诊断查询
- **THEN** 渲染线程只做内存写与趋势判定，VT 通道查询在 IO 协程完成，
      该帧 loop 耗时不出现每秒一次的 60–115ms 尖峰

#### Scenario: 压测环境不再踩满查询超时

- **WHEN** CI/模拟器高负载下 VT 线程积压、查询需 500ms 应答
- **THEN** 渲染循环的 p95 不被该同步查询抬到 500ms 量级

### Requirement: VT 通道查询的期限重试与兜底语义

会话栏外的所有 VT 通道查询（标题、回滚量、滚动上下文等）共用一条有界
通道查询样板。系统过载导致 VT 线程整体错过一个 `QUERY_TIMEOUT_MS` 窗口
（本机全量 cargo 套件与深缓冲用例并行时实测过 `scrollback_rows` 一次
读到兜底 0）：该类查询 MUST 在下一轮立刻重试一次而不是直接退回兜底值
——重试通常命中 VT 线程已落盘的缓存，把「偶发错过」与「VT 线程真卡
死」区分开；重试用尽后 MUST 维持原兜底语义并用 WARN 日志给出原始原因，
不得静默吞掉错误。

#### Scenario: 偶发错过窗口命中重试

- **WHEN** 高负载下第一次查询等待至期限且未收到应答
- **THEN** 立即重试一次，第二轮返回到真实值（缓存已落盘或线程恢复），
      调用方看到的不是兜底值

#### Scenario: 查询端持续阻塞时兜底语义不变

- **WHEN** 连续两轮查询一律超时（VT 线程真卡死）
- **THEN** 返回与旧实现相同的兜底值，且 WARN 日志携带方法名与原始错误，
      不含「看起来成功」的空数据
