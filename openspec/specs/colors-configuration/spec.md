# colors-configuration Specification

## Purpose

让用户用 `~/.termux/colors.properties` 改终端配色：启动时读一次，作为当前主题的**部分
覆盖**下发。未写的槽沿用主题原色，切换主题后覆盖仍然生效。

## Requirements

### Requirement: 读取路径与容错

应用 MUST 在启动时读取一次 `~/.termux/colors.properties`。该路径是上游唯一的路径
（`TermuxConstants.TERMUX_COLOR_PROPERTIES_FILE`），MUST NOT 另读
`~/.config/termux/colors.properties`。文件不存在是正常状态（等价于无覆盖），不是错误；
读取或解析失败 MUST 记错误并按无覆盖处理，MUST NOT 抛给调用方、MUST NOT 影响启动。

只认**常规文件**且 MUST NOT 跟随符号链接：文件由外部写入，链接可以把读取带到家目录
之外，也可能指向设备。读取 MUST 为 UTF-8；单文件超过 64 KiB MUST 视为无效并跳过
（MUST NOT 读进内存）。应用 MUST NOT 创建、写入或修改该文件。

#### Scenario: 文件不存在

- **WHEN** `~/.termux/colors.properties` 不存在
- **THEN** 终端配色完全由主题决定，日志无错误

#### Scenario: 文件写到一半

- **WHEN** 文件被外部进程截断成半行
- **THEN** 应用不崩溃，该文件按无覆盖处理，日志里有一条解析错误

#### Scenario: 配置是符号链接

- **WHEN** `~/.termux/colors.properties` 是指向别处的符号链接
- **THEN** 该路径被跳过，终端配色完全由主题决定

#### Scenario: 配置过大

- **WHEN** 文件超过 64 KiB
- **THEN** 文件不被读取，日志里有一条体积超限的说明

#### Scenario: 启动后不重读

- **WHEN** 应用启动后外部进程修改了该文件
- **THEN** 当前进程继续使用启动时解析出的覆盖，直到进程重启

### Requirement: 键的语义

解析 MUST 接受 `foreground`、`background`、`cursor`、`color0`..`color15`，以及
`color256`、`color257`、`color258`——后者是前三个键的别名（上游
`TerminalColorScheme.updateWith` 把三者写进同一个调色板数组）。显式键与别名同时出现
时 MUST 以显式键为准：上游按 `Properties` 的遍历顺序覆盖，同一文件里两种写法并存时
结果不确定，本仓 MUST NOT 让结果依赖哈希顺序。

`color16`..`color255` MUST 被忽略并记警告：本仓调色板只有 16 个 ANSI 槽，256 色立方
由 `libghostty-vt` 按 xterm 标准生成（见 `theme-configuration`）。其它未知键 MUST 被
忽略。

#### Scenario: 十六色覆盖

- **WHEN** 文件写 `color1 = #FF0000`
- **THEN** ANSI 槽 1 为不透明红色，其余槽沿用主题

#### Scenario: 别名与显式键并存

- **WHEN** 文件同时写 `foreground = #FFFFFF` 与 `color256 = #000000`
- **THEN** 前景取 `#FFFFFF`

#### Scenario: 256 色立方键被忽略

- **WHEN** 文件写 `color16 = #000000`
- **THEN** 该键不生效，日志里明确说明不支持

### Requirement: 色值的语法

色值 MUST 按 XQueryColor 语法解析（上游 `TerminalColors.parse`）：`#RGB`、`#RRGGBB`、
`#RRRGGGBBB`、`#RRRRGGGGBBBB` 与 `rgb:R/G/B`（每段 1..4 位十六进制），alpha 恒为
不透明。非法色值 MUST 只作废该键并记错误，MUST NOT 作废整份文件。

#### Scenario: 短十六进制

- **WHEN** 文件写 `background = #f00`
- **THEN** 背景为不透明红色（`#FF0000`）

#### Scenario: rgb 记法

- **WHEN** 文件写 `foreground = rgb:00/FF/00`
- **THEN** 前景为不透明绿色（`#00FF00`）

#### Scenario: 非法色值只作废该键

- **WHEN** 文件写 `background = not-a-color` 与 `cursor = #00FF00`
- **THEN** 背景沿用主题，光标取 `#00FF00`，日志里有一条背景色值非法的说明

### Requirement: 覆盖是主题的部分覆盖

`colors.properties` MUST NOT 是完整主题：它 MUST 只覆盖既有主题的指定槽位，MUST NOT
替代主题。覆盖槽位为背景、前景、光标与 16 个 ANSI 色；未覆盖的槽 MUST 沿用主题原色。
选区底色没有对应键，MUST 沿用主题的值。

覆盖 MUST 与主题一起下发并在下发前合并：终端默认色、ANSI 调色板、光标色与搜索高亮色
MUST 全部取自合并后的同一份调色板，MUST NOT 存在第二份颜色副本。换主题后覆盖 MUST
仍然生效：覆盖是全局配置，不随主题切换消失。

#### Scenario: 只写一个键

- **WHEN** 文件只写 `background = #101010`
- **THEN** 背景为 `#101010`，前景、光标、选区底色与十六色均沿用主题

#### Scenario: 换主题后覆盖仍生效

- **WHEN** 已配置 `background = #101010` 后用户在设置页切换主题
- **THEN** 新主题的其余颜色生效，背景仍为 `#101010`

### Requirement: 覆盖后的背景决定光标色与明暗

背景被覆盖而光标未指定时，光标色 MUST 按背景的感知亮度取白色或黑色（上游
`TerminalColorScheme.setCursorColorForBackground` 的口径：HSP 亮度低于 130 取白，
否则取黑）；背景未被覆盖时，光标 MUST 沿用主题自带的光标色。

明暗上报（`CSI ? 997 ; 1|2 n`）MUST 按**覆盖后**的背景亮度判定，MUST NOT 按主题
自带的深浅标记判定。判定口径与主题自身的深浅标记一致（ghostty 的背景感知亮度不超过
0.5 即为深底），故未覆盖时同一主题的明暗上报与覆盖后按同一规则算出，不会出现「主题
说是深色、上报却说日间」的分裂。

光标自适应与明暗判定都是**调色板规则**，MUST 落在调色板真源所在的原生侧；Kotlin 侧
只搬运用户写了的色值，MUST NOT 另算一份。

#### Scenario: 浅背景配深色光标

- **WHEN** 文件只写 `background = #FFFFFF`
- **THEN** 光标为黑色

#### Scenario: 深背景配浅色光标

- **WHEN** 文件只写 `background = #101010`
- **THEN** 光标为白色

#### Scenario: 显式光标色优先

- **WHEN** 文件写 `background = #FFFFFF` 与 `cursor = #FF0000`
- **THEN** 光标为红色，不按背景亮度重算

#### Scenario: 明暗按覆盖后的背景上报

- **WHEN** 深色主题下文件写 `background = #FFFFFF`
- **THEN** 应用向 PTY 上报日间模式
