# modifier-bar-sticky-encoding Specification

## Purpose

粘滞 CTRL/ALT 必须在所有输入路径一致生效。可配置键栏的普通按键（ESC、TAB、
HOME、END、PGUP、PGDN 等）此前走裸序列直写 PTY，不读粘滞态：点亮 CTRL 后点其他
键没有组合效果，且 Once 状态无人消费。键栏普通按键须与 IME、硬件路径同样经
`TerminalInputEncoder.encodeKeyEvent` 编码，并在发送后消费 Once 态。

## Requirements

### Requirement: 按键跟随粘滞修饰编码

可配置键栏普通按键点击时 MUST 读取当前 CTRL/ALT 态并经编码器编码；
无修饰时输出 MUST 与原序列一致；带修饰发送后 MUST 消费 Once 态。

#### Scenario: 无修饰点 ESC

- **WHEN** 未点亮修饰且点击 ESC
- **THEN** 发送 `ESC` 单字节，不消费任何状态

#### Scenario: 点亮 CTRL 后点 C 相关键

- **WHEN** CTRL 为 Once 且点击可编码按键
- **THEN** 按编码器输出发送组合序列，且 CTRL 回到 Off
