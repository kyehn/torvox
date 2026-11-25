# shell-entry Specification

## Purpose

Shell 启动入口设置与解析行为：具名入口列表默认空、名称唯一且只有一个激活；无启动目录设置；默认入口按存在性依次探测。

## Requirements

### Requirement: 具名启动入口默认空、名称唯一且只有一个激活

入口由「名称 + 命令行」组成；列表 MUST 默认为空，名称 MUST NOT 重复（同名保存即更新）。仅一个入口处于激活状态：激活项的命令行写入 `shell` 设置，未配置入口时该值为空，走默认入口探测链。保存命令行时 MUST NOT 检查文本、路径存在性或参数合法性。

#### Scenario: 未配置入口时使用默认入口

- **WHEN** 入口列表为空
- **THEN** 设置页显示未配置提示，`shell` 为空，会话按默认入口探测结果启动

#### Scenario: 保存同名入口是更新

- **WHEN** 用已有名称再次保存
- **THEN** 该名称对应的命令行被更新，列表长度不变，名称仍唯一

#### Scenario: 删除激活项

- **WHEN** 删除当前激活的入口
- **THEN** `shell` 清空，后续会话走默认入口探测链

#### Scenario: 保存空命令行入口

- **WHEN** 用非空名称与空命令行保存入口
- **THEN** 该入口被保存且命令行原样为空；激活它时 `shell` 为空，会话走默认入口探测链

### Requirement: 长按新建会话列出未激活入口

侧边面板「新建会话」按钮长按 MUST 按名称列出未激活入口；点选后 MUST 以该入口命令行新建会话，MUST NOT 改变激活项。

#### Scenario: 用未激活入口新建会话

- **WHEN** 长按新建会话并点选一个未激活入口
- **THEN** 新会话使用点选的命令行启动，激活入口保持不变

#### Scenario: 命令行按 shell 引号规则解析

- **WHEN** 命令行含带引号的参数（如 `-c 'echo a b'`）
- **THEN** 参数按引号规则切分；引号不闭合时启动失败并报出原命令行，不静默传给 execve

### Requirement: 启动目录默认家目录且可由 termux.properties 覆盖

MUST NOT 提供启动目录设置框；`workingDirectory` MUST 默认为家目录，MAY 由 `termux.properties` 的 `default-working-directory` 覆盖，非法值回退家目录。

#### Scenario: 设置页无启动目录项

- **WHEN** 打开设置页终端配置区
- **THEN** 不出现启动目录输入框

#### Scenario: 会话工作目录默认家目录

- **WHEN** 未设置 `default-working-directory` 或其值非法
- **THEN** 子进程初始工作目录为家目录

#### Scenario: 配置覆盖启动目录

- **WHEN** `default-working-directory` 指向存在的目录
- **THEN** 子进程初始工作目录为该目录

### Requirement: 默认入口按存在性依次探测

设置为空时 MUST 依次寻找 `/data/data/com.termux/files/usr/bin/{login, bash, zsh, fish, sh}`，文件存在即启动，不检查文件权限，失败不回退；均不存在时启动 `/system/bin/sh`。命中前缀入口时 MUST 以登录 shell 语义启动：`bin/login` MUST NOT 带参数（上游 termux-tools 脚本自身以 `exec "$SHELL" -l "$@"` 启动 shell，多带参数会吞掉 motd），其余入口 MUST 追加 `-l`。

#### Scenario: login 优先于其余候选

- **WHEN** Shell 设置为空且 prefix 下 `bin/login` 与 `bin/bash` 均存在
- **THEN** 启动 `bin/login`，且不带参数

#### Scenario: login 不存在时按顺序探测

- **WHEN** Shell 设置为空且 prefix 下 `bin/login` 不存在而 `bin/bash` 存在
- **THEN** 启动 `bin/bash -l`

#### Scenario: 均不存在时用系统 sh

- **WHEN** Shell 设置为空且 prefix 下 login、bash、zsh、fish、sh 均不存在
- **THEN** 启动 `/system/bin/sh`

#### Scenario: 自定义入口原样透传

- **WHEN** Shell 设置为非空文本
- **THEN** 原样作为启动入口（含参数），不做检查与特殊处理

### Requirement: 非正常退出保留现场

shell 崩溃（非主动正常退出）MUST 保留现场不关闭会话，显示 `[Process completed (code 255) - press Enter]`（`exit -1` 示例）；正常退出时关闭会话，且最后一个会话消失时关闭应用（termux 语义）；启动入口失败不得回退，保留输出显示。

#### Scenario: 非零退出保留现场

- **WHEN** shell 以非零码退出
- **THEN** 会话保留并显示 `[Process completed (code X) - press Enter]`，回车后关闭

#### Scenario: 零退出直接关闭

- **WHEN** shell 以零码退出
- **THEN** 会话直接关闭

#### Scenario: 最后一个会话结束后关闭应用

- **WHEN** 会话表中最后一个会话消失（零码退出、回车确认 `[Process completed]`、或用户关闭会话）
- **THEN** 应用 MUST 随之关闭，MUST NOT 停在无会话的空界面
- **AND** 冷启动时「会话尚未创建」的空表 MUST NOT 触发关闭（会话由运行期异步创建）
