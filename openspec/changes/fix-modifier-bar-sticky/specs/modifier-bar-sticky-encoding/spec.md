# modifier-bar-sticky-encoding Specification

## Purpose

键栏按键与 IME/硬件路径一致地跟随粘滞修饰键。

## ADDED Requirements

### Requirement: 按键跟随粘滞修饰编码

可配置键栏普通按键点击时 MUST 读取当前 CTRL/ALT 态并经编码器编码；
无修饰时输出 MUST 与原序列一致；带修饰发送后 MUST 消费 Once 态。

#### Scenario: 无修饰点 ESC

- **WHEN** 未点亮修饰且点击 ESC
- **THEN** 发送 `ESC` 单字节，不消费任何状态

#### Scenario: 点亮 CTRL 后点 C 相关键

- **WHEN** CTRL 为 Once 且点击可编码按键
- **THEN** 按编码器输出发送组合序列，且 CTRL 回到 Off
