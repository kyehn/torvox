## Purpose

本能力规定 bootstrap 安装与 shell 子进程环境的绝对必要设置（对照 termux-app 实现提取），作为安装器与 PTY 启动路径的行为契约与回归依据。规范依据：`docs/specification/DESIGN.md` 设置·Bootstrap、终端·Shell、禁止实现条款。

## Requirements

### Requirement: 安装原子性

安装 bootstrap zip 时，系统 SHALL 解压到 staging 目录、按 `SYMLINKS.txt`（`old←new` 格式）创建符号链接，全部成功后原子重命名为正式 prefix；任一步失败 MUST 丢弃 staging 目录并保留旧 prefix，不得留下半安装状态。

#### Scenario: 安装失败不破坏旧环境

- **WHEN** 解压/建链/重命名任一步失败
- **THEN** staging 目录被清理，旧 prefix 保持可用，安装报告失败

#### Scenario: 安装成功标记

- **WHEN** 安装完成
- **THEN** `etc/termux/termux.env` 存在（`isInstalled` 判定依据），缺失时视为未安装

### Requirement: 可执行权限

`bin/`、`libexec/`、`lib/apt/apt-helper`、`lib/apt/methods/` 下的文件（另支持 `EXECUTABLES.txt` 显式列表）在安装后 MUST 具有可执行位（`EXECUTABLE_FILE_MODE=0755`，应用私有目录下与 termux 的 0700 等效安全），否则 shell 与包管理器无法启动。

#### Scenario: 安装后 shell 可执行

- **WHEN** 安装完成
- **THEN** `bin/` 下 shell 二进制可直接执行，无需手动 chmod

### Requirement: 子进程环境变量

shell 子进程启动时，系统 SHALL 设置：`PREFIX=$FILES/usr`、`HOME=$FILES/home`、`PATH=$PREFIX/bin` 起始并透传系统 PATH、`TMPDIR=$PREFIX/tmp`（无 prefix 时 `/data/local/tmp`）、`LANG=C.UTF-8`、`TERM=xterm-256color`、`COLORTERM=truecolor`、`SHELL=<shell 路径>`、`PWD=<工作目录>`、`USER`、`LINES`/`COLUMNS`；Android 7+ MUST NOT 设置 `LD_LIBRARY_PATH`。

#### Scenario: 环境变量完整

- **WHEN** 新会话 shell 启动
- **THEN** 上述变量均已设置且路径指向应用私有目录，不得指向系统或其他应用路径

### Requirement: exec 桥接与证书

有 prefix 时系统 SHALL 设置 `LD_PRELOAD=$PREFIX/lib/libtermux-exec-ld-preload.so`（不存在则回退 `libtermux-exec.so`），使子进程经系统 linker 执行（Android 15+ SELinux 生存必需）；`$PREFIX/etc/tls/cert.pem` 存在时 SHALL 设置 `SSL_CERT_FILE`/`CURL_CA_BUNDLE` 指向它；系统 SHALL 透传 `ANDROID_*`/`BOOTCLASSPATH` 等宿主变量（`am`/`content` 调用必需），仅转发宿主进程中存在的变量，不得硬编码值。

#### Scenario: 子进程可执行 prefix 二进制

- **WHEN** shell 执行 `$PREFIX/bin` 下程序
- **THEN** 不得出现 EACCES，`cargo`/`curl` HTTPS 不得报证书错误

### Requirement: 登录 shell 语义

`executable` 未指定时，系统 SHALL 按 `login`/`bash`/`zsh`/`fish`/`sh` 顺序在 `$PREFIX/bin` 中查找仅 ELF 文件，找不到时降级 `/system/bin/sh`。当前实现以普通 `argv[0]`（无 `-` 前缀）启动，即非 login 语义——login argv 为已注册的未来工作，本契约只锁定查找顺序与降级行为。

#### Scenario: 默认 shell 解析

- **WHEN** 用户未指定启动入口且 `$PREFIX/bin` 含 bash
- **THEN** 启动该 bash；`~/.bashrc` 生效，`~/.bash_profile` 不生效（非 login，与 termux login 语义的差异见上）

### Requirement: 工作目录默认

新会话工作目录 SHALL 默认为 `HOME`（`.../files/home`），用户设置的终端启动目录优先。

#### Scenario: 默认目录

- **WHEN** 用户未设置启动目录
- **THEN** shell 起始于 `HOME` 而非 `/`

### Requirement: 与 termux 的有意差异

以下与 termux-app 实现有意不同，MUST 保持现状：子进程实际 `LANG` 为 `C.UTF-8`（`$PREFIX/etc/termux/termux.env` 兼容文件沿用 termux 上游模板仍写 `en_US.UTF-8`，该文件仅供包内脚本 source，不代表子进程环境）；禁止随 zip 分发的外部 `.sha256` sidecar 校验（本地 `.bootstrap-version.json` 版本 pin 用于判断是否需要重装，不属 sidecar，不违反）。

#### Scenario: 差异不回归

- **WHEN** 审查安装器与环境代码
- **THEN** 不得出现外部 sidecar 校验逻辑，不得将子进程 `LANG` 改为 `en_US.UTF-8`
