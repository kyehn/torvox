# shell-entry Specification

## ADDED Requirements

### Requirement: Shell 设置框默认为空并经保存按钮持久化

`shell` 设置 MUST 默认为空字符串；设置框 MUST 在未设置时显示空文本；MUST 提供保存按钮，仅点击保存时写入设置。保存时 MUST NOT 检查文本、路径存在性或参数合法性。

#### Scenario: 未设置时显示空文本

- **WHEN** 用户未设置 Shell 启动入口
- **THEN** 设置框显示空文本，不预填任何路径

#### Scenario: 保存按钮写入

- **WHEN** 用户在框中输入文本并点击保存
- **THEN** 文本原样存入 `shell` 设置（含参数，不做任何检查）

### Requirement: 无启动目录设置

MUST NOT 提供启动目录设置框；`workingDirectory` MUST 恒为家目录。

#### Scenario: 设置页无启动目录项

- **WHEN** 打开设置页终端配置区
- **THEN** 不出现启动目录输入框

#### Scenario: 会话工作目录恒为家目录

- **WHEN** 启动任意会话
- **THEN** 子进程初始工作目录为家目录

### Requirement: 默认入口按存在性依次探测

设置为空时 MUST 依次寻找 `/data/data/com.termux/files/usr/bin/bash`、`/data/data/com.termux/files/usr/bin/login`、`/system/bin/sh`，文件存在即启动，不检查文件权限，失败不回退。

#### Scenario: bash 存在即启动

- **WHEN** Shell 设置为空且 prefix 下 `bin/bash` 文件存在
- **THEN** 启动 `bin/bash`

#### Scenario: 均不存在时用系统 sh

- **WHEN** Shell 设置为空且 prefix 下 bash 与 login 均不存在
- **THEN** 启动 `/system/bin/sh`

#### Scenario: 自定义入口原样透传

- **WHEN** Shell 设置为非空文本
- **THEN** 原样作为启动入口（含参数），不做检查与特殊处理

### Requirement: 非正常退出保留现场

shell 崩溃（非主动正常退出）MUST 保留现场不关闭会话，显示 `[Process completed (code 255) - press Enter]`（`exit -1` 示例）；正常退出时关闭会话；启动入口失败不得回退，保留输出显示。

#### Scenario: 非零退出保留现场

- **WHEN** shell 以非零码退出
- **THEN** 会话保留并显示 `[Process completed (code X) - press Enter]`，回车后关闭

#### Scenario: 零退出直接关闭

- **WHEN** shell 以零码退出
- **THEN** 会话直接关闭
