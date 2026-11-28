# Proposal

## Why

Kotlin 侧并列了三套格式化工具：spotless（以 ktlint 格式化并校验）、detekt 的
`detekt-rules-ktlint-wrapper`（承载 ktlint 规则）与两个只提供任务、却无任何门禁请求的
Gradle 插件——`com.ncorti.ktfmt.gradle` 与 `org.jlleitschuh.gradle.ktlint`。

实测两者的自检任务在当前代码树上恒判红，且原因都不是代码风格问题：

- `:app:ktfmtCheck` 报「89 个文件未按 ktfmt 格式化」：代码树按 spotless 的 ktlint 规则
  排版，与 ktfmt 的风格互斥。CI 的格式化动作（`scripts/check.nu` 与 `fmt.yml`）只跑
  `spotlessApply`，ktfmt 的任务从不被请求——它是纯负担，且其失败会误导排查方向。
- `:app:ktlintCheck` 报 `app/build.gradle.kts` 的链式调用换行与多行表达式位置违规：该
  插件按 ktlint 自己的代码风格判定，与 spotless 的 ktlint 步骤口径不同；两者对同一文件
  给出相反结论，任何一侧都无法同时满足。

这两个插件还带来额外的构建期成本：`ktfmt` 插件自身调用了 Gradle 11 将移除的
`Configuration.setVisible(boolean)`，即那唯一一条构建期弃用警告的三个调用方之一。

## What Changes

- 移除 `com.ncorti.ktfmt.gradle` 与 `org.jlleitschuh.gradle.ktlint` 两个插件的声明与
  apply
- 随之移除三个模块 `dependencies` 里只服务于 ktlint CLI 的
  `ktlint("io.github.detekt.sarif4k:sarif4k:0.8.0")` 与
  `ktlint("io.github.oshai:kotlin-logging:8.0.7")`
- 弃用警告的调用方登记改为实测剩下的两个：`dev.detekt` 与 Kotlin Gradle Plugin
- 规范补上「Kotlin 格式化门禁只有一套」的要求与场景

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `build-and-validation`：构建期弃用警告的调用方登记随插件移除而更新。
- `code-health`：新增「Kotlin 格式化门禁只有一套」的要求与场景。

## Impact

- `android/build.gradle.kts`：插件声明与 `subprojects` 中的 apply
- `android/app/build.gradle.kts`、`android/benchmark/build.gradle.kts`、
  `android/baselineprofile/build.gradle.kts`：插件 apply 与 ktlint CLI 依赖
- 受影响的行为：构建期弃用警告少一个调用方；detekt 的 ktlint 规则集不受影响
  （`dev.detekt:detekt-rules-ktlint-wrapper` 自带 ktlint 引擎，移除插件后仍实际生效）
