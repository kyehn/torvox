# terminal-foreground-service Specification

## Purpose

终端在前台服务中维持常驻通知、后台存活与用户可触发的会话退出能力。通知内容与
操作入口以 `docs/specification/DESIGN.md` 的软件要求及 termux-app 行为为准。

## Requirements

### Requirement: 常驻通知显示会话数量

存在活动会话时，应用 MUST 运行前台服务并发布一条常驻通知。通知 MUST 显示当前会话
数量；会话增减后 MUST 更新该数量，降到零时 MUST 停止服务并释放相关资源。

#### Scenario: 单会话通知

- **WHEN** 应用只有一个活动会话
- **THEN** 常驻通知显示单会话状态文案

#### Scenario: 多会话通知

- **WHEN** 应用有三个活动会话
- **THEN** 常驻通知显示会话数量 3

#### Scenario: 最后一个会话关闭

- **WHEN** 最后一个活动会话关闭
- **THEN** 前台服务停止，通知不再常驻

### Requirement: 通知提供唤醒锁与退出入口

常驻通知 MUST 提供“申请唤醒锁”与“退出”两个操作。申请唤醒锁 MUST 获取可续期的
部分唤醒锁，使应用位于后台时会话仍能继续活动；退出 MUST 关闭全部会话并停止前台服务。

#### Scenario: 申请唤醒锁

- **WHEN** 用户点击通知中的“申请唤醒锁”
- **THEN** 应用持有部分唤醒锁，终端在后台仍可继续运行

#### Scenario: 从通知退出

- **WHEN** 用户点击通知中的“退出”
- **THEN** 全部会话关闭，前台服务停止，唤醒锁被释放

### Requirement: 申请通知权限

应用 MUST 在需要发布常驻通知时申请 `POST_NOTIFICATIONS` 运行时权限。权限缺失或被拒绝
MUST NOT 使终端会话崩溃；平台允许前台服务继续时，应用 MUST 保持会话可用。

#### Scenario: 首次启动申请权限

- **WHEN** 应用首次启动且平台需要通知权限
- **THEN** 应用发起一次 `POST_NOTIFICATIONS` 权限请求

#### Scenario: 用户拒绝通知权限

- **WHEN** 用户拒绝通知权限
- **THEN** 终端仍可创建会话并输入，应用不崩溃
