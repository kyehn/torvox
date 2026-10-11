# modifier-bar-arrow-encoding Specification

## Purpose

可配置修饰键栏的方向键须与输入法、硬件箭头路径共用同一编码来源。点击时刻由上游
`key::Encoder` 在 VT 线程内读活体终端状态，按应用光标模式（DECCKM）产出普通模式
CSI（`ESC [ A`）或应用光标模式 SS3（`ESC O A`）。键栏此前走裸序列，在应用光标模式
下（vim、tmux 等）发出的 CSI 不被识别，方向键表现为无响应。

编码归属（谁编码、在哪个线程、模式从何而来）见 `key-input-encoding`；本规范只约束
键栏方向键的可观察行为。

## Requirements

### Requirement: 可配置方向键跟随 DECCKM

可配置键栏的 `ARROW_UP/DOWN/LEFT/RIGHT`（含长按重复）MUST 经上游 `key::Encoder`
编码，应用光标模式的判定 MUST 取自活体终端状态而非应用侧查询：普通模式得 CSI
（`ESC [ A`），应用光标模式得 SS3（`ESC O A`）。

#### Scenario: 普通模式点可配置上键

- **WHEN** 未启用 DECCKM 且点击可配置键栏上键
- **THEN** 发送 `ESC [ A`

#### Scenario: 应用光标模式点可配置上键

- **WHEN** 已启用 DECCKM 且点击可配置键栏上键
- **THEN** 发送 `ESC O A`

#### Scenario: 应用光标模式长按重复

- **WHEN** 已启用 DECCKM 且长按可配置键栏上键触发重复
- **THEN** 重复序列同样为 `ESC O A`
