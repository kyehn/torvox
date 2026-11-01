# Proposal

## Why

`extra-keys` 的迷你解析器承诺「写坏的项 MUST 只丢自己那一项」，但**语法**层面写坏的
按键对象会作废**整份**布局：解析器在对象内部抛出的任何语法错误（缺冒号、尾随逗号、
空对象）都会一路冒泡到 `ExtraKeysParser.parse` 的兜底 catch，整份布局退回默认键栏。

用户视角的后果是：一份配了两行键位、其中一行有个对象少写一个冒号，点开设置后键栏
**整块变回默认**——用户配好的键位无一幸存，且没有任何界面上的提示说明发生了什么。

实测（改动前）：

| `extra-keys` | 解析结果 |
| --- | --- |
| `[['ESC',{key:'TAB',}]]` | 整份退回默认布局（尾随逗号被当成语法错误） |
| `[['ESC',{key 'TAB'}]]` | 整份退回默认布局（缺冒号） |
| `[['ESC',{},'CTRL']]` | 整份退回默认布局（空对象） |

而对象内部**语义**层面的坏项（`key` 与 `macro` 同时写、只有 `display`、空键名）早已
被逐项隔离——两条路径的处置不一致，正是缺陷所在。

规范侧已有成文口径（`openspec/specs/termux-configuration` 的「键栏布局的容错」）：
「可解析布局里的坏项 MUST NOT 让整份布局回落到默认」，并把例外限定为「语法上无法配平
的**整份文本**」。一个对象少写冒号时，整份文本仍然配平、仍然可解析，故它属于「坏项」，
不属于例外。实现此前没有兑现这条口径。

## What Changes

- `ExtraKeysParser.parseKey`：按键对象内部出现语法错误时，只丢这一项——把光标同步到
  该对象的闭合符之后，其余键位照常解析，并留下「该项被丢弃」的说明。
- `ExtraKeysParser.parseKeyObject`：收下**尾随逗号**（`{key:'X',}`）与**空对象**（`{}`）。
  - 尾随逗号：上游用的 JSON 解析器同样收这种写法，为一个逗号丢掉整个键位没有道理；
    键照常解析出来，并记一条「尾随逗号」的说明。
  - 空对象：解析为一个没有 `key`/`macro` 的对象，按既有的「只有 display 没有可执行内容」
    口径丢弃——与 `{popup:'|'}` 的处置一致。
- 结构本身配不平（缺闭合符、引号未闭合）的整份文本仍退回默认布局并记错误：
  那种配置无从可靠恢复，逐项恢复会解析出用户没写的键位。

## Capabilities

### New Capabilities

无。本次不引入任何新的能力面。

### Modified Capabilities

- `termux-configuration`：「键栏布局的容错」补两条 Scenario，把「对象内部的语法错误
  只丢自己那一项」与「尾随逗号照常解析」写成可核对的判据。

## 不做什么

- 不引入 JSON 库：这份语法只有数组、对象、字符串三种记号，且允许单引号与裸键名，
  标准 JSON 解析器反而不收。
- 不改 `docs/specification/`（禁改区）与 `docs/STATUS.md`（文件首行明禁修改）。
- 不放宽「整份文本配不平时退回默认」这条既有边界。

## Impact

- Affected specs: `termux-configuration`（「键栏布局的容错」两条 ADDED Scenario）。
- Affected code:
  - `android/app/src/main/java/terminal/emulator/settings/TermuxProperties.kt`
    （`ExtraKeysParser.parseKey` / `parseKeyObject` / `Cursor.skipPastComposite`）
  - `android/app/src/test/java/terminal/emulator/settings/TermuxPropertiesParserTest.kt`
    （三条回归用例）
