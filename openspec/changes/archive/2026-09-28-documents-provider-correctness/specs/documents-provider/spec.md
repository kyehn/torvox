## ADDED Requirements

### Requirement: 提供者权威用应用标识前缀

文档提供者的 authority MUST 取 `${applicationId}.documents`（即 `com.termux.documents`），
与 `FileProvider` 同源。包名（`terminal.emulator`）只用于 Kotlin 包与类命名，
不是应用标识；用包名拼 authority 会与参考实现 `TermuxDocumentsProvider` 不一致，
并在多应用同设备时失去隔离。

#### Scenario: 与 FileProvider 同源

- **WHEN** 检查两个提供者的 authority
- **THEN** 均为 `com.termux` 前缀，仅末段不同（`documents` / `fileprovider`）

### Requirement: 查询行只写请求投影内的列

`queryRoots`/`queryDocument`/`queryChildDocuments`/`querySearchDocuments` 组装行时
MUST 只写请求投影包含的列，缺失列跳过而非填 `null`。对请求投影无条件写入会在
系统文件选择器只请求少数字段时抛 `IndexOutOfBoundsException`，中断整次目录浏览。
默认投影（全列）行为 MUST NOT 改变。

#### Scenario: 子集投影不崩溃

- **WHEN** 以仅含 `COLUMN_ROOT_ID` 与 `COLUMN_FLAGS` 的投影查询根
- **THEN** 正常返回一行，不抛异常

#### Scenario: 默认投影不变

- **WHEN** 以 `null` 投影查询文档
- **THEN** 返回全部六个文档列
