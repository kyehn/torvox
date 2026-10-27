# shell-env Specification

## Purpose

声明终端子进程环境变量白名单：`build_env` 只设置白名单内的变量，使 mksh 能通过 `ENV` 加载短提示符 rc，同时保证 bash 等其他 shell 行为不变；`PATH` 保留宿主原值并附加 prefix 的 bin 目录，Termux 工具优先、系统命令亦可按名解析。

## Requirements

### Requirement: ENV 指向 mksh rc

`build_env` MUST 注入 `ENV`，其值为调用方经 `ShellEnv.mkshrc_path` 传入的 mksh rc 绝对路径（`DESIGN.md` Shell 节：`/data/data/com.termux/.mkshrc`，应用私有根目录，在 `files/` 用户数据树之外）；mksh 交互 shell 经由 `ENV` 加载该文件获得短提示符。`mkshrc_path` 为 `None` 时 MUST NOT 注入 `ENV`。

#### Scenario: mksh 加载短提示符

- **WHEN** 子进程以 mksh 交互方式启动且 `mkshrc_path` 指向的应用私有 rc 文件存在
- **THEN** 提示符为 `PS1='$ '` 定义的短提示符，不出现长提示符横滚与左缘裁剪

#### Scenario: 路径不指向 HOME

- **WHEN** 构建子进程环境变量
- **THEN** `ENV` 的值等于传入的 `mkshrc_path`，不为 `{home}/.mkshrc`，不落在 `$HOME` 或用户数据目录内

#### Scenario: 未传入路径时不注入 ENV

- **WHEN** `ShellEnv.mkshrc_path` 为 `None`（如测试或调用方未提供）
- **THEN** 构建结果中不出现 `ENV` 键

#### Scenario: bash 忽略 ENV

- **WHEN** 子进程以 bash 启动
- **THEN** `ENV` 被忽略，bash 启动行为与注入前一致

### Requirement: 会话标题按 shell 族注入上报钩子

终端 MUST 为**自己不报标题**的 shell 族注入标题上报钩子，使会话标题（OSC 0/2）
在这些 shell 下同样可用：mksh 一族由应用写的 rc 文件在提示符里带 OSC 0 序列
（当前目录名）实现；bash 一族由子进程环境注入 `PROMPT_COMMAND` 实现。

钩子 MUST NOT 覆盖用户自己的设置：bash 的 `PROMPT_COMMAND` 只在 shell 内未
定义时取环境值，用户 rc 里自己设过的以用户为准；mksh 的提示符只改应用自己
写的那份 rc，不碰用户的 `~/.mkshrc`。

注入 MUST 只针对确需钩子的 shell：`build_env` 的白名单本身不变，bash 的
`PROMPT_COMMAND` 是 spawn 时按 shell 可执行文件名追加的。

#### Scenario: mksh 会话上报标题

- **WHEN** 子进程以 mksh 交互方式启动，`mkshrc_path` 指向应用写的 rc 文件
- **THEN** 每次画提示符时发出 `ESC ] 0 ; <当前目录名> BEL`，终端标题随之更新；
  提示符的**可见**宽度不变（转义序列不占列）

#### Scenario: bash 会话上报标题

- **WHEN** 子进程以 bash 启动
- **THEN** 子进程环境含 `PROMPT_COMMAND`，其值在每个提示符前发出 `ESC ] 0 ; <当前目录名> BEL`

#### Scenario: 其他 shell 不注入钩子

- **WHEN** 子进程以 bash 之外的 shell 启动
- **THEN** 不注入 `PROMPT_COMMAND`（自带标题上报的 shell 不必注入；其余 shell
  无可靠钩子，标题留空，抽屉按「未设置标题时仅显示序号」渲染）

### Requirement: 白名单外变量仍被拒绝

`build_env` MUST NOT 设置白名单外的任何变量，`LD_LIBRARY_PATH` / `PWD` / `LD_PRELOAD` 保持被禁止。

#### Scenario: 拒绝未声明变量

- **WHEN** 构建子进程环境变量
- **THEN** 结果中不出现 `LD_LIBRARY_PATH`、`PWD`、`LD_PRELOAD` 及其他未声明变量

### Requirement: PATH 保留宿主原值并附加 prefix 的 bin

`build_env` MUST 设置 `PATH`：宿主进程的 `PATH` 原值在前，`{prefix}/bin` 在后；`{prefix}/bin` 仅在目录存在时附加（`DESIGN.md` Shell 节：原 `PATH` 附加 `/data/data/com.termux/files/usr/bin`，仅目录存在时附加设置）。`prefix` 为 `None`（无引导或 failsafe）时 MUST 只保留宿主 `PATH`，不附加 Termux 的 bin 目录。宿主没有 `PATH` 时 MUST 只给 `{prefix}/bin`，不得拼出空路径项。

#### Scenario: 有 prefix 的会话解析 Termux 与系统命令

- **WHEN** 子进程以带 prefix 的会话启动，且 `{prefix}/bin` 目录存在
- **THEN** `PATH` 以 `{prefix}/bin` 开头并保留宿主 `PATH` 原值；`command -v coreutils` 解析到 `$PREFIX/bin` 下的命令（login 的 termux-exec 探针靠它工作），`am`/`pm` 等系统命令亦可按名解析

#### Scenario: Termux 工具优先于系统同名命令

- **WHEN** `PATH` 同时含 `{prefix}/bin` 与宿主 `PATH`
- **THEN** Termux 工具（GNU 版 `grep`/`sed`/`ls`）先于系统同名命令（toybox）被解析

#### Scenario: prefix 的 bin 目录不存在

- **WHEN** `{prefix}/bin` 目录不存在
- **THEN** `PATH` 只含宿主原值，不含该不存在的目录

#### Scenario: failsafe 会话保留宿主 PATH

- **WHEN** `ShellEnv.prefix` 为 `None`（无引导或 failsafe 会话）
- **THEN** `PATH` 等于宿主原值，不含 Termux 的 bin 目录

#### Scenario: 宿主没有 PATH

- **WHEN** 宿主进程没有 `PATH` 环境变量
- **THEN** `PATH` 只含 `{prefix}/bin`，不以分隔符开头
