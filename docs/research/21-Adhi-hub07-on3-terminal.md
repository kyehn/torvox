# 21 Adhi-hub07/on3-terminal

上游：<https://github.com/Adhi-hub07/on3-terminal>
本地：/tmp/competitor-study/Adhi-hub07__on3-terminal
版本：主线 2026-06-18 取样，40 文件。termite 范式：C PTY 加 Kotlin 自研 VT 子集。核心约 2900 行已全部通读。

## 架构

`termite.c:213` 建真 PTY：开主端、授权解锁取从端名、设 UTF8 与流控关、置窗口像素尺寸、fork 后子进程关主端、建会话、开从端、重定向三流、关多余 fd、清空环境后装载传入环境、切目录、执行。`TerminalSession.kt:215` 管会话。`TerminalEmulator.kt:837` 为 VT 子集。`TerminalView.kt:330` 与 `TerminalRenderer.kt:213` 为绘制。`TermiteService.kt:123` 为服务。

## 亮点

提交信息明示修过关键 bug：子进程开从端而非主端。此为 PTY 正确性要点：父持主端，子经会话与从端建立控制终端。termios 置 IUTF8 关 IXON/IXOFF、窗口带像素尺寸、子进程信号全开、fd 清理、环境清空后重装，均可作 Rust 侧 fork/exec 实现的核对清单。

## 与本项目对照

自研 VT 与 Canvas 路线不可抄。PTY 子进程语义可作核对清单，但本项目会话归属 Rust 且规范未细化 fork 步骤，引入前须先立规范。`clearenv` 后重装与本项目固定白名单加宿主透传方向一致，但须以本项目名单为准。

## 可参考点

- P0：子进程开从端、setsid 后开从端建控终端、关多余 fd、清空环境后重装的核对清单。
- P1：termios 与窗口像素尺寸设置；exec 失败写从端报错后退出的诊断姿势。

## 禁止项勿抄

自研 VT、Canvas 渲染、服务形态照搬。

证据：`termite.c:1-213`、`TerminalSession.kt:215`、`TerminalEmulator.kt:837`。
