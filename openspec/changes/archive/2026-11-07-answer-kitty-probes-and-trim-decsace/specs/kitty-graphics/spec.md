## ADDED Requirements

### Requirement: 无法服务的介质由本层按上游错误语义代答

本应用 MUST 对 Kitty 能力探测中本环境无法服务的介质（平台无 POSIX 共享内存、
文件介质路径打不开）按上游的错误语义逐字节回答，MUST NOT 自创错误消息、
MUST NOT 让上游为客户端探测记警告；能服务的介质 MUST 交回上游处理，
MUST NOT 本层代答。静默规则与「无图像 id 不编码应答」 MUST 与上游一致。

#### Scenario: 共享内存介质在 Android 上被代答

- **WHEN** 客户端探测 `t=s` 且平台无 POSIX 共享内存
- **THEN** 本层 MUST 回 `EINVAL: unsupported medium`，该命令 MUST NOT 到达上游

#### Scenario: 文件介质打不开被代答

- **WHEN** 客户端以文件介质传输且路径按进程工作目录打不开
- **THEN** 本层 MUST 回 `EINVAL: invalid data`，客户端据此回退到直接介质

#### Scenario: 能服务的介质交回上游

- **WHEN** 介质为直接介质或文件可打开
- **THEN** 命令 MUST 交回上游，本层 MUST NOT 代答

#### Scenario: 探测命令不回包与静默规则

- **WHEN** 查询无图像 id 或 `q=2`
- **THEN** 命令 MUST 丢弃且 MUST NOT 回包（与上游一致）
