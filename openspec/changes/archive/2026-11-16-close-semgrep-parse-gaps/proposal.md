# Proposal

## Why

门禁里的 `semgrep scan --config .semgrep/rules.yml` 对 11 个 cucumber 步骤定义文件报
`PartialParsing`：这些文件用了 CJK 标识符作步骤注解（`io.cucumber.java.zh_cn.假如` 等），
semgrep 的 Kotlin 解析器解析不了，104 个区域（每个步骤定义的注解与函数体）被整段跳过。
扫描器「没报缺陷」与「扫过了」不是一回事——落在这些区域里的规则命中永远不会被报出，
而适用于仪器测试源码的规则恰恰有 `no-junit-ignore`、`no-junit-assume`、
`no-test-early-return`、`no-inline-focus-event` 等几条。

`code-health` 要求审计留档覆盖当前代码树。解析缺口使这条要求在事实上无法成立：
缺口既没被登记，也没被消除，而扫描器的退出码仍是 0。

## What Changes

- 代码：11 个 cucumber 步骤定义文件的步骤注解由 `io.cucumber.java.zh_cn.{假如,当,那么}`
  改为 `io.cucumber.java.en.{Given,When,Then}`。步骤正则、Gherkin 特性文件与匹配语义
  均不变：cucumber 按正则匹配步骤文本，不校验注解的关键词类别（仓内已有反例——
  特性文件里的 `而且 …` 由 `@那么` 的步骤定义接住）。
- 规格：`code-health` 的「规范声明的静态检查工具必须与验证门禁一致」增「解析失败即跳过」
  （11 文件 / 104 区域 → 0）。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `code-health`：门禁内静态检查工具不得留下解析缺口，缺口须计入留档并被消除。
