# build-and-validation Specification

## MODIFIED Requirements

### Requirement: 构建期弃用警告必须归因到具体调用点

构建期出现的弃用警告 MUST 归因到具体插件与调用点并登记，不得笼统记为「第三方问题」，
也不得凭插件名猜来源。归因 MUST 由实测得出：调用方 MUST 在插件字节码中定位到被弃用
方法的真实调用者。消除该警告若需改动禁改区的构建脚本（插件版本声明），本项只登记、
不实施。

#### Scenario: 当前唯一的弃用警告及其调用方

- **WHEN** 以 Gradle 9.9.3 + AGP 9.6.0-alpha07 执行 `:app:compileDebugKotlin --warning-mode all`
- **THEN** 弃用警告 MUST 只有一条，即 Gradle 对 `Configuration.setVisible(boolean)` 的弃用
      （Gradle 11 移除）
- **AND** 调用方 MUST 登记为构建期插件：`dev.detekt` 2.0.0-alpha.6
      （`DetektBasePlugin`）与 Kotlin Gradle Plugin 2.4.21（`ConfigurationsKt`）
      各调用一次
- **AND** 仓内 Gradle 脚本对 `setVisible` / `visible =` 的引用 MUST 为 0 处，AGP 的
      `gradle`、`builder`、`gradle-api` 三个 jar 对该方法的引用亦 MUST 为 0 处

#### Scenario: 禁改区内的插件升级只登记不实施

- **WHEN** 消除该警告需要改动禁改区的构建脚本（插件版本声明）
- **THEN** MUST 维持登记状态，未经维护者明确同意 MUST NOT 实施
