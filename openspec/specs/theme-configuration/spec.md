# theme-configuration Specification

## Purpose

固定终端主题目录与预览、日夜跟随切换：构建期从上游读取配置，运行时按标识单源寻址。
自定义终端主题是禁止实现。

## Requirements

### Requirement: 固定主题目录

终端主题目录 MUST 有且只有 10 个标识，不可增减，清单以 DESIGN 终端主题节 10 标识为准。
构建期 MUST 从
`https://github.com/alacritty/alacritty-theme` 读取配置，仓库 MUST NOT
硬编码任何颜色。

#### Scenario: 目录数量固定

- **WHEN** 审查主题清单
- **THEN** 标识数量为 10，不多不少

#### Scenario: 颜色不硬编码

- **WHEN** 审查主题颜色来源
- **THEN** 所有颜色来自上游 TOML 解析，仓内无手写颜色副本

### Requirement: 单源按标识寻址

主题真源 MUST 在 Rust 侧：清单经 `include_str!` 在构建期嵌入 TOML，
运行时解析为调色板。标识既是文件名也是设置落盘键值，各出现且只出现一次，
默认主题用清单下标引用。颜色语法与深浅判定 MUST 取自上游 ghostty，
本仓 MUST NOT 另造颜色算法。清单内每个主题 MUST 写全两组十六色，
缺项即解析失败，MUST NOT 回退手写默认色。Kotlin 侧 MUST NOT 存名字与
颜色副本：经原生直接量按标识取回，不经 JSON，不做序号算术。下发
`setTheme(标识)` 后终端默认色、ANSI 调色板、光标色与搜索高亮色都由原生按
同一份调色板解析，MUST NOT 存在第二份副本。

#### Scenario: 标识即文件名即落盘键

- **WHEN** 按标识请求主题
- **THEN** 原生按同一标识返回调色板，Kotlin 无颜色副本可漂移

#### Scenario: 缺十六色即失败

- **WHEN** 某主题缺一组十六色
- **THEN** 解析失败，不回退默认色

#### Scenario: 不经 JSON 与序号算术

- **WHEN** 审查 Kotlin 取主题路径
- **THEN** 只见一次原生目录调用与其后的字节解包，不见 JSON 解析、逐个标识的往返与序号换算

### Requirement: 主题目录一次取回

设置页一次显示全部主题，故 Kotlin MUST 经单次同步调用取回整个目录（`u16` 主题数后跟逐条「标志 + 名称长度 + 名称 + 54 字节颜色」），MUST NOT 逐个标识各发一次查询。标志位 MUST 携带深底与默认日夜标记，使默认主题无须另设查询。目录字节不完整 MUST 抛错，MUST NOT 截断或补默认色；光标色与选区色 MUST NOT 入包（终端页由渲染器按同一份调色板绘制，设置页不消费）。

#### Scenario: 一次调用即取回全部主题

- **WHEN** 审查主题查询路径
- **THEN** 每个进程只发一次主题查询，列表与默认标识同源解析

#### Scenario: 目录字节不完整即失败

- **WHEN** 目录长度或某条固定字段越界
- **THEN** 解析抛错，不截断、不补默认色

### Requirement: 日夜默认与跟随

默认夜间主题 MUST 为 `dracula_plus`，默认日间主题 MUST 为 `catppuccin_latte`。
开启跟随系统后 MUST 按软件主题在两者间自动切换。

#### Scenario: 默认主题生效

- **WHEN** 首次启动且无主题设置
- **THEN** 夜间用 `dracula_plus`，日间用 `catppuccin_latte`

#### Scenario: 跟随系统切换

- **WHEN** 软件主题在日夜间切换且终端跟随开关开启
- **THEN** 终端主题随之在日夜默认间切换

### Requirement: 应用与错误处理

主题选择 MUST 即时生效。主题名显示于预览框下方，长名 MUST 完整显示，
MUST NOT 在预览框内。设置中存着无法解析的名字 MUST 清除对应键，
MUST NOT 静默替换为默认主题。

#### Scenario: 选择即时生效

- **WHEN** 选择另一主题
- **THEN** 终端配色立即按新调色板渲染

#### Scenario: 预览名在框下方

- **WHEN** 预览主题
- **THEN** 主题名在预览框下方完整显示

#### Scenario: 无效主题名被清除

- **WHEN** 设置中的主题名在清单中不存在
- **THEN** 该设置键被清除，后续读取回落默认，不静默替换
