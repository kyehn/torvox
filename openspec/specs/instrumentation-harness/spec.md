# instrumentation-harness Specification

## Purpose

仪器化用例共享 JUnit 规则，而规则会驱动真实的 `MainActivity` 与真实的原生会话。
一条多余的 Activity 规则就会多拉起一个 Activity 实例：两个实例同时轮询同一套
原生事件通道，退出事件、渲染事件被另一个实例消费，用例于是偶发失败且失败信息
指向别处。本能力固定「哪类用例用哪个基类」，把这条竞态挡在编写阶段。

## Requirements

### Requirement: Compose 规则只属于 UI 用例基类

需要 Compose 语义树的仪器化用例 MUST 继承 `ComposeTerminalLogcatTest`；不驱动
界面的用例 MUST 继承 `TerminalLogcatTest`，后者 MUST NOT 声明
`createAndroidComposeRule`。

UI 用例 MUST NOT 再自行声明 `createAndroidComposeRule`。确需自行管理 Activity
的用例（例如只用 UiAutomator 驱动系统界面的用例）MUST 只声明一条 Activity
规则，且 MUST 继承 `TerminalLogcatTest`。

#### Scenario: 非 UI 用例不启动 Activity

- **WHEN** 用例只经 JNI、`startService` 或文件系统验证行为
- **THEN** 它继承的基类 MUST NOT 拉起 `MainActivity`
- **AND** 原生会话事件 MUST 只由用例自身的 `pollEvent` 消费

#### Scenario: UI 用例只有一条 Activity 规则

- **WHEN** 用例通过 `composeTestRule` 断言界面
- **THEN** 规则链中 MUST 只有一条 Activity 启动规则
- **AND** `TerminalLogcatRule` MUST 位于其外层，使 Activity 启动失败同样携带日志尾部
