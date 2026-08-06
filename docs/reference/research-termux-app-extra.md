# 深度研究（补充）：termux-app v0.119.0-beta.3 全模块

> 研究日期：2026-08-06 | 克隆位置：`repositories/refs/termux-app`（depth 1，branch `github-releases/v0.119.0-beta.3`）
> 本文档是 `research-termux-app.md` 的**补充**：前者只覆盖 `terminal-view` 的文本选择系统（TextSelectionCursorController），本文完整覆盖 `terminal-emulator/`、`termux-shared/`、`app/`、`docs/` 全部其余模块，并与 torvox 逐项对比。
> 语言：Java（终端核心，197 个 Java/Kotlin 文件）；torvox 侧对照文件为 Rust（native/）与 Kotlin（android/）。

## 0. 版本差异说明（重要）

任务描述中提到的若干文件在 **v0.119.0-beta.3 已不存在**，其职能已被重构拆分，研究时必须按当前结构理解：

| 旧版文件 | 当前版本去向 |
|---|---|
| `TerminalIO.java`（terminal-emulator） | 消失。输入输出队列内化为 `TerminalSession` 的两个 `ByteQueue`（`mProcessToTerminalIOQueue` / `mTerminalToProcessIOQueue`，TerminalSession.java:44-49） |
| `Term.java`（termux-shared） | 消失。transcript/选择文本提取逻辑在 `TerminalBuffer.getSelectedText()`（terminal-emulator）+ `ShellUtils.getTerminalSessionTranscriptText()`（termux-shared）；app shell 执行逻辑在 `AppShell.java` |
| `TermuxPreferences.java`（app） | 改为 `TermuxAppSharedPreferences`（termux-shared，纯数据层）+ `TermuxPreferencesFragment`（app，UI 层）+ `TermuxAppSharedProperties`（termux.properties 文件层，三层分离） |
| `termux-exec` C 源码 | **不在本仓库**。它是独立仓库 `termux/termux-exec`，termux-app 只通过 `LD_PRELOAD=$PREFIX/lib/libtermux-exec.so` 环境变量使用它（机制见 §2.8） |

## 1. terminal-emulator 模块（终端核心，纯 Java）

### 1.1 模块定位

除 `android.system.Os` 外零 Android 依赖的**可独立移植终端模拟器**（unit test 在 JVM 上直接跑）。数据流单向：

```
PTY master fd ──读线程──> ByteQueue ──主线程 Handler──> TerminalEmulator.append()
                                                              │ 逐字节
                                                              ▼
                                              processByte() → processCodePoint() → emitCodePoint()
                                                              │
                                                              ▼
                                              TerminalBuffer（双缓冲：mMainBuffer / mAltBuffer）
```

模块文件清单（`terminal-emulator/src/main/java/com/termux/terminal/`）：

| 文件 | 行数 | 职责 |
|---|---|---|
| `TerminalEmulator.java` | ~3700 | VT/xterm 转义序列状态机（核心） |
| `TerminalBuffer.java` | 497 | 环形转录缓冲（screen + scrollback） |
| `TerminalRow.java` | ~283 | 单行单元格存储（文本+样式） |
| `WcWidth.java` | ~600 | wcwidth(3) for Unicode 15（表驱动） |
| `TerminalSession.java` | 373 | PTY 会话：3 线程 I/O + 主线程回调 |
| `ByteQueue.java` | 108 | 单生产者单消费者环形字节队列 |
| `JNI.java` / `jni/termux.c` | 42 / ~218 | PTY 子进程创建（fork+execvp） |
| `KeyHandler.java` | ~500 | 物理键盘 → 转义序列 |
| `TextStyle.java` / `TerminalColors.java` / `TerminalColorScheme.java` | 90 / 113 / ~200 | 样式编码、调色板 |
| `TerminalOutput.java` / `TerminalSessionClient.java` | 32 / 50 | 回调接口 |
| `Logger.java` | ~60 | 日志 |

### 1.2 TerminalEmulator.java —— VT 状态机

**架构**：单线程逐字节驱动，全部状态机用 int 常量表达（`ESC_*`，:45-89），无正则、无流式解析器。

关键方法（文件:行号）：

| 方法 | 行号 | 说明 |
|---|---|---|
| `TerminalEmulator(...)` 构造 | :328-330 | `mScreen = mMainBuffer = new TerminalBuffer(columns, transcriptRows, rows)`；`mAltBuffer = new TerminalBuffer(columns, rows, rows)`（alt buffer 无 scrollback） |
| `append(byte[], len)` | :500-503 | 入口：逐字节喂给 processByte |
| `processByte(byte)` | :505-568 | **手写 UTF-8 解码器**（含 overlong 检测 :515、C1 控制符丢弃 :523、非法序列→U+FFFD :541/563） |
| `processCodePoint(int)` | :570-626 | 控制字符分发（NUL/BEL/BS/HT/LF…）+ 转义状态机跳转 |
| `emitCodePoint(int)` | :2348-2496 | 落格：line drawing 映射（:2350-2453）、`WcWidth.width`、自动换行（`mAboutToAutoWrap` 延迟换行 :2459-2493）、insert mode |
| `resize(...)` | :386-422 | 记录新尺寸→`resizeScreen()`；行列任一变才重排 |
| `resizeScreen()` | :416-422 | 列变化时把主缓冲整屏重排（`TerminalBuffer.resize` :203 快/慢路径） |
| `doCsi(int)` | :1529-1983 | CSI 主分发（约 450 行，最大分支） |
| `doCsiQuestionMark(int)` | :1102-1285 | DECSET/DECRST：位标志映射 `mapDecSetBitToInternalBit` :296 |
| `doEsc(int)` | :1407-1528 | ESC 序列 |
| `doOsc(int)` | :1984-2013 | OSC（标题/剪贴板/颜色）；`MAX_OSC_STRING_LENGTH=8192` :95 |
| `doDeviceControl(int)` | :918-1042 | DCS（DECRQSS 应答等） |
| `doApc(int)` | :1043-1052 | APC（自定义协议，terminal-view 用来传光标样式） |
| `doLinefeed()` | :1373-1394 | LF 处理（含 scroll region 判定） |
| `scrollDownOneLine()` | :2206-2217 | 行滚动：有水平 margin 用 `blockCopy`，否则 `mScreen.scrollDownOneLine`（进 scrollback） |
| `parseArg(int)` | :2238+ | CSI 参数解析，支持 `;` 分隔 + `:` 子参数（:2220-2237 注释引用 alacritty/vte#22） |
| `setDecsetinternalBit` | :280-295 | DECSET 位（autowrap/alt screen/mouse tracking…） |
| `sendMouseEvent(...)` | :365-384 | SGR 鼠标上报编码 |
| `isAlternateBufferActive()` | :351-353 | `mScreen == mAltBuffer` |

**要点**：
1. **双缓冲**：`mMainBuffer`（transcript 2000 行默认，可配 100-50000，:147-149）与 `mAltBuffer`（无历史），切换点 `doCsiQuestionMark` :1254-1272；`SavedScreenState` 保存/恢复光标与滚动区（:1502/:1516）。
2. **滚动计数**：`mScrollCounter` 递增统计（scrollDownOneLine :2207）。
3. **delayed wrap**：光标在最后一列时并不立即换行，记 `mAboutToAutoWrap`，下一个可打印字符才真正换行——这是 `BS` 回退上一行（processCodePoint :590-599）的基础。
4. `mClient`（TerminalOutput 回调）在 emulator 内直接触发 `onBell`/`onClipboardText` 等（如 :587）。

### 1.3 TerminalBuffer.java —— 环形转录缓冲（滚动/选择/宽字符核心）

**架构**：定长环形数组 `TerminalRow[] mLines`（总行数 = transcript + screen），`mScreenFirstRow` 指向屏幕首行，`mActiveTranscriptRows` 记录历史行数。外部坐标：`-mActiveTranscriptRows … mScreenRows-1`（0 为屏幕顶）；内部坐标经 `externalToInternalRow` 换算。

```java
// 外部 ↔ 内部坐标映射（TerminalBuffer.java:176-181）
public int externalToInternalRow(int externalRow) {
    final int internalRow = mScreenFirstRow + externalRow;
    return (internalRow < 0) ? (mTotalRows + internalRow) : (internalRow % mTotalRows);
}
```

| 方法 | 行号 | 说明 |
|---|---|---|
| `getTranscriptText*()` | :40-50 | 三种拼接模式：默认 / 不拼接软换行 / 全文拼接 |
| `getSelectedText(x1,y1,x2,y2,...)` | :52-106 | **选择文本提取核心**（详见下） |
| `getWordAtLocation(x,y)` | :108-145 | 单词级选择：先取整条 wrap 行的文本再按空格切词（:134-144） |
| `externalToInternalRow` | :176-181 | 坐标换算（注释含完整 ASCII 图示 :155-175） |
| `setLineWrap/getLineWrap/clearLineWrap` | :183-193 | 软换行标志（行末自动 wrap 记在**上一行**上） |
| `resize(newCols,newRows,newTotalRows,cursor,style,altScreen)` | :203-360 | 快路径（仅行数变，:205-232，跳过底部空行/从 transcript 补行）；慢路径（列数变，:233-360，逐字符重放整个屏幕） |
| `scrollDownOneLine(top,bottom,style)` | :384-418 | 滚动区内下移一行：最上行释放进 transcript（`mScreenFirstRow` 前移），空行 `allocateFullLineIfNecessary` 复用 |
| `blockCopy(sx,sy,w,h,dx,dy)` | :420-436 | 矩形块拷贝（滚动区部分滚动、CSI 插入/删除行） |
| `blockSet(sx,sy,w,h,val,style)` | :437-446 | 矩形块填充（清屏/擦除） |
| `allocateFullLineIfNecessary(row)` | :447-486 | 行复用：null 行才新建，否则 clear |
| `clearTranscript()` | :487-495 | 清历史（数组区间置 null） |
| `getActiveTranscriptRows/getActiveRows` | :147-153 | 历史行数 / 总行数 |

**getSelectedText 的宽字符与换行语义（:60-106，torvox 对比锚点）**：
1. 列→字符索引用 `lineObject.findStartOfColumn(x)`（TerminalRow:92），**宽字符首列选取自动扩展到整字符**（:77-82：`x2Index == x1Index` 时取下一列起点）；
2. 行尾处理：`lastPrintingCharIndex` 找到最后非空格字符；**若该行是软换行（wrap），保留尾部空格**（:87-89），否则裁掉——保证 wrap 行拼接时无空格、非 wrap 行不拖尾空格；
3. 行间分隔：`joinBackLines`（wrap 行不插 `\n`）与 `joinFullLines`（整行占满才不插 `\n`）两个开关组合出三种 transcript 模式（:101-103）。

### 1.4 TerminalRow.java —— 单元格存储（宽字符模型）

- `char[] mText`（容量 1.5×列数，:12/:56）+ `long[] mStyle`（每列一个 8 字节样式，:49）；
- **列 ≠ 字符索引**：宽字符/组合字符按"列"记账，`findStartOfColumn(column)`（:92-120）线性扫描列宽并跳过组合字符；
- `setChar(columnToSet, codePoint, style)` :152：写入时处理宽字符第二半（占位）、超长裁切；
- 组合字符上限 `MAX_COMBINING_CHARACTERS_PER_COLUMN = 15`（:38，防恶意输入内存攻击，注释引用 Unicode Stream-Safe 30 上限）；
- `mHasNonOneWidthOrSurrogateChars` 标志（:51）：整行全宽 1 字符时走快速路径（`copyInterval` :62-85 中优化）。

### 1.5 WcWidth.java —— wcwidth(3)（Unicode 15）

表驱动实现（移植自 jquast/wcwidth）：`ZERO_WIDTH` 区间表 :19、`WIDE_EASTASIAN` :368，`width(int ucs)` :514 与 `width(char[], index)` :536（代理对感知）。**必须与 C 侧 libandroid-support/wcwidth 保持同步**（头注释 :8-12）——这是 termux 宽字符一致性的来源。

### 1.6 TerminalSession.java —— PTY 会话与线程模型

```java
// TerminalSession.java:123-172 initializeEmulator()
mTerminalFileDescriptor = JNI.createSubprocess(mShellPath, mCwd, mArgs, mEnv, processId, rows, columns, cellWidth, cellHeight);
new Thread("TermSessionInputReader") {  // :133  PTY → mProcessToTerminalIOQueue → MSG_NEW_INPUT
    int read = termIn.read(buffer);     // 阻塞读 4KB
    mProcessToTerminalIOQueue.write(...); mMainThreadHandler.sendEmptyMessage(MSG_NEW_INPUT);
}
new Thread("TermSessionOutputWriter") { // :150  mTerminalToProcessIOQueue → PTY
    int bytesToWrite = mTerminalToProcessIOQueue.read(buffer, true); // 阻塞等
    termOut.write(buffer, 0, bytesToWrite);
}
new Thread("TermSessionWaiter") {       // :166  waitpid → MSG_PROCESS_EXITED
    int processExitCode = JNI.waitFor(mShellPid);
    mMainThreadHandler.sendMessage(MSG_PROCESS_EXITED, processExitCode);
}
```

要点：
1. **延迟启动**：构造（:82-89）只存参数；`updateSize()`（:103-110）首次调用时才 `initializeEmulator`（创建子进程 + 起线程），之后每次 resize 只 `JNI.setPtyWindowSize` + `mEmulator.resize`；
2. **双 ByteQueue 解耦**：读线程与主线程、主线程与写线程之间零锁等待（ByteQueue 内部 synchronized + wait/notify）；
3. **主线程回调**：`MainThreadHandler` 处理 `MSG_NEW_INPUT`（把队列字节喂给 emulator 后 `notifyScreenUpdate()` → `mClient.onTextChanged`）与 `MSG_PROCESS_EXITED`（写 `[Process completed (code N) - press Enter]` 到屏幕 :355-365）；
4. `write(byte[],off,count)` :178 与 `writeCodePoint(prependEscape, codePoint)` :183-217（手写 UTF-8 编码，含 ESC 前缀用于粘贴模式）供 UI 输入；
5. `finishIfRunning()` :235：`Os.kill(pid, SIGKILL)`；
6. `mHandle = UUID`（:36）：Activity 用 handle 在 preference 中持久化"当前会话"。

### 1.7 ByteQueue.java —— 环形队列

`synchronized` 单锁环形缓冲（4KB 容量，:44），`read(block)` 支持阻塞/非阻塞，`close()` 后 `read` 返回 -1（写线程退出信号），满/空时 `notify()` 唤醒对侧（:20-52/:59-107）。生产/消费各一条线程，是"零拷贝转发"的关键。

### 1.8 JNI.java + jni/termux.c —— PTY 子进程（Android 版 fork/exec）

`termux.c create_subprocess()`（:25-115）流程：
1. `open("/dev/ptmx", O_RDWR|O_CLOEXEC)` :36 → `grantpt/unlockpt/ptsname_r` :44-49；
2. termios：**`IUTF8` 置位、`IXON|IXOFF` 清除**（:54-59，注释明确：防 Ctrl+S 锁死显示——Android mksh 的坑）；
3. `TIOCSWINSZ` 设初始尺寸（ws_xpixel=cols×cellWidth，:62-63）；
4. `fork()` :65；父进程返回 ptm fd；
5. **子进程**：`sigprocmask(SIG_UNBLOCK)` 解除 Java 进程屏蔽的信号 :73-75（Android 主线程常屏蔽信号）、`setsid()` :78、`dup2(pts,0/1/2)` :83-85、**遍历 /proc/self/fd 关闭所有多余 fd** :87-96、`clearenv()+putenv` :98-99、`chdir(cwd)` :101-107、`execvp(cmd, argv)` :108；
6. `waitFor`（:117-213）：`waitpid` + `WEXITSTATUS`/`WTERMSIG` 编码（≥0 退出码，<0 信号取反）；
7. `JNI.java`：`createSubprocess` native :23、`setPtyWindowSize` :29、`waitFor` :36、`close` :39。

### 1.9 TextStyle.java —— 64 位样式编码

- 位布局（头注释 :8-13）：16 位标志 + 24 位前景 + 24 位背景；11 个标志：BOLD/ITALIC/UNDERLINE/BLINK/INVERSE/INVISIBLE/STRIKETHROUGH/PROTECTED/DIM/TRUECOLOR_FG/TRUECOLOR_BG（:17-37）；
- 索引色 9 位（256 色 + 256/257 表示默认前/背景，:39-40），真彩色 24 位；
- `encode` :54、`decodeForeColor` :69、`decodeBackColor` :78、`decodeEffect` :86。

### 1.10 KeyHandler.java

物理键盘键码→转义序列映射（含 CTRL/ALT 组合、功能键、数字键盘应用模式），与 `TerminalEmulator.isKeypadApplicationMode` 联动。torvox 有对应 `TerminalInputEncoder`（Kotlin），两者思路相同，无特殊吸收价值。

### 1.11 测试套件（terminal-emulator/src/test）

13 个 JUnit 测试类（TerminalTest、ScreenBufferTest、ScrollRegionTest、ResizeTest、WcWidthTest、UnicodeInputTest、TerminalRowTest、TextStyleTest…），**纯 JVM 可跑**——termux 终端核心可独立验证的工程实践，torvox 的 vt_conformance.rs（98KB）思路类似但更激进。

## 2. termux-shared 模块（共享库）

### 2.1 定位

`com.termux.shared`：供 app 与 7 个插件（api/float/tasker/widget/boot/styling）共用的**常量、工具、偏好、shell 执行框架**。torvox 单应用无此需求，但其中的路径常量体系与 shell 环境构建值得参考。

### 2.2 TermuxConstants.java（84KB，路径体系）

关键常量（TermuxConstants.java:588-600 附近）：
- `TERMUX_FILES_DIR_PATH = /data/data/com.termux/files`（:588）
- `TERMUX_PREFIX_DIR_PATH = .../files/usr`（:595，即 $PREFIX）
- `TERMUX_HOME_DIR_PATH`、`TERMUX_TMP_PREFIX_DIR_PATH`、`TERMUX_ENV_FILE_PATH`/`TERMUX_ENV_TEMP_FILE_PATH`（termux.env 原子写，见 §2.4）
- **fork 指南**在类 javadoc（:294-295）：binaries 硬编码 $PREFIX，换包名必须重编 bootstrap——torvox 用 `files/usr` 同构前缀，受益于同一约定。

### 2.3 TermuxBootstrap.java

`TERMUX_APP_PACKAGE_VARIANT` / `TERMUX_APP_PACKAGE_MANAGER`（:21-24）由 BuildConfig 字段注入（:27-44）：变体如 `apt-android-7`。bootstrap 与包管理器的**变体矩阵**决定 PATH/LD_LIBRARY_PATH 行为（§2.4 :100-108）。

### 2.4 TermuxShellEnvironment.java —— 子进程环境

`getEnvironment()`（:68-112）：Android 基础环境 + Termux 项：
- `PREFIX`/`TERMUX__PREFIX`、`HOME`/`TERMUX__HOME`、`TERMUX__ROOTFS_DIR`、`TERMUX__APPS_DIR`（:89-95）；
- 非 failsafe：`TMPDIR=$PREFIX/tmp`、`PATH=$PREFIX/bin`（apt-android-7，:106）；**Android 5 变体额外 `LD_LIBRARY_PATH=$PREFIX/lib`**（:100-103）——Android 7+ 依赖 DT_RUNPATH 故不设；
- `writeEnvironmentToFile()`（:46-63）：**temp 文件 + rename 原子写** `termux.env`（shell 侧可 source）。

### 2.5 ShellUtils.java —— transcript 提取（旧 Term.java 职能）

- `getPid(Process)` :20-32：反射取 `Process.pid`（Android 无公开 API 的绕法）；
- `setupShellCommandArguments` :36-44：可执行 + 参数拼接；
- `getTerminalSessionTranscriptText(session, linesJoined, trim)`（:52-74）：`terminalEmulator.getScreen()` → `getTranscriptTextWithFullLinesJoined()` / `WithoutJoinedLines()` → trim。**这就是旧 Term.java 的 transcript 逻辑落点**。

### 2.6 TermuxSession.java（termux-shared，runner 层）

`execute(...)`（:77-285）：
1. 可执行文件选择：未指定时按 `LOGIN_SHELL_BINARIES` 在 $PREFIX/bin 探测（:96-103），失败回退 `/system/bin/sh`（failsafe，:105-114），命中则 `-l` 登录 shell；
2. 组装环境（TermuxShellEnvironment）+ `TerminalSession` 构造；
3. 与 `ExecutionCommand`（插件/任务框架）绑定；`setStdoutOnExit` 时退出回调携带 transcript（:68-74 注释）。

### 2.7 TermuxAppSharedPreferences.java —— 偏好中心（旧 TermuxPreferences 职能）

- 单例 `build(context, exitAppOnError)` :44-61（包名校验失败弹窗退出）；
- 关键项：`getFontSize/setFontSize` :154-159、`getCurrentSession/setCurrentSession` :174-181（**会话 handle 持久化**）、`getLogLevel` :184、`getAndIncrementTerminalSessionNumberSinceBoot` :215（环境变量 SHELL_CMD__APP_TERMINAL_SESSION_NUMBER_SINCE_BOOT 用）。
- 三层分离：`TermuxAppSharedProperties`（termux.properties 文件）↔ `TermuxAppSharedPreferences`（SharedPreferences）↔ `TermuxPreferencesFragment`（UI）。

### 2.8 termux-exec 机制（独立仓库，本仓库只负责启用）

**原理**：bootstrap 安装 `$PREFIX/lib/libtermux-exec.so`；子进程环境带 `LD_PRELOAD=$PREFIX/lib/libtermux-exec.so`（本仓库唯一相关代码在 termux-packages 的 profile 脚本，termux-app 侧无 Java 代码）；该 so 用 `dlsym(RTLD_NEXT, "execve")` 拦截 execve，**当可执行文件不在 PATH 中时自动补 `$PREFIX/bin`**，解决 `#!/data/data/com.termux/files/usr/bin/env bash` 这类 shebang 找不到解释器的问题。
torvox 已自行实现等效机制（见 §5.4 对比与 §7 建议）。

### 2.9 其他

- `local-socket.cpp`（23KB）：本地 unix socket JNI（旧插件通信遗留，非核心）；
- `TermuxTerminalSessionClientBase.java`：shared 层会话客户端默认实现（日志/剪贴板回调基类）；
- `TermuxUtils`（37KB）：安装信息、apt 源检测脚本（res/raw/apt_info_script.sh，@TERMUX_PREFIX@ 占位替换 :605）；
- `FileUtils.java`（104KB）：超大全能文件工具（含 `directoryFileExists` 等语义），体量过大不宜照搬。

## 3. app 模块（主应用）

### 3.1 TermuxService.java —— 前台服务 + 会话容器（多会话核心）

| 方法 | 行号 | 说明 |
|---|---|---|
| `onCreate` | :111-123 | `TermuxShellManager.getShellManager()`（进程级单例，mTermuxSessions 列表）、`runStartForeground()` |
| `runStartForeground` | :204-207 | `startForeground(TERMUX_APP_NOTIFICATION_ID, buildNotification())`——**常驻前台保活** |
| `onStartCommand` | :127-166 | ACTION_STOP_SERVICE/WAKE_LOCK/WAKE_UNLOCK/SERVICE_EXECUTE 分发；返回 `START_NOT_STICKY`（:165，被杀不自动重启） |
| `onDestroy` | :169-183 | 清 TMPDIR、释放锁、`killAllTermuxExecutionCommands()`（:263，SIGKILL 全部会话/任务） |
| `createTermuxSession(ExecutionCommand)` | :575-626 | → `TermuxSession.execute(...)`（:594）→ 加入 `mShellManager.mTermuxSessions` → 通知 Activity client + 更新通知 |
| `removeTermuxSession` | :629+ | 按索引移除 |
| `onTermuxSessionExited` | :640+ | 会话退出清理 |
| `handleSessionAction` | :684-701 | 切换/新建会话动作 |
| `getTerminalSessionForHandle` | :919-927 | handle → 会话（Activity 恢复用） |

要点：**会话容器在 Service 而非 Activity**——Activity 销毁重建（旋转/后台回收）后会话仍在；`onUnbind`（:192-201）同时清理 activity client 引用防泄漏。

### 3.2 TermuxActivity.java —— 生命周期编排

| 回调 | 行号 | 关键动作 |
|---|---|---|
| `onCreate` | :198-280 | 读 properties → `setActivityTheme()` → setContentView → `TermuxAppSharedPreferences.build`（:220）→ `setTermuxTerminalViewAndClients()` :244 → 工具栏/按钮 :246-252 → **`startService` + `bindService`**（:260-266，异常时 toast 提示"app is in background"）→ 发 BROADCAST_TERMUX_OPENED :279 |
| `onStart` | :283-302 | `mIsVisible=true`；通知两个 client（session client 恢复当前会话/视图 client 恢复状态）；注册广播接收器 |
| `onResume` | :305-320 | client.onResume（terminal view 恢复渲染、IME 状态） |
| `onStop` | :326-346 | client.onStop（停渲染/回收资源） |
| `onDestroy` | :348-367 | `unbindService` |
| `onSaveInstanceState` | :369-375 | 存 `ARG_ACTIVITY_RECREATED=true`（重建后不重复加会话） |
| `onServiceConnected` | :387-430 | **关键**：`setTermuxSessionsListView()` → 若 `mTermuxService.isTermuxSessionsEmpty()` 且可见 → `TermuxInstaller.setupBootstrapIfNeeded(activity, () -> addNewSession(failsafe, null))`（:397-410）——**bootstrap 完成后才建第一个会话**；最后把 activity client 挂到 service（:429） |
| `onServiceDisconnected` | :433-438 | 尊重用户从通知栏"停止"→ finish |
| `getCurrentSession` | :893-897 | `mTerminalView.getTermSession()` |

要点：Activity 生命周期 = **与 Service 的绑定生命周期**；"重建"与"首次启动"用 `mIsActivityRecreated` + `isTermuxSessionsEmpty()` 区分，避免重复建会话。

### 3.3 TermuxInstaller.java —— bootstrap 安装（含 getZip）

流程（类 javadoc :45-63 与代码一致）：
1. **前置校验**：files 目录可访问（:75）、主用户检查（:80，Android N+ 多用户）、SDK 变体兼容（:109，`checkIfMinOrMaxSdkVersionIsIncompatible` :282-316）；
2. **幂等**：$PREFIX 存在且非空 → 直接 `whenDone.run()`（:116-121）；
3. 后台线程（:128-279）：
   - 删 staging 与 prefix（:137-148）→ 重建两个目录（:151-162）；
   - `loadZipBytes()`（:454-458，`System.loadLibrary("termux-bootstrap")` + `getZip()`）→ **`ZipInputStream` 流式解压**（:170-216）：`SYMLINKS.txt` 先收集（:173-179），其余条目写入 staging，`bin/`、`libexec/`、`lib/apt/*`、second-stage 脚本 `Os.chmod(0700)`（:207-212）；
   - `Os.symlink` 全部链接（:220-222）→ `TERMUX_STAGING_PREFIX_DIR.renameTo(TERMUX_PREFIX_DIR)`（:226-228，**staging 原子换入**）；
   - **second stage**：执行 `$PREFIX/etc/termux/bootstrap/termux-bootstrap-second-stage.sh`（:231-257，AppShell runner），失败则删 prefix 报错；
   - `TermuxShellEnvironment.writeEnvironmentToFile`（:262）重建 termux.env；
4. 错误路径：`showBootstrapErrorDialog`（:318-340）"重试"= 删 prefix 重装；崩溃通知 `sendBootstrapCrashReportNotification`（:342+）。

```java
// TermuxInstaller.java:454-461 —— zip 从 native 库按 ABI 取
public static byte[] loadZipBytes() {
    System.loadLibrary("termux-bootstrap");
    return getZip();
}
public static native byte[] getZip();
```

### 3.4 app/src/main/cpp/（bootstrap zip 内嵌）

- `termux-bootstrap.c`（11 行）：`getZip` JNI 返回 `blob` 数组（:6-10）；
- `termux-bootstrap-zip.S`（18 行）：`.incbin "bootstrap-<abi>.zip"`（i686/x86_64/aarch64/arm 四选一，:4-15），`blob_size = 1b - blob`（汇编期算长度）——**zip 以原始字节嵌入 .so，零解压成本**。app/build.gradle 负责下载对应 ABI 的 bootstrap zip 并重命名。

### 3.5 TermuxTerminalSessionActivityClient.java —— 多会话 UX 逻辑

| 方法 | 行号 | 说明 |
|---|---|---|
| `onCreate/onStart/onResume/onStop` | :60/:68/:85/:95 | 随 Activity 生命周期转发（onStart 恢复 stored session :72-83） |
| `addNewSession(isFailSafe, name)` | :364-389 | 上限 `MAX_SESSIONS` 弹窗（:368-370）；**工作目录继承当前会话 cwd**（:374-379）；`service.createTermuxSession` → `setCurrentSession` → 关抽屉 |
| `setCurrentStoredSession` | :391-397 | 当前 handle 写 preference |
| `getCurrentStoredSession` | :419-431 | preference handle → `service.getTerminalSessionForHandle` |
| `onSessionFinished` | :139-183 | 自动切换到下一会话；退出码 0/130 静默，否则 toast；LEANBACK 特判 |
| `onTextChanged` | :118-123 | 转 `mActivity.refreshTerminalView()`（当前会话才刷） |
| `onTitleChanged` | :125-137 | 非当前会话也更新列表标题 |
| `onBell` | :200-217 | SoundPool 铃声（bell behaviour 配置） |

### 3.6 TerminalView.java 滚动与渲染刷新（补充选择研究之外的视图层）

| 方法 | 行号 | 说明 |
|---|---|---|
| `onScreenUpdated(boolean skipScrolling)` | :453-502 | 数据刷新入口：**选择中或 `isAutoScrollDisabled()` 时不动 topRow**（:463-473）；否则回到底部（`mTopRow != 0` 才 invalidate，:484） |
| `doScroll(event, rowsDown)` | :574-591 | 鼠标滚轮：`mTopRow` 加减 |
| `getTopRow/setTopRow` | :1051-1064 | **mTopRow ≤ 0**（0=底部，负值=上滚），clamp 到 `-getActiveRows()+mRows` |
| `onScroll`（GestureDetector） | :170-187 | 手指拖动滚动（非选择态） |
| `onFling` | :197-231 | 惯性滚动（`mScroller`，:210-230 post 动画） |
| `getColumnAndRow` | :546-554 | 触摸点→(列,行)，`relativeToScroll` 时叠加 `mTopRow` |
| `attachSession` | :290-305 | 会话↔视图绑定，重建 emulator 引用 |
| `onGenericMotionEvent` | :593-604 | 鼠标/触控板滚动 |

### 3.7 其他 app 组件（简要）

- `TermuxApplication.java`（3.6KB）：初始化 logger、SharedProperties、崩溃处理；
- `RunCommandService.java`（17KB）：`am startservice` 执行命令的入口（widget/外部调用）；
- `TermuxOpenReceiver.java`（9.5KB）：`com.termux.RUN_COMMAND`/打开 URL 的广播；
- `TermuxTerminalViewClient.java`（35KB）：视图回调实现（工具栏/extra keys/上下文菜单/手势路由）；
- `TermuxActivityRootView.java`（15.6KB）：insets/软键盘避让（WindowInsetsListener）；
- `TermuxSessionsListViewController.java`（4.6KB）：抽屉会话列表 RecyclerView。

## 4. docs/ 目录

`docs/en/index.md` 仅为占位（指向 wiki）。实质文档：
- `README.md`：安装源差异（F-Droid 通用 APK vs GitHub 分 ABI，:76-90）、**Android 12+ phantom process 警告**（:20，`[Process completed (signal 9)]`）、fork 指南（:261-266）；
- 其余在 GitHub wiki（TermuxConstants javadoc 链接）。

## 5. 功能对比：torvox 有没有？

### 5.1 TerminalBuffer 的滚动/选择/宽字符处理 —— **torvox 有（架构不同）**

| 维度 | termux | torvox |
|---|---|---|
| 缓冲结构 | 环形 `TerminalRow[]` + 外部坐标（-transcript…screen，TerminalBuffer.java:13-21/:176） | ghostty grid（`GhosttyTerminal`，session.rs:378-386，scrollback_lines 可配）；rust 侧无外部坐标概念 |
| 滚动实现 | 屏内滚动改 `mScreenFirstRow`（TerminalBuffer.scrollDownOneLine :384，行对象复用） | ghostty grid scrollback 原生 |
| 滚动视图状态 | `mTopRow` 负值，0=底部（TerminalView.java:1051-1064） | `scrollOffset` 正值，0=底部；`gridRow = scrollbackLen - scrollOffset + row`（TerminalSurface.kt:1110/:1294/:1378） |
| 选择文本提取 | `getSelectedText`（TerminalBuffer.java:52-106）：列模型 + `findStartOfColumn` 宽字符吸附 + wrap 标志拼接 | `TerminalViewModel.extractSelectedText`（TerminalViewModel.kt:476-556）：逐行 `bridge.scrollbackLine(row)` **字符串** + 手动 `\n` 拼接 + `smartJoinLines`（:558-583，URL 续行拼接） |
| 宽字符 | 列↔字符双向换算（TerminalRow.findStartOfColumn :92、setChar :152），选区起点吸附到整字符（TerminalBuffer.java:77-82） | ghostty 内部按列管理；**scrollbackLine 返回字符串后列号与 char 索引可能错位**——代码注释已自认 soft-wrap 检测缺口（TerminalViewModel.kt:484-488） |
| 单词选择 | `getWordAtLocation`（TerminalBuffer.java:108-145）：wrap 行文本 + 空格切词 | `SelectionExpander`（Kotlin，URL/引号/语义扩展，功能超集） |
| 软换行拼接 | wrap 行不插 `\n`、保留尾空格（:87-103） | 每行固定插 `\n`（TerminalViewModel.kt:553），**缺口**（注释 :487-488） |

结论：**torvox 缺 termux 的"列→字符索引"双向换算与 wrap 感知拼接**。影响：选择含宽字符（CJK）的行时,`extractSelectedText` 的 `substring(col)` 会切在 char 边界而非列边界（子串可能多/少字符）。

### 5.2 TerminalSession 的 PTY/输出线程 —— **torvox 有（模型不同）**

| 维度 | termux | torvox |
|---|---|---|
| PTY 创建 | JNI/termux.c：open /dev/ptmx → termios（IUTF8、禁 IXON/IXOFF :54-59）→ TIOCSWINSZ → fork → setsid → dup2 → 关 fd → clearenv → chdir → execvp（:25-115） | `PtyPair::spawn`（pty.rs:109-372）：nix `openpty` → fork 前预构 CString（:122-144）→ fork 后手写 setsid/ioctl（注释 :219-226，避免 login_tty 的 malloc）→ 同样 `configure_raw_mode`（pty.rs:523，**IUTF8 + 清 IXON/IXOFF，与 termux.c 同源注释** :1012 测试） |
| 输出管线 | 3 线程：InputReader→ByteQueue→主线程 Handler；OutputWriter 阻塞读队列；Waiter waitpid（TerminalSession.java:133-172） | 1 个 reader 线程 + bounded channel(128)（session.rs:376）；渲染线程 `poll_pty_output(MAX_CHUNKS_PER_FRAME)` 限流（session.rs:492-565） |
| 输出处理线程 | **主线程**（Handler，保证 UI 单线程） | **渲染线程**（每帧轮询，天然背压） |
| 退出 | Waiter → MSG_PROCESS_EXITED → 屏幕打印 `[Process completed ...]`（:166-172/:355-365） | wait 线程 + `exited_flag`/`exit_code_now`（session.rs:618-641）+ `mark_exit_reported` 幂等（:638） |
| 输入 | `writeCodePoint` 手写 UTF-8（:183-217） | `Session::write` 直接写 PTY（session.rs:414-420） |
| 信号 | `Os.kill(SIGKILL)`（finishIfRunning :235） | `send_signal(signum)`（session.rs:470-477，更通用） |

结论：**torvox 的线程模型更现代**（channel 背压 + 渲染线程轮询），termux 的主线程 Handler 模型在 torvox 的 GPU 渲染架构下不可行。无可吸收项，但 termux 的"resize 时才初始化 emulator"（TerminalSession.java:103-110）与"PTY 尺寸含像素值"（termux.c:62）值得对照——torvox resize 已处理 dropped grid 自愈（session.rs:428-455，round-112/113）。

### 5.3 TermuxInstaller 的 bootstrap 安装（含 termux-bootstrap.c getZip）—— **torvox 有（分发方式不同）**

| 维度 | termux | torvox |
|---|---|---|
| zip 来源 | **APK 内嵌**：汇编 `.incbin bootstrap-<abi>.zip`（termux-bootstrap-zip.S:4-15）→ JNI `getZip()` 返回字节（termux-bootstrap.c:6-10）→ `loadZipBytes`（TermuxInstaller.java:454-458） | **网络下载**：`BootstrapOrchestrator.ensureBootstrap(bootstrapUrl)`（BootstrapOrchestrator.kt:40-118）+ `detectAbi` :129；compareAndSet 防并发 :49 |
| 安装流程 | 删 staging/prefix → staging 解压（SYMLINKS.txt 先行 :173-179）→ chmod 0700（bin/ 等 :207-212）→ symlink :220 → **renameTo 原子换入** :226 → second-stage 脚本 :231-257 | 几乎一一对应：`cleanupOld` :75 → `processZipEntries`（SYMLINKS.txt :124、chmod `EXECUTABLE_FILE_MODE` :160）→ `createSymlinks` → `atomicRename` :276（**旧 prefix 先改名备份再换入**，比 termux 更稳）→ `SecondStageRunner.run` :24-173（**锁文件防并发** :26、postinst 脚本 30s 超时 :154） |
| second stage | 由 bootstrap 包自带的 `termux-bootstrap-second-stage.sh` 执行（bash + AppShell） | Kotlin 直接跑各包 `postinst`（SecondStageRunner.kt:69-159），并 **`patchPostinstForLinker`**（:280-305，把 shebang 改为 linker 间接执行——Android 15 SELinux 必需） |
| 环境文件 | `TermuxShellEnvironment.writeEnvironmentToFile` 原子写 termux.env（TermuxShellEnvironment.java:46-63） | `SecondStageRunner.writeTermuxEnv`（:316+） |
| 失败恢复 | 错误弹窗"重试=删 prefix"（TermuxInstaller.java:318-340） | 报错列表返回给 UI；prefix 备份保留可回滚 |

结论：**torvox 的 bootstrap 安装是 termux 的强化版**（原子性更细、锁、超时、linker 补丁）。唯一缺项：**termux 的 ABI 内嵌 zip 方案**（离线可用、无需网络权限）——torvox 目前依赖用户配置的 bootstrap URL（TerminalViewModel.setBootstrapUrl :1017）。

### 5.4 termux-exec 机制 —— **torvox 已有等效实现（且更完整）**

torvox `pty.rs`：
- `build_env`（:676-703）：**`LD_PRELOAD=$PREFIX/lib/libtermux-exec.so`**（:679-688，round-215 注释完整复述了 termux-exec 原理）；
- `spawn`（:157-166+）：**`use_linker` 分支**——shell 在 $PREFIX 下时用 `/system/bin/linker64 $PREFIX/bin/bash` 间接 exec（Android 15+ SELinux 拒绝 app_data_file 直接 execute_no_trans，注释 :148-156）；
- 测试覆盖：`build_env_adds_ld_preload_for_prefixed_shell`（pty.rs:1086）、`build_env_no_ld_preload_without_prefix`（:1105）。

结论：termux 靠独立 C 仓库 + 用户侧 profile 设置 LD_PRELOAD；torvox 把同一机制内建在 PTY spawn 环境里，**对 torvox 无吸收需求**，反而是 termux 侧值得回看。

### 5.5 Term.java 的 transcript/选择文本提取 —— **torvox 有（见 §5.1 表格）**

补充：termux 的 `getTranscriptTextWithFullLinesJoined`（TerminalBuffer.java:48-50）被 `ShellUtils.getTerminalSessionTranscriptText`（ShellUtils.java:52-74）用于插件命令 stdout 回传；torvox 无插件体系，无对应需求。

### 5.6 TermuxActivity 生命周期与多会话 —— **torvox 有（架构不同）**

| 维度 | termux | torvox |
|---|---|---|
| 会话容器 | **Service**（TermuxService + TermuxShellManager，进程级单例），Activity 只是绑定者 | **TerminalRuntime**（Kotlin 单例，TerminalRuntime.kt:139）+ 前台服务按需启停（`startForegroundServiceIfNeeded` :378-387、`updateForegroundSessionCount` :429） |
| 首会话创建 | onServiceConnected → bootstrap → addNewSession（TermuxActivity.java:397-410） | ViewModel `ensureDefaultSession`（TerminalViewModel.kt:996-1008） |
| 会话切换/恢复 | preference 存 `sessionHandle`（TermuxTerminalSessionActivityClient.java:391-431），Activity 重建后按 handle 恢复 | `RuntimeState.activeSessionId`（TerminalRuntime.kt:40），`attachPendingSurface` :1712 |
| 退出处理 | `onSessionFinished` 自动切下一会话 + toast（:139-183） | `handleSessionExit` :248-329（死会话清理 + `activateReplacementSession` 重试 :322） |
| 服务保活 | 无条件前台通知（TermuxService.java:204-207）+ wake/wifi lock | 有会话才前台服务；无 `START_NOT_STICKY` 语义（Kotlin 协程生命周期） |
| 崩溃恢复 | START_NOT_STICKY 不重启；onDestroy 杀全部会话（:175-176） | **渲染线程看门狗**：`startRenderMonitor`/`checkSessions`/`handleDeadRenderThread`（TerminalRuntime.kt:567-697，最多 `RENDER_MAX_RESTART_ATTEMPTS` 重启）——torvox 独有且远超 termux |

结论：**torvox 的会话生命周期比 termux 更健壮**（看门狗、优雅替换、前台服务按需）。termux 唯一值得借鉴：**handle 持久化恢复当前会话**（TermuxAppSharedPreferences.getCurrentSession :174）——torvox 目前 activeSessionId 是否跨进程持久化需核实。

### 5.7 torvox 完全没有的

1. **内嵌 bootstrap zip（getZip 方案）**——离线安装（§5.3）；
2. **ByteQueue 双队列解耦**——torvox 用 bounded channel，等效；
3. **termios 陷阱清单**（IUTF8/IXON/IXOFF 注释，termux.c:54-59）——torvox 已吸收（pty.rs:523 同款注释）；
4. **termux.env 原子写 + shell source 生态**（TermuxShellEnvironment.java:46-63）——torvox 的 `writeTermuxEnv` 已做；
5. **WcWidth 的"与 C 侧同步"工程约定**（WcWidth.java:8-12）——torvox 宽字符完全依赖 ghostty 移植，无同步问题；
6. **多用户/主用户检查**（TermuxInstaller.java:80-90）——torvox 未做；
7. **包变体矩阵**（apt-android-7/5 的 PATH/LD_LIBRARY_PATH 差异，TermuxShellEnvironment.java:100-108）——torvox 只有一套现代路径，无此复杂度。

## 6. 依赖分析：是否适用于 torvox？

| 模块 | 是否适用 | 理由 |
|---|---|---|
| terminal-emulator（TerminalEmulator/TerminalBuffer/TerminalRow） | ❌ 不适用 | torvox 已基于 ghostty 移植（vt_conformance.rs 98KB 验证），功能覆盖 VT 状态机全部核心；换用 termux 的 Java 模拟器是倒退 |
| TerminalBuffer.getSelectedText 的列→字符换算 | ⚠️ 部分适用 | 逻辑可移植为 Kotlin 的 scrollbackLine 后处理（见 §7-1） |
| TerminalSession 线程模型 | ❌ 不适用 | torvox channel + 轮询更优 |
| JNI/termux.c PTY 创建 | ⚠️ 已吸收 | torvox pty.rs 已是同款（含 IUTF8/IXON 处理） |
| TermuxInstaller 内嵌 zip | ⚠️ 可选 | 离线安装的工程成本高（每 ABI 一个 zip 打进 APK），需权衡 |
| termux-exec | ❌ 已实现 | pty.rs:679-688 + linker 分支 |
| TermuxActivity/Service 生命周期 | ⚠️ 部分 | handle 持久化值得借鉴；看门狗 torvox 已有且更强 |
| TermuxConstants 路径体系 | ✅ 适用 | torvox 已用同构 `files/usr` 前缀（PREFIX/TERMUX__PREFIX 环境，pty.rs:664） |
| termux-shared 大工具类（FileUtils 104KB 等） | ❌ 不适用 | 面向插件生态，torvox 单应用不需要 |

## 7. 可吸收到 torvox 的具体内容（含代码注释建议）

1. **wrap 感知的选择文本拼接**（最高价值）：`TerminalViewModel.extractSelectedText`（TerminalViewModel.kt:476-556）目前每行硬插 `\n`（:553）。termux 的语义（TerminalBuffer.java:86-103）：wrap 行拼接不插换行、保留尾空格。**建议**：给 `TerminalQueryPort.scrollbackLine` 增加 `lineWrap(row): Boolean` 查询（ghostty grid 已有 wrap 标志），Kotlin 侧仿 :87-103 逻辑：
   ```kotlin
   // 参考 termux-app TerminalBuffer.getSelectedText :86-103 (research-termux-app-extra.md §1.3)
   // wrap 行: 不插 '\n' 且保留行尾空格; 非 wrap 行: 裁尾空格后插 '\n'
   ```
2. **宽字符列边界修正**（高价值）：`extractSelectedText` 的 `substring(lo.col, hi.col+1)`（:501-503）在含 CJK 行上会切错 char 边界。termux 的 `findStartOfColumn`（TerminalRow.java:92-120）思想：**用 WcWidth 在字符串上把列号换算成 char 索引**。建议新增 `TerminalViewModel` 工具函数 `columnToCharIndex(line, col)`，或在 native 侧提供列→字节偏移查询（ghostty 有 `grid.cursorCell` 类似 API 可查）。
3. **当前会话 handle 持久化**（中价值）：参照 `TermuxAppSharedPreferences.getCurrentSession/setCurrentSession`（:174-181）+ `getTerminalSessionForHandle`（TermuxService.java:919-927），把 `RuntimeState.activeSessionId` 写入 DataStore/SharedPreferences，Activity 冷启动后恢复会话而非新建。
4. **内嵌 bootstrap 的 ABI 汇编技巧**（可选）：`termux-bootstrap-zip.S` 的 `.incbin` + 汇编期 `blob_size`（:16-18）是零成本内嵌二进制的标准手法；若未来做离线安装，可直接照搬到 Android.mk/CMake（.S 汇编器通用）。
5. **`isTermuxSessionsEmpty` 幂等引导**（低价值）：TermuxActivity.java:397-410 的"空会话才引导"模式，torvox `ensureDefaultSession` 已有同等语义。
6. **注释建议落点**（供直接复制进代码）：
   - `TerminalViewModel.kt` extractSelectedText 处补 wrap 语义注释（见 1）；
   - `TerminalSurface.kt:1110` gridRow 换算处补 termux 坐标对照（`termux 外部坐标 -mActiveTranscriptRows…mScreenRows-1，mTopRow≤0；torvox scrollOffset≥0，gridRow = scrollbackLen - offset + row`）；
   - `BootstrapInstaller.kt:160` chmod 处补 termux 对照（`termux 只对 bin/、libexec/、lib/apt/* 设 0700，TermuxInstaller.java:207-212`）。

## 8. 项目文档吸收价值

1. **`docs/architecture.md` / `adr/0007-session-lifecycle.md`**：补充"会话容器位置"决策对照——termux 用 Service 承载（TermuxService.java:575-626），torvox 用 Runtime 单例 + 按需前台服务（TerminalRuntime.kt:378-429），各自权衡（保活 vs 电池）；
2. **`docs/reference/research-termux-app.md`**（已有）：在 §7 结论处链接本文档，形成完整研究闭环；
3. **README/项目文档**：可吸收 termux README.md:20 的 **Android 12+ phantom process 警告**表述——torvox 用户在 Android 12+ 同样会撞 `[Process completed (signal 9)]`，值得在 torvox 文档中预告排查路径（电池优化白名单）；
4. **`docs/lessons/04-vt-terminal.md`**：可补"PTY 必须 IUTF8 + 清 IXON/IXOFF"（termux.c:54-59）与"fork 子进程要 sigprocmask 解除阻塞"（termux.c:73-75）两条 Android 经验——torvox pty.rs 已实现但 lessons 未记录出处；
5. **`docs/acceptance.md` 测试矩阵**：termux 的 `ScrollRegionTest`/`ResizeTest`/`TerminalRowTest` 用例名可作为 torvox vt_conformance 的补充清单（resize 时 wrap 行保留尾空格、宽字符选区吸附等场景）。

## 9. 结论

termux-app v0.119.0-beta.3 的其余模块与 torvox 的关系是"**同题异解，torvox 整体领先**"：
- PTY/线程模型、termux-exec、bootstrap 安装：torvox 已有等效或更强实现（linker 间接 exec、原子换入、看门狗）；
- **真正缺口只有两个**，都集中在选择文本提取：① 软换行（wrap）感知的行拼接；② 宽字符列→char 索引换算（§7-1/7-2）。二者都有 termux 的成熟参考实现（TerminalBuffer.java:52-106 + TerminalRow.java:92-120），移植成本低、收益直接（CJK 场景选择复制正确性）；
- 文档层面值得吸收：Android 12+ phantom process 预警、PTY 两条 Android 经验（IUTF8/IXON、sigprocmask）的出处标注。

## deep-v5 增量（复核第 2 轮：MORE 按钮机制）

TextSelectionCursorController.java：
- `ACTION_COPY=1 / ACTION_PASTE=2 / ACTION_MORE=3`（:33-35）；菜单构建 :116-120（COPY + PASTE（`setEnabled(clipboard.hasPrimaryClip())`）+ MORE）
- **MORE 语义**（:145-149）：先 `mStoredSelectedText = getSelectedText()` 存文本，再交 TerminalViewClient 处理（分享/翻译等扩展）——`getStoredSelectedText()`（:380）/`unsetStoredSelectedText()`（:385）成对管理
- 防竞态（:130-132）：菜单点击时 `!isActive()` 直接 return（对话框关闭中误点防护）

**torvox 对照**：ActionMode 菜单仅 COPY/SELECT ALL/PASTE——**无 MORE 扩展点**。P2 记录：若未来加"分享选中文本"等扩展，MORE 模式（先存文本再回调）是蓝本；`!isActive()` 防误点 torvox 已有等价（menuDismissed）。

## deep-v1 增量（2026-08-07 全文件精读轮 #2）

### 本次精读文件
- `termux-shared/.../shell/am/AmSocketServer.java` + `net/socket/local/LocalSocketManager.java`
- `termux-shared/.../terminal/io/BellHandler.java`
- app 模块文件清单核对（TermuxShellManager/extrakeys/settings fragments——extra 已覆盖）

### AmSocketServer + LocalSocketManager（AF_UNIX 本地 IPC——torvox MCP socket 同架构族）

| 特性 | termux | torvox | 差异 |
|------|--------|--------|------|
| 传输 | **AF_UNIX/SOCK_STREAM**（文件系统 socket，注释明确"abstract namespace socket 不安全"） | Unix socket（tower-mcp） | 同架构 |
| 协议 | `exit_code\0stdout\0stderr\0` 三字段 NUL 分隔 | MCP JSON-RPC | 协议不同（termux am 专用） |
| **对端凭据** | **getPeerCred()（JNI 读 SO_PEERCRED）**——服务器可验证连接进程 uid/pid；"只允许 server app 用户和 root" | **无 peer-cred 检查**（仅 socket 文件权限） | **P2 安全参考**：torvox MCP socket 文件在 app 私有目录，但若未来共享给其他 uid（如 root 的 CLI），peer-cred 检查可防任意同 uid 进程。当前 socket 路径方案（app 私有目录 0700）足够 |
| 超时 | setSocketReadTimeout/sendTimeout + deadline 参数 | tower-mcp 内建 | 等价 |
| 错误 | JNI 异常必须 catch Throwable（:105 注释） | torvox jni_export_guard 同理念 | 确认 |

### BellHandler（termux 铃声 62 行完整）
- **振动** 50ms + `MIN_PAUSE = 3×DURATION` 合并策略（快速连续 BEL 合并为一次，`lastBell` 跟踪下次调度）+ Samsung Android 8 已知异常捕获
- **torvox 对照**：torvox bellToneGenerator（ToneGenerator 缓存实例）——**音调 vs 振动差异**。torvox 用音调（TerminalRuntime 缓存 ToneGenerator）。termux 用振动（无音频输出场景更可靠）。**P3 记录**：torvox 可加"铃声+振动"双通道或设置项

### 新增汇总
| # | 发现 | 级别 |
|---|------|------|
| 1 | getPeerCred 对端凭据检查（SO_PEERCRED）——torvox MCP 无 | P2 |
| 2 | BellHandler 振动合并策略 vs torvox 音调 | P3 |
| 3 | termux `exit_code\0stdout\0stderr\0` IPC 协议——torvox MCP JSON-RPC 更标准 | 确认 |
