## MODIFIED Requirements

### Requirement: 快捷方式在应用未启动时也能新建会话

用户触发快捷方式时，应用 MUST 新建一个使用 `/system/bin/sh` 的 Failsafe 会话：Failsafe
不依赖已安装的 bootstrap，宿主未启动时也 MUST 能用。

每次收到 Failsafe Intent，应用 MUST 建出一个 Failsafe 会话：应用已启动时新建的会话
同样 MUST 以 `/system/bin/sh` 启动，已有会话 MUST NOT 受影响。Activity 重建时重复
投递的同一个启动 Intent MUST NOT 再建会话，也 MUST NOT 留下未消费的请求——否则用户
下一次任意新建会话会被劫持成应急会话。

#### Scenario: 冷启动触发快捷方式

- **WHEN** 应用进程未运行时用户触发快捷方式
- **THEN** 应用启动并创建一个使用 `/system/bin/sh` 的会话

#### Scenario: 应用已运行时触发快捷方式

- **WHEN** 应用已在运行时用户触发快捷方式
- **THEN** 应用创建一个使用 `/system/bin/sh` 的会话，已有会话不受影响

#### Scenario: Activity 重建不重复建会话

- **WHEN** Activity 因配置变更重建并再次投递同一个启动 Intent
- **THEN** 不新建会话，也不留下未消费的 Failsafe 请求

#### Scenario: Surface 未就绪时请求不丢失

- **WHEN** 应用已运行时收到 Failsafe Intent，但建会话所需的 Surface 尚未交付
- **THEN** 请求被保留，Surface 交付后建出会话，而不是丢弃请求
