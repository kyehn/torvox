# termux-config Specification

## Purpose

允许读取 `termux.properties` 中的 `default-working-directory`、`terminal-transcript-rows`、`terminal-cursor-style` 三个键，并按校验规则应用到新会话。

## Requirements

### Requirement: 仅读取允许的三个键

解析器 MUST 仅消费 `default-working-directory`、`terminal-transcript-rows`、`terminal-cursor-style`；其他键 MUST 忽略。

#### Scenario: 其他键被忽略

- **WHEN** `termux.properties` 含未允许的键
- **THEN** 该键不改变任何会话行为

### Requirement: 首个可读常规文件生效

解析器 MUST 依次检查主路径与次路径，仅首个可读常规文件生效；符号链接与非常规文件 MUST 跳过；文件不存在时全部取默认值。

#### Scenario: 主路径优先

- **WHEN** 主路径与次路径均存在
- **THEN** 仅主路径生效

#### Scenario: 符号链接跳过

- **WHEN** 候选路径为符号链接
- **THEN** 该路径被跳过

### Requirement: 工作目录校验

`default-working-directory` MUST 仅在路径存在且为目录时生效，否则回退家目录。

#### Scenario: 非法路径回退

- **WHEN** `default-working-directory` 指向不存在路径
- **THEN** 工作目录为家目录

### Requirement: 回滚行数范围

`terminal-transcript-rows` MUST 仅在 100–50000 范围内生效，非法回退 2000。

#### Scenario: 超出范围回退

- **WHEN** `terminal-transcript-rows` 为 50
- **THEN** 回滚行数为 2000

### Requirement: 光标样式映射

`terminal-cursor-style` MUST 接受 `block`、`bar`、`beam`、`underline`，非法回退 `block`。

#### Scenario: beam 映射为竖线

- **WHEN** `terminal-cursor-style=beam`
- **THEN** 默认光标样式为竖线