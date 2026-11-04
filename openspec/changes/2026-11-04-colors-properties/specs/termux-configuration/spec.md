## MODIFIED Requirements

### Requirement: 读取路径与容错

解析器 MUST 在应用启动时读取一次 `~/.termux/termux.properties`；该路径不存在时再读取
`~/.config/termux/termux.properties`。两条路径不合并。文件读取或解析失败 MUST 使用内置
默认值并记录错误，MUST NOT 抛给调用方；进程存活期间 MUST NOT 因文件变化再次读取。

只认**常规文件**且 MUST NOT 跟随符号链接：该文件由外部写入，链接可以把读取带到
家目录之外，也可能指向设备；非常规文件（管道、目录）同样跳过。主路径存在但因符号链接
或非常规文件被跳过时，MUST NOT 再检查次路径。

读取 MUST 为 UTF-8；单文件超过 64 KiB MUST 视为无效并跳过（MUST NOT 读进内存），
使用内置默认值。

应用 MUST NOT 创建、写入或修改该文件。

`termux.float.properties` MUST NOT 被读取或解析；浮动窗口功能不在本仓范围。终端配色的
部分覆盖由 `colors-configuration` 能力定义，不在本能力范围内。

#### Scenario: 文件写到一半

- **WHEN** `termux.properties` 被外部进程截断成半行
- **THEN** 应用使用内置默认值，日志里有一条解析错误，应用不崩溃

#### Scenario: 主路径不存在时读取次路径

- **WHEN** `~/.termux/termux.properties` 不存在且 `~/.config/termux/termux.properties` 存在
- **THEN** 解析次路径

#### Scenario: 两份路径都存在

- **WHEN** `~/.termux/termux.properties` 与 `~/.config/termux/termux.properties` 同时存在
- **THEN** 以前者为准，后者不参与解析

#### Scenario: 配置是符号链接

- **WHEN** `~/.termux/termux.properties` 存在且是指向别处的符号链接
- **THEN** 该路径被跳过，且不再检查次路径，全部取默认值

#### Scenario: 配置过大

- **WHEN** `termux.properties` 超过 64 KiB
- **THEN** 该文件不被读取，日志里有一条体积超限的说明，应用使用内置默认值

#### Scenario: 启动后不重读

- **WHEN** 应用启动后外部进程修改了 `termux.properties`
- **THEN** 当前进程继续使用启动时解析出的配置，直到进程重启

#### Scenario: 其他 properties 文件不生效

- **WHEN** 用户写入 `~/.termux/termux.float.properties`
- **THEN** 该文件不被读取，不影响窗口行为或任何其他行为
