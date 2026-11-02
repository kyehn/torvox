# app-lifecycle Specification

## Purpose

Android Activity 重建、进程回收与前后台切换时的终端生命周期。Kotlin 持有会话表，
Rust 持有 PTY、VT 与终端状态；生命周期事件不得制造重复会话，也不得把已消失的
Rust 状态伪装成可恢复状态。

## Requirements

### Requirement: Activity 重建复用现有会话

Activity 因旋转或其他配置变化被重建时，应用 MUST 复用进程内已有的会话、PTY 与终端
状态，MUST NOT 为同一意图重复创建会话或重复启动 Shell。

#### Scenario: 旋转后仍使用原会话

- **WHEN** 终端已有活动会话时 Activity 被重建
- **THEN** 重建后的界面连接原会话，Shell 进程数量与重建前一致

### Requirement: 进程回收后不恢复会话

系统回收应用进程时，Rust 状态与 PTY 子进程 MAY 随之消失。应用 MUST NOT 持久化或
恢复会话数据，也 MUST NOT 在新进程中伪造已恢复的终端状态；新进程按冷启动路径建立
新的会话。

#### Scenario: 进程回收后冷启动

- **WHEN** 系统终止应用进程后用户重新启动应用
- **THEN** 应用从冷启动状态开始，不恢复旧会话、标题或终端内容

### Requirement: 启动后自动弹出输入法

应用启动并进入终端页面时 MUST 自动弹出输入法，使用户无需先点一下终端就能输入。
`termux.properties` 的 `hide-soft-keyboard-on-startup` 为 `true` 时 MUST NOT 弹出，
且该行为 MUST 只影响启动这一次。

#### Scenario: 默认启动弹出输入法

- **WHEN** 应用启动并显示终端页面
- **THEN** 输入法自动弹出，键盘上方的修饰键栏不被遮挡

#### Scenario: 配置关闭启动弹出

- **WHEN** `termux.properties` 设置 `hide-soft-keyboard-on-startup = true`
- **THEN** 应用启动后不自动弹出输入法

### Requirement: 前后台切换暂停与恢复渲染

应用进入后台时 MUST 暂停当前会话的渲染；切回前台时 MUST 恢复渲染并呈现最新可用帧。
暂停渲染 MUST NOT 关闭会话、停止 PTY 或销毁终端状态。

#### Scenario: 后台期间暂停渲染

- **WHEN** 应用进入后台
- **THEN** 当前会话停止提交新帧，PTY 与会话仍存活

#### Scenario: 回到前台恢复渲染

- **WHEN** 应用从后台回到前台
- **THEN** 当前会话恢复渲染，不出现黑屏或永久停帧
