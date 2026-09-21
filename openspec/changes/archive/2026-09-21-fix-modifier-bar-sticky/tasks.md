# 键栏粘滞修饰编码

## 上下文

- `ModifierBar.kt:984-991` 普通按键裸序列直发；`ModifierBarActions` 无修饰态与消费回调。
- `TerminalInputEncoder.encodeKeyEvent` 已有完整修饰编码；无修饰输出与原序列一致。
- `TerminalScreen.kt:984-986` 接线 `onKeyClick` 直写 PTY；`consumeOneShotModifiers` 在 IME/硬件路径消费。

## 任务

1. 可配置路径普通按键经编码器跟随 CTRL/ALT 态；带修饰发送后调消费回调。
2. TerminalScreen 接消费回调到 `viewModel.consumeOneShotModifiers()`。
3. 单测：无修饰不变、组合折叠、消费回调触发。
4. 真机验证：CTRL+C、ALT 相关组合。
