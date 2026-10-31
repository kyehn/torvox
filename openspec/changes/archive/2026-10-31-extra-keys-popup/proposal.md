# extra-keys 的 popup 次级键语义

## Why

上游 termux 的 `popup` 是一个**完整的按键**（`ExtraKeysInfo.normalizeKeyConfig` 把字符串
包成 `{key: 'X'}`，对象原样成为次级按键），本应用此前只把字符串 `popup` 当纯文本发出，
对象形式整个丢弃并记警告：同一份 `extra-keys` 在两个应用里长按的结果不同。

`ctrl-space-workaround` 的抬起事件此前漏回输入法/编码器，同一次组合键有两条去路。

`openspec/specs/termux-configuration` 的「键栏中不可渲染的项」单条 requirement 超过
500 字符，`openspec validate --all --strict` 不过。

## What Changes

- `ExtraKey.popup` 改为 `ExtraKey?`：popup 与主键同构，可为键名、字面量或宏。
- 解析器支持对象形式的 popup；`key` 与 `macro` 同时出现、popup 里再套 popup 等写坏的
  情况只丢自己那一项并留警告。
- 键栏引入 `KeyAction`（编码器键码 / 特殊动作 / 字面量文本 / 宏），主键与 popup 共用
  同一条派发路径；「能不能渲染」与「渲染成什么」合并到一处判定。
- `ctrl-space-workaround` 的下按与抬起两路事件都被消费，NUL 只发一次。
- 拆分过长的 requirement，popup 语义单独成条。

## Impact

- 受影响能力：`termux-configuration`。
- 用户可见：对象形式的 popup 不再被丢弃；`popup: 'PASTE'` 之类不再被当作纯文本写出；
  Ctrl+空格不再出现两个 NUL。
