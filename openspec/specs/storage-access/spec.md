# storage-access Specification

## Purpose

外部存储的可达性取决于两件事：本应用**自己**声明了文件权限（不是靠传递依赖
顺带捎上），以及用户在系统设置里自行打开了「所有文件访问」。本规范钉住声明
这一侧，并规定运行期不得主动弹窗索要——授权入口在系统设置里，由用户掌握。

## Requirements

### Requirement: 应用显式声明外部存储权限

本应用 MUST 在自己的清单里显式声明 `WRITE_EXTERNAL_STORAGE` 与
`MANAGE_EXTERNAL_STORAGE`。MUST NOT 依赖传递依赖顺带捎上这些声明：依赖升级
或移除会让声明消失，用户看到的是「装了却访问不了外部存储」，而清单里查不到
任何授权来源。

`targetSdk` 由构建规范固定为 28，故 `WRITE_EXTERNAL_STORAGE` 在安装时即授予；
`MANAGE_EXTERNAL_STORAGE` 是「所有文件访问」开关，由用户在系统设置里自行开启。

#### Scenario: 清单显式声明两项权限

- **WHEN** 读取应用合并后的清单
- **THEN** `requestedPermissions` MUST 同时包含 `WRITE_EXTERNAL_STORAGE` 与
  `MANAGE_EXTERNAL_STORAGE`，且来源是本应用自身的清单

#### Scenario: 声明不因依赖变动而消失

- **WHEN** 移除所有传递声明存储权限的依赖
- **THEN** 合并清单里的两项声明 MUST 仍然存在

### Requirement: 运行期不主动索要存储授权

本应用 MUST NOT 在运行期主动弹出存储权限对话框。授权入口 MUST 留在系统设置，
由用户自行决定是否开启——应用内的提示只说明「可在系统设置中开启」，MUST NOT
替用户做决定。

#### Scenario: 首次进入不弹权限框

- **WHEN** 应用首次启动且外部存储尚未授权
- **THEN** MUST NOT 出现存储权限对话框；终端照常可用

#### Scenario: 未授权时的说明指向系统设置

- **WHEN** 用户在设置里查看存储相关项
- **THEN** 说明 MUST 指向系统设置的「所有文件访问」，MUST NOT 表现为可在应用内
  直接授权

### Requirement: 授权后外部存储对宿主与子进程可读写

用户开启「所有文件访问」后，应用进程 MUST 能列出并读写外部存储根目录。子
进程与宿主同 UID，MUST 继承同一份授权。

#### Scenario: 授权后根目录可列可写

- **WHEN** 用户已开启「所有文件访问」
- **THEN** 应用进程 MUST 能列出外部存储根目录的条目，并能在其中创建、读回文件

#### Scenario: 未授权时不得伪造可读

- **WHEN** 用户未开启「所有文件访问」
- **THEN** 应用 MUST NOT 报出「外部存储可用」的结论；可达性检查 MUST 如实失败
  并说明原因
