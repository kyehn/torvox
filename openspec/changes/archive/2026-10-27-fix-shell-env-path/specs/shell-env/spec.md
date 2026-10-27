# shell-env Specification

## Purpose

声明终端子进程环境变量白名单：`build_env` 只设置白名单内的变量，使 mksh 能通过 `ENV` 加载短提示符 rc，同时保证 bash 等其他 shell 行为不变；`PATH` 保留宿主原值并附加 prefix 的 bin 目录，Termux 工具优先、系统命令亦可按名解析。

## REMOVED Requirements

### Requirement: prefix 可用时 PATH 指向 prefix 的 bin

`PATH` 的取值规则改为保留宿主原值并附加 prefix 的 bin 目录：旧规则「仅 `{prefix}/bin`、
无 prefix 时完全不设 `PATH`」与 `DESIGN.md` Shell 节「原 `PATH` 附加
`/data/data/com.termux/files/usr/bin`，仅目录存在时附加设置。failsafe 时，保留原
`PATH`，不附加」相悖——系统命令无法按名解析，failsafe 会话也拿不到宿主 `PATH`。

## ADDED Requirements

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
