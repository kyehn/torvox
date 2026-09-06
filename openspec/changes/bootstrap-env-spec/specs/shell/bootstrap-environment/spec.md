## Purpose

本能力规定 bootstrap 安装与 shell 子进程环境的绝对必要设置（对照 termux-app 实现提取），作为安装器与 PTY 启动路径的行为契约与回归依据。

## ADDED Requirements

### Requirement: 安装原子性

安装 bootstrap zip 时，系统 SHALL 解压到 staging 目录、按 `SYMLINKS.txt`（`old←new` 格式）创建符号链接，全部成功后原子重命名为正式 prefix；任一步失败 MUST 丢弃 staging 目录并保留旧 prefix，不得留下半安装状态。

#### Scenario: 安装失败不破坏旧环境

- **WHEN** 解压/建链/重命名任一步失败
- **THEN** staging 目录被清理，旧 prefix 保持可用，安装报告失败

#### Scenario: 安装成功标记

- **WHEN** 安装完成
- **THEN** `etc/termux/termux.env` 存在（`isInstalled` 判定依据），缺失时视为未安装

### Requirement: 可执行权限

`bin/`、`libexec/`、`lib/apt/apt-helper`、`lib/apt/methods/` 下的文件在安装后 MUST 具有可执行位（0700），否则 shell 与包管理器无法启动。

#### Scenario: 安装后 shell 可执行

- **WHEN** 安装完成
- **THEN** `bin/` 下 shell 二进制可直接执行，无需手动 chmod

### Requirement: 子进程环境变量

shell 子进程启动时，系统 SHALL 设置：`PREFIX=$FILES/usr`、`HOME=$FILES/home`、`PATH=$PREFIX/bin:/system/bin:/system/xbin`、`TMPDIR=$PREFIX/tmp`、`LANG=C.UTF-8`、`TERM=xterm-256color`；Android 7+ MUST NOT 设置 `LD_LIBRARY_PATH`。

#### Scenario: 环境变量完整

- **WHEN** 新会话 shell 启动
- **THEN** 上述变量均已设置且路径指向应用私有目录，不得指向系统或其他应用路径

### Requirement: 登录 shell 语义

`executable` 未指定时，系统 SHALL 按 `login`/`bash`/`zsh`/`fish`/`sh` 顺序在 `$PREFIX/bin` 中查找，命中则以 login 形式启动（`argv[0]` 带 `-` 前缀），由 shell 自行读取 profile/rc 文件；找不到时降级 `/system/bin/sh` 非登录启动。

#### Scenario: 默认登录 shell

- **WHEN** 用户未指定启动入口且 `$PREFIX/bin` 含 bash
- **THEN** bash 以 login 形式启动并读取 `~/.bash_profile` 等初始化文件

### Requirement: 工作目录默认

新会话工作目录 SHALL 默认为 `HOME`（`.../files/home`），用户设置的终端启动目录优先。

#### Scenario: 默认目录

- **WHEN** 用户未设置启动目录
- **THEN** shell 起始于 `HOME` 而非 `/`

### Requirement: 与 termux 的有意差异

以下两点与 termux-app 实现有意不同，MUST 保持现状：`LANG` 为 `C.UTF-8`（非 termux 的 `en_US.UTF-8`，避免 locale 数据依赖）；禁止 bootstrap zip 的 sha256 sidecar 校验（规范禁止实现条款）。

#### Scenario: 差异不回归

- **WHEN** 审查安装器与环境代码
- **THEN** 不得出现 sidecar 校验逻辑，不得将 LANG 改为 `en_US.UTF-8`
