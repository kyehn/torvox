# termux-app

## 来源与口径

来源：<https://github.com/termux/termux-app/tree/github-releases/v0.119.0-beta.3>（`DESIGN.md` 指定的参照版本）。文中 `:行号` 指该锚点下的上游源码行号，`文件名:行号` 指上游源文件；本项目的落点另行标注。

## 概况

四模块结构：`terminal-emulator`（Java 终端引擎）、`terminal-view`（View 渲染层）、`termux-shared`（修饰键栏、主题、shell 环境、通知）、`app`（Service/Installer/Activity）。applicationId `com.termux`，`sharedUserId` 绑定全部插件。

## 关键实现

### 终端会话

`TerminalSession`：JNI `createSubprocess` 建 PTY；双向各一条 `ByteQueue`（进程→终端 64 KiB，终端→进程 4 KiB）；三线程分工——InputReader 读 PTY 写队列、OutputWriter 把队列写回 PTY、Waiter 阻塞 `waitFor` 上报退出码；仿真回调全部经主线程 Handler 消息驱动（`MSG_NEW_INPUT`/`MSG_PROCESS_EXITED`）。工作目录查询走 `/proc/<pid>/cwd` symlink。会话经 `finishIfRunning` 发 SIGKILL 结束。

### bootstrap 安装

`TermuxInstaller`：prefix 存在即跳过；staging 目录先清后用；zip 流式提取（`SYMLINKS.txt` 记录延迟建链，其余条目直接落盘）；完成才 rename 发布；非主用户与 files 目录不可达均有明确的失败对话框。

### 修饰键栏

`SpecialButtonState` 三态（`isCreated`/`isActive`/`isLocked`）：单击激活、长按锁定、读后自动失效（锁定除外）——本项目粘滞键语义的出处。激活反馈是 MaterialButton 文字变色，无缩放动画。auto-repeat 用定时器逐次触发，长按时长取系统 `ViewConfiguration.getLongPressTimeout`（随无障碍设置变化），并有上下限钳制。默认布局为 2 行 7 列（`DEFAULT_IVALUE_EXTRA_KEYS`：上行 `ESC`、`DRAWER`（弹 `PASTE`）、`SCROLL`、`HOME`、`UP`、`END`、`PGUP`；下行 `TAB`、`CTRL`、`ALT`、`LEFT`、`DOWN`、`RIGHT`、`PGDN`）。

### 其他权威常量

回滚默认 2000 行（钳制区间 100–50000）；`TERM=xterm-256color`；Android 7+ 因 DT_RUNPATH 显式移除 `LD_LIBRARY_PATH`；failsafe 模式保留宿主 `PATH`/`TMPDIR`。光标为块状并反色其下文字（`invertCursorTextColor`）。`KeyHandler` 持有完整 KEYCODE→escape 序列映射表（F1–F12、方向、Insert/Home/End/PgUp/PgDn、小键盘数字、Break/Back 映射 ESC）。

### 前台服务

`TermuxService` 持会话列表与后台任务；Activity client 与 Service client 分离（`onUnbind` 解除 Activity 引用防泄漏）；wake 与 wifi 锁成对持有；`START_NOT_STICKY`；`onDestroy` 清 TMPDIR 并 kill 全部会话。

### 测试

`terminal-emulator` 配 15+ JVM 测试（`WcWidth`、`KeyHandler`、`DecSet`、`OSC`、`DCS`、`APC`、`Resize`、`History`、`UnicodeInput`、`RectangularAreas`、`ScreenBuffer` 等），以 `TerminalTestCase` 为基座直接驱动仿真器断言屏幕文本。

## 目录布局与路径常量

全部路径常量集中于 `termux-shared/src/main/java/com/termux/shared/termux/TermuxConstants.java`，应用与插件共用同一份定义。

| 常量 | 默认值 |
| --- | --- | --- |
| `TERMUX_INTERNAL_PRIVATE_APP_DATA_DIR_PATH` | `/data/data/com.termux` |
| `TERMUX_FILES_DIR_PATH` | `/data/data/com.termux/files` |
| `TERMUX_PREFIX_DIR_PATH` | `/data/data/com.termux/files/usr` |
| `TERMUX_BIN_PREFIX_DIR_PATH` | `/data/data/com.termux/files/usr/bin` |
| `TERMUX_ETC_PREFIX_DIR_PATH` | `/data/data/com.termux/files/usr/etc` |
| `TERMUX_TMP_PREFIX_DIR_PATH` | `/data/data/com.termux/files/usr/tmp` |
| `TERMUX_STAGING_PREFIX_DIR_PATH` | `/data/data/com.termux/files/usr-staging` |
| `TERMUX_HOME_DIR_PATH` | `/data/data/com.termux/files/home` |
| `TERMUX_DATA_HOME_DIR_PATH` | `/data/data/com.termux/files/home/.termux` |
| `TERMUX_CONFIG_HOME_DIR_PATH` | `/data/data/com.termux/files/home/.config/termux` |
| `TERMUX_CONFIG_PREFIX_DIR_PATH` | `/data/data/com.termux/files/usr/etc/termux` |
| `TERMUX_STORAGE_HOME_DIR_PATH` | `/data/data/com.termux/files/home/storage` |
| `TERMUX_APPS_DIR_PATH` | `/data/data/com.termux/files/apps` |
| `TERMUX_ENV_FILE_PATH` | `/data/data/com.termux/files/usr/etc/termux/termux.env` |
| `TERMUX_ENV_TEMP_FILE_PATH` | `…/termux.env.tmp` |
| `TERMUX_CRASH_LOG_FILE_PATH` | `/data/data/com.termux/files/home/crash_log.md` |
| `TERMUX_CRASH_LOG_BACKUP_FILE_PATH` | `/data/data/com.termux/files/home/crash_log_backup.md` |

三个设计要点：

- **`$PREFIX` 判定为「空」时的忽略清单**：`TERMUX_PREFIX_DIR_IGNORED_SUB_FILES_PATHS_TO_CONSIDER_AS_EMPTY` 列出 `usr/tmp`、`termux.env`、`termux.env.tmp`——这三者存在不算 bootstrap 已安装。
- **双配置目录**：`~/.termux/`（`XDG_DATA_HOME` 语义）与 `~/.config/termux/`（`XDG_CONFIG_HOME` 语义）并存，构成 primary/secondary 两级回落。
- **包名**：`TERMUX_PACKAGE_NAME = "com.termux"`，插件为 `com.termux.api`、`.boot`、`.window`、`.styling`、`.tasker`、`.widget`，全部经 `sharedUserId="com.termux"` 共享 uid（`AndroidManifest.xml`）。

## 对外契约

### Manifest 暴露面

`app/src/main/AndroidManifest.xml`：

| 组件 | exported | 门禁 | 说明 |
| --- | --- | --- | --- |
| `TermuxActivity` | true | 无 | `launchMode="singleTask"`，MAIN/LAUNCHER + LEANBACK_LAUNCHER；别名 `HomeActivity` 额外接 IOT_LAUNCHER |
| `SettingsActivity` | true | 无 | — |
| `FileShareReceiverActivity`（别名） | true | 无 | ACTION_SEND，7 个 mime 类型 |
| `FileViewReceiverActivity`（别名） | true | 无 | ACTION_VIEW，5 个 mime 类型 |
| `TermuxDocumentsProvider` | true | `MANAGE_DOCUMENTS` | authority `com.termux.documents` |
| `TermuxOpenReceiver$ContentProvider` | true | `com.termux.permission.RUN_COMMAND` | authority `com.termux.files` |
| `RunCommandService` | true | `com.termux.permission.RUN_COMMAND` | action `com.termux.RUN_COMMAND` |
| `TermuxService` | false | — | 仅应用内 |
| `SystemEventReceiver` | false | — | 静态注册 BOOT_COMPLETED |

应用级属性：`installLocation="internalOnly"`、`allowBackup="false"`、`requestLegacyExternalStorage="true"`。

自定义权限 `com.termux.permission.RUN_COMMAND` 声明为 `protectionLevel="dangerous"`——即调用方需用户显式授权。

### 权限双门禁

两条独立防线，缺一不可：

1. **Android 权限**：`PERMISSION_RUN_COMMAND = "com.termux.permission.RUN_COMMAND"`（`TermuxConstants.java`）。`RunCommandService` 在 Manifest 以 `android:permission` 声明，系统层强制。
2. **属性二次确认**：`PROP_ALLOW_EXTERNAL_APPS = "allow-external-apps"`，默认 `"false"`，定义在 `~/.termux/termux.properties`。

`RunCommandService.onHandleIntent` 中该校验位于所有 extras 解析之后、可执行文件校验之前（`RunCommandService.java`）：

```java
errmsg = TermuxPluginUtils.checkIfAllowExternalAppsPolicyIsViolated(this, LOG_TAG);
if (errmsg != null) { /* state failed + 强制通知 + stopService */ }
```

违规时除 `stopService` 外还会**强制弹出通知**（`processPluginExecutionCommandError(..., true)` 的末参为 `forceNotification`），理由是「 termux 上下文内执行命令可能来自恶意应用或导入的 Tasker 工程，而非用户本人」。

同时，文件型结果（`EXTRA_RESULT_DIRECTORY`）必须在 `allow-external-apps` 确认之后才赋值，因为 `ResultSender` 会把调用方控制的 `resultFileErrorFormat` 直接交给 `String.format`，可用于覆写 `termux.properties` 自身来打开 `allow-external-apps`。

`TERMUX_PROPERTIES_FILE_PATHS_LIST`与 `TERMUX_FLOAT_PROPERTIES_FILE_PATHS_LIST`被 `ContentProvider.openFile` 强制改为只读模式（见 4.8）。

### RUN_COMMAND Intent 契约

`RunCommandService`（action `com.termux.RUN_COMMAND`） extras 全集（`TermuxConstants.java`）：

| extra | 常量值 | 类型 |
| --- | --- | --- |
| `EXTRA_COMMAND_PATH` | `com.termux.RUN_COMMAND_PATH` | String |
| `EXTRA_ARGUMENTS` | `com.termux.RUN_COMMAND_ARGUMENTS` | String[] |
| `EXTRA_REPLACE_COMMA_ALTERNATIVE_CHARS_IN_ARGUMENTS` | `com.termux.RUN_COMMAND_REPLACE_COMMA_ALTERNATIVE_CHARS_IN_ARGUMENTS` | boolean |
| `EXTRA_COMMA_ALTERNATIVE_CHARS_IN_ARGUMENTS` | `com.termux.RUN_COMMAND_COMMA_ALTERNATIVE_CHARS_IN_ARGUMENTS` | String |
| `EXTRA_STDIN` | `com.termux.RUN_COMMAND_STDIN` | String |
| `EXTRA_WORKDIR` | `com.termux.RUN_COMMAND_WORKDIR` | String |
| `EXTRA_RUNNER` | `com.termux.RUN_COMMAND_RUNNER` | String |
| `EXTRA_BACKGROUND` | `com.termux.RUN_COMMAND_BACKGROUND` | boolean（已废弃） |
| `EXTRA_BACKGROUND_CUSTOM_LOG_LEVEL` | `com.termux.RUN_COMMAND_BACKGROUND_CUSTOM_LOG_LEVEL` | String |
| `EXTRA_SESSION_ACTION` | `com.termux.RUN_COMMAND_SESSION_ACTION` | String |
| `EXTRA_SHELL_NAME` | `com.termux.RUN_COMMAND_SHELL_NAME` | String |
| `EXTRA_SHELL_CREATE_MODE` | `com.termux.RUN_COMMAND_SHELL_CREATE_MODE` | String |
| `EXTRA_COMMAND_LABEL` | `com.termux.RUN_COMMAND_COMMAND_LABEL` | String |
| `EXTRA_COMMAND_DESCRIPTION` | `com.termux.RUN_COMMAND_COMMAND_DESCRIPTION` | String |
| `EXTRA_COMMAND_HELP` | `com.termux.RUN_COMMAND_COMMAND_HELP` | String |
| `EXTRA_PENDING_INTENT` | `com.termux.RUN_COMMAND_PENDING_INTENT` | Parcelable |
| `EXTRA_RESULT_DIRECTORY` | `com.termux.RUN_COMMAND_RESULT_DIRECTORY` | String |
| `EXTRA_RESULT_SINGLE_FILE` | `com.termux.RUN_COMMAND_RESULT_SINGLE_FILE` | boolean |
| `EXTRA_RESULT_FILE_BASENAME` | `com.termux.RUN_COMMAND_RESULT_FILE_BASENAME` | String |
| `EXTRA_RESULT_FILE_OUTPUT_FORMAT` | `com.termux.RUN_COMMAND_RESULT_FILE_OUTPUT_FORMAT` | String |
| `EXTRA_RESULT_FILE_ERROR_FORMAT` | `com.termux.RUN_COMMAND_RESULT_FILE_ERROR_FORMAT` | String |
| `EXTRA_RESULT_FILES_SUFFIX` | `com.termux.RUN_COMMAND_RESULT_FILES_SUFFIX` | String |

处理顺序（`RunCommandService.java`）：

1. 逗号替代字符替换——Tasker/Locale 类插件宿主会吃掉参数中的逗号，故约定用 `COMMA_ALTERNATIVE`（U+201A `‚`）替代 `COMMA_NORMAL`；字符集可由 extra 覆盖。
2. stdin / workdir / runner / sessionAction / shellName / shellCreateMode / label 等赋值。`runner` 缺省为 `TERMINAL_SESSION`，`EXTRA_BACKGROUND=true` 时取 `APP_SHELL`；非法 runner 直接失败。
3. `allow-external-apps` 校验。
4. 结果目录 extras。
5. 可执行文件：取 canonical path，要求是 regular file 且可读可执行；**不自动补齐缺失的读/执行权限**。
6. 工作目录：canonical path，要求目录可读可写；仅对允许的 termux 工作目录才自动创建与补齐权限。
7. symlink 特例：若调用方原始路径是 symlink（busybox/coreutils applet），回退用原路径而非 canonical path，否则参数会被传给 coreutils 本体。
8. 组装 `TERMUX_SERVICE.ACTION_SERVICE_EXECUTE` intent 转发。

`EXTRA_SESSION_ACTION` 取值 4 个：`0` 切换并打开 Activity、`1` 保持当前并打开 Activity、`2` 切换到新会话但不打开 Activity、`3` 保持当前且不打开 Activity，`MIN/MAX` 边界见。

### 结果回传契约

两条并行通道。

**PendingIntent 通道**（bundle key `"result"`）：`stdout`、`stdout_original_length`、`stderr`、`stderr_original_length`、`exitCode`、`err`、`errmsg`。

**文件通道**（`resultDirectoryPath`）：

- 多文件模式（`resultSingleFile=false`）：文件名 = 前缀 + `resultFilesSuffix`，前缀为 `err`、`errmsg`、`stdout`、`stderr`、`exit_code`（`ShellCommandConstants.java`）。`err` 文件先写带毫秒时间戳的临时文件再 rename（`ResultSender.java`）。
- 单文件模式（`resultSingleFile=true`）：默认格式字符串（`ShellCommandConstants.java`）：

  | 常量 | 格式 | 选用条件 |
  | --- | --- | --- |
  | `FORMAT_SUCCESS_STDOUT` | `"%1$s%n"` | `err==-1` 且 `stderr` 为空且 `exit_code==0` |
  | `FORMAT_SUCCESS_STDOUT__EXIT_CODE` | `"%1$s%n%n%n%nexit_code=%2$s%n"` | `err==-1` 且 `stderr` 为空且 `exit_code!=0` |
  | `FORMAT_SUCCESS_STDOUT__STDERR__EXIT_CODE` | `"stdout=%n%1$s%n%n%n%nstderr=%n%2$s%n%n%n%nexit_code=%3$s%n"` | `err==-1` 且 `stderr` 非空 |
  | `FORMAT_FAILED_ERR__ERRMSG__STDOUT__STDERR__EXIT_CODE` | 五段 markdown | `err!=-1` |

  调用方可用 `resultFileOutputFormat` / `resultFileErrorFormat` 覆写。格式串禁止定义超过 `%5$s` 的位置参数（`ShellCommandConstants.java`），否则抛 `IllegalFormatException`。

### TERMUX_SERVICE Intent 契约

`TermuxService` 内部 action（`TermuxConstants.java`），由 `RunCommandService` 与 `FileReceiverActivity` 驱动：

| 项 | 常量值 |
| --- | --- |
| `ACTION_SERVICE_EXECUTE` | `com.termux.service_execute` |
| `URI_SCHEME_SERVICE_EXECUTE` | `com.termux.file` |
| `ACTION_STOP_SERVICE` | `com.termux.service_stop` |
| `ACTION_WAKE_LOCK` / `ACTION_WAKE_UNLOCK` | `com.termux.service_wake_lock` / `com.termux.service_wake_unlock` |
| `EXTRA_ARGUMENTS` | `com.termux.execute.arguments` |
| `EXTRA_STDIN` | `com.termux.execute.stdin` |
| `EXTRA_WORKDIR` | `com.termux.execute.cwd` |
| `EXTRA_RUNNER` | `com.termux.execute.runner` |
| `EXTRA_BACKGROUND_CUSTOM_LOG_LEVEL` | `com.termux.execute.background_custom_log_level` |
| `EXTRA_SESSION_ACTION` | `com.termux.execute.session_action` |
| `EXTRA_SHELL_NAME` | `com.termux.execute.shell_name` |
| `EXTRA_SHELL_CREATE_MODE` | `com.termux.execute.shell_create_mode` |
| `EXTRA_COMMAND_LABEL` / `_DESCRIPTION` / `_HELP` | `com.termux.execute.command_label` / `command_description` / `command_help` |
| `EXTRA_PLUGIN_API_HELP` | `com.termux.execute.plugin_help` |
| `EXTRA_PENDING_INTENT` | `pendingIntent`（**无包名前缀**） |
| `EXTRA_RESULT_DIRECTORY` | `com.termux.execute.result_directory` |
| `EXTRA_RESULT_SINGLE_FILE` | `com.termux.execute.result_single_file` |
| `EXTRA_RESULT_FILE_BASENAME` | `com.termux.execute.result_file_basename` |
| `EXTRA_RESULT_FILE_OUTPUT_FORMAT` | `com.termux.execute.result_file_output_format` |
| `EXTRA_RESULT_FILE_ERROR_FORMAT` | `com.termux.execute.result_file_error_format` |
| `EXTRA_RESULT_FILES_SUFFIX` | `com.termux.execute.result_files_suffix` |

`TERMUX_SERVICE` 侧的差异（`TermuxService.actionServiceExecute`）：

- `isFailsafe` 取自 `TERMUX_ACTIVITY.EXTRA_FAILSAFE_SESSION`，与 `RunCommandService` 无关。
- `APP_SHELL` runner 才读取 `stdin`，终端会话不用。
- uri 带 fragment 时一并作为路径（`UriUtils.getUriFilePathWithFragment`）。
- `shellCreateMode` 缺省 `ALWAYS`。
- `executableUri` 参与 intent data，scheme 为 `com.termux.file`。

### 广播契约

| 项 | 常量值 | 方向 |
| --- | --- | --- |
| `BROADCAST_TERMUX_OPENED` | `com.termux.app.OPENED` | app → 外部 receiver |
| `TERMUX_ACTIVITY.ACTION_NOTIFY_APP_CRASH` | `com.termux.app.notify_app_crash` | 插件 → app |
| `TERMUX_ACTIVITY.ACTION_RELOAD_STYLE` | `com.termux.app.reload_style` | 外部 → app |
| `TERMUX_ACTIVITY.EXTRA_RECREATE_ACTIVITY` | `com.termux.app.TermuxActivity.EXTRA_RECREATE_ACTIVITY` | 随 ACTION_RELOAD_STYLE |
| `TERMUX_ACTIVITY.ACTION_REQUEST_PERMISSIONS` | `com.termux.app.request_storage_permissions` | 外部 → app |

`TERMUX_ACTIVITY.EXTRA_RELOAD_STYLE`（与 action 同值）已废弃；历史值 `"storage"` 仍会被改写为 `ACTION_REQUEST_PERMISSIONS`（`TermuxActivity.java`）。

`BROADCAST_TERMUX_OPENED` 采用**逐个显式投递**：先 `queryBroadcastReceivers` 枚举，再为每个命中者 `setComponent` 后单独 `sendBroadcast`，以绕过 Android 8.0 对隐式广播的限制（`TermuxUtils.java`）。

`ACTION_RELOAD_STYLE` 注册于 `TermuxActivity.registerTermuxActivityBroadcastReceiver`，分发要求 `mIsVisible`。

### am.sock UNIX socket 协议

由 `TermuxAmSocketServer`（`termux-shared/.../termux/shell/am/`）承载，socket 文件为 `/data/data/com.termux/files/apps/com.termux/termux-am/am.sock`（`TermuxConstants.java`）。`$PREFIX/bin/termux-am` 客户端经 `$PREFIX/bin/termux-am-socket` 连接，替代每次拉起 dalvik VM 的 `/system/bin/am`。

**协议**（`AmSocketServer` 类注释、实现）：

- 传输：AF_UNIX / SOCK_STREAM，filesystem socket（而非 abstract namespace，出于安全考虑）；`sun_path` 上限 108 字节（`LocalSocketRunConfig.java`）。
- 请求：客户端把 am 命令作为字符串写入输出流并关闭，**不含首词 `am`**。
- 分词：`ArgumentTokenizer.tokenize`，按空白切分，支持单/双引号与双引号内反斜杠转义。
- 响应：`exit_code \0 stdout \0 stderr \0`，以 NUL 分隔，**结尾无 NUL**。
- `exit_code` 钳制到 0-255，越界强制改为 `1`，否则 shell 会报 `Channel number out of range`。
- stdout/stderr 为 `null` 时写空串。

**鉴权**（`LocalServerSocket.accept`）：经 `SO_PEERCRED` 取得 peer uid，仅当 peer uid 等于 server 应用的 uid **或等于 0（root）** 才接受；否则 `onDisallowedClientConnected` 并立即关闭 socket。root 发来的命令将以 termux 用户的权限、能力与 selinux 上下文执行（`TermuxAmSocketServer` 类注释）。

**开关与状态**：

- `run-termux-am-socket-server` 属性（默认 `true`，见 `TERMUX_DEFAULT_TRUE_BOOLEAN_BEHAVIOUR_PROPERTIES_LIST`）控制是否启动；修改后需 force stop 才生效（`TermuxAmSocketServer` 类注释）。
- 状态由 `TERMUX_APP_AM_SOCKET_SERVER_ENABLED` 导出为环境变量，**应用启动后即冻结**，避免旧 shell 读到过期值。
- Android 10+ 执行 `start`/`startservice` 时额外校验 `SYSTEM_ALERT_WINDOW`（`AmSocketServer.java`）。

### 文件共享与文档提供者

**`com.termux.files` ContentProvider**（`TermuxOpenReceiver.ContentProvider`）：

- authority `com.termux.files`，权限 `RUN_COMMAND`，`grantUriPermissions=true`。
- `openFile` 的白名单校验：canonical path 必须以 `TERMUX_FILES_DIR_PATH` 或外部存储根开头，否则抛 `IllegalArgumentException`。
- 随后做 `allow-external-apps` 校验；再对命中 `TERMUX_PROPERTIES_FILE_PATHS_LIST` / `TERMUX_FLOAT_PROPERTIES_FILE_PATHS_LIST` 的路径把 mode 强制改为 `"r"`。注释明确：这是防止外部应用静默改写 `allow-external-apps` 等安全属性。
- 仅 `openFile` 与 `query`、`getType` 有意义；`insert`/`delete`/`update` 返回空/0。
- 配套 `TermuxOpenReceiver` 广播接收 ACTION_VIEW/ACTION_SEND，把 `file://` 与 `content://com.termux.files/...` 交給系统打开；mime 由扩展名推导，缺省 `application/octet-stream`。

**`com.termux.documents`**：`TermuxDocumentsProvider`，权限 `MANAGE_DOCUMENTS`，向系统文件选择器暴露目录树。

### 环境变量契约

三层叠加，每层只负责自己的部分。

**第一层 `AndroidShellEnvironment`**（`termux-shared/.../shell/command/environment/AndroidShellEnvironment.java`）：

| 变量 | 值 |
| --- | --- |
| `HOME` | `/` |
| `LANG` | `en_US.UTF-8` |
| `PATH` | `System.getenv("PATH")`（宿主原值） |
| `TMPDIR` | `/data/local/tmp` |
| `COLORTERM` | `truecolor` |
| `TERM` | `xterm-256color` |

仅宿主存在时才透传：`ANDROID_ASSETS`、`ANDROID_DATA`、`ANDROID_ROOT`、`ANDROID_STORAGE`、`EXTERNAL_STORAGE`、`ASEC_MOUNTPOINT`、`LOOP_MOUNTPOINT`、`ANDROID_RUNTIME_ROOT`、`ANDROID_ART_ROOT`、`ANDROID_I18N_ROOT`、`ANDROID_TZDATA_ROOT`、`BOOTCLASSPATH`、`DEX2OATBOOTCLASSPATH`、`SYSTEMSERVERCLASSPATH`（`putToEnvIfInSystemEnv`）。

**第二层 `TermuxShellEnvironment`**（`TermuxShellEnvironment.java`）：

- `HOME` → `$PREFIX/home`，`PREFIX` → `$PREFIX`。
- 非 failsafe：`TMPDIR` → `usr/tmp`；`apt-android-7`（及以后）设 `PATH=usr/bin` 并 **remove `LD_LIBRARY_PATH`**（Android 7+ 起二进制依赖 DT_RUNPATH）；`apt-android-5` 变体额外追加 `usr/bin/applets` 并设 `LD_LIBRARY_PATH=usr/lib`。
- failsafe：保留宿主 `PATH` 与 `TMPDIR`，以便使用系统二进制。
- 默认工作目录 `$HOME`，默认 bin 路径 `usr/bin`。
- `setupShellCommandEnvironment` 另设 `PWD`（绝对化后的 workingDirectory）并 `createHomeDir`（`AndroidShellEnvironment.java`）。

**第三层 `TermuxAppShellEnvironment`**（`TermuxAppShellEnvironment.java`）：

前缀 `TERMUX_`（`TERMUX_ENV_PREFIX_ROOT`，`TermuxConstants.java`）与应用前缀 `TERMUX_APP__`：

`TERMUX_VERSION`；`TERMUX_APP__VERSION_NAME`、`_VERSION_CODE`、`_PACKAGE_NAME`、`_PID`、`_UID`、`_TARGET_SDK`、`_IS_DEBUGGABLE_BUILD`、`_APK_RELEASE`、`_APK_PATH`、`_IS_INSTALLED_ON_EXTERNAL_STORAGE`、`_SE_PROCESS_CONTEXT`、`_SE_FILE_CONTEXT`、`_SE_INFO`、`_USER_ID`、`_PROFILE_OWNER`、`_PACKAGE_MANAGER`、`_PACKAGE_VARIANT`、`_FILES_DIR`、`_AM_SOCKET_SERVER_ENABLED`。

写入策略：`putToEnvIfSet` 只写非空值；termux app 自身的环境表只计算一次并缓存，其他调用方（插件）每次重算，以反映 termux 安装/更新/卸载。`APK_RELEASE` 由签名 SHA256 映射到 `FDROID` / `GITHUB_DEBUG_BUILD` / `GOOGLE_PLAYSTORE`，并把非字母字符替换为 `_` 再大写。

**`termux.env` 文件**（注意：本项目 `PROHIBITED.md` 禁止）：

- 路径 `usr/etc/termux/termux.env`，临时文件 `termux.env.tmp`。
- 内容为 `export NAME="VALUE"` 行，按变量名排序，值中的 `` " ` \ $ `` 反斜杠转义（`ShellEnvironmentUtils.java`）。
- 写入为原子操作：先写 `.tmp` 再 `moveRegularFile`（`TermuxShellEnvironment.java`），避免与 source/read 竞争。
- 触发点：`TermuxApplication.onCreate`、`SystemEventReceiver` 收到包增删改、`TermuxInstaller` 安装完成。

### bootstrap zip 与崩溃日志契约

**bootstrap**（`app/build.gradle`）：

- URL 模板 `https://github.com/termux/termux-packages/releases/download/bootstrap-<version>/bootstrap-<arch>.zip`。
- 当前 `apt-android-7`：`2026.02.12-r1+apt.android-7`，四架构 `aarch64`/`arm`/`i686`/`x86_64` 的 SHA-256 硬编码；`apt-android-5`（`2022.04.28-r6`）为旧变体，`DESIGN.md` 明令不提供。
- zip 内含 `SYMLINKS.txt`（`<target>←<path>` 每行一条）用于延迟建链，其余条目流式落盘（`TermuxInstaller.java`）；缺失 `SYMLINKS.txt` 抛 `RuntimeException`。
- 安装为 staging → `renameTo` 原子发布；`$PREFIX` 已存在则跳过。
- `~/storage` 下建立 `shared`、`documents`、`downloads`、`dcim`、`pictures`、`music`、`movies`、`podcasts`、`audiobooks`（Android 10+）、`external-N`、`media-N` 一组符号链接。本项目 `DESIGN.md` 禁止创建这些路径。

**崩溃日志**：`~/crash_log.md` 由 `CrashHandler` 写入，读取展示后 `moveRegularFile` 到 `~/crash_log_backup.md`（`TermuxCrashUtils.java`）；通知频道 `termux_crash_reports_notification_channel`（`TermuxConstants.java`），导出路径 `Environment.getExternalStorageDirectory/Termux-Crash-Report.log`。

**通知 id**：`TERMUX_APP_NOTIFICATION_ID = 1337`、`TERMUX_RUN_COMMAND_NOTIFICATION_ID = 1338`、`TERMUX_FLOAT_APP_NOTIFICATION_ID = 1339`。

**其他常量**：Widget 令牌 `com.termux.shortcut.token`；Widget action `com.termux.widget.ACTION_WIDGET_ITEM_CLICKED` / `ACTION_REFRESH_WIDGET`、extra `com.termux.widget.EXTRA_FILE_CLICKED`；Float action `com.termux.window.ACTION_STOP_SERVICE` / `ACTION_SHOW` / `ACTION_HIDE`。

## `~/.termux` 目录

`~/.termux`（`TERMUX_DATA_HOME_DIR_PATH`）下一共 6 个条目：

```text
~/.termux/
├── termux.properties          # 主 app 读取
├── termux.float.properties    # 主 app 只读保护，解析方为 Termux:Float
├── colors.properties          # 主 app 读取
├── font.ttf                   # 主 app 读取
├── boot/                      # 仅常量声明，消费方 Termux:Boot
└── tasker/                    # 仅常量声明，消费方 Termux:Tasker
```

### `termux.properties`

**路径**：primary `~/.termux/termux.properties`（`TermuxConstants.java`），secondary `~/.config/termux/termux.properties`。

**文件选择**（`SharedProperties.getPropertiesFileFromList`）：按列表顺序取**第一个 `FileType.REGULAR` 且 `canRead`** 的文件；**明确不跟随符号链接**（`FileUtils.getFileType(path, false)`）。非 REGULAR 或不可读仅告警并继续。两者皆无则全部使用代码内默认值。每次 `loadTermuxPropertiesFromDisk` 都重新搜索（`TermuxSharedProperties.java`），因为高优先级文件可能在运行后才被创建（issue #2836）。

**编码**：`properties.load(new InputStreamReader(in, StandardCharsets.UTF_8))`（`SharedProperties.java`）。与 `colors.properties` 的 Latin-1 形成对照（见 5.3）。

**语法**：标准 `.properties`。键值对，`#` 注释。

**键白名单**：`TERMUX_APP_PROPERTIES_LIST`（`TermuxPropertyConstants.java`）定义可加载键，文件中出现但不在集合内的键既不进 `mMap` 也不进 `mProperties`（`SharedProperties.loadPropertiesFromDisk` 只遍历 `propertiesList`）。**未知键被静默忽略**，这是 properties 体系的安全边界——外部应用经 ContentProvider 也无法注入新语义。

**键全集与默认值**：

| 类型 | 键 | 取值 | 默认 |
| --- | --- | --- | --- |
| boolean（默认 false） | `disable-file-share-receiver` | true/false | false |
| | `disable-file-view-receiver` | true/false | false |
| | `disable-hardware-keyboard-shortcuts` | true/false | false |
| | `disable-terminal-session-change-toast` | true/false | false |
| | `enforce-char-based-input` | true/false | false |
| | `hide-soft-keyboard-on-startup` | true/false | false |
| | `terminal-onclick-url-open` | true/false | false |
| | `ctrl-space-workaround` | true/false | false |
| | `fullscreen` | true/false | false |
| | `use-fullscreen-workaround` | true/false | false |
| | `allow-external-apps` | true/false | false |
| boolean（默认 true） | `extra-keys-text-all-caps` | true/false | true |
| | `run-termux-am-socket-server` | true/false | true |
| int | `bell-character` | `vibrate` / `beep` / `ignore` | `vibrate` |
| | `delete-tmpdir-files-older-than-x-days-on-exit` | −1…100000 | 3 |
| | `terminal-cursor-blink-rate` | 视 `TerminalView` min/max | 0 |
| | `terminal-cursor-style` | `block` / `underline` / `bar` | 引擎默认 |
| | `terminal-margin-horizontal` | 0…100（dp） | 3 |
| | `terminal-margin-vertical` | 0…100（dp） | 0 |
| | `terminal-transcript-rows` | 视 `TerminalEmulator` min/max | 引擎默认 |
| float | `terminal-toolbar-height` | 0.4…3 | 1 |
| Integer（可空） | `shortcut.create-session` | 1 | — |
| | `shortcut.next-session` | 2 | — |
| | `shortcut.previous-session` | 3 | — |
| | `shortcut.rename-session` | 4 | — |
| String | `back-key` | `back` / `escape` | `back` |
| | `default-working-directory` | 任意路径 | `$HOME` |
| | `extra-keys` | JSON 布局串 | 2 行 7 列 |
| | `extra-keys-style` | 样式名 | `default` |
| | `night-mode` | `true` / `false` / `system` | `system` |
| | `soft-keyboard-toggle-behaviour` | `show/hide` / `enable/disable` | `show/hide` |
| | `volume-keys` | `virtual` / `volume` | `virtual` |

`extra-keys` 默认值：

```text
[['ESC',{key: 'DRAWER', popup: 'PASTE'},'SCROLL','HOME','UP','END','PGUP'], ['TAB','CTRL','ALT','LEFT','DOWN','RIGHT','PGDN']]
```

即上行 `ESC`、`DRAWER`（弹 `PASTE`）、`SCROLL`、`HOME`、`UP`、`END`、`PGUP`；下行 `TAB`、`CTRL`、`ALT`、`LEFT`、`DOWN`、`RIGHT`、`PGDN`。本项目 `DESIGN.md` 的「2 行 7 列、布局与 Termux 完全相同」即出自此处。

`shortcut.*` 取 Android 键码（`getCodePointForSessionShortcuts`），未设置时为 `null`（`Integer` 装箱而非 int 拆箱）。

**类型转换与错误处理**（`TermuxSharedProperties.getInternalTermuxPropertyValueFromValue`）：

- `MAP_*` 双向映射（`ImmutableBiMap`）负责枚举类。值不在映射内 → 用内部默认值，不报错。
- boolean 一律经 `MAP_GENERIC_BOOLEAN`（`true`/`false` 大小写不敏感）；非 `true` 的值一律取默认值，`logErrorOnInvalidValue=true` 时记录 error 日志（`SharedProperties.getBooleanValueForStringValue`）。
- 数值越界/非数字由各 `getXxxInternalPropertyValueFromValue` 钳制或回落默认，不崩溃。
- `use-black-ui`（已废弃）由 `preProcessPropertiesOnReadFromDisk` 钩子移除并改写为 `night-mode`（`replaceUseBlackUIProperty`）：仅当 `night-mode` 未设置时才迁移，迁移后记 warn 日志。
- **只读**：类注释明示「当前仅有读支持，写支持后续再加」（`SharedProperties.java`）。

**触发时机**：`TermuxActivity.onCreate` 调 `reloadProperties`；`TermuxApplication.onCreate` 调 `TermuxAppSharedProperties.init(context)`。

### `termux.float.properties`

primary `~/.termux/termux.float.properties`，secondary `~/.config/termux/termux.float.properties`，列表。

**主 app 不解析它**。唯一生产引用是 `TermuxOpenReceiver.ContentProvider.openFile`把它与 `TERMUX_PROPERTIES_FILE_PATHS_LIST` 一起强制为只读模式。`settings/properties/` 下只有 `TermuxAppSharedProperties`、`TermuxPropertyConstants`、`TermuxSharedProperties` 三个类，无 Float 解析器——解析由 Termux:Float（`com.termux.window`）自行完成。

设计意图：两份文件承载同类安全语义（`allow-external-apps` 一类的开关），因此共享同一套「ContentProvider 不可写」保护。

### `colors.properties`

**路径** `~/.termux/colors.properties`（`TermuxConstants.java`）。

**读取链**：

```text
TermuxActivity.onCreate 
  └─ setTermuxTerminalViewAndClients 
       └─ TermuxTerminalSessionActivityClient.onCreate 
            └─ checkForFontAndColors 
                 ├─ props.load(new FileInputStream(colorsFile))   
                 └─ TerminalColors.COLOR_SCHEME.updateWith(props)  
```

**第二个触发点**：收到 `com.termux.app.reload_style` 广播（`TermuxConstants.java`）→ `TermuxActivity` 的 `onReceive` 的 `ACTION_RELOAD_STYLE` 分支→ `reloadActivityStyling`→ `onReloadActivityStyling`（`TermuxTerminalSessionActivityClient.java`）→ 再次 `checkForFontAndColors`。若 extra `EXTRA_RECREATE_ACTIVITY` 为 true 则再 `recreate` 走一遍 onCreate。广播发送方 `TermuxActivity.updateTermuxActivityStyling`，当前仓库内由 `TermuxService` 新建会话时调用，Termux:Styling 应用亦可从外部发同 action。

**编码**：`props.load(InputStream)`，**不带编码参数**，按 JDK 语义等价 ISO-8859-1；而 `termux.properties` 用 UTF-8 Reader。颜色值本身全为 ASCII，实际只影响 value 中的 0x80-0xFF 字节与注释。这是上游的一处不一致，复刻时值得注意。

**合法 key**（`TerminalColorScheme.updateWith`）只有四类：

| key | 目标索引 | 常量 |
| --- | --- | --- |
| `foreground` | 256 | `TextStyle.COLOR_INDEX_FOREGROUND` |
| `background` | 257 | `TextStyle.COLOR_INDEX_BACKGROUND` |
| `cursor` | 258 | `TextStyle.COLOR_INDEX_CURSOR` |
| `color<N>` | N（0-255） | 解析 `key.substring(5)` |

`TextStyle.NUM_INDEXED_COLORS = 259`，因此 `color255` 是上限。`color` 单独出现会走 `Integer.parseInt("")` 抛 `NumberFormatException`。

**失败行为**：任何非法 key 抛 `IllegalArgumentException("Invalid property: '<key>'")`；颜色值解析失败抛 `IllegalArgumentException("Property '<key>' has invalid color: '<value>'")`。**不忽略、不跳过该行**。异常一路传播到 `checkForFontAndColors` 的 `catch (Exception e)`，只记 `logStackTraceWithMessage`，不崩溃。由于 `updateWith` 入口即 `reset`再逐项写入，中途抛异常意味着**后续条目全部丢失**：一份含坏 key 的 `colors.properties` 会让配置静默地部分生效，同时 `font.ttf` 也不会被设置（同一 try 块内之后）。这是本契约最需要留意的失败模式。

**颜色语法**（`TerminalColors.parse`）：

- `#` 前缀：`#RGB`、`#RRGGBB`、`#RRRGGGBBB`、`#RRRRGGGGBBBB`，即每通道 1-4 个十六进制位，`skipInitial=1`、`skipBetween=0`。
- `rgb:` 前缀：`rgb:<r>/<g>/<b>`，每通道 `h | hh | hhh | hhhh`，`skipInitial=4`、`skipBetween=1`。
- 其余前缀（含 `RGB:`、`rgb` 后带空格、空串）直接返回 0。
- `charsForColors % 3 != 0` 返回 0（三通道位数不相等或总长度不符）。
- `NumberFormatException` / `IndexOutOfBoundsException` 返回 0（非十六进制字符、通道超 4 位、substring 越界，如 `rgb:ff/gg`）。
- 期望值按 `mult = 255 / (2^(componentLength*4) - 1)` 缩放到 0-255，返回 `0xFF<<24 | r<<16 | g<<8 | b`。

**cursor 缺省推导**：props 中无 `cursor` 键时调 `setCursorColorForBackground`：按感知亮度 `sqrt(r²·0.241 + g²·0.691 + b²·0.068)`，亮度 <130 用白光标、≥130 用黑光标；亮度为 0 时保持默认。

**副作用**：`updateWith` 后对当前 session 执行 `session.getEmulator.mColors.reset`，再 `updateBackgroundColor`把 decorView 背景设为新的 `COLOR_INDEX_BACKGROUND`，最后 `setTypeface`。

**写入方**：Termux:Styling 应用（`com.termux.styling`）的调色器写此文件，再发 `ACTION_RELOAD_STYLE` 广播让主 app 重载——这是一个「外部写文件 + 通知重载」的松耦合设计。

### `font.ttf`

**路径** `~/.termux/font.ttf`（`TermuxConstants.java`）。

**判定与回退**（`TermuxTerminalSessionActivityClient.java`）：

```java
final Typeface newTypeface = (fontFile.exists && fontFile.length > 0)
    ? Typeface.createFromFile(fontFile) : Typeface.MONOSPACE;
```

即：文件不存在**或长度为 0 字节**都回落 `Typeface.MONOSPACE`。长度检查避免空文件导致 `createFromFile` 抛异常。

**读取时机与 `colors.properties` 完全相同**（同处 `checkForFontAndColors`，同受 `ACTION_RELOAD_STYLE` 触发，同受同一个 try/catch 保护）。

**格式**：路径常量只声明 `.ttf`，但 `Typeface.createFromFile` 依内容识别，`.ttc`/`.otf` 亦可。本项目 `DESIGN.md` 把这一点显式化为 `font.ttf` / `font.ttc` / `font.otf` 三选一（`FontUtils.termuxDefaultFontFile`）。

**生效路径**：`setTypeface` → `TerminalView.setTypeface`（`terminal-view/.../TerminalView.java`）重建 `TerminalRenderer` 并 `updateSize` + `invalidate`。

**上游的 `fonts` 目录**：termux-app **没有** `~/.termux/fonts` 目录概念，仅单文件 `font.ttf`。`~/.termux/fonts` 是本项目 `DESIGN.md` 的扩展要求（若目录存在则加入字体扫描路径并在字体列表展示，且软件自身不得创建它）。

### `boot/`

**路径** `~/.termux/boot`（`TermuxConstants.java`）。

**主 app 完全不使用**。`TERMUX_BOOT_SCRIPTS_DIR` 在全仓（含 `termux-shared`）只有这两处常量定义，无任何生产代码读取；常量注释明示其用途是「scripts to be run at boot by **Termux:Boot**」，即独立插件 `com.termux.boot`。

主 app 自身的 `SystemEventReceiver` 对 `BOOT_COMPLETED` 的处理与该目录无关：它只调用 `TermuxShellManager` 的 `resetAppShellNumberSinceBoot` / `resetTerminalSessionNumberSinceBoot`（`TermuxShellManager.java`），重置「自本次启动以来」的 shell/会话计数偏好，以保证 `*_SINCE_BOOT` 环境变量正确；`termux.env` 的写入只发生在包增删改分支（`SystemEventReceiver.java`）。

**契约语义**（由 Termux:Boot 侧定义，termux-app 只提供路径常量）：设备启动后由 Termux:Boot 枚举该目录下的脚本并以 shell 执行。本项目 `PROHIBITED.md` 已禁止插件体系，故此项不适用。

### `tasker/`

**路径** `~/.termux/tasker`（`TermuxConstants.java`）。

同样**主 app 无使用点**。注释说明供 Tasker 宿主应用经 Termux:Tasker（`com.termux.tasker`）插件客户端调用。主 app 内唯一的关联代码是 `SettingsActivity` 检测该插件是否安装以决定是否显示对应设置项（`SettingsActivity.java`）。本项目已禁止，不适用。

### secondary 配置目录 `~/.config/termux/`

严格说不属于 `~/.termux`，但与其构成同一契约：`termux.properties` 与 `termux.float.properties` 各有 primary/secondary 两个路径，primary 优先、缺失或不可读（或为符号链接）才回落 secondary。此项即 XDG 惯例的落地，复刻时应保留两级回落语义而非只读 `~/.termux`。

## 与本项目规范的对照

### 已对齐（`DESIGN.md` 依 Termux 定规且已实现）

| Termux 契约 | 本项目落点 |
| --- | --- |
| `~/.termux/colors.properties` + `ACTION_RELOAD_STYLE` 重载 | 主题改为构建期产出 + `ThemeRepository`，不读用户 colors 文件（`DESIGN.md` 规定主题为硬编码清单） |
| `~/.termux/font.ttf` 存在即设为主字体 | `FontUtils.termuxDefaultFontFile`（`font.ttf`/`font.ttc`/`font.otf` 按序取第一个），`TerminalViewModel` 与 `Bridge` 装配 |
| `~/.termux/fonts/` | 本项目扩展：`FontUtils.termuxFontDir`，存在则并入字体扫描路径 |
| 不设置 `LD_LIBRARY_PATH`（Android 7+）、failsafe 保留宿主 `PATH`/`TMPDIR` | `DESIGN.md` 逐条对应 |
| bootstrap 环境变量集合（`HOME`/`PREFIX`/`TMPDIR`/`LANG`/`COLORTERM`/`TERM`/`TERMUX_VERSION`/`PATH`） | `DESIGN.md` |
| `com.termux` applicationId、`com.termux.app.failsafe_session` extra | `MainActivity.EXTRA_FAILSAFE_SESSION`（保留原名以便既有快捷方式/Tasker 任务继续工作） |
| `com.termux.documents` authority | `TerminalDocumentsProvider.AUTHORITY` |
| 回滚 2000 行、`TERM=xterm-256color`、`COLORTERM=truecolor` | `DESIGN.md` + openspec/specs/shell-env |

### 不适用（`PROHIBITED.md` 已禁止）

- `termux.env` 文件及相关逻辑（对应上游 1.9 节末段）。
- 插件体系全部：`com.termux.api`、`com.termux.boot`、`com.termux.window`、`com.termux.styling`、`com.termux.tasker`、`com.termux.widget`，以及 `~/.termux/boot/`、`~/.termux/tasker/`、`~/.shortcuts/`、`termux.float.properties`。
- `termux.properties` 及其设置项体系（本项目设置已另行定规，故 5.1 整节作为「上游契约记录」而非「待复刻清单」）。本文补齐了键级别的完整清单与错误处理语义。
- `RunCommandService` / `com.termux.permission.RUN_COMMAND` / `allow-external-apps` / `com.termux.files` ContentProvider（外部命令执行面）。
- `am.sock` 与 `run-termux-am-socket-server`（内建命令均不提供）。
- `~/storage` 符号链接组、`~/crash_log.md` 轮转（`DESIGN.md` 不得创建 `~/storage`）。

### 值得留意的设计点

1. **properties 的键白名单是安全边界，不只是便利**：未知键被丢弃，使外部应用即便拿到文件写权限也无法引入新语义。本项目若不实现 properties 体系，`DESIGN.md` 的「只允许设置下列环境变量」即等价约束。
2. **`colors.properties` 的「一个坏条目毁掉整份配置」**：`updateWith` 先 `reset` 再逐项写，中途抛异常导致后续条目丢失且不提示用户。本项目若将来支持用户颜色文件，应在解析阶段做全量校验后再原子提交。
3. **两份 properties 文件编码不一致**（`termux.properties` UTF-8 vs `colors.properties` Latin-1）：上游的实现细节差异，复刻时应统一为 UTF-8。
4. **符号链接一律不跟随**：文件选择（`getPropertiesFileFromList`）与可执行文件校验（`FileUtils.getFileType(..., false)`）都显式关闭符号链接跟随，并各自注明安全理由。本项目 bootstrap 与文档提供者可对照自查。
5. **am.sock 的 root 例外**：接受 uid 0 的连接但以 termux 用户身份执行，兼顾 root 场景与最小权限。此模式对本项目无对应面，但「peer credential 鉴权 + 权限降级」是本地 socket 服务的标准做法。
6. **逗号替代字符 `‚`（U+201A）**：为迁就 Tasker/Locale 类插件宿主的参数解析而设计的转义层，是历史上「插件生态约束渗入核心协议」的典型样本。本项目无插件面，不需要。

## 结论

- termux-app 的对外契约共 10 类（Manifest 暴露面、权限双门禁、RUN_COMMAND Intent、结果回传、TERMUX_SERVICE Intent、广播、am.sock 协议、文件共享/文档提供者、环境变量三层、bootstrap zip 与崩溃日志），全部常量集中在 `TermuxConstants` 与 `TermuxPropertyConstants` 两个文件，第三方插件可原样 import 而非复制。
- `~/.termux` 下 6 个条目中，主 app 实际读取 4 个：`termux.properties`（键白名单 + 两级回落 + 只读）、`colors.properties`（4 类 key + 2 种颜色语法 + 异常部分生效）、`font.ttf`（exists && length>0 否则 MONOSPACE）、`termux.float.properties`（仅强制只读）。`boot/` 与 `tasker/` 是给独立插桩的路径声明。
- 对本项目：已对齐项集中在字体与 bootstrap 环境变量；禁止项覆盖其余全部。两份「设计陷阱」（colors 的部分生效、编码不一致）记录为将来若放开用户配置文件时的自查点。
