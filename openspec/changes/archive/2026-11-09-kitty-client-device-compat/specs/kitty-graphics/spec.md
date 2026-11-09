# kitty-graphics Specification

## MODIFIED Requirements

### Requirement: 无法服务的介质由本层按上游错误语义代答

本应用 MUST 对 Kitty 能力探测中本环境无法服务的介质（平台无 POSIX 共享内存、文件介质
路径打不开、文件不是普通文件、按 `S` 声明的长度读不满）按上游的错误语义逐字节回答，
MUST NOT 自创错误消息、MUST NOT 让上游为客户端探测记警告；能服务的介质 MUST 交回上游
处理，MUST NOT 本层代答。静默规则与「无图像 id 不编码应答」 MUST 与上游一致。

临时文件介质（`t=t`）的白名单 MUST 与上游同源（`/tmp`、`/dev/shm`、会话 `TMPDIR` 及其
规范化形式）；本层 MUST 只在能确定上游给出同一条错误时代答，MUST NOT 替上游给出
「不在临时目录」「名字不对」等更具体的错误。

#### Scenario: 共享内存介质在 Android 上被代答

- **WHEN** 客户端探测 `t=s` 且平台无 POSIX 共享内存
- **THEN** 本层 MUST 回 `EINVAL: unsupported medium`，该命令 MUST NOT 到达上游

#### Scenario: 文件介质打不开被代答

- **WHEN** 客户端以文件介质传输且路径按进程工作目录打不开
- **THEN** 本层 MUST 回 `EINVAL: invalid data`，客户端据此回退到直接介质

#### Scenario: 按声明长度读不满被代答

- **WHEN** 客户端以文件介质探测，声明的数据长度超出文件实际长度
- **THEN** 本层 MUST 回 `EINVAL: invalid data`，该命令 MUST NOT 到达上游，
      上游 MUST NOT 为该探测记警告

#### Scenario: 临时文件路径不在白名单时交回上游

- **WHEN** 客户端以临时文件介质探测且路径不在上游同源的白名单内
- **THEN** 命令 MUST 交回上游，本层 MUST NOT 代答更具体的错误

#### Scenario: 能服务的介质交回上游

- **WHEN** 介质为直接介质或文件可打开
- **THEN** 命令 MUST 交回上游，本层 MUST NOT 代答

#### Scenario: 探测命令不回包与静默规则

- **WHEN** 查询无图像 id 或 `q=2`
- **THEN** 命令 MUST 丢弃且 MUST NOT 回包（与上游一致）
