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

#### Scenario: 空名新建取默认名而非静默失败

- **WHEN** 客户端以空名或纯空白名请求新建（DocumentsUI 在名字为空时仍放行 SAVE，
      系统自带 DownloadsProvider 对同一手势会建出占位文件）
- **THEN** MUST 创建成功而非抛异常：文件按 mimeType 取默认名（`text/plain` →
      `New Document.txt`，目录 mimeType → `New Folder`），返回的 docId 指向该文件
- **AND** 重命名路径 MUST NOT 套用此默认名——空名在重命名语境下仍是客户端契约违反，
      照旧抛 `FileNotFoundException`

### Requirement: 提供者权威用应用标识前缀

文档提供者的 authority MUST 取 `${applicationId}.documents`（即 `com.termux.documents`）。
包名（`terminal.emulator`）只用于 Kotlin 包与类命名，不是应用标识；用包名拼 authority
会与参考实现 `TermuxDocumentsProvider` 不一致，并在多应用同设备时失去隔离。

#### Scenario: authority 取应用标识前缀

- **WHEN** 检查文档提供者的 authority
- **THEN** 等于 `com.termux.documents`

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

### Requirement: 变更操作按树 URI 客户端契约覆盖

`copyDocument` / `moveDocument` / 写回 MUST 按**外部文件管理器的真实寻址形式**覆盖，
即客户端持有 `content://<authority>/tree/<root>/document/<docId>` 形式的 URI：
docId 含 `/` 且需经 URI 编解码往返，平台对每次访问先执行 `enforceTree`
（即 `isChildDocument`）再经 `call()` 路由到本提供者。

只覆盖 plain document URI 会在真实使用中留下无声失败：客户端列表能列出、能新建、
能删除，而复制/移动/覆写正是走树 URI 的那几个动作。覆盖 MUST 断言落盘结果与
返回 docId 一致，MUST NOT 只断言「没抛异常」——docId 与实际落盘不符时客户端下一轮
回查会指到别的文件上。

目录**树**的复制与移动 MUST 一并覆盖：递归复制、孙目录、符号链接 inode 各自就位；
链接只复制 inode 自身，MUST NOT 展开成目标目录树，也不得把 home 之外的目标吸入家目录。

树根前缀别名：外部文件管理器经树 URI 寻址时，传进来的 docId 可能带上树根前缀
（`terminal_home/nix.conf`），而本提供者行内只发行无前缀的相对路径。解码 MUST 先按精确路径
寻址（家目录里真有 `terminal_home` 子目录时，其行内地址不受影响）；仅当精确路径不存在
且去前缀后仍在根内时接受别名。返回的 docId MUST 恒为无前缀的行内形式。

#### Scenario: 树 URI 覆写已存在文件

- **WHEN** 客户端以树 URI 形式打开已存在文件并以 `w` 覆写
- **THEN** 新内容落盘，旧内容被截断

#### Scenario: 树 URI 之间复制

- **WHEN** 客户端在两棵树的子文档 URI 之间 `copyDocument`
- **THEN** 副本落盘于目标父目录，源原样保留，返回的 docId 指向真实落盘文件

#### Scenario: 树 URI 之间移动

- **WHEN** 客户端在两棵树的子文档 URI 之间 `moveDocument`
- **THEN** 文档整体改换父目录，源父目录中不再有该文档，返回的 docId 与实际落盘一致

#### Scenario: 复制目录树

- **WHEN** 客户端复制一个含子目录、孙目录与符号链接的目录
- **THEN** 子目录与孙目录一并落盘，符号链接仍是符号链接且不展开成目标目录树，
      源目录树原样保留

#### Scenario: 站外符号链接不可寻址

- **WHEN** 客户端寻址一个指向 home 之外的符号链接
- **THEN** 以 `FileNotFoundException` 拒绝，且 home 内不留下任何复制产物，
      home 之外的目标不被搬走或改名

### Requirement: 删除按幂等语义覆盖

删除 MUST 幂等：目标已不存在即视为成功。删除的后置条件是「不再存在」，
已满足就没有失败可言；真机 logcat 实证过对已消失目标抛 `IOException`，
经 `DocumentsProvider.call` 变成 `RuntimeException` 交给客户端，文件管理器只能报整次操作失败。
走到这一步的真实场景有三类：终端侧 `rm` 之后客户端缓存里的**幽灵行**、客户端重试、
以及「覆盖」＝先删目标再拷入。判「还在不在」MUST 用 `LinkOption.NOFOLLOW_LINKS` 语义，
否则悬空符号链接（`File.exists()` 为假而其 inode 确实存在）会被误判成已删除，真实失败被吞掉。

#### Scenario: 删除已不存在的条目

- **WHEN** 客户端删除一个已被终端侧删除（或上次调用已删掉）的文档
- **THEN** 调用 MUST 正常返回，MUST NOT 抛异常，且 MUST NOT 影响同目录其他条目

#### Scenario: 删除悬空符号链接

- **WHEN** 客户端删除一个指向已消失目标的符号链接
- **THEN** 链接 inode MUST 被删除，MUST NOT 因目标是悬空的而误报成功或失败

#### Scenario: 真删不掉时给出可处理的失败

- **WHEN** 目标确实存在却删不掉（权限不足、目录非空）
- **THEN** 抛 `FileNotFoundException`，MUST NOT 抛被框架包成 `RuntimeException` 的裸 `IOException`

### Requirement: 重名冲突在全部变更入口取唯一名

重名冲突 MUST 在全部入口（`createDocument`/`renameDocument`/`copyDocument`/`moveDocument`）
走同一策略：追加 `" (2)"` 之类的唯一名后缀，MUST NOT 因重名失败。
框架合约明确允许提供者「为避开重名改写名字」（`DocumentsProvider#createDocument`
与 `#renameDocument` 的注释同此措辞），硬失败只会让「先新建、后改名」的客户端
（文件管理器的建文件/建文件夹、另存为）界面报「路径已存在」并丢掉整次操作。

#### Scenario: 重命名撞上已存在的名字

- **WHEN** 客户端把 `a.txt` 重命名为已存在的 `b.txt`
- **THEN** 落盘为 `b.txt (2)` 并返回其 docId，`b.txt` 的原内容 MUST NOT 被覆盖

### Requirement: 重命名到自身是空操作

重命名到与自身相同的名字 MUST 是空操作，并 MUST 按框架约定返回 `null`
（「原 docId 仍有效时返回 null」，`DocumentsProvider.call` 只在非 `null` 时才重建 URI）。
缺少该守卫时源文件会被当成冲突改名成 `x (2)`——静默换个名字比拒绝更糟。

#### Scenario: 重命名到自身

- **WHEN** 客户端以当前名字重命名文档
- **THEN** 返回 `null`，文件 MUST 仍用原名，MUST NOT 变成 `name (2)`

### Requirement: openDocument 必须兑现 FLAG_DIR_SUPPORTS_CREATE 的承诺

目录在 `queryDocument` 里声明了 `FLAG_DIR_SUPPORTS_CREATE` 与 `FLAG_SUPPORTS_WRITE`。
`openDocument` MUST 按模式位决定是否创建：含 CREATE 的模式（w/rw/rwa/rwt…）在文档
不存在时 MUST 创建它，只读模式 MUST 因不存在而失败。

`openDocument` MUST NOT 自行附加「条目必须已存在」这类限制。此类限制只有在该限制
也能在其余创建入口（`createDocument`）上成立时才有效；否则同一能力一条路放开、
另一条路拦着，拦下的只有按平台惯例行事的客户端。本 provider 的
`sanitize` 只拒空名、`"."`、`".."` 与路径分隔符，不拦点文件，因此「防止凭空造出
`.mkshrc`」不构成有效理由。

含 CREATE 的模式对已存在文档 MUST 按模式语义截断或追加，MUST NOT 因已存在而失败。

#### Scenario: 客户端按平台惯例用 MODE_CREATE 保存

- **WHEN** 客户端对 `FLAG_DIR_SUPPORTS_CREATE` 目录下的某个 docId 以 `"w"` 或 `"rw"`
  调用 `openDocument`，而该文件尚不存在
- **THEN** 调用 MUST 成功并创建该文件，写入内容 MUST 落盘

#### Scenario: 只读打开不存在的文档

- **WHEN** 客户端以 `"r"` 调用 `openDocument`，而该文档不存在
- **THEN** 调用 MUST 以 `FileNotFoundException` 失败，且 MUST NOT 创建任何文件

#### Scenario: 创建能力不得越过根边界

- **WHEN** docId 解析后越出根目录，无论模式是否含 CREATE
- **THEN** 调用 MUST 以 `FileNotFoundException` 失败，且 MUST NOT 在根外创建文件

#### Scenario: 改名不跟随符号链接

- **WHEN** 被操作条目是指向其他条目的符号链接
- **THEN** rename/copy/move MUST 作用于链接自身，MUST NOT 误改链接目标的名字或内容；
      目录树 MUST 连带子目录、孙目录、文件与链接一并就位

#### Scenario: 树 URI 是外部文件管理器的唯一寻址方式

- **WHEN** 客户端经 `ContentResolver.call()` 走树 URI（docId 含 `/`）发起修改、复制或移动
- **THEN** 三者 MUST 各自可用，目标内容 MUST 落盘且返回的 docId MUST 可再次寻址
