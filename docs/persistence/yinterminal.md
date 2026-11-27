# YinTerminal 上游审查

- 上游：https://github.com/ditzabcd/YinTerminal @ `c4d29fd`（2026-11-25）；GPL-3.0。
- 定位：轻量 Android shell 客户端，持久化 root/user shell、ANSI 着色、内嵌 Python、脚本与快捷工具；不是完整 VT 终端。

## 架构与数据流

- `AnsiParser.kt` 用正则分段处理 ANSI 颜色（16/256/RGB）并写入 `AnnotatedString`；输出按 stdout/stderr 分流，emoji 交由 Android 绘制。
- `CommandInterpreter.kt` 解析内置命令（哈希、Base64/Hex/URL、JSON、正则、系统信息、Morse），命令输出与 shell 输出混在同一文本流。
- `NativePty.kt`/`ShellManager.kt` 维持单个 `su` 或 `/system/bin/sh`，使 `cd` 与环境变量跨命令保留；`app/src/main/cpp/yinterm_pty.cpp` 提供 forkpty 桥。
- Chaquopy 提供离线 Python 3.13；脚本、历史与快命令存为本地 JSON；`TerminalView` 直接绘制 `AnnotatedString`，没有网格、光标寻址或滚动区状态机。

## 已核实边界

- 只有 2 个测试文件；正则解析在转义序列分片、嵌套 SGR 与 OSC 上会出错，缺少 grid/cursor/alt-screen/滚动区/鼠标协议实现。
- 隐藏崩溃状态文件与持久化 root shell 是隐私/安全敏感点；自动请求 `su`、本地明文 JSON 存储不符合最小暴露原则。
- GPL-3.0 与本地依赖策略不相容，不能复制代码。

## 对 torvox 的取舍

- 反例：ANSI 正则不能替代 VT 解析器；本地 `native/src/terminal/ghostty_terminal/public_api.rs` 的 `sanitize_vt_input` 与 `output_processor.rs` 的分派才是正确边界。
- 内置命令解释器与持久 shell 不进入终端核心；本地 PTY 由 `native/src/terminal/pty.rs` 管理，命令语义留给 shell。
- 跳过：未运行 Gradle 或在 root 设备上复现持久化 shell；风险是仅按源码核对。
