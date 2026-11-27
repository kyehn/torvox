# yinterminal 研究留档

## 定位

YinTerm 是“持久 root/user shell + 内置 Python 3.13”的轻量 Android 终端。它**不是网格终端**：输出以日志行进入 `LazyColumn`，ANSI 被解析成 `AnnotatedString` 的 `SpanStyle`，没有 cell 网格、光标寻址、备用屏或 TUI 布局。这一点决定了它能借鉴的范围。

## 关键实现与文件锚点

- `app/src/main/java/yin/term/core/AnsiParser.kt`：CSI／OSC 识别与样式拆分。
- `app/src/main/java/yin/term/data/datasource/TerminalSession.kt`：`ROOT_HANDSHAKE_TIMEOUT_MS = 12_000`、`USER_HANDSHAKE_TIMEOUT_MS = 3_000`、`STREAM_BUFFER_SIZE = 4096`、`echoSuppressQueue`。
- `app/src/main/java/yin/term/data/datasource/NativePty.kt`：`posix_openpt`／`grantpt`／`unlockpt`／`ptsname_r` 的 JNI 封装。
- `app/src/main/cpp/yinterm_pty.cpp`：`TIOCSWINSZ`、`SIGWINCH`、termios 设置。
- `app/src/main/java/yin/term/ui/terminal/{TerminalViewModel,TerminalView}.kt`：日志行驱动的 UI。

## 实现

`AnsiParser` 用一组正则识别 CSI、OSC、字符串序列、单字符 ESC 和控制字符；`extractCompleteChunk` 在分块边界保留未结束的 ESC 序列，避免跨块截断。`AnsiStyleState` 处理 16 色、256/RGB、粗体、斜体、下划线、删除线与反显，`toSpanStyle` 按默认前景背景解析实际颜色。

`TerminalSession` 优先寻找 `su`，失败回退 `/system/bin/sh`，用 `__YIN_HS__<pid>__<cwd>__` 握手标记确认 shell 已就绪，root 超时 12 秒、user 超时 3 秒。启动时向 shell stdin 注入 `stty -echo` 与初始化命令，并用 `echoSuppressQueue` 抑制注入命令的回显；`SIGINT`/`SIGKILL` 与 `isAlive` 保活。stdout 与 stderr 分成两个协程流，`STREAM_BUFFER_SIZE` 为 4096。`NativePty` 是 JNI 封装：`posix_openpt`/`grantpt`/`unlockpt`/`ptsname_r`，`TIOCSWINSZ` 更新窗口并在有前台进程组时发 `SIGWINCH`，termios 关闭 ECHO 族标志但保留 `ISIG` 与 `OPOST|ONLCR`。

数据层有房间实体与本地存储，提供历史、脚本、设置、终端日志的仓库；UI 包含终端、历史、脚本与主题定制界面，另有哈希、Base64/Hex、JSON、正则等内建工具。

## 已核实局限

- 日志行模型无法承载光标移动、备用屏、滚动区、宽字符占位与 TUI 重绘：`less`、`vim`、`htop` 这类程序不能正确显示。
- 握手标记、回显抑制与 `stty -echo` 都靠向 shell stdin 注入命令完成，属于应用层与 shell 的隐式协议；握手超时后走 fallback 进程，行为分支较多。
- 内建工具与脚本管理器属于本地规范明确不实现的功能类别，不能移植。

## 与本仓对照

- 可借鉴的只有“持久 shell + 握手超时 + 窗口变化时发 SIGWINCH”这几条工程细节；本仓的 shell 环境变量与启动目录由规范固定，不需要握手标记注入。
- 本仓必须以 cell 网格与 Ghostty VT 状态为准，不能把输出当成文本行集合；滚动、选区、搜索、IME 的可见区域都依赖网格坐标。
- 它把 stdout/stderr 分流显示，而本仓把它们合并进同一个 PTY 字节流交给 VT 解析，这是有意的差异。
