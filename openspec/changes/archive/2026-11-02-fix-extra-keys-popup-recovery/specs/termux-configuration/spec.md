## MODIFIED Requirements

### Requirement: extra-keys 的 popup 语义

`popup` MUST 被解析为**完整的次级按键**（上游 termux 的 `ExtraKeysInfo.normalizeKeyConfig`
把字符串包成 `{key: 'X'}`，对象原样成为次级按键）：字符串形式是键名或字面量，对象形式
可带 `macro`。长按该键 MUST 执行次级按键，且 MUST NOT 连带触发主键。

次级按键 MUST 与主键同构：键名走原生键编码器，特殊键（`PASTE`、`KEYBOARD` 等）走键栏
自身的动作，宏按顺序播放，键名表之外的文本整串写出。次级按键的 `display` MUST NOT 改变
它执行的动作。

次级按键不可用（不可用的宏、`FN`、`SHIFT`）时 MUST 丢弃该次级按键并记一条说明，MUST NOT
丢弃主键。popup 是**对象**形式时，其内部的语法错误（缺冒号、多余符号）同样 MUST 只丢
这个次级按键：MUST 把解析位置同步到该 popup 对象的闭合符之后，使主键的其余字段与其余
键位照常解析，并 MUST 记一条说明。

#### Scenario: 长按执行次级按键

- **WHEN** `extra-keys` 里有一项 `{key: '-', popup: 'ESC'}` 且用户长按它
- **THEN** 终端收到 ESC，且不收到主键的 `-`

#### Scenario: 长按播放次级宏

- **WHEN** `extra-keys` 里有一项 `{key: 'HOME', popup: {macro: 'CTRL f d'}}` 且用户长按它
- **THEN** 终端依次收到 Ctrl+f 与 d

#### Scenario: 不可用的次级键只丢长按动作

- **WHEN** `extra-keys` 里有一项 `{key: '-', popup: 'FN'}`
- **THEN** 该键照常渲染并发送 `-`，日志里说明长按动作被丢弃

#### Scenario: 坏掉的次级键对象只丢长按动作

- **WHEN** `extra-keys = [[{key:'TAB',popup:{key:'X' bad}},'CTRL']]`（popup 对象里缺逗号）
- **THEN** 键栏有 TAB 与 CTRL，坏掉的 popup 被丢弃（日志里说明长按动作被丢弃），
      TAB 照常渲染与发送
