# 11 irealashu/ApexShell

上游：<https://github.com/irealashu/ApexShell>
本地：/tmp/competitor-study/irealashu__ApexShell
版本：主线 2026-10-04 取样，63 文件。Compose 壳加 Room 持久化，每命令 `ProcessBuilder("/system/bin/sh", "-c", command)` 一次执行，无 PTY、无控制终端、无交互会话。全部源码已通读。

## 架构

`engine/` 管一次性执行与 ANSI 正则染色与提权探测；`model/` 管行、会话、主题；`data/` 管 Room 历史与脚本；`ui/` 管 Compose 屏与附件栏与输出视图；`viewmodel/` 管会话编排。无 PTY 层、无 VT 状态机、无渲染循环。

## 机制

执行经 Flow 包装进程输出，每命令起停。提权分普通、Shizuku、root 三档，Shizuku 经反射调系统服务，root 轮询 su 路径，失败回落普通 shell。环境每次组装 HOME、TMPDIR、SHELL、TERM、USER 与 PATH。输出用正则 strip ANSI 后按行染色展示。附件栏为常用键快捷行。

## 与本项目对照

无 PTY 即无作业控制、无全屏、无交互 TUI，与终端模拟器定义不符，整体不可参考。逐命令执行模式与 Rust 持有长会话方向相反。Shizuku 与 su 提权撞低安全限制且无权限管理设计的边界。Room 历史持久化撞会话数据不持久化的精神。正则染色与本项目 Ghostty 真源冲突。

## 可参考点

- P2：附件栏常用键组织（仅键位思路，须按 Termux 2x7 落）；提权状态徽标的展示思路（不引入提权本身）。
- 反面价值 P0：无 PTY 方案不可交互，证明真 PTY 必要，汇总引用。

## 禁止项勿抄

逐命令执行模式、反射调 Shizuku、su 路径轮询、正则染色输出、历史持久化、脚本库执行。

证据：`ShellExecutionEngine.kt:262-368`、`AnsiParser.kt:101`、`PrivilegeDetector.kt:216`、`TerminalViewModel.kt:439`、`TerminalAccessoryBar.kt:159`。
