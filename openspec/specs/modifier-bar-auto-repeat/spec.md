# modifier-bar-auto-repeat Specification

## Purpose

修饰键栏的长按锁定与自动重复共用一条时间阈值。阈值若写死，用户在系统设置里调长
「长按时长」（无障碍需求）后，键栏仍按固定节奏抢跑；Termux 的 `ExtraKeysView` 取
`ViewConfiguration.getLongPressTimeout()` 并对越界值回退，本能力记录同一口径。

## Requirements

### Requirement: 长按阈值取自系统配置

长按锁定与自动重复起点 MUST 取系统长按超时（`ViewConfiguration.longPressTimeoutMillis`），
超出 200–3000ms 时 MUST 退回 400ms；自动重复间隔 MUST 为 80ms。锁定与重复 MUST 共用
同一阈值，MUST NOT 另设一个固定常量。

#### Scenario: 系统值在合理区间

- **WHEN** 系统长按超时为 800ms
- **THEN** 锁定与自动重复起点均为 800ms，重复间隔仍为 80ms

#### Scenario: 系统值越界

- **WHEN** 系统长按超时为 5000ms 或 100ms
- **THEN** 阈值退回 400ms（用例 `out of range timeout falls back to the default`）

#### Scenario: 重复序列仍按活体模式编码

- **WHEN** 长按重复触发方向键
- **THEN** 每次重复都经上游编码器按当前光标模式发码，见 `modifier-bar-arrow-encoding`
