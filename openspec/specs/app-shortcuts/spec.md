# app-shortcuts Specification

## Purpose

启动器长按图标暴露的静态应用快捷方式。`docs/specification/DESIGN.md` 只允许一项：
新建 Failsafe 会话。

## Requirements

### Requirement: 只有新建 Failsafe 会话一项快捷方式

静态快捷方式 MUST 有且只有「新建会话（Failsafe）」一项，MUST NOT 增加其他快捷方式。
图标 MUST 与 termux-app 的 `ic_new_session` 一致。

#### Scenario: 快捷方式清单只有一项

- **WHEN** 读取 `res/xml/shortcuts.xml`
- **THEN** 其中只声明一个 `shortcut`，其 `shortcutId` 表示新建 Failsafe 会话

#### Scenario: 图标与 termux 一致

- **WHEN** 对比快捷方式图标
- **THEN** 使用与 termux-app `ic_new_session` 相同的图标资源

### Requirement: 快捷方式在应用未启动时也能新建会话

用户触发快捷方式时，应用 MUST 新建一个使用 `/system/bin/sh` 的 Failsafe 会话：Failsafe
不依赖已安装的 bootstrap，宿主未启动时也 MUST 能用。

每次收到 Failsafe Intent，应用 MUST 建出一个 Failsafe 会话：应用已启动时新建的会话
同样 MUST 以 `/system/bin/sh` 启动，已有会话 MUST NOT 受影响。Activity 重建时重复
投递的同一个启动 Intent MUST NOT 再建会话，也 MUST NOT 留下未消费的请求——否则用户
下一次任意新建会话会被劫持成应急会话。

#### Scenario: 冷启动触发快捷方式

- **WHEN** 应用进程未运行时用户触发快捷方式
- **THEN** 应用启动并创建一个使用 `/system/bin/sh` 的会话

#### Scenario: 应用已运行时触发快捷方式

- **WHEN** 应用已在运行时用户触发快捷方式
- **THEN** 应用创建一个使用 `/system/bin/sh` 的会话，已有会话不受影响

#### Scenario: Activity 重建不重复建会话

- **WHEN** Activity 因配置变更重建并再次投递同一个启动 Intent
- **THEN** 不新建会话，也不留下未消费的 Failsafe 请求

#### Scenario: Surface 未就绪时请求不丢失

- **WHEN** 应用已运行时收到 Failsafe Intent，但建会话所需的 Surface 尚未交付
- **THEN** 请求被保留，Surface 交付后建出会话，而不是丢弃请求

### Requirement: 启动 Intent 与 termux 兼容

快捷方式 MUST 以 `android.intent.action.RUN` 启动 `MainActivity`，并携带
`com.termux.app.failsafe_session=true` extra——与 termux-app 同名，使既有快捷方式与
Tasker 任务继续可用。`targetPackage` MUST 保持为 `com.termux`。

#### Scenario: 携带 termux 同名 extra

- **WHEN** 读取快捷方式声明的 Intent
- **THEN** action 为 `android.intent.action.RUN`，extra `com.termux.app.failsafe_session`
      为 `true`，target package 为 `com.termux`

#### Scenario: 收到外部 Intent 时按 Failsafe 处理

- **WHEN** 应用收到带 `com.termux.app.failsafe_session=true` 的 Intent
- **THEN** 应用创建一个 Failsafe 会话，而不是普通会话
