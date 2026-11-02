## ADDED Requirements

### Requirement: 命令入队失败的告警必须限频并保留次数

命令通道有界，VT 线程繁忙时命令入队会失败。该状况属**一种**异常（通道满），
MUST 只按窗口输出一次，MUST NOT 按失败次数逐条输出：一次输出高峰即可刷出两万余行
同文案告警，把其余诊断淹没。

告警行 MUST 带上窗口内累计的丢弃次数：限频吞掉的是条数，不是次数，次数丢失后
排障时看不出「多久一次、一次多少」。

同一类告警 MUST 只有一处限频判定实现：全部命令入队调用点 MUST 共用同一把闸门，
MUST NOT 各自另写一份口径不同的判定。闸门 MUST 可被多个线程共享（命令入队来自
渲染线程与输入线程）。

命令入队失败本身 MUST 仍记为 warn：被丢弃的命令不会重放，这是真实异常。

实现：`GhosttyTerminal::cmd_drop_log`（`native/src/slow_log.rs` 的
`CountedLogThrottle`）为唯一闸门，`public_api.rs` 的
`warn_command_dropped` 为唯一告警出口，12 处命令入队调用点全部经此。

#### Scenario: 输出高峰不再刷屏

- **WHEN** 命令通道连续多次满，命令入队反复失败
- **THEN** 告警 MUST 按 500ms 窗口至多输出一行，MUST NOT 每失败一次输出一行

#### Scenario: 限频不得吞掉丢弃次数

- **WHEN** 一个限频窗口内发生了多次命令入队失败
- **THEN** 放行时输出的那一行 MUST 带上该窗口内累计的失败次数

#### Scenario: 全部入队调用点共用一把闸门

- **WHEN** 不同的命令入队调用点（写输出、改尺寸、滚动、选区等）先后失败
- **THEN** 它们 MUST 记入同一把闸门，MUST NOT 各自另起一份限频判定

#### Scenario: 入队失败仍是告警而非静默

- **WHEN** 命令入队失败
- **THEN** 该次失败 MUST 记为 warn，MUST NOT 因限频而完全静默

#### Scenario: 共享闸门不得丢计数

- **WHEN** 多个线程同时向同一把闸门记事件
- **THEN** 已交出的计数与窗口内待交出的计数之和 MUST 等于事件总数
