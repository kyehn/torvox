## REMOVED Requirements

### Requirement: colors.properties 覆盖

## MODIFIED Requirements

### Requirement: 读取路径与容错

解析器 MUST 依次尝试 `~/.termux/termux.properties` 与 `~/.config/termux/termux.properties`，
前者不存在或不可读时回落后者。文件读取或解析失败 MUST 保留上一次成功的解析结果
并记录错误，MUST NOT 抛给调用方、MUST NOT 清空已有配置。

只认**常规文件**且 MUST NOT 跟随符号链接：该文件由外部写入，链接可以把读取带到
家目录之外，也可能指向设备；非常规文件（管道、目录）同样跳过。

读取 MUST 为 UTF-8；单文件超过 64 KiB MUST 视为无效并跳过（MUST NOT 读进内存）。
应用 MUST NOT 创建、写入或修改该文件。

`colors.properties` MUST NOT 被读取或解析，终端配色只来自内置主题。

#### Scenario: 文件写到一半

- **WHEN** `termux.properties` 被外部进程截断成半行
- **THEN** 界面沿用上一次成功解析的配置，日志里有一条解析错误，应用不崩溃

#### Scenario: 两份路径都存在

- **WHEN** `~/.termux/termux.properties` 与 `~/.config/termux/termux.properties` 同时存在
- **THEN** 以前者为准，后者不参与解析

#### Scenario: 配置是符号链接

- **WHEN** `~/.termux/termux.properties` 是指向别处的符号链接
- **THEN** 该路径被跳过，改用次路径；次路径不存在时全部取默认值

#### Scenario: 配置过大

- **WHEN** `termux.properties` 超过 64 KiB
- **THEN** 该文件不被读取，日志里有一条体积超限的说明，配置取默认值

#### Scenario: colors.properties 不生效

- **WHEN** 用户写入 `~/.termux/colors.properties`
- **THEN** 终端配色仍取当前主题，该文件不影响任何颜色
