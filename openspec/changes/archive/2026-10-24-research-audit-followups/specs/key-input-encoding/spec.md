## ADDED Requirements

### Requirement: Android 键码映射覆盖 Termux 键表

`KeyEvent` 键码到上游 `key::Key` 的映射 MUST 覆盖 Termux `KeyHandler` 的键集。
未映射的键退回按键文本；对没有文本的键（`KEYCODE_BREAK` 等）该回退为空，整个键
静默失效。映射只表达「是哪个键」，转义序列 MUST 仍由上游编码器按活体模式产出。

#### Scenario: Pause/Break 到达编码器

- **WHEN** 硬件键盘按下 Pause/Break（`KEYCODE_BREAK`）
- **THEN** 事件按 `key::Key::Pause` 编码发出，MUST NOT 因无文本回退而被丢弃
      （用例 `pause_break_key_maps_to_pause`）

#### Scenario: 系统导航键仍不进键表

- **WHEN** 按下返回键（`KEYCODE_BACK`）
- **THEN** 该键不属于终端键表，由宿主的手势与返回处理消费
