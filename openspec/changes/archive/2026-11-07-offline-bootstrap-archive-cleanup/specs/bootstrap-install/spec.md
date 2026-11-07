## ADDED Requirements

### Requirement: 离线安装的中转包必须清理

离线安装（本地路径选包）MUST 把引导归档复制到应用数据目录下的中转位置，并 MUST 在复制
与安装**全过程**结束后删除该中转副本：复制中途失败留下的半截内容同样必须删除。

中转包落在用户数据树内，系统从不清理该目录，故残留即永久占用。

#### Scenario: 使用结束后中转包已删除

- **WHEN** 中转归档复制成功且安装流程结束（无论安装成败）
- **THEN** 中转位置 MUST NOT 留下任何文件（用例 `PreservedArchiveTest`）

#### Scenario: 复制失败不留残件

- **WHEN** 复制中转归档失败
- **THEN** 中转位置 MUST NOT 留下半截内容（用例 `PreservedArchiveTest`）
