# documents-provider Specification

## REMOVED Requirements

### Requirement: 缺失路径的查询按 AOSP 契约抛 FileNotFoundException

缺失路径改回 null 游标：框架的 `DocumentsProvider#query` 捕获 `FileNotFoundException`
后回的同样是 null 游标，客户端看到的响应逐位相同，差异只在框架的 catch 块多打一条
W 级「Failed during query」。

### Requirement: 缺失文档的契约异常不得被改写为成功响应

缺失文档改回 null 游标：null 与占位行同样不暗示文档存在，却不再触发框架的 W 级日志。

## ADDED Requirements

### Requirement: 缺失路径的查询回 null 游标

对不存在的 docId 调 `queryDocument`、对不存在的目录调 `queryChildDocuments`、对越界的
搜索根调 `querySearchDocuments`，MUST 回 null 游标，MUST NOT 抛
`FileNotFoundException`，MUST NOT 回带行的空游标。MUST 用 lstat 语义判存在
（`Files.isSymbolicLink` 为真即地址可解析），断链自身仍在列举里可见、回查同样可达；
判存在 MUST 先于 `watch()` 登记，避免对已消失目录登记观察者。

#### Scenario: 缺失文档回 null 游标

- **WHEN** 客户端（content CLI 或 SAF）以不存在的 docId 查询，无论树与非树形式
- **THEN** 得到 null 游标，MUST NOT 得到占位行或带行的空游标

#### Scenario: 断链可见性与回查一致

- **WHEN** 目录存在一个指向缺失目标的符号链接并在列举中出现
- **THEN** 以该 docId 回查 `queryDocument` 仍返回（lstat 可见），只有真正无
        inode 的缺失路径才回 null

#### Scenario: 缺失父目录不登记观察者

- **WHEN** 对不存在的目录发起 `queryChildDocuments`
- **THEN** 回 null 游标且不得对该目录调用 `watch()` 登记

#### Scenario: 目录列举失败不按空目录回答

- **WHEN** 客户端查询一个存在但内容列举失败的目录
- **THEN** MUST 回 null 游标，MUST NOT 回带行的空游标把「读不到」说成「空目录」

### Requirement: 缺失文档的响应不得被改写为存在

提供者 MUST 对不存在的 docId 回 null 游标，MUST NOT 回占位行或任何暗示文档存在的
响应。回带行的空游标会把「文档不存在」伪装成「文档存在但没有内容」，客户端据此把
新建与改名撞成「路径已存在」，并停在旧快照上。

#### Scenario: 让位文件尚不存在时的回查

- **WHEN** 客户端在「替换保存」流程中回查尚未创建的让位文件
- **THEN** 提供者 MUST 回 null 游标，MUST NOT 回占位行

#### Scenario: 改名让位后原 docId 的回查

- **WHEN** 原文件已被改名让位，客户端回查原 docId
- **THEN** 提供者 MUST 回 null 游标，且 MUST NOT 引发框架 W 级「Failed during query」
