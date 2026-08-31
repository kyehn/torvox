## Purpose

防止同一设备上其他 uid 进程通过 MCP Unix socket 越权调用终端能力，确保仅应用自身可访问。

## ADDED Requirements

### Requirement: Peer Credential 校验

系统 SHALL 在 `mcp` Unix socket `accept` 后调用 `getsockopt(SO_PEERCRED)` 获取对端 `uid`，若 `uid != appUid` 则立即 `close` 连接且不处理请求。

#### Scenario: 同 uid 放行

- **WHEN** 应用内 `MCPClient` 以相同 uid 连接 socket
- **THEN** 请求正常处理，返回结果

#### Scenario: 异 uid 拒绝

- **WHEN** 其他应用（不同 uid）尝试连接
- **THEN** 连接在握手前被关闭，服务端日志含 `peer uid mismatch`

#### Scenario: 无法获取凭证时拒绝

- **WHEN** `getsockopt` 失败（如非 Unix socket）
- **THEN** 系统 SHALL 拒绝连接，视为不安全

### Requirement: 隐私与最小权限

系统 SHALL 不在校验失败时泄露会话内容，仅返回通用拒绝错误。

#### Scenario: 拒绝不泄露

- **WHEN** 校验失败
- **THEN** 对端收到的错误为 `PermissionDenied`，不含会话路径或内容
