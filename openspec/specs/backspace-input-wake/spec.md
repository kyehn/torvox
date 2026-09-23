# backspace-input-wake Specification

## Purpose

用户主诉"退格慢"：终端空闲 5 秒后（渲染循环进入 500ms 空闲 latch，
vsync 泵帧停止），按键后画面更新明显延迟。滚动卡顿根因（T1）是渲染循环
停在 500ms 空闲 latch；退格/按键在空闲态命中同一条延迟路径。本 spec
锁定输入→回显链路上缺失的渲染唤醒。

## Requirements

### Requirement: 一次输入写入必须唤醒渲染循环

任何写 PTY 的输入（`Bridge.writeToPty` / `processKeyEvent` /
`encodeMouseEvent`）MUST 使渲染线程离开 idle park 立即渲染一帧并刷新
idle-clock，使循环回到 17ms 活跃 latch，shell 回显在下一次 latch tick
（≤17ms）内渲染，不得等满 500ms 空闲 latch。

#### Scenario: 空闲 5 秒后实体键盘/IME sendKeyEvent 退格

- **WHEN** 终端空闲 >5s（循环停在 500ms idle latch）且用户经
      `processKeyEvent` 路径按退格
- **THEN** 退格字节写入 PTY 即刻唤醒渲染线程（无需等待超时），
      回显在 ≤17ms 活跃 latch 内上屏，输入→回显不出现 ~500ms 级延迟

#### Scenario: 空闲 5 秒后 IME deleteSurroundingText 退格

- **WHEN** 终端空闲 >5s 且退格经 `TerminalViewModel.writeToPty` 写入
- **THEN** 行为不变（已唤醒）：回显 ≤17ms 活跃 latch 内渲染，
      不得回退

### Requirement: 空闲回落策略不变

输入写入唤醒只作用于当次输入；输入停止 >5s 后循环 MUST 仍回落 500ms
idle latch 保持省电，不得因输入唤醒形成自持活跃循环。

#### Scenario: 单次输入后回落 idle

- **WHEN** 单次退格后无后续输入/输出超过 5s
- **THEN** idle-clock 过期后循环回到 500ms idle latch（≈2fps），
      与 T1 回落语义一致

### Requirement: latch 门控决策可测试

latch 选择（idle 500ms / active 17ms）MUST 由纯函数门控
（idle-clock 过期 ∧ 无滚动运动 → idle latch），与
`hasScrollMotion()` 门控风格一致；门控函数与新
`notifyRender` 刷新 idle-clock 的契约必须有单元测试。

#### Scenario: 新鲜 idle-clock 选活跃 latch

- **WHEN** idle-clock 新鲜（如刚 notifyRender）或滚动运动窗口内
- **THEN** 门控返回活跃 latch（17ms），回显快速上屏
