## ADDED Requirements

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
