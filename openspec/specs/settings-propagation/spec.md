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

### Requirement: 文本框只吞掉自己刚写下去的那一次回流

设置项文本框 MUST 只忽略「自己刚写入的那一次」回流（值相等即吞），MUST NOT 用一个
「编辑过」布尔闩把整个编辑期钉住：清除应用数据后存储值变空串而闩仍为真，输入框继续
显示清除前的旧值，而安装读到的是空串，两者静默分叉。

#### Scenario: 清除应用数据后输入框回填

- **WHEN** 用户在引导 URL 框输入过内容并确认清除应用数据
- **THEN** 输入框显示清除后的空值，与安装实际使用的值一致

#### Scenario: 防抖回流不回灌正在输入的字符

- **WHEN** 用户连续击键输入 URL，防抖落盘后回流值抵达
- **THEN** 输入框内容不被回流覆盖
