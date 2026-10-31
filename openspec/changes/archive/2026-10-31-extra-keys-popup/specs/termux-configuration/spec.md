## MODIFIED Requirements

### Requirement: 键栏中不可渲染的项

未知键名 MUST 被跳过并记警告，MUST NOT 当作字面量渲染。键名 MUST 先套用 termux 上游的
别名表（`ESCAPE`=`ESC`、`CONTROL`=`CTRL`、`RETURN`=`ENTER`、`PAGEUP`=`PGUP`、
`DELETE`=`DEL`、`BACKSLASH`=`\` 等）：上游按同一张表归一，不套表则同一份 `extra-keys`
在两个应用里画出的键栏不一样。

`FN` 与 `SHIFT` 键 MUST NOT 渲染：`SHIFT` 由粘滞修饰栏承担，`FN` 在本应用没有二次映射。

只含坏字段的对象 MUST 只丢自己那一项，MUST NOT 作废整行或整份布局。只有 `display`、
没有 `key`/`macro` 的对象 MUST 被丢弃并记一条说明。

`display` MUST 覆盖键帽文本（`extra-keys-text-all-caps` 照常作用于它），MUST NOT 改变键码、
特殊动作或长按内容。

#### Scenario: 未知键名不落到键栏

- **WHEN** `extra-keys` 里出现本应用不认识的键名
- **THEN** 该键不出现在键栏上，日志里有一条警告，同行其余按键照常渲染

#### Scenario: 别名键名与原名等价

- **WHEN** `extra-keys` 里写 `ESCAPE` 而非 `ESC`
- **THEN** 键栏画出一个与 `ESC` 等价的键，按下发出 ESC

#### Scenario: 空对象只丢自己

- **WHEN** `extra-keys = [['ESC',{popup:'|'},'TAB']]`
- **THEN** 键栏只有 ESC 与 TAB，日志里说明那一项被丢弃

#### Scenario: display 覆盖键帽

- **WHEN** `extra-keys = [[{key:'ESC',display:'quit'}]]` 且 `extra-keys-text-all-caps = false`
- **THEN** 键帽显示 `quit`，按下仍发出 ESC

### Requirement: 返回键、音量键与 Ctrl+空格

`back-key` MUST 支持 `back`/`escape`（默认 `back`）：`escape` 时返回键改发 ESC。

`volume-keys` MUST 支持 `virtual`/`volume`（默认 `virtual`）：`virtual` 时音量减按下即置为
虚拟 Ctrl，按住期间持续生效、抬起时回收；`volume` 时不拦截，系统照常调节音量。外接字母
键盘发出的音量键 MUST NOT 拦截（termux 上游口径：完整键盘在场时不抢它的音量键）；音量加
MUST NOT 拦截——上游把它映射成虚拟 FN，本应用没有 FN 映射。

`ctrl-space-workaround` MUST 为布尔（默认 `false`）：为真时把 Ctrl+空格补发为 NUL。
补发 MUST 只在下按发生一次，而下按与抬起两路事件 MUST 都被消费：抬起事件漏回输入法或
编码器时，同一次组合键会有两条去路。

#### Scenario: escape 模式下返回键发 ESC

- **WHEN** `back-key = escape` 且用户按系统返回键
- **THEN** 终端收到 ESC，而不是退出应用

#### Scenario: 外接字母键盘的音量键不被拦截

- **WHEN** 外接字母键盘在场，用户按它的音量减
- **THEN** 该按键不被应用拦截，交给键盘自身处理

#### Scenario: Ctrl+空格补发为 NUL

- **WHEN** `ctrl-space-workaround = true` 且用户按下 Ctrl+空格
- **THEN** 终端收到一个 NUL

#### Scenario: Ctrl+空格只发一次

- **WHEN** `ctrl-space-workaround = true`，用户按下并抬起 Ctrl+空格
- **THEN** 终端只收到一个 NUL，抬起事件不再产生第二个

## ADDED Requirements

### Requirement: extra-keys 的 popup 语义

`popup` MUST 被解析为**完整的次级按键**（上游 termux 的 `ExtraKeysInfo.normalizeKeyConfig`
把字符串包成 `{key: 'X'}`，对象原样成为次级按键）：字符串形式是键名或字面量，对象形式
可带 `macro`。长按该键 MUST 执行次级按键，且 MUST NOT 连带触发主键。

次级按键 MUST 与主键同构：键名走原生键编码器，特殊键（`PASTE`、`KEYBOARD` 等）走键栏
自身的动作，宏按顺序播放，字面量直接写出。次级按键的 `display` MUST NOT 改变它执行的动作。

次级按键不可用（未知键名、不可用的宏）时 MUST 丢弃该次级按键并记一条说明，MUST NOT
丢弃主键。

#### Scenario: 长按执行次级按键

- **WHEN** `extra-keys` 里有一项 `{key: '-', popup: 'ESC'}` 且用户长按它
- **THEN** 终端收到 ESC，且不收到主键的 `-`

#### Scenario: 长按播放次级宏

- **WHEN** `extra-keys` 里有一项 `{key: 'HOME', popup: {macro: 'CTRL f d'}}` 且用户长按它
- **THEN** 终端依次收到 Ctrl+f 与 d

#### Scenario: 不可用的次级键只丢长按动作

- **WHEN** `extra-keys` 里有一项 `{key: '-', popup: 'NOT_A_KEY'}`
- **THEN** 该键照常渲染并发送 `-`，日志里说明长按动作被丢弃
