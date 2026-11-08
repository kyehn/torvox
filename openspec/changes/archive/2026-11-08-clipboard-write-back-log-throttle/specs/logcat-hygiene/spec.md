## ADDED Requirements

### Requirement: 剪贴板应答回写失败的告警必须限频并保留次数

剪贴板应答回写失败是真实异常，MUST 记为 warn；但洪泛削减时每帧最多回写 16 次应答，
逐次记 warn 会把 logcat 刷成洪水。该告警 MUST 按窗口汇总输出：每个窗口至多一行，
该行 MUST 带上窗口内累计的失败次数，窗口内的计数 MUST 延后交出而非丢弃。

会话已结束导致的回写失败是正常退化路径，MUST 记为 debug，MUST NOT 计入失败窗口。

同一类告警 MUST 只有一处限频判定实现：两条回写调用点 MUST 共用同一把闸门。

实现：`native/src/android/ffi/render/clipboard.rs` 的
`CLIPBOARD_WRITE_BACK_FAILURE_LOG`（`native/src/slow_log.rs` 的
`CountedLogThrottle`）为唯一闸门。

#### Scenario: 回写失败不再逐次刷屏

- **WHEN** 剪贴板应答回写在同一窗口内连续失败多次
- **THEN** 告警 MUST 按 500ms 窗口至多输出一行，MUST NOT 每失败一次输出一行

#### Scenario: 限频不得吞掉失败次数

- **WHEN** 一个限频窗口内发生了多次回写失败
- **THEN** 放行时输出的那一行 MUST 带上该窗口内累计的失败次数

#### Scenario: 回写失败仍是告警而非静默

- **WHEN** 剪贴板应答回写失败
- **THEN** 该次失败 MUST 记为 warn，MUST NOT 因限频而完全静默

#### Scenario: 会话已结束不计入失败窗口

- **WHEN** 会话已结束导致应答回写失败
- **THEN** 该次失败记为 debug，MUST NOT 计入限频窗口的失败次数
