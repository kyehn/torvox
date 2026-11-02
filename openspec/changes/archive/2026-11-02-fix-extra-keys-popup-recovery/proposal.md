# Proposal

## Why

`extra-keys` 的迷你解析器承诺「写坏的项 MUST 只丢自己那一项」，上一轮（对象语法错误只丢
自己）把**按键对象**这一层兑现了，但**次级键（popup）对象**这一层没有：`popup` 的值若是
一个对象，其内部的语法错误会一路冒泡到顶层 `parseKey` 的兜底 catch，把**整个主键**丢掉。

用户视角的后果是：一份配了多行键位、其中一项的 `popup` 里少写一个逗号，点开设置后
**整份键栏布局退回默认**——主键（日常使用的那一个）连同其余键位无一幸存，日志里只有
一句笼统的「malformed key object dropped」，看不出是 popup 写坏了。

实测（改动前）：

| `extra-keys` | 解析结果 |
| --- | --- |
| `[[{key:'TAB',popup:{key:'X' bad}},'CTRL']]` | 整份退回默认布局（主键 TAB 与 CTRL 都消失） |

而 popup **语义**层面的坏项（没有 `key`/`macro`、不可用的宏、`FN`/`SHIFT`）早已被逐项
隔离——「丢这个 popup、保住主键」这条路已经存在，唯独**语法**层面没接上。

规范侧已有成文口径：`openspec/specs/termux-configuration` 的「键栏布局的容错」把坏项
定义为「按键对象内部的语法错误」，并要求「这类错误 MUST 只丢该对象那一项」；同一份规范
的「extra-keys 的 popup 语义」也要求「次级按键不可用时 MUST 丢弃该次级按键，MUST NOT
丢弃主键」。popup 对象自己就是一个按键对象，两条口径都指向「只丢 popup」，实现此前
没有兑现。

## What Changes

- `ExtraKeysParser.parsePopup`：popup 是**对象**形式时，其内部的语法错误只丢这个 popup
  ——把光标同步到 popup 对象的闭合符之后，主键与其余字段照常解析，并留下「该 popup 被
  丢弃」的说明。
- 主键的 `key` / `macro` / `display` 语义不变：popup 没了，主键照常渲染与发送，只是不再有
  长按动作。
- 对象本身配不平（缺闭合符）时仍按既有口径处理：`skipPastComposite` 抛出，整份布局退回
  默认并记错误——那种配置无从可靠恢复。

## Capabilities

### New Capabilities

无。本次不引入任何新的能力面。

### Modified Capabilities

- `termux-configuration`：「extra-keys 的 popup 语义」补一条 Scenario，把「popup 对象里的
  语法错误只丢这个 popup、主键照常保留」写成可核对的判据。

## 不做什么

- 不引入 JSON 库：这份语法只有数组、对象、字符串三种记号，且允许单引号与裸键名，
  标准 JSON 解析器反而不收。
- 不改 `docs/specification/`（禁改区）与 `docs/STATUS.md`（文件首行明禁修改）。
- 不放宽「整份文本配不平时退回默认」这条既有边界。
- 不改 popup 的语义口径（没有 `key`/`macro` 的 popup 仍按「不可用」丢弃）。

## Impact

- Affected specs: `termux-configuration`（「extra-keys 的 popup 语义」一条 ADDED Scenario）。
- Affected code:
  - `android/app/src/main/java/terminal/emulator/settings/TermuxProperties.kt`
    （`ExtraKeysParser.parsePopup` / 新增 `dropMalformedPopup`）
  - `android/app/src/test/java/terminal/emulator/settings/TermuxPropertiesParserTest.kt`
    （一条回归用例）
