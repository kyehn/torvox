## ADDED Requirements

### Requirement: 缺失文档的契约异常不得被改写为成功响应

提供者 MUST 按 AOSP 契约对不存在的 docId 抛 `FileNotFoundException`，MUST NOT 以空游标、
占位行或改抛其他异常类型替代。框架据该异常在提供者的 catch 块打出 W 级日志；改写响应
语义会把「文档不存在」伪装成「文档存在但没有内容」，客户端据此把新建与改名撞成
「路径已存在」，并停在旧快照上。

#### Scenario: 让位文件尚不存在时的回查

- **WHEN** 客户端在「替换保存」流程中回查尚未创建的让位文件
- **THEN** 提供者 MUST 抛 `FileNotFoundException`，MUST NOT 返回空游标或占位行

#### Scenario: 改名让位后原 docId 的回查

- **WHEN** 原文件已被改名让位，客户端回查原 docId
- **THEN** 提供者 MUST 抛 `FileNotFoundException`；该次回查引发的框架告警归入客户端
      流程，应用 MUST NOT 为其改写契约语义
