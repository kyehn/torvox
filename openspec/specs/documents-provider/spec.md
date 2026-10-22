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

### Requirement: 终端侧改动即时广播，列表不留幽灵行

客户端列表是对提供者的一次查询快照；终端里的 `rm`/`mv`/解压/`git` 绕过提供者，
不广播，快照就永远停在旧状态。提供者 MUST 监视**曾被 `queryChildDocuments` 查询过**
的目录（查询即登记），把条目出现、消失、改名、移动转成对「该目录子文档 URI」的
`notifyChange`；属性变化与带名字的事件同时播报「条目文档 URI」，使文件选择器与终端
互相即时反映对方造成的改动。

真机实证的后果：列表残留终端侧已删除的幽灵行，客户端拿它当已占用的名字，创建/重命名/
粘贴被其自身拒成「路径已存在」，选择「覆盖」绕过预检后又能成功。

监视 MUST 设上限并按最近使用淘汰最久未看的目录：每个受监视目录占一个 inotify 实例，
应用级上限约 128，耗尽会让**全部**监视静默失效。合并窗口内同一目录的多条事件 MUST
合并成一轮广播，批量操作（解压、`git checkout`）不得把客户端逼成逐条重载。

广播与查询端注册 URI 的 docId MUST 一律取行内无前缀形式，MUST NOT 透传客户端字符串。

#### Scenario: 前缀寻址的目录同样收得到广播

- **WHEN** 客户端经树 URI 以带树根前缀的 docId（`terminal_home/sub`，见「树根前缀别名」）
  查询该目录列表，随后终端侧在其中 `rm`／`mv`／解压
- **THEN** `setNotificationUri` 与 `notifyChange` 落在同一个无前缀 URI 上，列表刷新；
  若不归一，两者是**兄弟** URI，平台按 path 段下降匹配永不命中，游标停在旧快照，
  外部文件管理器表现为「创建/复制/重命名没反应」

#### Scenario: 终端侧删除后列表自动消失该条目

- **WHEN** 文件管理器正显示某目录，终端侧删掉其中一个条目
- **THEN** 该目录的子文档 URI MUST 被 `notifyChange` 广播，列表刷新后不再显示该条目

#### Scenario: 终端侧新建后列表自动出现该条目

- **WHEN** 文件管理器正显示某目录，终端侧在其中新建文件或目录
- **THEN** 该目录的子文档 URI MUST 被 `notifyChange` 广播，列表刷新后显示该条目

#### Scenario: 批量改动合并成一轮通知

- **WHEN** 合并窗口内同一目录接连发生多条事件（解压、`git checkout`）
- **THEN** 该目录的子文档 URI MUST 只被广播一次

#### Scenario: 属性变化不改列表

- **WHEN** 目录内某条目的属性（大小、修改时间）变化
- **THEN** 条目自身的文档 URI MUST 被广播，该目录的子文档 URI MUST NOT 被广播

#### Scenario: 只改内容的写入同样被广播

- **WHEN** 终端侧 `>>`、`sed -i` 一类只改文件内容的写入（inode 元数据不变，内核只报 `MODIFY`）
- **THEN** 条目自身的文档 URI MUST 被广播，使客户端的大小/时间列不再停在旧值；
  监视的事件集缺少 `MODIFY` 时这条静默失效

#### Scenario: 被反复查询的目录不在淘汰端

- **WHEN** 客户端反复查询同一目录（每次查询刷新其最近使用位置），其后容量满触发淘汰
- **THEN** 被淘汰的是最久未被**查询**的目录，MUST NOT 是仍在被查询的这一个；
  判重时读一次即刷新的容器访问序是唯一依据，`containsKey` 不算访问

提供者被系统停止（`shutdown`）时 MUST 注销全部已登记的监视、丢弃待广播事件并结束
广播线程：进程可能继续服务其他组件，不释放会让每个看过的目录白占一个 inotify 实例。
停止后再次被拉起时，下一次查询 MUST 重新登记监视并恢复广播，MUST NOT 永久静默。

#### Scenario: 停止后再次浏览目录

- **WHEN** 提供者收到 `shutdown` 后又收到同一目录的 `queryChildDocuments`
- **THEN** 该目录被重新登记，终端侧改动仍会触发该目录子文档 URI 的广播

#### Scenario: 停止时不遗留观察者

- **WHEN** 提供者登记过若干目录后收到 `shutdown`
- **THEN** 每个已登记目录的监视都被注销，广播线程在有限时间内结束，
  停止前已排队但未播报的事件 MUST NOT 在停止后补发

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

### Requirement: 缺失路径的查询按 AOSP 契约抛 FileNotFoundException

对不存在的 docId 调 `queryDocument`、对不存在的目录调 `queryChildDocuments`
MUST 抛 `FileNotFoundException`（裸类型，不得被框架包装成
`RuntimeException`/`IllegalStateException`），树 URI 与非树 URI 一致——AOSP
`DocumentsProvider#queryDocument` 的文档写明「若文档不存在 Implementation
should throw FileNotFoundException」（也即让 SAF 界面把失效路径当成
「文档已消失」重列，而不是把脏错误弹给用户）。实现 MUST 用 lstat 语义判
存在（`Files.isSymbolicLink` 为真即地址可解析），断链自身仍在列举里可见、
回查同样可达；判存在 MUST 先于 `watch()` 登记，避免对已消失目录登记观察者。

#### Scenario: 缺失文档按树/非树 URI 一致拒绝

- **WHEN** 客户端（content CLI 或 SAF）以不存在的 docId 查询，无论树与非树形式
- **THEN** 得到 `FileNotFoundException`（CLI 呈现为 No result found），不抛包装异常

#### Scenario: 断链可见性与回查一致

- **WHEN** 目录存在一个指向缺失目标的符号链接并在列举中出现
- **THEN** 以该 docId 回查 `queryDocument` 仍返回（lstat 可见），只有真正无
      inode 的缺失路径才被拒绝

#### Scenario: 缺失父目录不登记观察者

- **WHEN** 对不存在的目录发起 `queryChildDocuments`
- **THEN** 抛 `FileNotFoundException` 且不得对该目录调用 `watch()` 登记

### Requirement: 树寻址的前置校验由提供者按根内包含性回答

平台对树 URI 的每次访问先调用 `DocumentsProvider.isChildDocument`（`enforceTree`），
默认实现恒为 `false`，会让客户端的树内访问全部被拒成 `SecurityException`——列表能列出、
一操作就失败。提供者 MUST 覆写它，并按**根内包含性**回答：两侧 docId 都解码到合法路径后，
子项解析后的路径须以父项路径加分隔符为前缀（符号链接按目标判定，故 home 内互指的链接
仍可经父目录寻址）。

越界路径与畸形 docId MUST 回答 `false` 并留 WARN，MUST NOT 抛异常：平台在
`enforceTree` 里把异常当作客户端越权，客户端只会看到笼统的拒绝。

#### Scenario: 树内子文档可被树客户端访问

- **WHEN** 外部文件管理器以 `tree/<root>/document/<docId>` 访问 `docId` 为
  `<父>/<子>` 的文档
- **THEN** `isChildDocument(父, 子)` 回答 `true`，访问放行到本提供者的查询/变更实现

#### Scenario: 指向根外的链接不算子项

- **WHEN** `docId` 解析后落在 home 之外（含经符号链接逃逸）
- **THEN** 回答 `false`，且不因该判断抛异常给平台

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

#### Scenario: 树根前缀别名与链接语义

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
走同一策略：生成唯一名消歧（序号插在最后一个点前，无扩展名或前导点则尾部
追加 `" (1)"` 并逐次自增，与 AOSP `FileUtils.buildUniqueFileWithExtension`
的 `n = 0` 先自增约定同型），MUST NOT 因重名失败。
框架合约明确允许提供者「为避开重名改写名字」（`DocumentsProvider#createDocument`
与 `#renameDocument` 的注释同此措辞），硬失败只会让「先新建、后改名」的客户端
（文件管理器的建文件/建文件夹、另存为）界面报「路径已存在」并丢掉整次操作。

#### Scenario: 重命名撞上已存在的名字

- **WHEN** 客户端把 `a.txt` 重命名为已存在的 `b.txt`
- **THEN** 落盘为 `b (1).txt`（序号插在扩展名前、首个冲突序号为 1）并返回其 docId，`b.txt` 的原内容 MUST NOT 被覆盖

### Requirement: 复制保留 POSIX 权限位

复制 MUST 保留源条目的 POSIX 权限位：普通文件与目录 MUST 把源权限位写到目标上，
目录树里的每一项（含嵌套的普通文件与子目录）都适用。复制造的是新 inode，权限位
不会自动跟随——只搬内容不搬权限时，脚本拷过去就不可执行（执行位按进程 umask
落成默认 0644），「可执行文件被修改后不丢失权限」这条承诺随之失效。源或目标不落
在支持 POSIX 权限的文件系统上时 MUST 静默跳过：那是平台限制，不是本次复制引入的
错误。符号链接 MUST 按链接本身复制（不跟随目标），且 MUST NOT 因权限位而报错——
符号链接的权限位在 Linux 上恒为 0777，`lchmod` 无从设置。目录权限 MUST 在整棵子树
写入完成后再落：先落只读权限会让随后创建子项 EACCES，只读目录树必然复制失败。

#### Scenario: 复制可执行文件

- **WHEN** 客户端复制一个带执行位的脚本
- **THEN** 副本的执行位 MUST 与源一致，脚本拷过去仍可直接运行

#### Scenario: 复制目录树

- **WHEN** 客户端复制一个目录，且其中含带执行位的脚本与带执行位的子目录
- **THEN** 副本里的脚本与子目录 MUST 保留各自的执行位

#### Scenario: 复制只读目录树

- **WHEN** 客户端复制一个权限为 `0555` 且含子项的目录
- **THEN** 复制 MUST 成功，副本目录与子项都在，且副本目录的权限仍为 `0555`

#### Scenario: 复制含符号链接的目录树

- **WHEN** 客户端复制的目录树里含符号链接
- **THEN** 副本里的链接 MUST 仍指向同一目标，MUST NOT 被展开成普通文件，
  且复制 MUST NOT 因链接的权限位而失败

### Requirement: 重命名到自身是空操作

重命名到与自身相同的名字 MUST 是空操作，并 MUST 按框架约定返回 `null`
（「原 docId 仍有效时返回 null」，`DocumentsProvider.call` 只在非 `null` 时才重建 URI）。
缺少该守卫时源文件会被当成冲突改名成 `x (1)`——静默换个名字比拒绝更糟。

#### Scenario: 重命名到自身

- **WHEN** 客户端以当前名字重命名文档
- **THEN** 返回 `null`，文件 MUST 仍用原名，MUST NOT 变成 `name (1)`

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
