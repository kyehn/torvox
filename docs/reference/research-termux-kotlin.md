# 深度研究：termux-kotlin（termux-kotlin-app，reapercanuk39/termux-kotlin-app）

> 研究日期：2026-08-06 | 克隆位置：`repositories/refs/termux-kotlin`（depth 1，`main`）
> 参考索引：本项目 `docs/reference-projects.md`；姊妹文档 `docs/reference/research-termux-app.md`（Java 上游 v0.119.0-beta.3）
> 语言：100% Kotlin（官方 termux-app 的 Kotlin 移植），342 个 Kotlin 文件 + fork 特有 Python agents/（遗留）与 Kotlin 代理框架
> 许可证：GPLv3（fork）

**一句话定位**：这是 termux-app（Java）的**全量 Kotlin 重写**，包名保持 `com.termux`（与官方包 100% 兼容，bootstrap 直接用官方产物），在此之上叠加了大量 fork 特有功能（内置 Termux:API/Boot/Widget/Styling 插件、Kotlin 原生 Agent Daemon、MCP 风格的工具框架、包管理器路径兼容层）。对 torvox 而言，它的价值不在 UI（termux 仍是经典 View 体系 + 少量 Compose），而在**进程/会话/shell 命令管理的工程范式**：`ExecutionCommand` 状态机、`AppShell`/`TermuxSession` 双 runner、`AmSocketServer`（AF_UNIX 服务端）、`ArgumentTokenizer`、`StreamGobbler`、分层 `Logger`、`BellHandler`、隐藏 API 绕过（`ReflectionUtils`）。

---

## 1. 项目定位与来源

| 项 | 内容 |
|---|---|
| 上游 | [termux/termux-app](https://github.com/termux/termux-app)（Java，v0.119.0-beta.3 同源） |
| fork | reapercanuk39/termux-kotlin-app，**100% Kotlin 转换**，包名仍为 `com.termux` |
| 兼容策略 | 与官方 Termux 同包名不同签名 → **不能共存安装**，但所有官方包（`pkg install python` 等）免改路径直接工作；借鉴 ZeroTermux 策略 |
| APK 规模 | arm64-v8a ≈ 35 MB（bootstrap 30 MB 内含 66 个自编译包，原生路径已编进 ELF） |
| 现代化 | Hilt DI、Compose 设置界面（部分）、Kotlin coroutines、`agents/` 代理框架（v2.0.5+ 纯 Kotlin，45+ capabilities，4 个 skill：pkg/fs/git/diagnostic，swarm 协调） |
| 内置插件 | Termux:Boot / Termux:Styling / Termux:Widget / Termux:API 全部内置（不再需要插件 APK） |

README 关键声明（`README.md:37-55`）：完整 Kotlin 转换、保持与上游 100% 兼容；v2.0.0+ 使用 `com.termux` 包名换取官方包零修改兼容。`README.md:64-75` 描述 Kotlin-Native Agent Daemon（45+ capabilities、4 个纯 Kotlin skills、stigmergy 多代理 swarm、Python 回退）。

**研究重点声明**：本研究聚焦（a）termux-shared 的 shell 命令 runner 体系（TermuxSession/AppShell/ExecutionCommand/ResultSender）、（b）AmSocketServer 及其 AF_UNIX socket 基础设施、（c）ArgumentTokenizer/StreamGobbler/Logger/ReflectionUtils/BellHandler/TerminalExtraKeys/TermuxTerminalViewClientBase 等可移植工具、（d）bootstrap 安装与路径兼容层（与 torvox 的 BootstrapInstaller 直接对标）、（e）fork 特有 agent/MCP 工具框架（与 torvox mcp.rs 对标）。

---

## 2. 完整架构

### 2.1 模块划分（`settings.gradle:18-19`）

```
rootProject.name = "termux-kotlin-app"
include ':app', ':termux-shared', ':terminal-emulator', ':terminal-view'
```

| 模块 | 包根 | 职责 | 与 torvox 对应 |
|---|---|---|---|
| `app` | `com.termux.app` | Activity/Service/安装/bootstrap/代理框架/插件内置 | `android/app`（Kotlin + Compose） |
| `termux-shared` | `com.termux.shared` | 共享工具：shell 命令体系、AF_UNIX socket、logger、reflection、属性/偏好、crash、文件系统、Termux 常量与 shell 环境 | 分散在 torvox 的 Kotlin 端 + Rust 端 |
| `terminal-emulator` | `com.termux.terminal` | **纯 Kotlin VT 解析器**（TerminalEmulator/TerminalBuffer/TerminalRow/WcWidth）+ PTY 会话（TerminalSession，JNI 到 C `jni/termux.c`） | `native/src/terminal/`（libghostty-vt + pty.rs + session.rs） |
| `terminal-view` | `com.termux.view` | **View 体系渲染**（TerminalView 自绘 Canvas）+ 手势 + 文本选择 | `native/src/render/`（wgpu）+ `ui/TerminalSurface.kt` |

### 2.2 架构数据流

```
TermuxActivity ── TerminalView ── TerminalSession（terminal-emulator）
                     │  ▲           │  ▲
                     │  │           │  │  JNI.createSubprocess / setPtyWindowSize / waitFor（jni/termux.c）
                     ▼  │           ▼  │
              TerminalViewClient  TerminalSessionClient（TermuxTerminalSession*Client）
                     │
        TermuxService（前台服务，持有 TermuxShellManager）
           ├── mTermuxSessions: MutableList<TermuxSession>（Runner.TERMINAL_SESSION）
           ├── mTermuxTasks: MutableList<AppShell>（Runner.APP_SHELL，Runtime.exec）
           └── mPendingPluginExecutionCommands: MutableList<ExecutionCommand>
                     ▲
        RunCommandService / AgentService / TermuxAmSocketServer（AF_UNIX ← $PREFIX/bin/termux-am-socket 客户端）
```

shell 命令的统一模型：**一切可执行操作（插件调用、RUN_COMMAND intent、am socket、agent 工具）都先构建 `ExecutionCommand`，由 `TermuxService` 按 `Runner`（TERMINAL_SESSION 前台 / APP_SHELL 后台）分派**，结果经 `ResultSender` 用 PendingIntent 或结果目录回传。这是 termux 生态最值得借鉴的"命令总线"设计。

### 2.3 代码规模（重点目录）

- `termux-shared` ≈ 130 文件（工具为主，`FileUtils.kt` 52 KB、`TermuxConstants.kt` 29 KB）
- `terminal-emulator` 14 文件（`TerminalEmulator.kt` 85 KB、`WcWidth.kt` 44 KB）
- `terminal-view` 10 文件（`TerminalView.kt` 48 KB）
- `app` ≈ 190 文件，其中 fork 特有 `agents/` ≈ 55 文件（AgentDaemon/MCP/ToolRegistry/Swarm/Ollama 等）、`boot/` 4、`widget/` 6、`styling/` 5、`pkg/`（backup/cli/doctor）、`ui/compose/`（commandpalette/settings/theme）

---

## 3. 核心子系统深度详解（重点文件，函数/类级）

### 3.1 shell 命令执行体系（termux-shared/shell）

#### 3.1.1 ExecutionCommand —— 命令的"唯一事实源"
`termux-shared/.../shell/command/ExecutionCommand.kt`
- `class ExecutionCommand`（:13）—— 一个可执行操作的完整描述：`mPid`(:98)、`executable`(:106)、`executableUri`(:108)、`arguments`(:110)、`stdin`(:112)、`workingDirectory`(:114)、`runner`(:120，见下)、`sessionAction`/`shellName`/`shellCreateMode`(:369-371 附近)、`commandLabel`/`commandDescription`/`commandHelp`/`pluginAPIHelp`、`resultConfig`（ResultConfig，:376-384 由 Intent 填充）、`isPluginExecutionCommand`、`isFailsafe`。
- `enum ExecutionState`（:27）—— 五态状态机：`PRE_EXECUTION(0) → EXECUTING(1) → EXECUTED(2) → SUCCESS(3)/FAILED(4)`。**关键语义**（:19-24 注释）：shell 命令 exitCode 非零 ≠ 命令失败；只有 `errCode`（Termux 内部错误）非零才算失败。`setState`/`setStateFailed` 实现幂等推进（已 FAILED 后不可再 EXECUTED，TermuxSession.finish() :54-56 依赖此）。
- `enum Runner`（:37）—— `TERMINAL_SESSION("terminal-session")` / `APP_SHELL("app-shell")`，`runnerOf()` 解析。
- `enum ShellCreateMode`（:70）—— `ALWAYS` / `NO_SHELL_WITH_NAME`（复用同名 shell 会话）。

#### 3.1.2 AppShell —— 后台命令 runner（Runtime.exec）
`termux-shared/.../shell/command/runner/app/AppShell.kt`
- `class AppShell private constructor`（:32）—— 包一个 `Process` + `ExecutionCommand`。
- `executeInner()`（:48）—— 核心流程：
  1. `ShellUtils.getPid(mProcess)`（反射取 `Process.pid` 字段）写回 `mExecutionCommand.mPid`（:49）；
  2. 创建 `DataOutputStream(STDIN)` + **两个 `StreamGobbler`**（stdout/stderr，:56-58），启动（:61-62）；
  3. `stdin` 非空则写入并 flush/close（:64-85），**EPIPE/Stream closed 特判**（:70-74，"The command is not a shell, the shell closed STDIN…" —— 注释值得抄）;
  4. `mProcess.waitFor()`（:88）→ join gobblers → `mProcess.destroy()`（:100-102）；
  5. 写回 `resultData.exitCode`，`setState(EXECUTED)`（:116-118）→ `processAppShellResult()`。
- `companion.execute()`（工厂，:130+）—— 后台线程执行 `executeInner`，完成后经 `AppShellClient.onAppShellExited`（:325-333）回调；无 client 且未失败则直接置 SUCCESS（:329-331）。

#### 3.1.3 TermuxSession —— 前台命令 runner（TerminalSession 包装）★重点
`termux-shared/.../termux/shell/command/runner/terminal/TermuxSession.kt`
- `class TermuxSession private constructor`（:27）—— 持有 `mTerminalSession`（com.termux.terminal.TerminalSession）、`mExecutionCommand`、`mTermuxSessionClient`、`mSetStdoutOnExit`。
- `finish()`（:42）—— 由 `TerminalSessionClient.onSessionFinished` 触发：
  1. `if (mTerminalSession.isRunning) return`（:44，防重复）；
  2. 若命令已 FAILED 则忽略结果（:54-57，"SIGKILL was sent"场景）；
  3. `resultData.exitCode = exitStatus`（:59）；`mSetStdoutOnExit` 时把**终端 transcript 全文**（`ShellUtils.getTerminalSessionTranscriptText(session, true, false)`，:62）追加为 stdout —— 这是"前台会话结果=屏幕内容"的关键设计，torvox 的 MCP `terminal_info` 只给元数据，没有 transcript 回传；
  4. `setState(EXECUTED)`（:64）→ `processTermuxSessionResult()`（:67）。
- `killIfExecuting()`（:78）—— 置 FAILED（`Errno.ERRNO_FAILED`，:86）、exitCode=137（SIGKILL，:88）、收集当前 transcript、`mTerminalSession.finishIfRunning()`（:99，实际 `Os.kill(pid, SIGKILL)`）。
- `interface TermuxSessionClient`（:108）—— `onTermuxSessionExited(termuxSession)`；companion `processTermuxSessionResult`（:117+）负责向 `ResultSender` 回传（无 client 时直接置 SUCCESS，:288-290）。

#### 3.1.4 ResultSender —— 结果回传通道
`termux-shared/.../shell/command/result/ResultSender.kt`
- `sendCommandResultData()`（:39）—— 双通道：`resultPendingIntent`（PendingIntent）与/或 `resultDirectoryPath`（写文件）。
- `sendCommandResultDataWithPendingIntent()`（:69）—— **Binder 事务上限保护**：stdout/stderr 截断到 `DataUtils.TRANSACTION_SIZE_LIMIT_IN_BYTES`（:87-105，双流各半，errmsg 1/4 且从尾部裁以保留栈顶，:115-120）；`resultBundleKey` 用 Bundle 承载。torvox 的 MCP 响应无此限制（走 JNI 字符串），但若未来走 PendingIntent 回传（如分享），此截断逻辑可直接借鉴。
- `sendCommandResultDataToDirectory()`（:300+）—— 原子写：先写 `.tmp` 再 `FileUtils.moveRegularFile` 改名（:333-339），支持 stdout/stderr/err 三文件 + 单文件模式（`resultSingleFile`）。

#### 3.1.5 shell 环境分层
- `IShellEnvironment`（`shell/command/environment/IShellEnvironment.kt:9`）—— 接口：`getEnvironment(context, isFailSafe)`、`setupShellCommandArguments`、`setupShellCommandEnvironment`。
- `UnixShellEnvironment`（:13）—— 抽象基类，定义 `ENV_COLORTERM/HOME/LANG/LD_LIBRARY_PATH/PATH/PREFIX/TMPDIR/SHELL/TERM` 等常量（:17-48）；`LOGIN_SHELL_BINARIES = [login, bash, zsh, fish, sh]`（:51）。
- `AndroidShellEnvironment`（`environment/AndroidShellEnvironment.kt:16`）—— Android 层（/system/bin 等）。
- `TermuxShellEnvironment`（`termux/.../environment/TermuxShellEnvironment.kt:17`）—— **fork 增强**：`getEnvironment()`（:24）在 Android 环境之上叠加 TermuxApp/TermuxAPI 环境；`!isFailSafe` 时注入（:39-70）：`PATH=$PREFIX/bin`、`LD_LIBRARY_PATH=$PREFIX/lib`（RUNPATH 被硬编码为 /data/data/com.termux，必须覆盖，:46-52 注释）、**`DPKG_ADMINDIR`/`DPKG_DATADIR`（dpkg 路径覆盖）、`TERMINFO`、`SSL_CERT_FILE`/`CURL_CA_BUNDLE`**（:59-69）—— 这 5 个是 fork 为了让官方包在 com.termux 路径下工作的关键补丁，**torvox 目前没有 dpkg 路径覆盖，值得吸收**。
- `TermuxAppShellEnvironment`（`termux/.../environment/TermuxAppShellEnvironment.kt:17`）—— 注入 `TERMUX_APP__*` 系列（VERSION_NAME/VERSION_CODE/PACKAGE_NAME/PID/UID/TARGET_SDK/APK_PATH/SE_PROCESS_CONTEXT/...，:31-70）；`setTermuxAppEnvironment()`（:82）带 `@Synchronized` 与"只算一次"缓存；`updateTermuxAppAMSocketServerEnabled()`（:156）在 am socket 启停后更新 `TERMUX_APP__AM_SOCKET_SERVER_ENABLED`。
- `TermuxShellEnvironment.writeEnvironmentToFile()`（:118-140）—— 把环境转成 dotenv 写入 `$PREFIX/etc/termux/termux.env`（先写 .tmp 再 move，:133-137）。**torvox 的 `isInstalled()` 就以 `etc/termux/termux.env` 存在为安装完成标志（BootstrapInstaller.kt:33-38），两者思路一致**。

#### 3.1.6 TermuxShellManager / TermuxShellUtils / ShellUtils
- `TermuxShellManager`（`termux/shell/TermuxShellManager.kt:10`）—— 单例：`mTermuxSessions`（:22）、`mTermuxTasks`（:26）、`mPendingPluginExecutionCommands`（:30）；`getNextShellId()`（:88 附近）、会话/任务编号自增（:95-115，溢出钳制到 MAX_VALUE）。
- `TermuxShellUtils.setupShellCommandArguments()`（`termux/shell/TermuxShellUtils.kt:22-67`）—— **shebang/ELF 嗅探**：读文件头 256 字节，ELF（0x7F 'E' 'L' 'F'）直接执行；`#!` 则解析 shebang，把 `/usr/bin/foo`、`/bin/foo` 重写为 `$PREFIX/bin/foo`（:40-57）；无 shebang 的脚本用 `$PREFIX/bin/sh` 解释（:23-27 注释）。torvox 的 postinst 执行（SecondStageRunner）走系统 linker + 显式解释器，未做前缀重写——此函数对 torvox 的"自定义脚本执行"场景有参考价值。
- `TermuxShellUtils.clearTermuxTMPDIR()`（:88-127）—— 按 `delete-tmpdir-files-older-than-x-days-on-exit` 属性清理 `$TMPDIR`（days<0 不清、0 全清、>0 按天删）。
- `ShellUtils`（`shell/ShellUtils.kt:7`）—— `getPid(Process)`（:13，反射）、`setupShellCommandArguments`（:31）、`getTerminalSessionTranscriptText(session, linesJoined, trim)`（:45-73，读 TerminalBuffer transcript）。

### 3.2 AmSocketServer 与本地 socket 族 ★重点

#### 3.2.1 AmSocketServer —— AF_UNIX am 命令服务器
`termux-shared/.../shell/am/AmSocketServer.kt`
- `object AmSocketServer`（:53）—— 类注释（:25-52）定义了协议：客户端向 socket 发送**不含 "am" 前缀的命令字符串**，服务端回 `exit_code\0stdout\0stderr\0`（`\0` 分隔）。参考 `termux/termux-am-socket`（原生 C 客户端）与 `termux-am-library` 的 `Am.java`。
- `start(context, localSocketRunConfig)`（:65）—— `@Synchronized` 创建 `LocalSocketManager` 并 `start()`。
- `processAmClient()`（:77）—— 每连接回调：`clientSocket.readDataOnInputStream(data, true)`（:82，读完整请求）→ `parseAmCommand` → `runAmCommand` → `sendResultToClient`。
- `sendResultToClient()`（:135）—— 组装 `exitCode + '\0' + stdout + '\0' + stderr`，`sendDataToOutputStream(result, true)`（:149，true=写完关流）。
- `sanitizeExitCode()`（:164）—— exitCode 超出 0-255 强制改 1（否则 shell 侧报 "Channel number out of range"）。
- `parseAmCommand()`（:186）—— 直接调 **`ArgumentTokenizer.tokenize()`**（:192）—— 这是 ArgumentTokenizer 在本仓库的唯一调用点。
- `runAmCommand()`（:213）—— 通过 `ByteArrayOutputStream`+`PrintStream` 捕获输出；`checkDisplayOverAppsPermission` 时校验 Android 10+ 的 `SYSTEM_ALERT_WINDOW`（:224-234）；实际执行委托 `com.termux.am.Am(...).run(amCommandArray)`（:236，进程内跑 am，无需起 dalvik）。
- `abstract class AmSocketServerClient : LocalSocketManagerClientBase`（:257）—— `onClientAccepted` 里调 `processAmClient`（:259-262）。
- 配套：`AmSocketServerRunConfig`（:12，`shouldCheckDisplayOverAppsPermission()` :28，默认 true :64）；`AmSocketServerErrno`（:5，`ERRNO_PARSE_AM_COMMAND_FAILED_WITH_EXCEPTION` code 100 :13、`ERRNO_RUN_AM_COMMAND_FAILED_WITH_EXCEPTION` code 101 :15）。

#### 3.2.2 LocalSocketManager 族（AF_UNIX 基础设施）
`termux-shared/.../net/socket/local/`
- `ILocalSocketManager`（:8）—— 回调接口：`onClientAccepted`/`onDisallowedClientConnected`/`onError`/`getLocalSocketManagerClientThreadUEH`。
- `LocalSocketManager`（`LocalSocketManager.kt:19`）—— `start()`（:43，首次 `System.loadLibrary("termux-shared")` 加载 JNI，:46-58）、`stop()`（:67）、回调全部**在新线程执行**（`startLocalSocketManagerClientThread` :104，避免阻塞 accept 循环）；JNI externals（:316-323）：`createServerSocket`/`createClientSocket`/`acceptServerSocket`/`readSocket`/`sendSocket`/`setSocketReceiveTimeoutNative`/`setSocketSendTimeoutNative`/`getPeerCredNative`（**SO_PEERCRED 取对端 uid/pid/gid**）。
- `LocalServerSocket`（`LocalServerSocket.kt:14`）—— `start()`（:30）：路径 ≤ 108 字节（UNIX_PATH_MAX，:44）预检、非 abstract 时校验父目录权限 `rwx`（`SERVER_SOCKET_PARENT_DIRECTORY_PERMISSIONS` :284）并**删除残留 socket 文件**（:71）、`createServerSocket` 取 fd、启动 `ClientSocketListener` 线程（:91-98）；`stop()`（:105）interrupt 监听线程 + close + 删文件。
- `LocalClientSocket`（`LocalClientSocket.kt:17`）—— 客户端包装：`readDataOnInputStream`（读满请求）/`sendDataToOutputStream`（写完关流）、`peerCred`（SO_PEERCRED）。
- `LocalSocketRunConfig`（`LocalSocketRunConfig.kt:12`）—— 路径（:42）、abstract namespace 检测（首字节 `\0`，:89-97）、`SO_RCVTIMEO`/`SO_SNDTIMEO`/deadline/backlog（:60-87）。
- `PeerCred`（`PeerCred.kt:12`）—— uid/pid/gid 模型，`getMinimalString()`。
- `LocalSocketErrno`（:5）—— 全套错误码（路径空/过长/非绝对、backlog 非法、fd 非法、库加载失败…）。

#### 3.2.3 TermuxAmSocketServer —— am socket 的 Termux 装配
`termux-shared/.../termux/shell/am/TermuxAmSocketServer.kt`
- `object TermuxAmSocketServer`（:55）—— 类注释（:21-54）：socket 文件在 `TermuxConstants.TERMUX_APP.TERMUX_AM_SOCKET_FILE_PATH`（= `$FILES_DIR/apps/termux-am/am.sock`，`TermuxConstants.kt:361`）；**允许本用户 + root 连接，root 发来的命令按 termux 用户权限执行**（:34-36）；`$PREFIX/bin/termux-am` 客户端通过 `termux-am-socket` 连接，比 `/system/bin/am` 快（无需每次起 dalvik，:38-41）；由 Application 启动、可用 `run-termux-am-socket-server=false` 属性关闭（:43-46）；状态导出为 `TERMUX_APP__AM_SOCKET_SERVER_ENABLED` 环境变量（:49）。
- `setupTermuxAmSocketServer()`（:76）—— 属性开启则 `start()`，并把 enabled 状态写进 shell 环境（:95-96）。
- `start()`（:104）—— stop 旧的 → 建 `AmSocketServerRunConfig(TITLE="TermuxAm", am.sock 路径, TermuxAmSocketServerClient())`。
- `TermuxAmSocketServerClient`（:200+）—— 覆写 `onError`/`onDisallowedClientConnected`：除日志外还发**插件错误通知**（TermuxPluginUtils），`onClientAccepted` 委托父类。

### 3.3 ArgumentTokenizer ★重点
`termux-shared/.../shell/ArgumentTokenizer.kt`（源自 DrJava，头部保留原版权块 :1-35）
- `object ArgumentTokenizer`（:50）—— 四态机：`NO_TOKEN_STATE`/`NORMAL_TOKEN_STATE`/`SINGLE_QUOTE_STATE`/`DOUBLE_QUOTE_STATE`（:51-54）。
- `tokenize(arguments)`（:62）/`tokenize(arguments, stringify)`（:73）—— 逐字符扫描：
  - 单引号内原样（:90-97）；双引号内 `\` 只转义 `"` 和 `\`（前瞻，:102-111）；
  - 普通态 `\` 置 escaped 标志、下一字符原样进当前 token（:118-120）；
  - 空白结束 token（:132-137）；末尾 `escaped` 残留则补 `\`（:148-150）；
  - `stringify=true` 时每个 token 用 `"..."` 包裹并转义（:157-161）。
- `escapeQuotesAndBackslashes()`（:171）—— 倒序遍历（索引安全）：`\`/`"` 前插 `\`，`\n \t \r \b \f` 转义（:178-204）。
- **语义要点**：这是"类 shell 但不完全 shell"的 tokenizer —— 不做变量展开、不处理 `$()`、`;`、管道；但引号/转义规则完整。**它把一个用户可读的命令字符串变成安全 argv 数组**，正是"把命令字符串交给 `exec` 风格执行器"前的正确前置（避免 `sh -c` 注入面）。

### 3.4 StreamGobbler ★重点
`termux-shared/.../shell/StreamGobbler.kt`（源自 Chainfire/libsuperuser）
- `class StreamGobbler : Thread`（:35）—— 构造函数注释（:82-94）点明动机：**STDOUT/STDERR 必须尽快读，否则 native 进程因管道缓冲满而阻塞、`waitFor()` 永不返回（死锁）**。
- `fun interface OnLineListener`（:40）—— 逐行回调，注释警告：回调里延迟会暂停 native 进程甚至死锁（:44-47）。
- `fun interface OnStreamClosedListener`（:56）。
- 构造 1（:96）—— `outputList` 模式：行写入 `MutableList<String>`（AppShell 用它）；构造 2（:120+）—— `stringBuilder` 模式 + 可选 `onLineListener`；`mLogLevel` 控制是否按行写 logcat。
- `run()`（:160+）—— 读 `BufferedReader` 逐行：写 list/stringBuilder/回调 + 按 `mLogLevel` 记 `Logger.logLine`（"Shell $shell: $line"）；EOF 触发 `onStreamClosed`（仅一次，`calledOnClose` volatile 防重，:79-80）。
- `incThreadCounter()`（:300）—— 线程名 `Gobbler#N`。

### 3.5 TerminalExtraKeys 与 ExtraKeysView ★重点
- `TerminalExtraKeys`（`termux/.../terminal/io/TerminalExtraKeys.kt:13`）—— `onExtraKeyButtonClick`（:15）：**宏展开** —— `buttonInfo.isMacro` 时按空格拆 `key` 串，逐个识别 `CTRL/ALT/SHIFT/FN` 修饰键并累积状态，其余 key 逐个派发（:16-36，修饰键是"粘滞"的，遇到普通键才重置）；`onTerminalExtraKeyButtonClick`（:42-73）：控制字符（如 ctrl+letter）合成 `KeyEvent(ACTION_DOWN/UP)` 走 `mTerminalView.onKeyDown`（:54-59），否则按 codePoint 走 `mTerminalView.inputCodePoint(KEY_EVENT_SOURCE_VIRTUAL_KEYBOARD, cp, ctrlDown, altDown)`（:63-65，N 以下 SDK 直接 `session.write(key)` :67-70）。
- `ExtraKeysView`（`termux/.../extrakeys/ExtraKeysView.kt:61`，GridLayout 子类）—— `IExtraKeysView` 接口（:67-95）；主题色常量（:99-114）；长按时长范围 200-3000ms（:117-119）；`reload(ExtraKeysInfo)`、按钮重复（`mRepetitiveKeys`）、Popup 子键盘、`readSpecialButton`（CTRL/ALT/FN 状态）。
- `TermuxTerminalExtraKeys`（`app/.../terminal/io/TermuxTerminalExtraKeys.kt:18`）—— 特殊键实现：`KEYBOARD`（软键盘开关，:88-90）、`DRAWER`（抽屉，:91-99）、`PASTE`（:101-103）、`SCROLL`（`toggleAutoScrollDisabled`，:104-107）；`setExtraKeys()`（:34）从 termux.properties 读 `extra-keys`/`extra-keys-style` JSON 并解析。
- `ExtraKeysInfo`（3.6 KB）/`ExtraKeysConstants`（5.3 KB）—— JSON 布局解析与 `PRIMARY_KEY_CODES_FOR_STRINGS` 映射；`SpecialButton`（CTRL/ALT/SHIFT/FN/ESC 等常量）。

### 3.6 TermuxTerminalViewClientBase / TermuxTerminalSessionClientBase ★重点
- `TermuxTerminalViewClientBase`（`termux/.../terminal/TermuxTerminalViewClientBase.kt:9`）—— `TerminalViewClient` 接口的**全空实现基类**（onScale :11 返回 1.0f、其余 no-op，log* 委托 `Logger`，:72-96）。**用途**：子类只覆写需要的回调，避免接口膨胀；这是 torvox `TerminalSurface`/`TerminalScreen` 回调参数过多的反面教材式解药。
- `TermuxTerminalSessionClientBase`（`termux/.../terminal/TermuxTerminalSessionClientBase.kt:7`）—— `TerminalSessionClient` 同理（log* 委托 Logger :40-66）。
- 接口本身：`TerminalViewClient`（`terminal-view/.../TerminalViewClient.kt:13`）—— onScale/onSingleTapUp/shouldBackButtonBeMappedToEscape/shouldEnforceCharBasedInput/shouldUseCtrlSpaceWorkaround/isTerminalViewSelected/copyModeChanged/onKeyDown/onKeyUp/onLongPress/readControlKey/readAltKey/onEmulatorSet/log*；`TerminalSessionClient`（`terminal-emulator/.../TerminalSessionClient.kt:8`）—— onTextChanged/onTitleChanged/onSessionFinished/onCopyTextToClipboard/onPasteTextFromClipboard/onBell/onColorsChanged/onTerminalCursorStateChange/setTerminalShellPid/getTerminalCursorStyle/log*。

### 3.7 BellHandler ★重点
`termux-shared/.../termux/terminal/io/BellHandler.kt`
- `class BellHandler private constructor(vibrator)`（:12）—— **防抖振动铃**：`doBell()`（:34）距上次 < `MIN_PAUSE`（=3×50ms，:63）则忽略；否则主线程 `Handler.postDelayed(bellRunnable, 150ms)`（:42-45 附近）执行 `VibrationEffect.createOneShot(DURATION=50ms, DEFAULT_AMPLITUDE)`（:21，O 以下用废弃 API :24）；catch 里注释三星 Android 8 的 `VibratorService$Vibration.mEffect` NPE 坑（:27-29）。
- `getInstance(context)`（:65）—— 双重检查锁单例（:67-75），持有 `applicationContext`。
- **torvox 对照**：native `session.rs` 有 `bel_triggered: Arc<AtomicBool>`（session.rs:146），事件队列出 `{"event":"bell"}`；Kotlin 侧播放铃（TerminalRuntime 处理）。termux 的防抖 + 主线程调度 + 三星 NPE 保护值得抄进 torvox 的 bell 处理。

### 3.8 Logger ★重点
`termux-shared/.../logger/Logger.kt`
- 级别常量（:18-24）：`LOG_LEVEL_OFF=0 / NORMAL=1 / DEBUG=2 / VERBOSE=3`，`CURRENT_LOG_LEVEL` 全局可调（:25）。
- **logcat 单条上限**：`LOGGER_ENTRY_MAX_PAYLOAD=4068`（:30）、`MAX_SAFE=4000`（:35）。
- `logMessage()`（:38）—— 按级别与当前阈值过滤（含 `Private` 变体：仅 DEBUG+ 才记错误 :93-110，用于敏感信息）。
- `logExtendedMessage()`（:49）—— **长消息自动分块**：`maxEntrySize = 4068 - 8(分块前缀"(xx/xx)") - tag长度 - 4("D/"后缀)`（:55）；优先在 `\n` 处切断（:62-65）；多块加 `(1/2)` 前缀（:75）。这是 logcat 4KB 限制的标准解法，torvox 的 `LogUtil` 是简化版，可升级。
- `getStackTracesStringArray`/`getStackTracesMarkdownString`（:150+）—— 异常栈转 markdown（crash 报告用）。
- `shouldEnableLoggingForCustomLogLevel`（:382）—— 命令级自定义日志级别的阈值判断。
- 配套：`Logger.logLine` 供 StreamGobbler 逐行记命令输出。

### 3.9 ReflectionUtils ★重点
`termux-shared/.../reflection/ReflectionUtils.kt`
- `object ReflectionUtils`（:11）—— `bypassHiddenAPIReflectionRestrictions()`（:23）：Android P+ 调 **LSPosed HiddenApiBypass.addHiddenApiExemptions("")** 绕过非 SDK 接口限制（:27，失败仅记日志）；`areHiddenAPIReflectionRestrictionsBypassed()`（:38）。
- `getDeclaredField`（:50）/`FieldInvokeResult`（:62）/`invokeField`（:81）—— 设 `isAccessible=true` 后取值，异常→日志+null/false（**永不抛**）。
- `getDeclaredMethod`（:95/:108）/`invokeMethod`/`invokeVoidMethod`（:119+）/`invokeConstructor`（:241）—— 同风格。
- 使用场景：`ShellUtils.getPid`（反射 `Process.pid`）、`TerminalSession.wrapFileDescriptor`（反射 `FileDescriptor.descriptor/fd` 私有字段，TerminalSession.kt:316-338）。torvox 目前只在 `TerminalDocumentsProvider`/ANR 监控等处小用反射；若要读 `Process.pid` 或私有字段，可整文件移植（含 HiddenApiBypass 依赖）。

### 3.10 terminal-emulator 模块（纯 Kotlin VT 解析 + PTY 会话）

- `TerminalSession`（`TerminalSession.kt:29`）—— 详见 §3.1.3 下游；关键实现：
  - 双 `ByteQueue(4096)`（:50/:54）：进程→终端（读线程写、主线程读）、终端→进程（主线程写、写线程读）；
  - `initializeEmulator()`（:107）：建 `TerminalEmulator` → `JNI.createSubprocess(shellPath, cwd, args, env, pid[], rows, cols, cellW, cellH)`（:111，C 代码在 `jni/termux.c`，fork+execvp+setsid+TIOCSCTTY）→ 起 3 线程：`TermSessionInputReader`（:117，读 pty→队列+`MSG_NEW_INPUT`）、`TermSessionOutputWriter`（:135，队列→pty）、`TermSessionWaiter`（:152，`JNI.waitFor(pid)`→`MSG_PROCESS_EXITED`）；
  - 主线程 `MainThreadHandler`（:279）：读队列→`mEmulator.append`→`notifyScreenUpdate`；进程退出时**追加 `\r\n[Process completed (code N) - press Enter]` 到屏幕**（:294-300）→ `onSessionFinished`；
  - `writeCodePoint`（:166）手写 UTF-8 编码（5 字节缓冲，:57）；`finishIfRunning`（:209，`Os.kill(SIGKILL)`）；`getCwd()`（:262，读 `/proc/$pid/cwd/` 符号链接）；`wrapFileDescriptor`（:316，反射私有字段，桌面 JDK 兼容分支 :321-324）。
- `TerminalEmulator`（`TerminalEmulator.kt`，85 KB，VT100/xterm 状态机）—— 关键入口：`append(bytes,len)`（:374）→ `processByte`（:380）→ `processCodePoint`（:434）；CSI 处理 `doCsiQuestionMark`（:778）/`doDecSetOrReset`（:846）/`doDeviceControl`（:670）/`doApc`（:742）；`resize`（:300）/`resizeScreen`（:330）；光标与 DECSET 位（:338-367）；`getSelectedText`（:1790）；`paste`（:1808）；`toggleAutoScrollDisabled`（:1738）；双缓冲（主屏/alt 屏，`isAlternateBufferActive` :269）。**torvox 用 libghostty-vt 替代了整个模块**，无需吸收，但 `WcWidth.kt`（44 KB，East Asian Width 表 + `wcwidth` 实现）可作为 ghostty 宽字符处理的对照测试基准。
- `KeyHandler`（`KeyHandler.kt:5`）—— `TERMCAP_TO_KEYCODE` 映射（:12-76，terminfo 名→Android keyCode+修饰位）；`getCodeFromTermcap`（:79）与 `getCode`（:103）把 keyCode 转义序列（含 cursorKeysApplication/keypadApplication 模式、修饰键组合 `transformForModifiers`）。**torvox 的 `encode_modifiers`（ffi.rs:308）+ 键盘模式处理可与此对照补缺（如 F1-F12、数字键盘 application 模式）**。
- `JNI`（`JNI.kt:8`）—— `createSubprocess`（:55 附近）/`setPtyWindowSize`（:72）/`waitFor`（:80）/`close`（:84），带 `isLibraryLoaded`/`libraryLoadError` 容错（:13-41，torvox NativeBridge.kt:25-33 同款）。
- `TerminalBuffer`（23 KB）/`TerminalRow`（12 KB）/`TerminalColors`/`TerminalColorScheme`/`TextStyle`/`ByteQueue`（3.5 KB，无锁环形队列）/`TerminalOutput`（接口，1 KB）/`Logger`（模块内简化版，2.4 KB）。

### 3.11 terminal-view 模块（View 渲染 + 手势 + 选择）

- `TerminalView`（`TerminalView.kt`，48 KB）—— 关键方法：`setTerminalViewClient`（:231）、`attachSession`（:249）、`onCreateInputConnection`（:265，IME 接入）、`onScreenUpdated`（:381/:385）、`getColumnAndRow`（:470，坐标→单元格）、`sendMouseEventCode`（:480）、`doScroll`（:498）、`onTouchEvent`（:528）、`onKeyPreIme`（:564）、`onKeyDown`（:587）、`inputCodePoint`（:664）、`handleKeyCode`/`handleKeyCodeAction`（:714/:728）、`onSizeChanged`（:778）、浮动工具栏（:1183-1198）。
- `TerminalRenderer`（12 KB）—— Canvas 渲染器（被 torvox wgpu 取代，无吸收价值）。
- `GestureAndScaleRecognizer`（3.4 KB）—— 缩放手势。
- `textselection/`—— `TextSelectionCursorController`（12.9 KB）/`TextSelectionHandleView`（10.5 KB）/`CursorController`（1.4 KB）；**Kotlin 版与 Java 版逐行对应**，已有 `research-termux-app.md` 详细分析（`onGetContentRect` 锚定、`updatePosition` 手柄拖动），此处不重复。

### 3.12 app 模块

#### 3.12.1 TermuxService（前台服务 + 会话仓库）
`app/.../TermuxService.kt`
- `class TermuxService : Service(), AppShell.AppShellClient, TermuxSession.TermuxSessionClient`（:59）—— 本地 Binder（:62-66）；持有 `TermuxShellManager`（:86）；wakelock/wifilock（:89-90）；前台通知（`runStartForeground` :116）。
- `onStartCommand`（:340 附近）—— 从 Intent 恢复 `ExecutionCommand` 全部字段（:353-388：executableUri、arguments、stdin、workdir、isFailsafe、sessionAction、shellName、shellCreateMode、commandLabel/Description/Help、pluginAPIHelp、resultConfig 的 PendingIntent/目录/文件格式，:376-384）→ 加入 `mPendingPluginExecutionCommands`（:391）→ 按 Runner 分派（:393-401）。
- `executeTermuxTaskCommand`（:405）—— APP_SHELL 路径：无 shellName 时用 executable basename（:411-413）、`ShellCreateMode.NO_SHELL_WITH_NAME` 复用同名任务（:418-425）。
- `createTermuxTask`（:431/:442）—— `AppShell.execute(this, execCmd, this, TermuxShellEnvironment(), null, false)`（:458-460）。
- `createTermuxSession`（:559）—— TERMINAL_SESSION 路径：建 `TerminalSession(shellPath, cwd, args, env, transcriptRows, serviceClient)` → 包 `TermuxSession` → 注册 `TermuxSessionClient`。
- 会话查找/切换/列表（:800-854 附近）：`getTermuxSessionForShellName`（:851）、`onAppShellExited`/`onTermuxSessionExited` 回调。

#### 3.12.2 TermuxActivity 与终端 client
- `TermuxActivity`（`TermuxActivity.kt`，35 KB）—— 经典 View 布局：`mTerminalView`/`mExtraKeysView`（:92-94）、抽屉会话列表（`TermuxSessionsListViewController`）、工具栏 ViewPager（`TerminalToolbarViewPager`）、广播接收（样式重载）、`reloadProperties`/`reloadActivityStyling`。
- `TermuxTerminalSessionActivityClient`（`terminal/TermuxTerminalSessionActivityClient.kt:29`）—— `onSessionFinished`（:94）：当前会话退出且 service 想停 → finish activity；插件命令有 pending result 则强制关闭会话（:104-110）；`onBell`（:142）：按 `bell-behaviour` 属性 vibrate（BellHandler）或 beep（SoundPool，:148-150）；字体/颜色检查（`checkForFontAndColors` :37-40）。
- `TermuxTerminalViewClient`（`terminal/TermuxTerminalViewClient.kt:43`）—— 虚拟 Ctrl/Fn 键状态（:49-50）、软键盘控制（:84-98）、光标闪烁（:87-90）、`copyModeChanged` 锁抽屉（:173-177）、`onKeyDown`（:180+，键盘快捷键：`KeyboardShortcut` 会话切换、URL 打开 `TermuxUrlUtils`）、`onSingleTapUp`（长按菜单）、`onLongPress`。
- `TermuxSessionsListViewController`（4.1 KB）/`TermuxActivityRootView`（14.9 KB，日志/调试覆盖层）。

#### 3.12.3 TermuxInstaller —— bootstrap 安装（fork 改造版）
`app/.../TermuxInstaller.kt`
- `object TermuxInstaller`（:50）—— `setupBootstrapIfNeeded(activity, whenDone)`（:56）：仅主用户（:61-75）、files 目录可访问性（:77-97）、`$PREFIX` 已存在且非空则跳过（:100-109）；否则 ProgressDialog + 后台线程：清 staging（:120）→ 解压（:122+）→ 完成回调。
- **fork 特有路径修复**：`isTextFileNeedingPathFix(entryName)`（:415）—— 对 `bin/` 已知脚本清单（login/chsh/su/am/pm/pkg/apt-key/dpkg-* 等，:422-435）、`termux-*`、`.sh/.bash`、`etc/`、`share/`、`libexec/`、`var/lib/dpkg/info/*.postinst`（:482-490）、`lib/pkgconfig/*.pc`、`lib/cmake/*.cmake`（:492-500）、特定头文件（:508-518）做**硬编码路径替换**（`/data/data/com.termux` ↔ 实际路径）；ELF 二进制绝不改（:420-421 注释）。
- `setupCompatLayer`（:1043）—— 生成 `libtermux_compat.c` 源码 + 编译脚本 `termux-compat-build`（:1075-1107）+ profile 追加 `LD_PRELOAD`（:1111-1123）—— LD_PRELOAD 截获文件系统 syscall 重定向路径。
- `setupAptWrapper`（:980-1031 附近）—— 生成 apt/dpkg wrapper 脚本：`exec .../usr/bin/$cmd.real -o Dir::Etc=... -o Dir::State=... -o Acquire::https::CaInfo=...`（:1001-1021）—— **这是 fork 对"上游二进制硬编码 /data/data/com.termux 路径"问题的三件套（wrapper 脚本 + LD_PRELOAD shim + 环境变量覆盖）之一**。
- `TermuxBootstrap` JNI（:1340-1367）—— `System.loadLibrary("termux-bootstrap")` + `external getZip(): ByteArray`（:1367，bootstrap zip 编进 .so）。**torvox 走下载 zip（BootstrapDownloader），不走内嵌 .so —— termux 的 30MB 内嵌换取离线可用，是依赖分析里值得权衡的点。**

#### 3.12.4 RunCommandService —— 第三方 RUN_COMMAND API
`app/.../RunCommandService.kt`
- `class RunCommandService`（:35）—— 接收 `ACTION_RUN_COMMAND`（:68 校验 action），解析 `EXTRA_COMMAND_PATH`/`EXTRA_ARGUMENTS`（:75-77）、**逗号替代字符还原**（`EXTRA_REPLACE_COMMA_ALTERNATIVE_CHARS_IN_ARGUMENTS`，:89-96，因 `am` 命令会吃逗号，tudo/sudo 的 `-r --comma-alternative` 之外的原生方案）、`EXTRA_STDIN`/`EXTRA_WORKDIR`（:98-99）、`EXTRA_RUNNER`/`EXTRA_BACKGROUND`（:102-103）、会话参数与 resultConfig（:111-120）→ 转交 `TermuxService`。`$PREFIX` 展开与 applet 处理（:196-200）。
- 这是 termux 插件生态的核心 API；torvox 若要做"从外部 App 启动命令"，此 Service 是现成模板（含失败时 `processPluginExecutionCommandError` 通知回传）。

#### 3.12.5 TermuxApplication
`app/.../TermuxApplication.kt` —— `onCreate`（:23）：crash handler（:29）→ 日志配置（:32，`setLogConfig` :97）→ `TermuxBootstrap.setTermuxPackageManagerAndVariant`（:37）→ 属性/ShellManager 初始化（:40/:43）→ 主题（:46）→ **`TermuxAmSocketServer.setupTermuxAmSocketServer(this)`（:60 附近）** → bootstrap 完成后 `AgentService.startAgentService`（:81-87）。

### 3.13 fork 特有子系统（torvox 对比对象）

- **agents/（约 55 文件）**：`AgentDaemon.kt`（18 KB，常驻守护）、`AgentRegistry.kt`（12.6 KB）、`AgentWorker.kt`（7.5 KB）、`HandoffExecutor.kt` 与各类 HandoffAgents（文件/网络/包管理，10-15 KB）、`ReasoningOrchestrator.kt`/`TaskPlanner.kt`/`IntentRecognizer.kt`（LLM 意图识别与任务规划）、`SkillExecutor.kt`/`FsSkill.kt`/`GitSkill.kt`/`PkgSkill.kt`/`DiagnosticSkill.kt`/`BusyBoxSkill.kt`（35 KB）、`McpServer.kt`（13.7 KB，Kotlin 侧 MCP 实现）、`Tool.kt`/`ToolRegistry.kt`/`EnhancedTool.kt`/`ResourceRegistry.kt`（MCP 工具框架）、`OllamaClient.kt`/`StreamingOllamaClient.kt`/`LlmProvider.kt`（本地 LLM）、`SwarmCoordinator.kt`/`Signal.kt`（stigmergy 信号）、`AgentSandbox.kt`/`AgentMemory.kt`/`CommandRunner.kt`（沙箱执行）。
- **boot/**：`BootService.kt`/`BootScriptExecutor.kt`（12.5 KB，开机脚本执行 + 属性解析）/`BootPreferences.kt`。
- **widget/**：`TermuxWidgetProvider.kt`（14 KB）/`WidgetConfigureActivity.kt`（13.3 KB）/`ShortcutScanner.kt`。
- **styling/**：`ColorScheme.kt`（12 KB，内置 11 配色）/`FontManager.kt`（7.3 KB）/`StylingManager.kt`（12.6 KB）/`StylingActivity.kt`（25 KB，Compose）。
- **pkg/**：`TermuxCtl.kt`（21.7 KB，包管理 CLI）+ `commands/`、`PackageBackupManager.kt`（17.3 KB）、`doctor/`（环境自检）。
- **ui/compose/**：`CommandPalette.kt`（命令面板，模糊搜索高亮）、`TerminalSettingsScreen.kt`/`SettingsScreen.kt`（Compose 设置）。
- **core/**：`TerminalEventBus.kt`（7.5 KB，事件总线）、`Result.kt`/`TerminalEvents.kt`/`TermuxError.kt`（13.6 KB，错误模型）。
- **agents/ 的 Python 遗留**：仓库根 `agents/`（Python 版 agentd.py 61 KB、skill_learner.py 等）—— v2.0.5 起被纯 Kotlin 取代（README:64-75），Python 目录是历史遗留。

---

## 4. 全文件功能索引（文件:行号）

> 行号 = 该文件内的类/对象/函数定义行。标 ★ 的文件在 §3 已详解，此处只列关键符号；其余为快速索引。路径省略前缀：termux-shared = `termux-shared/src/main/kotlin/com/termux/shared/`，其余同 §3。

### 4.1 termux-shared

**shell/**（命令执行体系）
| 文件 | 关键符号:行号 |
|---|---|
| ★ `shell/ArgumentTokenizer.kt` | object :50；tokenize() :62；tokenize(stringify) :73；escapeQuotesAndBackslashes() :170 |
| ★ `shell/StreamGobbler.kt` | class :35；OnLineListener :40；OnStreamClosedListener :56；构造1 :96；构造2 :120+；run() :160+；incThreadCounter :300 |
| `shell/ShellUtils.kt` | object :7；getPid :13；setupShellCommandArguments :31；getTerminalSessionTranscriptText :45 |
| ★ `shell/am/AmSocketServer.kt` | object :53；start :65；processAmClient :77；sendResultToClient :135；sanitizeExitCode :164；parseAmCommand :186；runAmCommand :213；AmSocketServerClient :257 |
| `shell/am/AmSocketServerRunConfig.kt` | class :12；shouldCheckDisplayOverAppsPermission :28；getLogString :38 |
| `shell/am/AmSocketServerErrno.kt` | class :5；ERRNO_PARSE_AM_COMMAND_FAILED :13；ERRNO_RUN_AM_COMMAND_FAILED :15 |
| ★ `shell/command/ExecutionCommand.kt` | class :13；ExecutionState :27；Runner :37；ShellCreateMode :70；getCommandIdAndLabelLogString :600+ |
| `shell/command/ShellCommandConstants.kt` | object :7（RESULT_SENDER 等常量） |
| `shell/command/environment/IShellEnvironment.kt` | interface :9 |
| `shell/command/environment/UnixShellEnvironment.kt` | abstract class :13；ENV_* 常量 :17-48；LOGIN_SHELL_BINARIES :51 |
| `shell/command/environment/AndroidShellEnvironment.kt` | open class :16 |
| `shell/command/environment/ShellCommandShellEnvironment.kt` | open class :9 |
| `shell/command/environment/ShellEnvironmentUtils.kt` | object :7（putToEnvIfSet、convertEnvironmentToDotEnvFile :118 附近） |
| `shell/command/environment/ShellEnvironmentVariable.kt` | data class :6 |
| `shell/command/result/ResultConfig.kt` | class :7（resultPendingIntent/resultDirectoryPath/文件格式） |
| `shell/command/result/ResultData.kt` | open class :10（stdout/stderr/exitCode/errorsList） |
| ★ `shell/command/result/ResultSender.kt` | object :19；sendCommandResultData :39；…WithPendingIntent :69；…ToDirectory :300+ |
| `shell/command/result/ResultSenderErrno.kt` | class :6 |
| ★ `shell/command/runner/app/AppShell.kt` | class :32；executeInner :48；execute 工厂 :130+；processAppShellResult :300+ |

**termux/shell/**（Termux 装配）
| 文件 | 关键符号:行号 |
|---|---|
| ★ `termux/shell/command/runner/terminal/TermuxSession.kt` | class :27；finish :42；killIfExecuting :78；TermuxSessionClient :108；processTermuxSessionResult :117+ |
| ★ `termux/shell/command/environment/TermuxShellEnvironment.kt` | open class :17；getEnvironment :24；getDefaultWorkingDirectoryPath :75；writeEnvironmentToFile :118 |
| ★ `termux/shell/command/environment/TermuxAppShellEnvironment.kt` | object :17；ENV_TERMUX_APP__* :31-70；getEnvironment :74；setTermuxAppEnvironment :82；updateTermuxAppAMSocketServerEnabled :156 |
| `termux/shell/command/environment/TermuxAPIShellEnvironment.kt` | object（TERMUX_API__* 环境，被 TermuxShellEnvironment :32 引用） |
| ★ `termux/shell/TermuxShellManager.kt` | class :10；mTermuxSessions :22；mTermuxTasks :26；init :51；getNextShellId :88 附近 |
| ★ `termux/shell/TermuxShellUtils.kt` | object :13；setupShellCommandArguments :22；clearTermuxTMPDIR :88 |
| ★ `termux/shell/am/TermuxAmSocketServer.kt` | object :55；setupTermuxAmSocketServer :76；start :104；TermuxAmSocketServerClient :200+ |

**net/socket/local/**（AF_UNIX 基础设施）
| 文件 | 关键符号:行号 |
|---|---|
| `ILocalSocketManager.kt` | interface :8 |
| ★ `LocalSocketManager.kt` | class :19；start :43；stop :67；onError/onDisallowedClientConnected/onClientAccepted :83-101；startLocalSocketManagerClientThread :104；JNI externals :316-323 |
| ★ `LocalServerSocket.kt` | open class :14；start :30；stop :105；closeServerSocket :120+；SERVER_SOCKET_PARENT_DIRECTORY_PERMISSIONS :284 |
| `LocalClientSocket.kt` | open class :17（readDataOnInputStream/sendDataToOutputStream/peerCred） |
| ★ `LocalSocketRunConfig.kt` | open class :12；abstract 命名空间检测 init :89-97；超时/backlog :60-87 |
| `LocalSocketManagerClientBase.kt` | abstract class :7 |
| `PeerCred.kt` | class :12（uid/pid/gid） |
| `LocalSocketErrno.kt` | class :5（全套错误码） |

**logger / reflection / 其余工具**
| 文件 | 关键符号:行号 |
|---|---|
| ★ `logger/Logger.kt` | object :14；级别 :18-25；LOGGER_ENTRY_MAX_PAYLOAD :30；logMessage :38；logExtendedMessage :49；logError* :81-110；logStackTraceWithMessage :150+；shouldEnableLoggingForCustomLogLevel :382 |
| ★ `reflection/ReflectionUtils.kt` | object :11；bypassHiddenAPIReflectionRestrictions :23；getDeclaredField :50；FieldInvokeResult :62；invokeField :81；getDeclaredMethod :95/:108；invokeConstructor :241 |
| `android/AndroidUtils.kt` object :20；`android/FeatureFlagUtils.kt` object :26；`android/PackageUtils.kt` object :22（31 KB）；`android/PermissionUtils.kt` object :26（display-over-apps :224 附近）；`android/PhantomProcessUtils.kt` object :18；`android/ProcessUtils.kt` object :7；`android/SELinuxUtils.kt` object :7；`android/SettingsProviderUtils.kt` object :7；`android/UserUtils.kt` object :7；`android/resource/ResourceUtils.kt` object :7 | Android 平台工具 |
| `crash/CrashHandler.kt` class :13 | 默认未捕获异常处理器 |
| `data/DataUtils.kt` object :9（TRANSACTION_SIZE_LIMIT_IN_BYTES、getTruncatedCommandOutput）；`data/IntentUtils.kt` object :7 | 数据/Intent 工具 |
| `errors/Errno.kt` open class :7；`errors/Error.kt` open class :8；`errors/FunctionErrno.kt` :4 | 错误模型 |
| `file/FileUtils.kt` object :36（52 KB）；`file/FileUtilsErrno.kt` class :6；`file/filesystem/`：FileAttributes :39 / FileKey :32 / FilePermission :39 / FilePermissions :38 / FileTime :42 / FileType :6 / FileTypes :5 / NativeDispatcher :9 / UnixConstants :39；`file/tests/FileUtilsTests.kt` object :12 | 文件系统（含 JNI 包装） |
| `interact/MessageDialogUtils.kt` object :13；`interact/ShareUtils.kt` object :22 | 对话框/分享 |
| `jni/models/JniResult.kt` class :14 | JNI 结果模型 |
| `markdown/MarkdownUtils.kt` object :33 | markdown 工具 |
| `models/ReportInfo.kt` open class :11；`models/TextIOInfo.kt` class :12 | 报告模型 |
| `net/uri/UriScheme.kt` object :9；`net/uri/UriUtils.kt` object :7；`net/url/UrlUtils.kt` object :8 | URI/URL 工具 |
| `notification/NotificationUtils.kt` object :11 | 通知工具 |
| `settings/preferences/AppSharedPreferences.kt` open class :7；`SharedPreferenceUtils.kt` object :8；`settings/properties/SharedProperties.kt` class :36；`SharedPropertiesParser.kt` interface :9 | 偏好/属性解析 |
| `activities/ReportActivity.kt` open class :42；`activities/TextIOActivity.kt` open class :39；`activity/ActivityUtils.kt` :11；`activity/ActivityErrno.kt` :5；`activity/media/AppCompatActivityUtils.kt` :11 | Activity 工具 |
| `theme/NightMode.kt` enum :7；`theme/ThemeUtils.kt` object :6 | 主题 |
| `tools/TermuxTools.kt` object :13 + ToolResult :190 | fork 特有工具执行 |
| `view/KeyboardUtils.kt` object :14；`view/ViewUtils.kt` object :16 | View 工具 |

**termux/**（Termux 域）
| 文件 | 关键符号:行号 |
|---|---|
| `termux/TermuxConstants.kt` object :12（TERMUX_FILES_DIR_PATH :203、PREFIX :206、BIN :209、LIB :218、TMP :227、AM_SOCKET_FILE_PATH :361 等全部路径常量） | 路径/常量全集 |
| `termux/TermuxBootstrap.kt` object :7；`termux/TermuxUtils.kt` object :25 | bootstrap/工具 |
| `termux/crash/TermuxCrashUtils.kt` class :31；`termux/data/TermuxUrlUtils.kt` object :7 | 崩溃报告/URL |
| ★ `termux/extrakeys/` | ExtraKeyButton.kt :1（isMacro）、ExtraKeysConstants.kt :1、ExtraKeysInfo.kt :1、ExtraKeysView.kt :61、SpecialButton.kt :1、SpecialButtonState.kt :1 |
| `termux/file/TermuxFileUtils.kt` object（isTermuxFilesDirectoryAccessible 等）；`termux/interact/TextInputDialogUtils.kt` object | 文件/输入 |
| `termux/models/`、`termux/notification/` | 模型/通知 |
| `termux/plugins/TermuxPluginUtils.kt` object :1（26 KB） | 插件工具 |
| `termux/settings/preferences/TermuxAppSharedPreferences.kt` class；`termux/settings/properties/TermuxAppSharedProperties.kt` class + `TermuxPropertyConstants.kt` | termux.properties 读写与全部属性键 |
| ★ `termux/terminal/TermuxTerminalSessionClientBase.kt` open class :7；★ `termux/terminal/TermuxTerminalViewClientBase.kt` open class :9 | 空实现基类 |
| ★ `termux/terminal/io/TerminalExtraKeys.kt` open class :13；★ `termux/terminal/io/BellHandler.kt` class :12；`termux/theme/TermuxThemeUtils.kt` object :7 | IO/主题 |

### 4.2 terminal-emulator（`terminal-emulator/src/main/kotlin/com/termux/terminal/`）

| 文件 | 关键符号:行号 |
|---|---|
| ★ `TerminalSession.kt` | class :29；双 ByteQueue :50/:54；updateSize :91；initializeEmulator :107；三线程 :117/:135/:152；write :161；writeCodePoint :166；finishIfRunning :209；cleanupResources :220；getCwd :262；MainThreadHandler :279；wrapFileDescriptor :316 |
| `TerminalSessionClient.kt` | interface :8 |
| ★ `TerminalEmulator.kt` | mRows/mColumns :165-166；getScreen :267；isAlternateBufferActive :269；sendMouseEvent :278；resize :300；append :374；processByte :380；processCodePoint :434；doDeviceControl :670；doApc :742；doCsiQuestionMark :778；doDecSetOrReset :846；getSelectedText :1790；paste :1808；toggleAutoScrollDisabled :1738 |
| ★ `KeyHandler.kt` | object :5；KEYMOD_* :7-10；TERMCAP_TO_KEYCODE :12；getCodeFromTermcap :79；getCode :103 |
| ★ `JNI.kt` | object :8；createSubprocess :55 附近；setPtyWindowSize :72；waitFor :80；close :84 |
| `TerminalBuffer.kt` / `TerminalRow.kt` / `WcWidth.kt`（44 KB）/ `TerminalColorScheme.kt` / `TerminalColors.kt` / `TextStyle.kt` / `TerminalOutput.kt` / `ByteQueue.kt` / `Logger.kt` | 屏幕缓冲/宽字符/配色/样式/环形队列 |

### 4.3 terminal-view（`terminal-view/src/main/kotlin/com/termux/view/`）

| 文件 | 关键符号:行号 |
|---|---|
| ★ `TerminalView.kt` | setTerminalViewClient :231；attachSession :249；onCreateInputConnection :265；onScreenUpdated :381；getColumnAndRow :470；sendMouseEventCode :480；doScroll :498；onTouchEvent :528；onKeyPreIme :564；onKeyDown :587；inputCodePoint :664；handleKeyCode :714；handleKeyCodeAction :728；onKeyUp :755；onSizeChanged :778；showFloatingToolbar :1183 |
| `TerminalViewClient.kt` | interface :13 |
| `TerminalRenderer.kt` / `GestureAndScaleRecognizer.kt` / `textselection/`（TextSelectionCursorController/TextSelectionHandleView/CursorController）/ `support/PopupWindowCompatGingerbread.kt` | 渲染/手势/文本选择（详见 research-termux-app.md） |

### 4.4 app（`app/src/main/kotlin/com/termux/app/`）

| 文件 | 关键符号:行号 |
|---|---|
| ★ `TermuxService.kt` | class :59；onStartCommand :340 附近；executeTermuxTaskCommand :405；createTermuxTask :431/:442；createTermuxSession :559；getTermuxSessionForShellName :851 |
| ★ `TermuxActivity.kt` class（35 KB）；★ `TermuxInstaller.kt` object :50（setupBootstrapIfNeeded :56、isTextFileNeedingPathFix :415、setupCompatLayer :1043、setupAptWrapper :980-1031、JNI :1340-1367）；★ `RunCommandService.kt` class :35；★ `TermuxApplication.kt` class :21（onCreate :23、setLogConfig :97）；`TermuxOpenReceiver.kt` class | 主流程 |
| `AgentService.kt` class（14 KB） | agent 前台服务 |
| ★ `terminal/TermuxTerminalSessionActivityClient.kt` class :29（onBell :142）；`terminal/TermuxTerminalSessionServiceClient.kt` class；★ `terminal/TermuxTerminalViewClient.kt` class :43；`terminal/TermuxActivityRootView.kt` class；`terminal/TermuxSessionsListViewController.kt` class；`terminal/io/TerminalToolbarViewPager.kt` class；★ `terminal/io/TermuxTerminalExtraKeys.kt` class :18；`terminal/io/KeyboardShortcut.kt` | 终端客户端 |
| `activities/HelpActivity.kt`/`SettingsActivity.kt`；`api/file/FileReceiverActivity.kt`（12 KB） | Activity |
| `boot/`：BootModule/BootPreferences/BootScriptExecutor（12.5 KB）/BootService | 开机插件 |
| `widget/`：ShortcutScanner/TermuxWidgetProvider（14 KB）/WidgetConfigureActivity（13.3 KB）/WidgetModule/WidgetPreferences/WidgetRemoteViewsService | 小部件插件 |
| `styling/`：ColorScheme（12 KB）+ BuiltInColorSchemes :97/FontManager（7.3 KB）/StylingActivity（25 KB）/StylingManager（12.6 KB）+ StylingSettings :380/StylingModule :16 | 主题插件 |
| `pkg/backup/`（BackupMetadata/PackageBackupManager 17.3 KB）；`pkg/cli/TermuxCtl.kt`（21.7 KB）+ commands/；`pkg/doctor/` | 包管理 |
| `ui/compose/commandpalette/CommandPalette.kt`（:51/:60/:278/:331）；`ui/compose/settings/TerminalSettingsScreen.kt`（:151/:183/:215）；`ui/settings/`（SettingsScreen/SettingsNavigation/SettingsViewModel/ProfilesScreen/ThemeGalleryScreen）；`ui/viewmodel/TerminalSettingsViewModel.kt` | Compose UI |
| `data/model/TerminalModels.kt`；`data/repository/`（CommandHistoryRepository/SshProfileRepository/TerminalSessionManager/TerminalSettingsRepository）；`di/`（AppModule/DeviceApiModule/SettingsModule）；`event/SystemEventReceiver.kt`；`models/UserAction.kt`；`core/api/`（Result/TerminalEvents/TermuxError）；`core/terminal/TerminalEventBus.kt` | 数据/Hilt/事件 |
| `agents/**`（约 55 文件，见 §3.13） | fork 特有代理框架 |

---

## 5. 与 torvox 功能对比

### 5.1 对比总表

| 功能 | termux-kotlin | torvox | 结论 |
|---|---|---|---|
| VT 解析器 | Kotlin `TerminalEmulator.kt`（85 KB，自研） | **libghostty-vt（Rust）**（`native/src/vt.rs`） | torvox 明显先进（成熟维护+性能）；termux 无吸收价值，仅 WcWidth 可作对照测试 |
| PTY 创建 | C `jni/termux.c` + `JNI.createSubprocess`（fork+execvp+setsid+TIOCSCTTY） | Rust `pty.rs:88 spawn`（posix_openpt/grantpt/unlockpt+login_tty） | 等价；torvox 全 Rust 更内聚 |
| 会话层 | `TerminalSession`（terminal-emulator）+ `TermuxSession`（shared）双包装 | `session.rs:101 Session`（Rust） | 等价；termux 的"Session 包装 Command"模型是 torvox `SessionManager` 的设计参照 |
| 后台命令 | `AppShell`（Runtime.exec + StreamGobbler×2） | 无对等物（MCP `cmd` 工具走 `Command` 结构体，`CommandRegistry`） | termux 有；torvox 用 Rust Command + output capture 已覆盖，但**无 transcript 语义** |
| shell 环境 | 分层 `IShellEnvironment`（Android→Termux→TermuxApp/API）+ `termux.env` 文件 | `shell_env.rs:4 ShellEnv`（静态默认环境） | termux 有完整分层；torvox 环境硬编码在 Rust（Android/termux 混合），**termux 的分层与 dpkg/TERMINFO/SSL_CERT 覆盖更完整** |
| 命令总线 | `ExecutionCommand` 状态机 + `Runner` 分派 + `ResultSender` 双通道 | `TerminalRuntime` + `CommandRegistry` + JNI 事件 | 思路同构；termux 的 **exitCode≠failed 语义**（errCode 才是失败）值得抄 |
| AF_UNIX 服务端 | `AmSocketServer`+`LocalSocketManager`（JNI SO_PEERCRED） | `mcp.rs`（tower-mcp + tokio，UnixStream listener） | torvox 的 MCP 是**结构化 JSON-RPC 协议**，termux 是**裸命令字符串 + `\0` 分隔**；协议上 torvox 先进，**但 termux 的 peer credential 校验与 `\0` 分帧是轻量场景（如 client 发"构建命令"）的极简方案** |
| 命令字符串解析 | `ArgumentTokenizer`（四态机，引号/转义） | **无**（所有参数走 argv 数组/JNI 字符串列表） | **torvox 缺口**：若要从字符串构建命令（postinst 字符串、用户粘贴命令、`termux-open` 风格 API），需要它 |
| 子进程输出收集 | `StreamGobbler`（双线程 + 行回调 + 死锁警告） | Rust `session.rs` ReaderTask / Command output | 等价；Rust 侧天然避免死锁（async read），无吸收必要，**但"必须尽快消费管道"的注释级认知值得记录** |
| 终端铃 | `BellHandler`（防抖 150ms + 50ms 振动 + 三星 NPE 保护） | `bel_triggered` flag → Kotlin 播放 | 行为等价；**termux 的防抖/主线程调度/厂商兼容注释值得抄** |
| 额外键 | `ExtraKeysView`（GridLayout）+ `TerminalExtraKeys` 宏（"CTRL ALT x" 空格拆解、粘滞修饰键）+ popup 子键盘 | `ModifierBar.kt`（Compose，ToolbarPreferences 自定义布局、长按重复、选择操作条） | UI 上 torvox 先进；**宏解析语义（isMacro 空格拆解+粘滞修饰键）torvox 没有，值得加** |
| 隐藏 API 绕过 | `ReflectionUtils.bypassHiddenAPIReflectionRestrictions`（HiddenApiBypass） | 无系统性方案 | termux 有；torvox 若需 `Process.pid`/私有字段可移植 |
| 日志 | `Logger`（分块 4068B、级别、Private 变体、markdown 栈） | `LogUtil`（简化） | **termux 的分块与 Private 级别值得吸收** |
| bootstrap 安装 | 内嵌 zip 于 `.so`（`TermuxInstaller`）+ **路径修复三件套**（wrapper 脚本/`LD_PRELOAD` shim/环境覆盖）+ 属性解析 symlink | 下载 zip + `BootstrapInstaller` 解压 + `SecondStageRunner` postinst | 大体等价；**termux 的 `isTextFileNeedingPathFix` 白名单、dpkg wrapper、`LD_PRELOAD` 方案是 torvox 没有的深度兼容手段** |
| postinst 执行 | 隐式（bootstrap 内由包管理器跑） | `SecondStageRunner.runPostInstalls`（`:64`）显式跑 `var/lib/dpkg/info/*.postinst`（`postinstCommand` :241 含 shebang 解析 + linker 包装） | 等价；torvox 显式执行更可控（SELinux linker 方案）；字符串→argv 工具缺口见 §7.1 |
| MCP/工具服务器 | fork 的 `McpServer.kt`（Kotlin 侧）+ am socket | `mcp.rs`（Rust tower-mcp，HTTP+Unix socket 双传输） | torvox 先进（原生 Rust、JSON-RPC 标准）；termux 的 agents/ 是"Agent Daemon"思路（常驻、skills、swarm），torvox 目前只有被动工具调用，**无主动 agent 循环** |
| 前台服务 | `TermuxService`（前台通知+wakelock） | `TerminalRuntime.startForegroundServiceIfNeeded`（:378） | 等价 |
| 文本选择 | View 手柄方案 | wgpu 合成层方案（`selection.rs`） | torvox 先进（滚轮/键盘选择） |
| 快捷键 | `KeyboardShortcut` + 会话切换 | `TerminalViewModel.handleLayoutAwareHardwareKey`（:1302） | 等价 |

### 5.2 关键对比详述

**5.2.1 AmSocketServer vs torvox MCP socket**

| 维度 | termux `AmSocketServer` | torvox `mcp.rs` |
|---|---|---|
| 协议 | 请求=一行命令字符串（无 "am" 前缀）；响应=`exit_code\0stdout\0stderr\0`（`\0` 分帧） | JSON-RPC 2.0（tower-mcp）：`initialize`→`tools/list`→`tools/call`，响应为 JSON |
| 传输 | AF_UNIX（路径 `$FILES_DIR/apps/termux-am/am.sock`），`LocalSocketManager` JNI（SO_PEERCRED 校验 uid/pid/gid） | AF_UNIX（`MCP_SOCKET_PATH`，mcp.rs:584 起）+ HTTP（可选）；tokio async |
| 身份 | **对端凭证校验**（本用户 + root 白名单），root 连接按 termux uid 执行（TermuxAmSocketServer.kt:34-36） | 无对端校验（socket 文件权限 0700 限定） |
| 命令解析 | `ArgumentTokenizer.tokenize`（引号/转义四态机） | JSON 天然结构化，无需 tokenize |
| 响应截断 | `ResultSender` 4008B 截断（Binder） | 无（走 JNI/JSON） |
| 线程模型 | 每连接新线程（JNI accept 回调） | tokio async task |
| 失败语义 | `sanitizeExitCode`（>255 强制 1）；exitCode≠failed（errCode 才算） | exit_code 直接透传 |

**结论**：MCP（torvox）在协议现代化、结构化、可扩展性（tools/resources）上全面领先，**不应回退到 am 协议**；但 termux 有三个点值得抄：(1) **对端凭证校验**（`getPeerCredNative` → 校验 uid∈{本 uid, root}）可加进 mcp.rs 的 UnixStream accept 逻辑（`SO_PEERCRED`）；(2) `\0` 分帧 + `sanitizeExitCode` 的"轻量命令通道"思路可用于 torvox 未来给 `termux-open` 类 API 或**同 App 内部 shell 脚本 ↔ Kotlin 通信**；(3) `TERMUX_APP__AM_SOCKET_SERVER_ENABLED` 式"能力开关导出到 shell 环境"的模式。

**5.2.2 TermuxSession/TerminalSession vs torvox session.rs**
- termux 双层（TerminalSession=pty+VT 线程，TermuxSession=ExecutionCommand 包装）⇔ torvox 单层 `Session`（pty+vt+状态，Rust）—— torvox 更内聚。
- **差距在"结果"**：termux 的 `TermuxSession.finish()` 能把**整个屏幕 transcript** 作为 stdout 回传（`getTerminalSessionTranscriptText` + `mSetStdoutOnExit`），并维持 `exitCode/errCode` 双轨。torvox 的 `terminal_info` 只给 `exit_code`/`pid`/尺寸等元数据。若 torvox 的 MCP 客户端想要"命令在终端里的完整输出"，目前需自己读屏幕网格；**建议：给 `terminal_info` 增加可选 `transcript` 字段（session.rs 的 `screen_contents` 已有类似能力，ffi.rs:1587 附近）**。

**5.2.3 ModifierBar vs ExtraKeys**
- torvox `ModifierBar` 已支持自定义布局 JSON（`ToolbarPreferences`）、长按重复、选择操作条（:119）、`ModifierKey`（:52）枚举 —— UI 层先进。
- **宏缺口**：termux 的 `TerminalExtraKeys.onExtraKeyButtonClick`（:15-36）支持 `"CTRL ALT a"` 空格拆解的**键序列宏** + **粘滞修饰键**（修饰键持续到普通键按下）。torvox 的 modifier bar 按键是单键（CTRL/ALT/SHIFT 状态切换）。**建议：ModifierBar 的按钮值支持空格分隔的键序列（第一个 token 前为修饰键）**——实现可直接移植 TerminalExtraKeys.kt 的 `isMacro` 分支逻辑（~20 行）。

**5.2.4 bootstrap 安装链**
- 相同：zip 下载/解压、`$PREFIX` 布局、`etc/profile` 生成、symlink 解析（`BootstrapInstaller.parseSymlinks` :197 ↔ termux `TermuxBootstrap` zip）、安装完成标志都用 `termux.env` 存在性。
- termux 独有：**路径修复白名单**（`isTextFileNeedingPathFix` :415-518，脚本/`.pc`/`.cmake`/dpkg postinst 硬编码路径替换）、**apt/dpkg wrapper**（`setupAptWrapper` :980-1031，`-o Dir::Etc=...` 全家桶）、**LD_PRELOAD shim**（`setupCompatLayer` :1043+）。**torvox 用自己包名 + 自己的 bootstrap 镜像，理论上不需要这三件套**；但若 torvox 未来支持"安装上游 termux 官方包"（复用 termux 仓库），这三件套是唯一已验证的方案，应作为 `docs/` 中的备选路线记录。

---

## 6. 依赖分析（是否适用于 torvox？是否先进激进？）

### 6.1 技术栈评估

| 依赖/技术 | termux-kotlin 用法 | 对 torvox 适用性 | 评价 |
|---|---|---|---|
| JNI C（termux.c/termux-shared/termux-bootstrap） | pty 创建、AF_UNIX、zip 内嵌 | ✗ 不适用（torvox 全 Rust，JNI 已由 `android/ffi.rs` 覆盖） | 落后做法（C 工具链、ABI 风险） |
| 反射 HiddenApiBypass | 私有字段/非 SDK API | ◯ 按需（torvox 已依赖部分反射） | 激进但实用；Android 15+ 仍有效（豁免列表） |
| Hilt DI | app 装配 | ✗ 不适用（torvox 无 DI 框架，手动构造更清晰） | 中性 |
| Compose | 仅设置/命令面板（约 10%） | ✓ torvox 全 Compose | 一致方向 |
| Rust | 无 | — torvox 核心 | torvox 领先 |
| libghostty-vt/wgpu | 无 | — torvox 核心 | torvox 领先 |
| 内置 Termux:API/Boot/Widget/Styling | 插件内置 | ◯ 部分值得（boot 脚本执行、widget 是 termux 生态独占价值） | 激进（APK 变大），torvox 有自己路线 |
| agents/（Ollama、swarm、LLM 意图） | fork 特有 | △ 方向参考（torvox 的 MCP 是"被调用"，agent 是"主动调用"） | 激进且质量参差（Python 遗留），**不建议直接吸收，但 McpServer.kt 的工具注册模式可对照** |

### 6.2 先进性结论
- **VT/渲染/会话**：torvox 全面领先（Rust + libghostty-vt + wgpu vs Kotlin 自研状态机 + Canvas）。
- **进程/shell 工程**：持平到 termux 领先——termux 的 `ExecutionCommand`/`ResultSender`/环境分层/transcript 回传/路径兼容三件套，是多年插件生态打磨的产物，torvox 的 MCP 体系年轻，**这些工程细节是本次研究的主要吸收对象**。
- **激进程度**：termux-kotlin 是"全量重写 + 大量冒险特性"（路径硬编码替换、LD_PRELOAD、agent 常驻服务），很多做法（如改文件内容里的路径）**依赖官方包的特定假设，脆弱**；torvox 应吸收其**思想**而非**实现**。

---

## 7. 可吸收到 torvox 的具体内容（含代码注释建议）

> 优先级：P0=立即有价值；P1=近期；P2=远期/按需。

### 7.1 P0：ArgumentTokenizer 移植 → "命令字符串 → argv" 安全转换（torvox 缺口）★
**现状（先澄清）**：`SecondStageRunner.runPostInstalls`（`android/.../installer/SecondStageRunner.kt:64`）→ `postinstCommand(script)`（:241-271）**已实现 shebang 解析**（`readShebang` :301 + canonical-path 判断：prefix 内解释器走 linker 包装，:263-270）—— argv 全部直接构造，**postinst 路径本身没有注入面，不需要 tokenizer**。文档此前的"硬编码 argv"表述不准确，已修正。

**真正的缺口**：torvox 目前**没有任何"把字符串安全拆成 argv"的工具**。存在真实需求的场景：
（a）MCP `cmd` 工具/未来扩展工具收到**字符串形式**命令（外部 agent 传 `"apt install \"foo bar\""` 而不是数组）—— 现在只能要求调用方传数组，或错误地 `split(" ")`（文件名含空格即崩）；
（b）`termux-open` 风格 API 或用户粘贴的命令字符串；
（c）未来支持第三方/社区 bootstrap 包时解析自定义构建命令。

**移植建议**（`app/src/main/java/terminal/emulator/installer/` 或 `util/` 新增 `ArgumentTokenizer.kt`，约 130 行，BSD 许可可直接抄）：
```kotlin
// 移植自 termux-shared/.../shell/ArgumentTokenizer.kt（DrJava 派生，BSD 许可，保留头部版权块）
// 用途：把 "sh -c 'echo hi > \"$file\"'" 这类字符串安全拆成 argv[]，
//      避免手写 split(" ") 导致的引号/转义错误（如文件名含空格）。
// 注意：本 tokenizer 不展开 $VAR、不处理 `;`/`|`/`&&` —— 这正是设计意图：
//      交给 exec 风格执行器（不经过 sh -c）时不存在注入面。
object ArgumentTokenizer {
    // tokenize(arguments: String): List<String> —— 四态机：NO_TOKEN/NORMAL/SINGLE_QUOTE/DOUBLE_QUOTE
    //   - 单引号：内容原样（shell 语义）
    //   - 双引号：仅 \" 与 \\ 转义
    //   - 普通态：\x 转义任意下一字符
    //   - stringify=true：每个 token 以 "..." 包裹（用于日志/回显）
}
```
**建议接入点**：
1. `TerminalViewModel`/`TerminalRuntime` 的 MCP 工具注册处新增 `shell.split` 工具（返回 token 数组），并让 `cmd` 工具接受 `command_string` 参数（`ArgumentTokenizer.tokenize` 后再走现有数组路径）—— 让外部 agent 传字符串、App 侧安全拆 argv；
2. `SecondStageRunner`/`BootstrapInstaller` 如需从属性/配置文件读命令字符串时复用。

### 7.2 P0：AmSocketServer 与 MCP socket 的差异吸收（不移植协议，移植三点）
1. **SO_PEERCRED 校验**（`LocalSocketManager.getPeerCredNative`，termux-shared:316-323）→ mcp.rs 的 UnixStream accept 后取 `getsockopt(SO_PEERCRED)`，校验 `uid ∈ {Process.myUid(), 0}`，拒绝其他连接并记日志。当前 mcp.rs 靠 socket 文件权限（0700）防外部，但**同 uid 的其它进程**（如 `com.android.shell` 以 shell uid 跑）仍可连——peer 校验是纵深防御。代码注释建议：
   ```rust
   // mcp.rs: accept 后立即校验对端凭证（SO_PEERCRED），仅接受本 uid 与 root；
   // 参照 termux LocalSocketManager.getPeerCredNative —— 防同 uid 越权进程连入。
   ```
2. **能力开关导出环境变量**（`TERMUX_APP__AM_SOCKET_SERVER_ENABLED`，TermuxAppShellEnvironment.kt:156）→ torvox 可在 `shell_env.rs` 注入 `TORVOX__MCP_SERVER_ENABLED`，让 shell 脚本可探测 MCP 是否可用（torvox 目前环境里无此信息）。
3. **sanitizeExitCode**（AmSocketServer.kt:164）→ `Command` 结果出口统一钳制 `0..=255`（Rust `u8` 天然如此，但 JNI 回传 `i32` 时注意；`session.rs` 的 exit_code 已用 u32 透传，保持即可，加注释说明为什么）。

### 7.3 P1：Logger 分块与 Private 级别 → torvox LogUtil 升级
`app/src/main/java/terminal/emulator/util/LogUtil.kt`（或 `TerminalRuntime` 内日志助手）升级：
- `logcat 单条 4068B` 上限：`Log.i(TAG, msg)` 超长会被 logcat 截断且破坏换行；termux 的 `logExtendedMessage`（Logger.kt:49-75）按 `4068 - 前缀 - tag - 4` 计算块长、**优先在 `\n` 处切断**、加 `(1/2)` 前缀。移植约 40 行。
- `logPrivate` 语义（Logger.kt:93-110）：敏感参数（如 postinst 输出的路径含 token？不常见；但 MCP 请求体含用户 prompt）默认只 DEBUG 记。

### 7.4 P1：TerminalExtraKeys 宏语义 → ModifierBar
`ModifierBar.kt` 的按钮配置字符串支持**空格分隔键序列**（移植 `TerminalExtraKeys.onExtraKeyButtonClick` :15-36 的 `isMacro` 分支）：
- 输入 `"CTRL ALT x"` → 依次合成 CTRL_DOWN、ALT_DOWN、X_DOWN/UP（粘滞修饰键：修饰键状态保持到普通键，之后全部释放）；
- 与现有 `ModifierKey`（:52）枚举兼容：先匹配 `CTRL/ALT/SHIFT/FN` token，其余 token 作为字符码。
- 注释建议：`// 键序列宏：空格分隔；修饰键粘滞，遇普通键释放（参照 termux TerminalExtraKeys）`。

### 7.5 P1：BellHandler 防抖 → TerminalRuntime bell 处理
torvox 现在 bell 事件直接播放；移植三点（`BellHandler.kt:34-63`）：
- `MIN_PAUSE` 防抖（3×50ms 内忽略）；
- **主线程 postDelayed(150ms)**（振动前合并快速连响）；
- try/catch 包裹振动调用 + 注释三星 Android 8 NPE（`VibratorService$Vibration.mEffect`）坑。
- 接入点：`TerminalRuntime` 处理 `{"event":"bell"}` 处（约 `TerminalRuntime.kt:1200` 附近）。

### 7.6 P1：ExecutionCommand 状态机语义（exitCode ≠ failed）→ Command 结果模型
torvox `Command`/`SessionManager` 的完成状态目前 `exit_code == 0` 即成功；参照 `ExecutionCommand.ExecutionState`（:19-24）：
- 区分 `exit_code`（进程退出码）与 `err`（App 内部错误：spawn 失败、超时、JNI 失败）；
- 语义注释建议：`// exit_code != 0 只是命令结果，不代表执行框架失败；框架错误（spawn/pty/io）单独记 err —— 参照 termux ExecutionCommand`。
- 好处：MCP `cmd` 工具可以如实返回 `{exit_code: 1, ok: true}`，客户端无需猜。

### 7.7 P1：StreamGobbler 的认知（不移植代码）
Rust 侧 `session.rs` 的 ReaderTask 已天然解决管道死锁；**但**：torvox 任何"用 `std::process::Command` 同步 wait_with_output 之外的裸管道"路径（如 `scripts/` 的 Rust 小工具）应记住 libsuperuser 的警告（StreamGobbler.kt:82-94 注释）：**不尽快读 stdout/stderr → native 进程阻塞在 write → waitFor 死锁**。把这条写进 `docs/` 的"进程管理注意事项"。

### 7.8 P1：ReflectionUtils → torvox 反射封装
torvox 目前散用 `java.lang.reflect`（`TerminalDocumentsProvider` 等）；统一封装 `ReflectionUtils.kt`（~250 行）：
- `invokeField`/`invokeMethod` 永不抛、失败记日志返回默认值；
- `bypassHiddenAPIReflectionRestrictions` 按需在 `TerminalApp.onCreate` 调（依赖 `org.lsposed.hiddenapibypass:hiddenapibypass:4.3`，6 KB，Apache-2.0）—— 为未来读 `Process.pid`/`FileDescriptor.fd` 做准备；
- 注释建议：`// 反射一律不抛异常：Android 厂商 ROM 可能移除字段（参照 termux ReflectionUtils 容错哲学）`。

### 7.9 P2：TermuxInstaller 路径修复白名单 → 文档化备选路线
不移植实现（torvox 用自己的镜像与包名，无此问题），但把 `isTextFileNeedingPathFix`（TermuxInstaller.kt:415-518）与 `setupAptWrapper`/`setupCompatLayer`（:980-1123）作为 **"如果 torvox 未来要复用 termux 官方仓库"** 的预案记录到 `docs/`（含风险：依赖官方包内部路径假设，脆弱）。

### 7.10 P2：transcript 回传 → `terminal_info` 增强
给 MCP `terminal_info` 增加可选 `transcript` 字段：退出时（session.rs `handle_exit` 路径）把屏幕缓冲文本（`ffi.rs:1587` 附近的 `screen_contents` 能力）一并返回。对应 termux `TermuxSession.finish` 的 `mSetStdoutOnExit`（TermuxSession.kt:62）。

### 7.11 P2：`getCwd`/peer 细节
- `TerminalSession.getCwd`（:262，读 `/proc/$pid/cwd` 符号链接）→ torvox `terminal_info` 的 cwd 字段目前来自哪？（`session.rs` 记录 spawn 时 cwd；若进程 chdir 过则失准）—— 可加"读 /proc 刷新 cwd"的选项。
- `TermuxShellUtils.clearTermuxTMPDIR`（:88-127）→ torvox 暂无 `$TMPDIR` 清理策略，可加 `delete-tmpdir-files-older-than-x-days-on-exit` 同款设置。

---

## 8. 项目文档吸收价值

| termux-kotlin 文档 | 内容 | 对 torvox 的价值 |
|---|---|---|
| `README.md`（19 KB） | 定位/兼容策略/roadmap 摘要 | 低（产品路线不同） |
| `ARCHITECTURE.md`（25 KB） | 模块职责、数据流、插件架构 | **高**：torvox 的 `docs/architecture.md` 可补充"命令总线/会话管理"章节（对照 ExecutionCommand 模型）；其"如何新增一个 runner"的叙述结构值得模仿 |
| `CHANGELOG.md`（56 KB） | 版本演进（含 Kotlin 转换与 fork 特性时间线） | 中：了解"全量重写"迁移策略（分模块、先 shared 后 app、JVM 兼容性陷阱） |
| `ROADMAP.md`（11.8 KB） | 计划 | 低 |
| `CONTRIBUTING.md` / `SECURITY.md` | 贡献/安全 | 低 |
| `docs/`（仓库内文档目录） | 构建/调试/发布 | 中：`debug.sh`（脚本）与 crash 处理（`error.md` 75 KB 错误记录）值得一看 |
| `site/`（docs 站点源码） | 文档站点 | 低 |
| fork 特有 `agents/` Python 遗留 | agentd.py 等 | 低（已被 Kotlin 取代；torvox 的 MCP 路线更现代） |
| **代码内注释** | 大量"为什么"注释（StreamGobbler 死锁、Binder 截断、三星 NPE、dpkg 路径、逗号替代字符） | **最高价值**：torvox 移植时保留注释（见 §7 各条"注释建议"） |

**具体建议**：在 `torvox/docs/reference/` 下维护"termux 兼容性备忘"（或并入本文档 §7），列出：逗号替代字符方案（RunCommandService.kt:89-96，`am` 吃逗号的坑，torvox 的 `cmd` 工具若未来支持 `am` 包装需注意）、`termux.env` 作为安装完成标志的约定（BootstrapInstaller 已用，保持）、`LD_PRELOAD` 方案的风险说明。

---

## 9. 结论

1. **termux-kotlin 是"全量 Kotlin 重写 + 激进 fork 特性"的参考仓库**：其终端技术栈（Kotlin VT 状态机、Canvas 渲染、JNI pty）全面落后于 torvox（Rust + libghostty-vt + wgpu），**无终端核心吸收价值**；其价值集中在 **shell 命令工程与生态兼容**。
2. **最值得吸收的五件事**（按性价比）：① `ArgumentTokenizer` → postinst/命令字符串解析（torvox 真实缺口，P0）；② `SO_PEERCRED` 校验 + 能力开关环境变量 → mcp.rs（P0）；③ `Logger` 分块/Private → LogUtil（P1）；④ `TerminalExtraKeys` 宏语义 → ModifierBar（P1）；⑤ `ExecutionCommand` 的 exitCode/errCode 双轨语义 → Command 结果模型（P1）。
3. **依赖评估**：torvox 不应引入 termux 的任何 Gradle/运行时依赖（JNI C 库、HiddenApiBypass 按需、Hilt 不要）；termux 的"路径修复三件套"是特定于 com.termux 包名的 hack，torvox 不适用，仅文档化。
4. **MCP 对比结论**：torvox 的 Rust MCP（tower-mcp，JSON-RPC）在协议层面全面优于 termux 的 am socket（裸字符串 + `\0` 分帧），**保持现状**；吸收的是安全校验与状态导出的工程细节。
5. **文档层面**：`ARCHITECTURE.md` 的模块/数据流叙述与代码注释的"为什么"文化是 torvox docs 可借鉴的模板；已完成的 `research-termux-app.md` 覆盖 Java 上游，本文档覆盖 Kotlin fork，两者结合即完整的 termux 参考面。

---
