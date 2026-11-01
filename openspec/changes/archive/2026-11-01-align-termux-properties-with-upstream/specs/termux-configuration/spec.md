# Spec Delta

## MODIFIED Requirements

### Requirement: 启动时的软键盘

`hide-soft-keyboard-on-startup` MUST 默认为 `false`（与 termux 上游同值）：冷启动时
MUST 在终端取得焦点后请求一次输入法，并把「本视图可接收文本输入」的答案置真——
否则焦点早已就位，平台不会再问，键盘永远不出现。请求 MUST 等窗口取得焦点后再执行；
窗口尚未取得焦点时 MUST 重试，最多 2 秒；成功或超时后 MUST 不再重试。

`true` MUST 表示启动时不主动请求输入法，且平台按 `onCheckIsTextEditor` 自动弹键盘
那条路径也不得在启动时把键盘带出来。

#### Scenario: 默认启动

- **WHEN** 没有写这个键，冷启动应用
- **THEN** 终端取得焦点后弹出一次输入法

#### Scenario: 要求启动即弹

- **WHEN** `hide-soft-keyboard-on-startup = false`，冷启动
- **THEN** 终端拿到焦点后弹出一次输入法；用户收起后不会被再次弹回

#### Scenario: 明确要求启动不弹

- **WHEN** `hide-soft-keyboard-on-startup = true`，冷启动
- **THEN** 键盘保持隐藏，直到用户轻点终端
