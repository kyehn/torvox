# documents-provider Specification

## Purpose

向系统文件选择器暴露终端用户文件（`filesDir/home`），并保证其他应用能编辑后回写。
对照 `termux-app` 参考实现 `TermuxDocumentsProvider`。

## Requirements

### Requirement: 提供者声明平台强制的读写权限

AOSP `DocumentsProvider.attachInfo` 要求 `readPermission` 与 `writePermission` **同时**为
`android.permission.MANAGE_DOCUMENTS`，且 `exported` 与 `grantUriPermissions` 为真，
否则 attach 时抛 `SecurityException`（`Provider must be protected by MANAGE_DOCUMENTS`）。
提供器 MUST 声明这两个属性，MUST NOT 用 `android:permission`：该字段填
`ProviderInfo.permission`，内容提供器访问路径不读它，写了等于没写，提供者对全设备应用裸奔。

SAF 客户端 MUST 仍能读写：`enforceReadPermissionInner` / `enforceWritePermissionInner`
先查 `checkUriPermission(本 uri)`，持有逐 URI 授权即直接放行，权限名只出现在报错信息里。

#### Scenario: attach 通过平台检查

- **WHEN** 提供器被系统 attach
- **THEN** 不抛 SecurityException（读写权限均为 MANAGE_DOCUMENTS）

#### Scenario: 持逐 URI 授权的外部应用可写

- **WHEN** 用户经文件选择器把某文档授权给编辑器，编辑器以 `rwt` 写回
- **THEN** 写入成功并落盘

#### Scenario: 无授权的进程被拒

- **WHEN** 既不持 MANAGE_DOCUMENTS 也无逐 URI 授权的进程访问该提供器
- **THEN** 抛 SecurityException

### Requirement: 打开通道可写并广播写回

`openDocument` MUST 按 SAF mode 语义打开可写句柄（`w` 截断、`wa` 追加、`rw` 创建）。
外部应用经该句柄写回后，MUST 广播该文档 URI 与其父目录子文档 URI，
使文件选择器与终端即时反映大小/时间变化。写入方非法（未知 mode、父不是目录、
名称含路径分隔符或为 `.`/`..`）时 MUST 抛 `FileNotFoundException` 而非
`IllegalArgumentException`：后者跨 Binder 会使客户端崩溃而非得到可处理的失败。

#### Scenario: 外部写回后文件选择器得到通知

- **WHEN** 其他应用经 `openOutputStream` 以 `rwt` 写回文档
- **THEN** 文件内容落盘，文档 URI 与父目录子文档 URI 均被 `notifyChange` 广播

#### Scenario: 截断语义不被破坏

- **WHEN** 以 mode `w` 打开已有内容的文档
- **THEN** 写入后文件仅含新内容，旧内容被截断

#### Scenario: 非法名称被拒绝而非静默改写

- **WHEN** 客户端请求的名字为 `a..b.txt` 或含 `/`
- **THEN** 抛 `FileNotFoundException`；MUST NOT 静默改写成别的名字后返回不匹配的 docId
