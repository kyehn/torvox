# shell-env Specification

## Purpose

声明终端子进程环境变量白名单：`build_env` 只设置白名单内的变量，不写 rc 文件、不注入提示符或标题钩子；`PATH` 以 prefix 的 bin 目录开头并保留宿主原值，Termux 工具优先、系统命令亦可按名解析。

## Requirements

### Requirement: Shell 钩子与 rc 文件一律不注入

终端 MUST NOT 设置 `ENV` 或 `PROMPT_COMMAND`，MUST NOT 创建或改写 `.mkshrc`
（`DESIGN.md` Shell 节）。会话标题与提示符由 shell 及其用户配置自行决定：
shell 未上报 OSC 0/2 时标题留空，抽屉按「未设置标题时仅显示序号」渲染。

#### Scenario: 构建环境时不含钩子

- **WHEN** 构建任意 shell 的子进程环境变量
- **THEN** 结果中不出现 `ENV` 与 `PROMPT_COMMAND`

#### Scenario: 不经环境变量传递 rc 路径

- **WHEN** 会话由应用数据目录或用户目录启动
- **THEN** 应用不创建 `.mkshrc`，子进程环境也没有指向 rc 文件的变量

### Requirement: 白名单外变量仍被拒绝

`build_env` MUST NOT 设置白名单外的任何变量，`LD_LIBRARY_PATH` / `PWD` / `LD_PRELOAD` / `ENV` / `PROMPT_COMMAND` 保持被禁止。

#### Scenario: 拒绝未声明变量

- **WHEN** 构建子进程环境变量
- **THEN** 结果中不出现 `LD_LIBRARY_PATH`、`PWD`、`LD_PRELOAD`、`ENV`、`PROMPT_COMMAND` 及其他未声明变量

### Requirement: 声明的环境变量集合固定

子进程环境 MUST 只包含下列变量及其取值（`DESIGN.md` Shell 节）：

- `HOME` 与 `TERMUX_HOME_DIR_PATH`：`/data/data/com.termux/files/home`
- `PREFIX` 与 `TERMUX_PREFIX_DIR_PATH`：`/data/data/com.termux/files/usr`
- `TMPDIR` 与 `TERMUX_TMP_PREFIX_DIR_PATH`：`/data/data/com.termux/files/usr/tmp`
- `LANG`：`en_US.UTF-8`
- `COLORTERM`：`truecolor`
- `TERM`：`xterm-256color`
- `TERMUX_VERSION`：`0.119.0-beta.3`
- `PATH`：见「PATH 以 prefix 的 bin 开头并保留宿主原值」

MUST NOT 设置 `LD_LIBRARY_PATH`、`PWD`、`LD_PRELOAD`、`ENV`、`PROMPT_COMMAND`。

#### Scenario: 变量取值符合声明

- **WHEN** 构建带 prefix 的会话环境变量
- **THEN** `HOME`/`TERMUX_HOME_DIR_PATH` 为 home 目录，`PREFIX`/`TERMUX_PREFIX_DIR_PATH`
      为 prefix 目录，`TMPDIR`/`TERMUX_TMP_PREFIX_DIR_PATH` 为 `{prefix}/tmp`，
      `LANG` 为 `en_US.UTF-8`，`COLORTERM` 为 `truecolor`，`TERM` 为
      `xterm-256color`，`TERMUX_VERSION` 为 `0.119.0-beta.3`

#### Scenario: failsafe 会话没有 prefix 变量

- **WHEN** 构建 failsafe 会话的环境变量（无 prefix）
- **THEN** 结果中不出现 `PREFIX`、`TERMUX_PREFIX_DIR_PATH`、`TMPDIR`、
      `TERMUX_TMP_PREFIX_DIR_PATH`

### Requirement: 只透传宿主存在的 Android 系统变量

`build_env` MAY 透传下列宿主变量，且 MUST 只透传宿主环境中**实际存在**的那些，
MUST NOT 为它们硬编码取值（各机型取值不同，硬编码 `ANDROID_ROOT=/system` 在
API 26+ 上是错的）：`ANDROID_ASSETS`、`ANDROID_DATA`、`ANDROID_ROOT`、
`ANDROID_STORAGE`、`EXTERNAL_STORAGE`、`ASEC_MOUNTPOINT`、`LOOP_MOUNTPOINT`、
`ANDROID_RUNTIME_ROOT`、`ANDROID_ART_ROOT`、`ANDROID_I18N_ROOT`、
`ANDROID_TZDATA_ROOT`、`BOOTCLASSPATH`、`DEX2OATBOOTCLASSPATH`、
`SYSTEMSERVERCLASSPATH`。

#### Scenario: 宿主存在才透传

- **WHEN** 宿主环境存在 `ANDROID_ROOT` 而不存在 `EXTERNAL_STORAGE`
- **THEN** 子进程环境含 `ANDROID_ROOT` 的宿主原值，不含 `EXTERNAL_STORAGE`

#### Scenario: 不硬编码系统变量取值

- **WHEN** 宿主环境没有任何 Android 系统变量
- **THEN** 子进程环境不出现这些变量名，应用不补默认值

### Requirement: PATH 以 prefix 的 bin 开头并保留宿主原值

`build_env` MUST 设置 `PATH`：`{prefix}/bin` 在前，宿主进程的 `PATH` 原值在后（`DESIGN.md` Shell 节：原 `PATH` 附加 `/data/data/com.termux/files/usr/bin`，仅目录存在时附加设置）；`{prefix}/bin` 仅在目录存在时附加。`prefix` 为 `None`（无引导或 failsafe）时 MUST 只保留宿主 `PATH`，不附加 Termux 的 bin 目录。宿主没有 `PATH` 时 MUST 只给 `{prefix}/bin`，不得拼出空路径项。

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
