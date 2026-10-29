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

### Requirement: 读终端 Surface 的用例 MUST 逐轮重取视图

仪器化用例需要从终端 Surface 取像素时，MUST 在每一轮轮询里重新解析视图，
MUST NOT 复用轮询开始时取到的 `SurfaceView` 引用。

系统重建 Activity（主题切换、输入法重排、旋转）后，旧视图的 Surface 已失效，
对着它做像素拷贝只会一直失败，而画面本身并无问题。用例 MUST 把「读回失败」
与「读回成功但颜色不符」分成两路计数并各自进断言信息，否则失败信息指向
错误的根因。

#### Scenario: Activity 重建后仍能读到当前画面

- **WHEN** 轮询期间 Activity 被重建，旧的 `SurfaceView` 引用已失效
- **THEN** 用例 MUST 读到重建后视图上的像素，MUST NOT 因旧引用恒定失败

#### Scenario: 失败信息区分两种形态

- **WHEN** 断言底色不符
- **THEN** 失败信息 MUST 同时给出读回失败次数与命中的最多像素数
