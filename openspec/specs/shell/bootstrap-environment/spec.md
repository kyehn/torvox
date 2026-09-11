## Purpose

本能力规定 bootstrap 安装与 shell 子进程环境的绝对必要设置（对照 termux-app 实现提取），作为安装器与 PTY 启动路径的行为契约与回归依据。规范依据：`docs/specification/DESIGN.md` 设置·Bootstrap、终端·Shell、禁止实现条款。

## Requirements

### Requirement: 安装原子性

安装 bootstrap zip 时，系统 SHALL 解压到 staging 目录、按 `SYMLINKS.txt`（`old←new` 格式）创建符号链接，全部成功后原子重命名为正式 prefix；任一步失败 MUST 丢弃 staging 目录并保留旧 prefix，不得留下半安装状态。安装状态不得写入任何标记文件，只认启动入口与 `termux.env` 存在性。旧目录轮转 SHALL 为固定单备份：安装开始时删除已存在的上上次备份，旧 prefix 整体移入备份，成功后保留该备份由用户手动删除。

#### Scenario: 安装失败不破坏旧环境

- **WHEN** 解压/建链/重命名任一步失败
- **THEN** staging 目录被清理，旧 prefix 保持可用，安装报告失败

#### Scenario: 安装成功判定

- **WHEN** 安装完成
- **THEN** 启动入口与 `etc/termux/termux.env` 均存在（`isInstalled` 判定依据），无任何标记文件参与判定

### Requirement: 可执行权限

`bin/`、`libexec/`、`lib/apt/apt-helper`、`lib/apt/methods/` 下的文件（另支持 `EXECUTABLES.txt` 显式列表）在安装后 MUST 具有可执行位（`EXECUTABLE_FILE_MODE=0755`，应用私有目录下与 termux 的 0700 等效安全），否则 shell 与包管理器无法启动。

#### Scenario: 安装后 shell 可执行

- **WHEN** 安装完成
- **THEN** `bin/` 下 shell 二进制可直接执行，无需手动 chmod

### Requirement: nix bootstrap 兼容

nix bootstrap 上游来源为 `nix-community/nix-on-droid`（其构建脚本生成 termux 兼容格式：`SYMLINKS.txt` 同为 `old←new`、`EXECUTABLES.txt` 为权威可执行列表）。torvox 安装器不假设任何 fork 仓库存在，只认包内格式契约。

nix-on-droid 等非 termux 布局的 bootstrap（可执行文件散布 `nix/store/<hash>/bin/`）MUST 随包提供 `EXECUTABLES.txt`（每行一路径），安装器 SHALL 按该列表逐个 chmod；其 `SYMLINKS.txt` 与 termux 同为 `old←new` 格式，安装器同一解析路径处理，不得为 nix 布局另建分支。

#### Scenario: nix bootstrap 可安装

- **WHEN** 安装含 `EXECUTABLES.txt` 的 nix 风格 bootstrap
- **THEN** `nix/store` 下所列二进制获得可执行位，shell 可启动

### Requirement: nix 二进制执行方式

glibc 动态链接的 nix/store 二进制直接 exec 报缺 interpreter（预期行为，非安装失败）；特权 shell 上下文（`run-as` 实证）经 `login`→`proot-static`→`login-inner` 全链可用。应用进程上下文（untrusted_app）的设备端结论（2026-09-11 实证）：`bin/login` 为 `/system/bin/` shebang 脚本时经解释器直调启动（`SPAWN_SCRIPT`）；其内部 `exec proot-static` 报 EACCES（`proot-static` 为 ET_EXEC，应用域无执行许可，`ptrace` 同样受限），shell 以退出码 126 结束并经 `[Process completed]` 保留输出显示，不做任何回退。`SYMLINKS.txt` 的 target MUST 为相对 link 父目录的路径（绝对路径被安装器拒绝）。

应用域内可用 nix 须经特权域执行（Shizuku 桥接，见独立变更），本规约只锁定安装器与启动路径行为。

参考对照（sylirre/ghostty-android-terminal 源码）：该项目为绕开同一限制（W^X 禁止 exec 应用数据下 ELF）采用进程内用户态 ELF 装载引擎，彻底避开 exec/proot/ptrace——独立证实本限制的真实性；但该方案体量远超本项目“简单”约束，列为已评估拒绝。

#### Scenario: nix 命令可用

- **WHEN** 在 adb shell 上下文经上述 proot 绑定运行 `nix --version`
- **THEN** 输出版本号；直接执行则报缺 interpreter（预期行为，非安装失败）；应用进程上下文经 Shizuku 桥接（开关 + 授权 + `shizuku-login.sh`，`Shizuku login bridge active` 为准）在真实终端输入下同样输出版本号，且 hermetic `nix build` 产物可 `cat` 读回

### Requirement: 子进程环境变量

shell 子进程启动时，系统 SHALL 设置：`PREFIX=$FILES/usr`、`HOME=$FILES/home`、`PATH=$PREFIX/bin` 起始并透传系统 PATH、`TMPDIR=$PREFIX/tmp`（无 prefix 时 `/data/local/tmp`）、`LANG=C.UTF-8`、`TERM=xterm-256color`、`COLORTERM=truecolor`、`SHELL=<shell 路径>`、`PWD=<工作目录>`、`USER`、`LINES`/`COLUMNS`；Android 7+ MUST NOT 设置 `LD_LIBRARY_PATH`。系统 MUST NOT 设置 `TERM_PROGRAM`/`TERM_PROGRAM_VERSION`（无消费者，已删除；用户覆盖可经 extra 机制加回）。

#### Scenario: 环境变量完整

- **WHEN** 新会话 shell 启动
- **THEN** 上述变量均已设置且路径指向应用私有目录，不得指向系统或其他应用路径

### Requirement: exec 桥接与证书

有 prefix 时系统 SHALL 设置 `LD_PRELOAD=$PREFIX/lib/libtermux-exec-ld-preload.so`（不存在则回退 `libtermux-exec.so`），使子进程经系统 linker 执行（Android 15+ SELinux 生存必需）；`$PREFIX/etc/tls/cert.pem` 存在时 SHALL 设置 `SSL_CERT_FILE`/`CURL_CA_BUNDLE` 指向它；系统 SHALL 透传 `ANDROID_*`/`BOOTCLASSPATH` 等宿主变量（`am`/`content` 调用必需），仅转发宿主进程中存在的变量，不得硬编码值。

#### Scenario: 子进程可执行 prefix 二进制

- **WHEN** shell 执行 `$PREFIX/bin` 下程序
- **THEN** 不得出现 EACCES，`cargo`/`curl` HTTPS 不得报证书错误

### Requirement: 登录 shell 语义

`executable` 未指定时，系统 SHALL 按 `login`/`bash`/`zsh`/`fish`/`sh` 顺序在 `$PREFIX/bin` 中查找可执行项（ELF 二进制，或首行 shebang 指向 `/system/bin/` 的启动脚本，后者经内核直接执行不走 linker 桥接），找不到时降级 `/system/bin/sh`。首行指向应用私有目录的脚本不计入（其解释器本身尚不可用）。启动入口本身执行失败时系统 MUST NOT 自动回退其他入口：输出保留显示，由用户确认关闭。当前实现以普通 `argv[0]`（无 `-` 前缀）启动，即非 login 语义——login argv 为已注册的未来工作，本契约只锁定查找顺序与降级行为。

#### Scenario: 默认 shell 解析

- **WHEN** 用户未指定启动入口且 `$PREFIX/bin` 含 bash
- **THEN** 启动该 bash；`~/.bashrc` 生效，`~/.bash_profile` 不生效（非 login，与 termux login 语义的差异见上）

### Requirement: 工作目录默认

新会话工作目录 SHALL 默认为 `HOME`（`.../files/home`），用户设置的终端启动目录优先。

#### Scenario: 默认目录

- **WHEN** 用户未设置启动目录
- **THEN** shell 起始于 `HOME` 而非 `/`

### Requirement: 与 termux 的有意差异

以下与 termux-app 实现有意不同，MUST 保持现状：子进程实际 `LANG` 为 `C.UTF-8`（对抗审查结论：最小 bootstrap 无 locale 数据时 `en_US.UTF-8` 会回退异常，`C.UTF-8` 永真；`$PREFIX/etc/termux/termux.env` 兼容文件沿用 termux 上游模板仍写 `en_US.UTF-8`，该文件仅供包内脚本 source，不代表子进程环境）；禁止随 zip 分发的外部 `.sha256` sidecar 校验（本地 `.bootstrap-version.json` 版本 pin 用于判断是否需要重装，不属 sidecar，不违反）。

#### Scenario: 差异不回归

- **WHEN** 审查安装器与环境代码
- **THEN** 不得出现外部 sidecar 校验逻辑，不得将子进程 `LANG` 改为 `en_US.UTF-8`
