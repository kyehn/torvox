# termux-bootstrap 必需环境变量与设置

> 核对时间：2026-09-11，上游 `master` 分支。
> 来源：`termux/termux-app`（`termux-shared`、`app`（含 `TermuxService`）、`terminal-emulator`）、
> `nix-community/nix-on-droid`（README 技术总览、`nix-on-droid.sh`、`default.nix`）、
> 实物 `bootstrap-x86_64.zip`（2026-06-21，3656 文件）。
> 溯源：`A`=读过官方文件全文或亲验实物；`B`=官方常量名已确认＋本仓移植交叉；
> `C`=未亲眼确认，见第六章。

## 一、组装链（A）

`terminal-emulator/.../termux.c`（`create_subprocess`）：先 `clearenv()` 再逐项
`putenv(envp)`，**C 层不补变量，Java 传入即全部**。

Java 层：`UnixShellEnvironment`（通用常量名）← `AndroidShellEnvironment`
← `TermuxShellEnvironment`（叠加 `TermuxAppShellEnvironment` 与
`TermuxAPIShellEnvironment`）。`TermuxService` 仅传递 `ExecutionCommand`
（`workingDirectory`、`isFailsafe` 等），最终 env 数组由
`setupShellCommandEnvironment` 链组装。安装成功后 `writeEnvironmentToFile()`
把同一份表写入 `termux.env`（经 `.tmp` 原子改名）。

`TermuxAPIShellEnvironment` 仅在 Termux:API 相关包检查通过时生效（取不到包信息即返回
空），变量前缀为 `TERMUX_API_APP__`（`TERMUX_ENV_PREFIX_ROOT + "_API_APP__"`），已确认含
`VERSION_NAME`（取 API 包 `versionName`），其余变量明细仍见第六章。

## 二、会话环境变量（`apt-android-7`，非 failsafe，A）

| 变量 | 值 | 出处 |
|---|---|---|
| `HOME` | `TERMUX_HOME_DIR_PATH`（默认 `files/home`） | `TermuxShellEnvironment`（`environment.put(ENV_HOME, …)`） |
| `PREFIX` | `TERMUX_PREFIX_DIR_PATH`（默认 `files/usr`） | `TermuxShellEnvironment`（`environment.put(ENV_PREFIX, …)`） |
| `PATH` | `$PREFIX/bin`，**删除 `LD_LIBRARY_PATH`**（二进制靠 `DT_RUNPATH`） | `TermuxShellEnvironment` |
| `TMPDIR` | `$PREFIX/tmp` | 同上 |
| `TERM` | `xterm-256color` | `AndroidShellEnvironment` |
| `COLORTERM` | `truecolor` | 同上 |
| `LANG` | `en_US.UTF-8`（官方值；本仓规范为 `C.UTF-8`，见第五章） | 同上 |
| `PWD` | 启动目录绝对路径（缺省 `$HOME`，必为绝对路径） | `AndroidShellEnvironment.setupShellCommandEnvironment` |
| `ANDROID_ASSETS`、`ANDROID_DATA`、`ANDROID_ROOT`、`ANDROID_STORAGE`、`EXTERNAL_STORAGE`、`ASEC_MOUNTPOINT`、`LOOP_MOUNTPOINT`、`ANDROID_RUNTIME_ROOT`、`ANDROID_ART_ROOT`、`ANDROID_I18N_ROOT`、`ANDROID_TZDATA_ROOT`、`BOOTCLASSPATH`、`DEX2OATBOOTCLASSPATH`、`SYSTEMSERVERCLASSPATH` | 仅宿主存在时透传，不硬编码（`EXTERNAL_STORAGE` 为 `/system/bin/am` 所需） | `AndroidShellEnvironment`＋本仓 `pty.rs` 交集（14 项） |
| `TERMUX_VERSION` | 应用 `versionName` | `TermuxAppShellEnvironment`（`TERMUX_ENV_PREFIX_ROOT + "_VERSION"`） |
| `TERMUX_APP__VERSION_NAME`、`VERSION_CODE`、`PACKAGE_NAME`、`PID`、`UID`、`TARGET_SDK`、`IS_DEBUGGABLE_BUILD`、`APK_RELEASE`、`APK_PATH`、`IS_INSTALLED_ON_EXTERNAL_STORAGE`、`SE_PROCESS_CONTEXT`、`SE_FILE_CONTEXT`、`SE_INFO`、`USER_ID`、`PROFILE_OWNER`、`PACKAGE_MANAGER`、`PACKAGE_VARIANT`、`FILES_DIR`、`AM_SOCKET_SERVER_ENABLED`（前缀 `TERMUX_APP__`） | 按包管理器与应用信息动态取值 | `TermuxAppShellEnvironment` |

变体差异（A）：`apt-android-5` 时 `PATH`=`$PREFIX/bin:$PREFIX/bin/applets` 且
`LD_LIBRARY_PATH`=`$PREFIX/lib`；failsafe 会话不覆盖 `PATH`/`TMPDIR`，沿用系统值；
默认工作目录为 `$HOME`。

注意：官方表**无 `LD_PRELOAD`**。`LD_PRELOAD=libtermux-exec*.so` 是 `termux-exec`
包＋本仓 linker 桥接的扩展（Android 15+ SELinux `execute_no_trans` 绕行），不得记为官方行为。

## 三、安装设置（A，路径字符串原文已抽查）

- 路径（`TermuxConstants.java`，默认包名 `com.termux` 下）：`files`=`/data/data/com.termux/files`、
  `$PREFIX`=`files/usr`、`$HOME`=`files/home`、`$STAGING_PREFIX`=`files/usr-staging`、
  `$PREFIX/bin`、`$PREFIX/lib`、`$PREFIX/libexec`、`$PREFIX/tmp`。

- 包变体（`TermuxBootstrap.java`）：包管理器仅 `apt`；变体仅 `apt-android-7` /
  `apt-android-5`，变体名前缀须与包管理器名一致。
- `$PREFIX` 已存在且非空即视为正确，跳过安装。
- 解压到 `$STAGING_PREFIX`（默认 `files/usr-staging`），成功后 `renameTo` 原子切换为
  `$PREFIX`（默认 `files/usr`，`$HOME` 默认 `files/home`）；失败不留半成品。
- 实物验证：`SYMLINKS.txt`（63906 字节）以 `←` 分隔 `目标←链接名`，缺失即抛错；
  仅 `bin/`、`libexec`、`lib/apt/apt-helper`、`lib/apt/methods` 置 `0700`。
- 仅主用户可装；装到便携 SD 卡拒绝报错。
- `termux-storage`：`~/storage/shared`、`documents`、`downloads`、`dcim`、
  `pictures`、`music`、`movies` 及 `external-N`、`media-N` 等 symlink。
- 环境文件：`$PREFIX/etc/termux/termux.env`（临时文件 `termux.env.tmp`）。

## 四、nix-on-droid 格式差异（A，本节依据对方 README 技术总览）

对方**同样提供 bootstrap zipball**，但内容与目的根本不同。构建与安装分九步：

开发者侧：针对 `bionic` 交叉编译 `proot`（伪造 `/nix/store` 等路径，
用户态 `chroot`）；取官方 release tarball 的目标 `nix`；初始化 `nix` 数据库；
用 `nix`＋模块系统构建支撑脚本与配置文件；打包成 bootstrap zipball 发布到 HTTP。

用户侧：在应用内填入 bootstrap URL，下载解压；首启时 `nix` 构建环境
（或从 Cachix 拉取），再由 `nix` 安装环境（登录脚本、配置文件等）。
设备端日常管理经 `nix-on-droid.sh`：`nix build` 生成 `activationPackage`，
执行其 `activate` 切换 generation（profile 位于
`/nix/var/nix/profiles/nix-on-droid`，含 `doSwitch`/`doSwitchGeneration`/`doBuild`）。

| 维度 | termux bootstrap | nix-on-droid |
|---|---|---|
| 分发物 | 按 ABI 预构建 zip（含 `SYMLINKS.txt` 的 `dpkg` 根文件系统） | bootstrap zipball（含 `proot`＋`nix`＋已初始化数据库＋支撑脚本，非根文件系统） |
| 包格式 | `dpkg`/`apt` | `nix store`（内容寻址） |
| 安装 | staging 解压＋`renameTo` 原子切换为 `$PREFIX` | 下载解压后首启由 `nix` 构建并安装环境 |
| 环境表 | `termux.env`＋会话 env 数组（含 `PREFIX` 语义） | activation 与用户 Nix 配置生成，无 `PREFIX` 语义 |
| 回滚 | 不支持 | `generations`/`rollback` 原生支持 |

## 五、本仓对照（A，本仓源码已读）

- `native/src/terminal/pty.rs`（`base_env`/`build_env`）：对应第二章；其中
  `LANG=C.UTF-8` 遵循本仓 `DESIGN.md`（官方为 `en_US.UTF-8`，差异有意）；
  `LD_PRELOAD` 优先 `lib/libtermux-exec-ld-preload.so`，回退 `lib/libtermux-exec.so`。
- `android/.../installer/BootstrapInstaller.kt`：对应第三章（staging、原子切换、
  `SYMLINKS.txt`、`EXECUTABLES.txt`、`0700`、zip-slip 与 symlink 越狱 guard）。
- `BootstrapInstallService.kt`：`filesDir/usr`、`home`、`usr-staging`，与官方路径常量同构。
- `SecondStageRunner.kt`：`postinst` 经 system linker 执行（SELinux 现状，本仓扩展）。

## 六、未亲眼确认项（C，不得视为定论）

- `TermuxConstants.java` 各 `*_DIR_PATH` 字符串原文（已抽查 `FILES/PREFIX/HOME/STAGING/BIN/LIB/LIBEXEC/TMP/ENV` 等，多组默认字符串已亲见，其余以本仓同构值为旁证）。
- `TermuxService.createTermuxSession` 是否直传 `getEnvironment` 结果（已确认其
  传递 `workingDirectory`/`isFailsafe` 的 `ExecutionCommand`，最终组装点待读
  `TermuxSession`/`ShellManager` 全文）。
- `TermuxAPIShellEnvironment` 除前缀、生效条件、`VERSION_NAME` 外的变量明细。
- 复核命令：`git clone --depth 1 https://github.com/termux/termux-app` 后
  `grep -rn "TERMUX_PREFIX_DIR_PATH =\|ENV_TERMUX_APP__AM_SOCKET" termux-shared`。
