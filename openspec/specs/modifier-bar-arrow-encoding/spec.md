# modifier-bar-arrow-encoding Specification

## Purpose

可配置修饰键栏的方向键须与输入法、硬件箭头路径共用同一编码来源。点击时刻查询
应用光标模式（DECCKM），经 `TerminalInputEncoder.arrowSequence` 得普通模式 CSI
（`ESC [ A`）或应用光标模式 SS3（`ESC O A`）。键栏此前走裸序列，在应用光标模式
下（vim、tmux 等）发出的 CSI 不被识别，方向键表现为无响应。

## Requirements

### Requirement: 可配置方向键跟随 DECCKM

可配置键栏的 `ARROW_UP/DOWN/LEFT/RIGHT`（含长按重复）在点击时刻查询应用光标模式，
MUST 按 `TerminalInputEncoder.arrowSequence` 编码：普通模式 CSI（`ESC [ A`），
应用光标模式 SS3（`ESC O A`）。

#### Scenario: 普通模式点可配置上键

- **WHEN** 未启用 DECCKM 且点击可配置键栏上键
- **THEN** 发送 `ESC [ A`

#### Scenario: 应用光标模式点可配置上键

- **WHEN** 已启用 DECCKM 且点击可配置键栏上键
- **THEN** 发送 `ESC O A`

#### Scenario: 应用光标模式长按重复

- **WHEN** 已启用 DECCKM 且长按可配置键栏上键触发重复
- **THEN** 重复序列同样为 `ESC O A`
