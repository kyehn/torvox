## ADDED Requirements

### Requirement: 外部寻址一律按根内包含性判定

提供者对每个外部寻址——查询、列举、创建、删除、改名、复制、移动与树寻址校验——MUST
先把 docId 解码成规范路径并判定其落在家目录根内；站外路径 MUST 拒绝，MUST NOT 回落
成宽松路径。根内的符号链接条目 MUST 按链接自身操作：删除只删链接 inode，复制只复制
inode，站外目标 MUST NOT 被读进家目录。

#### Scenario: 站外 docId 一律拒绝

- **WHEN** 客户端传入解码后落在家目录之外的 docId
- **THEN** 查询回 null 游标、变更拒绝该操作，MUST NOT 触达该路径

#### Scenario: 链接条目按链接自身操作

- **WHEN** 客户端删除或复制家目录内指向站外的符号链接
- **THEN** 只操作链接 inode，MUST NOT 删除目标内容或把目标树吸进家目录

#### Scenario: 路径解析失败即拒绝

- **WHEN** 规范路径解析失败
- **THEN** 该次操作失败，MUST NOT 改用词面路径继续处理
