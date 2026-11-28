# 任务

- [x] 定位两套冗余格式化插件：`:app:ktfmtCheck` 报 89 个文件未按 ktfmt 排版，
      `:app:ktlintCheck` 报 `app/build.gradle.kts` 违规且与 spotless 口径相反
- [x] 移除 `com.ncorti.ktfmt.gradle` 与 `org.jlleitschuh.gradle.ktlint` 的声明与 apply
- [x] 移除三个模块中只服务于 ktlint CLI 的 `ktlint(...)` 依赖
- [x] 复验 detekt 的 ktlint 规则仍实际生效：以通配符导入样例探针判定
      `detekt.ktlint.NoWildcardImports` 仍报出（移除前报、移除后仍报）
- [x] 复验 `detekt`、`spotlessCheck`、`:app:compileDebugKotlin --warning-mode all` 通过，
      弃用警告仍只有 `Configuration.setVisible(boolean)` 一条
- [x] 字节码复核剩余调用方：`dev.detekt`（`DetektBasePlugin`）与 Kotlin Gradle Plugin
      （`ConfigurationsKt`）各调用一次
- [x] 规范补上「Kotlin 格式化门禁只有一套」与调用方登记的更新
