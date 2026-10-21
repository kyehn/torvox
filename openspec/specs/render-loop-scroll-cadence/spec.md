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

模式应答类查询（`mode_get_on` 等自带期限并独立应答的样板）MUST NOT 参与
本重试；会话栏外的其余 VT 通道查询（标题、回滚量、滚动上下文等）共用一
条有界通道查询样板，默认 MUST 在期限超时后立即重试一次，把系统过载导致
VT 线程整体错过一个 `QUERY_TIMEOUT_MS` 窗口（本机全量 cargo 套件与深
缓冲用例并行时实测过 `scrollback_rows` 一次读到兜底 0）的情形与「VT 线
程真卡死」区分开：幂等的读缓存查询在第二次尝试命中 VT 线程已落盘的缓存（如
`all_text` 的网格未变复用），直读查询等到线程恢复后的第一次应答；重试
用尽后 MUST 维持原兜底语义，WARN 日志 MUST 给出原始原因与该轮错误，不
得静默吞掉错误。非幂等的动作型查询（如推进搜索游标的 `search_select`）
MUST NOT 参与重试，只发一次，防止滞留首查的应答丢失后被重发放大为副作用
双发。

#### Scenario: 偶发错过窗口命中重试

- **WHEN** 高负载下幂等查询第一次等待至期限且未收到应答
- **THEN** 立即重试一次，读缓存查询在第二次尝试命中已落盘缓存、直读查询命中
      线程恢复后的第一次应答，调用方看到的不是兜底值

#### Scenario: 动作型查询不重发

- **WHEN** 高负载下 `search_select` 首查滞留至期限
- **THEN** 只返回兜底不重发；滞留首查由 VT 线程按队列执行，副作用仍只
      发生一次，选区与视口不跳两个命中

#### Scenario: 查询端持续阻塞时兜底语义不变

- **WHEN** 已设默认重试的查询连续两轮超时（VT 线程真卡死）
- **THEN** 返回与旧实现相同的兜底值，且 WARN 日志携带方法名、重试轮数
      与原始错误，不含「看起来成功」的空数据

### Requirement: 空闲闭锁不得成为输出的投递延迟

渲染循环的等待 MUST 由原生唤醒源承担：PTY 读线程每读到一块输出、以及宿主侧
信号（vsync、输入、滚动、选区、forceRender）MUST 都推进同一个唤醒代数并立即
唤醒等待方。500ms 空闲闭锁 MUST 只作为「没有任何事件时」的兜底节拍，不得成为
无输入输出（后台任务、长时间命令完成）的上屏延迟来源。

唤醒 MUST 采用「代数 + 条件变量」语义：带上次代数进场，期间已发生通知则立即
返回，使「查完信号再进场」与通知竞争时不丢唤醒；等待 MUST NOT 持有会话注册表
锁，避免与销毁/切换会话的写锁互锁。

拆卸路径（`stop`、`restart`、`closeDeadSession`）MUST 先推进唤醒代数再中断渲染
线程：原生条件变量等待不受 `Thread.interrupt()` 影响，缺少通知会让 join 白等
满 `THREAD_JOIN_TIMEOUT_MS`。

#### Scenario: 静置后的后台输出立即上屏

- **WHEN** 终端静置超过 5s（循环已落到 500ms 空闲闭锁）后，PTY 才到达一段没有
  任何输入触发的输出
- **THEN** 循环在通知时立刻醒来渲染，投递延迟与活跃期同量级（仪器测试
  `IdleOutputWakeTest` 在 1080×2400 模拟器上 7 个样本均值 ≤ 120ms、中位数
  ≤ 100ms，实测 19ms / 7ms），而不是等满闭锁周期

#### Scenario: 拆卸时渲染线程立刻退出

- **WHEN** 会话 stop/restart/销毁时渲染线程正停在唤醒源上
- **THEN** 线程被立刻叫醒并在 join 期限内退出，不出现 "still alive after join"
  警告，也不把 `renderThreadPossiblyAlive` 置位

#### Scenario: 通知不持锁等待

- **WHEN** 任意线程调用唤醒（含渲染线程自身的 `onPtyWrite` 回调）
- **THEN** 只取一份唤醒源的引用并 `notify_all`，不等待会话锁，也不在持有会话锁
  时获取注册表锁

### Requirement: 循环自身工作不得含冗余写入与阻塞等锁

渲染循环在「门控判断之后、渲染 JNI 之前」这一段（报告里的 `pre`）MUST 只做循环自身的
轻量工作。向 GPU 队列的写入与跨线程锁等待 MUST NOT 出现在这一段：

- 滚动偏移的 uniform 由每一帧的开帧路径统一写出，设置偏移的入口 MUST 只登记待用值，
  MUST NOT 当场调用 `refresh_cell_uniforms`——同一线程紧接着的开帧会按同一偏移重写
  同一份字节，纯重复；队列繁忙时该写入实测阻塞 100ms 以上。
- 会话锁与渲染状态锁 MUST 用不等待的获取（`try_lock`）。拿不到即返回「未生效」，
  调用方保留上一次已提交的值并在下一帧重试，MUST NOT 静默丢弃更新。
  洪流期间 PTY 写入正持有这些锁，阻塞式获取会把渲染线程直接拖住。

判据是这一段自身的耗时，与设备 GPU 速度无关：仪器用例在持续拖动期间断言每帧
`pre_max` ≤ 8ms（模拟器实测 3ms；改前同一场景为 100–123ms）。

#### Scenario: 洪流期间循环段不吃时间

- **WHEN** 终端持续接收大量 PTY 输出（`yes` 一类）并同时被拖动滚动
- **THEN** 每一帧的 `pre` 段 ≤ 8ms，且实际有帧呈现（不是靠跳帧规避）

#### Scenario: 锁被占用时偏移在下一帧生效

- **WHEN** 设置滚动偏移时渲染状态锁正被其他线程持有
- **THEN** 该次设置返回「未生效」，调用方保留旧值并在下一帧重试，
      最终偏移与请求一致，无静默丢失

### Requirement: 帧时长窗口按帧数或时长先到者结算

渲染循环的帧时长/阶段耗时统计窗口 MUST 在采集满 60 帧**或**跨过 2 秒时结算，
以先到者为准。只按帧数结算时，低帧率设备（软件渲染模拟器实测 1.8–8 FPS）
凑满 60 帧要 8–33 秒；一段短促交互（拖动 2.5 秒只渲染出 3 帧）结束后循环转入
空闲闭锁、不再产生新帧，这些帧的耗时永远不会被报告——恰是最需要数字的设备
最缺诊断。2 秒上限对应 30 FPS 下的 60 帧，故 30 FPS 及更快的设备行为不变。

#### Scenario: 低帧率交互结束后仍有窗口结算

- **WHEN** 设备以 1.8 FPS 渲染，一次 2.5 秒拖动只产生 3 帧后循环转入空闲闭锁
- **THEN** 第 3 帧即触发结算（窗口起点到该帧已跨过 2 秒），
      `loop timing window` 报告 `(3 frames)` 及该窗口的 avg/p95/pre_max

#### Scenario: 快设备仍按 60 帧结算

- **WHEN** 设备以 60 FPS 渲染
- **THEN** 60 帧在 1 秒内采集完，窗口按帧数结算，时长上限从不触发
