## Why

修饰键栏普通按键（ESC/TAB/HOME/END/PGUP/PGDN 等）走裸序列直写 PTY，不读 CTRL/ALT 粘滞态：点亮 CTRL 后再点其他键无组合效果，Once 状态也无人消费。IME 提交与硬件按键两条路径都消费修饰，唯独键栏是裸路径。

## What Changes

- 可配置键栏普通按键点击时读取当前 CTRL/ALT 态，经 `TerminalInputEncoder.encodeKeyEvent` 编码；无修饰时输出与原序列一致。
- 发送带修饰的按键后消费 Once 粘滞态（Locked 不受影响）。
- 单测覆盖：无修饰输出不变、Ctrl 组合折叠、Once 消费回调。

## Capabilities

### New Capabilities

- `modifier-bar-sticky-encoding`: 键栏按键跟随粘滞修饰键编码。

### Modified Capabilities

无。

## Impact

- `ModifierBar.kt` 可配置路径；`TerminalScreen.kt` 接线（消费回调）。
- `ModifierBarRobolectricTest` 新增用例。
