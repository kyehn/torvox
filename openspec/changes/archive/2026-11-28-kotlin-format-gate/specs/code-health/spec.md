# code-health Specification

## ADDED Requirements

### Requirement: Kotlin 格式化门禁 MUST 只有一套

Kotlin 源码的格式化门禁 MUST 只有一套：spotless 以 ktlint 格式化并校验，detekt 以
`dev.detekt:detekt-rules-ktlint-wrapper` 承载 ktlint 规则。MUST NOT 再并列 apply 只
提供格式化任务、却无任何门禁请求的插件，MUST NOT apply 与 spotless 规则集冲突、其
自检任务在当前代码树上恒判红的第二套格式化器。

移除插件时 MUST 复验既有门禁仍然生效：以会触发被移除插件所承载规则的样例探针判定
规则仍被报出，MUST NOT 以「配置不再报错」代替「规则仍在跑」。只服务于被移除插件的
依赖 MUST 一并移除。

#### Scenario: 存在无人请求的格式化插件

- **WHEN** 构建脚本 apply 了只提供格式化任务、而门禁与持续集成都不请求的插件
- **THEN** 该插件 MUST 移除；移除后 MUST 复验既有门禁仍然生效

#### Scenario: 与主门禁冲突的第二套格式化器

- **WHEN** 第二套格式化器的规则集与 spotless 冲突、其自检任务在当前代码树上恒判红
- **THEN** MUST 移除第二套格式化器，并把格式化门禁收敛为一套
