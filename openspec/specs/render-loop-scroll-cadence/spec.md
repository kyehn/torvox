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
