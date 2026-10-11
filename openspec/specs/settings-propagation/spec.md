# settings-propagation Specification

## Purpose
设置下发的范围语义：live 设置推送全部会话并即时生效，spawn-time 设置仅影响新建会话。本能力覆盖设置存储到 Rust 会话之间的可观测下发契约。

## Requirements

### Requirement: live 设置推送全部会话

主题、字体等 live 设置变更 MUST 推送到全部现存会话并即时生效，MUST NOT 只作用于当前会话。

#### Scenario: 主题切换作用全部会话

- **WHEN** 用户切换终端主题且存在多个会话
- **THEN** 全部会话的配色同时更新，无需重建会话

#### Scenario: 字体变更作用全部会话

- **WHEN** 用户变更主字体且存在多个会话
- **THEN** 全部会话的渲染字体同时更新

### Requirement: spawn-time 设置仅影响新建

启动入口等 spawn-time 设置 MUST 只影响新建会话，MUST NOT 追溯修改现存会话的进程与环境。

#### Scenario: 启动入口变更不影响现存会话

- **WHEN** 用户修改 Shell 启动入口后查看现存会话
- **THEN** 现存会话仍运行原入口进程，仅新建会话使用新入口
