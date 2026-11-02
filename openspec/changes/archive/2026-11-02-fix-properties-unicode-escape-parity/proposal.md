# Proposal

## Why

`PropertiesFile` 把非法 `\u` 转义降级成字面量，好让 JDK 不至于抛
`IllegalArgumentException` 拒绝整份文件。降级用的正则是 `\\u(?![0-9a-fA-F]{4})`：
匹配到就把 `\u` 换成 `u`。

**它没有区分反斜杠的奇偶**。JDK 按两两配对消费反斜杠——`\\` 是字面反斜杠，其后的
`u` 只是普通字符，根本不是转义符。于是：

| 输入 | 降级后 | JDK 的结果 |
| --- | --- | --- |
| `\u12` | `u12` | 合法，值为 `u12` |
| `\\u12` | `\u12` | **抛「Malformed \uxxxx encoding」，拒绝整份文件** |
| `\\user` | `\user` | **同上，整份文件被拒** |

后果不只是「这个键没生效」，而是整份 `termux.properties` 被作废：解析抛出的
`IllegalArgumentException` 一路穿过 `TermuxPropertiesParser.parse` →
`TermuxPropertiesRepository.loadOnce` → `TerminalRuntime.start()`，而后者注释明写
「读失败不阻断启动——最坏是用内置默认值」。用户在值里写一个 `\\user`（转义反斜杠
加普通词）就会踩到：整个应用的配置退回内置默认值，且异常直接打断启动。

这条降级规则本身是上一轮「语法交给 JDK」引入的：自研解析器遇到不认识的转义时按
普通字符处理，不会拒绝整份文件；换成 JDK 后必须自己兜住这个差异，而兜底逻辑把
「不是转义符的 `u`」误判成了转义符。

## What Changes

- 降级前先判定「这个 `u` 到底是不是转义符」：只有反斜杠数为**奇数**时
  （JDK 消费掉成对的 `\\` 后还剩一个落单的 `\`）才构成 `\u` 转义，才需要降级。
- 降级时只删掉引出转义的那一个反斜杠，前面的成对反斜杠原样保留：`\\\u12` 降级成
  `\\u12`，JDK 解析出的值是 `\u12`，与「删一个反斜杠」的语义一致。
- 反斜杠数为偶数时一个字符都不动：`\\u12` 保持原样，JDK 正常解析成 `\u12`。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `termux-configuration`：「properties 语法」补一条「转义反斜杠后的 `u` 不是转义符」
  的容错口径。

## 不做什么

- 不改 `PropertiesFile` 委托 JDK 的整体设计：上一轮已把语法交给 JDK，本轮只修
  降级规则的判定条件。
- 不改「非法转义只降级该值、其余键照常生效」的既有口径。
- 不引入自研的转义解码：合法的 `\uXXXX` 仍由 JDK 解码。
- 不改 `docs/specification/`（禁改区）与 `docs/STATUS.md`。

## Impact

- Affected specs: `termux-configuration`（「properties 语法」一条 MODIFIED
  Requirement）。
- Affected code:
  - `android/app/src/main/java/terminal/emulator/settings/TermuxProperties.kt`
    （`MALFORMED_UNICODE_ESCAPE` 与新增的 `downgradeMalformedUnicodeEscapes`）
  - `android/app/src/test/java/terminal/emulator/settings/TermuxPropertiesParserTest.kt`
    （新增两条回归用例）
