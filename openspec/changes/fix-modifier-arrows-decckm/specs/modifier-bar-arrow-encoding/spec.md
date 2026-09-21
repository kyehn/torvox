# modifier-bar-arrow-encoding Specification

## Purpose

可配置修饰键栏方向键跟随 DECCKM 应用光标模式编码，与输入法/硬件箭头路径一致。

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
