## MODIFIED Requirements

### Requirement: 滚动更新由 vsync 合并

滚动入口（`setScrollRemainderPx` / `setScrollOffset`）MUST 只更新偏移与滚动/空闲时钟，
MUST NOT 直接置 `renderSignaled` 或立即唤醒渲染线程。唤醒 MUST 由 Choreographer 帧回调
在下一个显示帧置 `vsyncRequested` 后统一发出，使高刷触摸流（120Hz）每个 vsync
最多产出一帧。滚动事件频率高于显示刷新率时，逐事件立即渲染会把交换链排满，
`acquire` 反而要等两个 vsync，单帧被放大到 30–60ms。

活跃闭锁 MUST NOT 成为第二路帧节拍。vsync 链健康（最近一次帧回调落在
`RENDER_VSYNC_STALE_NANOS` 内）时，活跃闭锁超时 MUST 明显大于一个显示周期，
使每个显示帧至多产出一帧；链停摆时 MUST 退回 17ms 兜底节拍，保证主线程阻塞期间
输入回显与滚动不被拖死。健康判据 MUST 来自帧回调写入的时间戳，
MUST NOT 以「本帧是否由 vsync 唤醒」反推。

#### Scenario: 高刷触摸不塞满交换链

- **WHEN** 120Hz 触摸流连续更新亚行滚动余量
- **THEN** 渲染线程每个 vsync 最多渲染一帧，滚动期间 `SLOW_RENDER_PHASE` 的
      `acquire` 不再因两帧排队而占该帧总时长的大部分

#### Scenario: 单路节拍不产生成对呈现

- **WHEN** 60Hz 显示上持续滚动，vsync 链健康
- **THEN** 相邻两次呈现的间隔不低于一个显示周期，MUST NOT 出现 3–5ms 的成对呈现
      随后 60ms 以上的取帧空档

#### Scenario: vsync 链停摆时退回兜底

- **WHEN** 帧回调超过 `RENDER_VSYNC_STALE_NANOS` 未投递（主线程被长时间占用）
- **THEN** 活跃闭锁退回 17ms，循环仍按兜底节拍产出帧

#### Scenario: 滚动仍能及时上屏

- **WHEN** 用户开始一次新的滚动手势（距上次信号已超过空闲阈值）
- **THEN** 滚动入口刷新空闲时钟，Choreographer 回调在下一个 vsync 唤醒渲染线程，
      不因未直接唤醒而落入 500ms 空闲闭锁
