# termux-bootstrap 绝对必需项列表

> 核对时间：2026-09-12，上游 `termux/termux-app` 的 `/tmp/termux-app` 快照。
> 收录原则：每条均亲见上游源码原文，带文件与行号；未亲见的一律不收录。
> 默认包名 `com.termux` 下的默认路径仅作注释说明，路径以 `TermuxConstants` 拼接逻辑为准。

## 一、会话环境变量

组装链：`termux.c:create_subprocess` 先 `clearenv()` 再逐项 `putenv(envp)`，C 层不补变量（`terminal-emulator/src/main/jni/termux.c:98-99`）。Java 层由 `AndroidShellEnvironment`、`TermuxShellEnvironment`、`TermuxAppShellEnvironment`、`TermuxAPIShellEnvironment` 叠加组装，安装成功后 `writeEnvironmentToFile()` 把同一份表经 `.tmp` 原子改名写入 `termux.env`（`TermuxShellEnvironment.java:43-56`）。

| 变量 | 值 | 原文出处 |
|---|---|---|
| `HOME` | `TERMUX_HOME_DIR_PATH` | `TermuxShellEnvironment.java:78` |
| `PREFIX` | `TERMUX_PREFIX_DIR_PATH` | `TermuxShellEnvironment.java:79` |
| `TMPDIR` | `TERMUX_TMP_PREFIX_DIR_PATH` | `TermuxShellEnvironment.java:83` |
| `PATH`（`apt-android-7`） | `$PREFIX/bin`，并删除 `LD_LIBRARY_PATH` | `TermuxShellEnvironment.java:90-91`，注释原文：`Termux binaries on Android 7+ rely on DT_RUNPATH, so LD_LIBRARY_PATH should be unset by default` |
| `PATH`（`apt-android-5`） | `$PREFIX/bin:$PREFIX/bin/applets` | `TermuxShellEnvironment.java:86` |
| `LD_LIBRARY_PATH`（仅 `apt-android-5`） | `$PREFIX/lib` | `TermuxShellEnvironment.java:87` |
| `LANG` | `en_US.UTF-8` | `AndroidShellEnvironment.java:36` |
| `COLORTERM` | `truecolor` | `AndroidShellEnvironment.java:40` |
| `TERM` | `xterm-256color` | `AndroidShellEnvironment.java:41` |
| `PWD` | 启动目录绝对路径，缺省 `$HOME` | `AndroidShellEnvironment.java:89-90`，注释原文：`PWD must be absolute path` |
| `TERMUX_VERSION` | 应用 `versionName` | `TermuxAppShellEnvironment`（`TERMUX_ENV_PREFIX_ROOT + "_VERSION"`） |
| `TERMUX_APP__AM_SOCKET_SERVER_ENABLED` 等 `TERMUX_APP__` 前缀系列 | 按包管理器与应用信息动态取值 | `TermuxAppShellEnvironment.java:77,132,164-168` |
| `TERMUX_API_APP__VERSION_NAME` | 取 Termux:API 包 `versionName`，取不到包信息时整组返回空 | `TermuxAPIShellEnvironment.java:22-38` |

宿主透传变量（仅宿主存在时透传，不硬编码）：`ANDROID_ASSETS`、`ANDROID_DATA`、`ANDROID_ROOT`、`ANDROID_STORAGE`、`EXTERNAL_STORAGE`、`ASEC_MOUNTPOINT`、`LOOP_MOUNTPOINT`、`ANDROID_RUNTIME_ROOT`、`ANDROID_ART_ROOT`、`ANDROID_I18N_ROOT`、`ANDROID_TZDATA_ROOT`、`BOOTCLASSPATH`、`DEX2OATBOOTCLASSPATH`、`SYSTEMSERVERCLASSPATH`。

明确非官方：`LD_PRELOAD=libtermux-exec*.so` 在上游环境变量表中不存在，系 `termux-exec` 包扩展，不得记为官方行为。

## 二、安装设置

- 路径（`TermuxConstants.java`）：`files`（`:588`，默认 `/data/data/com.termux/files`）、`$PREFIX`（`:595`，`files/usr`）、`$PREFIX/bin`（`:601`）、`$PREFIX/lib`（`:619`）、`$PREFIX/tmp`（`:637`）、`$STAGING_PREFIX`（`:650`，`files/usr-staging`）、`$HOME`（`:657`，`files/home`）。
- 包变体（`TermuxBootstrap.java:180-183`）：仅 `APT_ANDROID_7("apt-android-7")`、`APT_ANDROID_5("apt-android-5")`。
- 流程（`TermuxInstaller.java`）：清除残留 `$STAGING_PREFIX`（`:127`）→ 解压到 staging（`:154`）→ 遇 `SYMLINKS.txt` 记录软链（`:163`），缺失即抛错（`:207-208`）→ `renameTo` 原子切换为 `$PREFIX`（`:215`）；装到便携 SD 卡拒绝报错（`:93,270`）。
- 权限位（`TermuxInstaller.java:197-201`）：仅 `bin/`、`libexec`、`lib/apt/apt-helper`、`lib/apt/methods` 置 `0700`。
- 环境文件（`TermuxConstants.java:668,784,787`）：`$PREFIX/etc/termux/termux.env`，临时文件 `termux.env.tmp`。

## 三、复核命令

- `grep -rn "TERMUX_PREFIX_DIR_PATH =" /tmp/termux-app/termux-shared/src/main/java/com/termux/shared/termux/TermuxConstants.java`
- `grep -n "ENV_HOME\|ENV_PREFIX\|ENV_TMPDIR\|ENV_PATH\|ENV_LD_LIBRARY_PATH" /tmp/termux-app/termux-shared/src/main/java/com/termux/shared/termux/shell/command/environment/TermuxShellEnvironment.java`
- `grep -n "ENV_LANG\|ENV_COLORTERM\|ENV_TERM\|ENV_PWD" /tmp/termux-app/termux-shared/src/main/java/com/termux/shared/shell/command/environment/AndroidShellEnvironment.java`
- `grep -n "SYMLINKS.txt\|renameTo\|Os.chmod" /tmp/termux-app/app/src/main/java/com/termux/app/TermuxInstaller.java`
