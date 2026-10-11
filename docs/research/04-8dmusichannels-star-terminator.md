# 04 8dmusichannels-star/terminator

上游：<https://github.com/8dmusichannels-star/terminator（用户所写> 8dmusichannels-star/terminator 即此仓）
本地：/tmp/competitor-study/8dmusichannels-star__terminator
版本：主线 2026-10-11 取样，115 文件。Kotlin Compose 加 C PTY，自研 VT 与 Canvas/Compose 渲染，GPL-3.0。全部源码已通读。

## 架构

`terminal-emulator/` 为独立模块：C 层 `pty.c:23-233` 经 `/dev/ptmx` 加 fork 加 exec 建真 PTY，含 seccomp 变通（`seccomp_safe_fork:52`，裸 clone 避 bionic fork 的 clone3 标志）；`NativePty.kt:29-75` 声明创建、等待、关闭；`TerminalSession.kt:35-786` 管会话规约（命令参数与文件两种）、环境组装、读写线程、历史落盘、退出保留现场；`TerminalEmulator.kt:27-5177` 为自研 VT（CSI/OSC/DCS/Sixel/kitty/超链接/OSC133/OSC52/OSC7/DECCOLM 等全矩阵）；`TerminalBuffer.kt:2077` 管回滚与图像；`TerminalView.kt:2466` 为 Compose Canvas 渲染与选择；`MouseGestureTracker.kt:617` 管鼠标手势。`app/` 管 Compose 壳、多窗格分屏、会话抽屉、虚拟键栏、设置与前台服务。

## 与本项目对照

本项目 Ghostty 为 VT 唯一真源、Rust 拥有会话、wgpu Vulkan 渲染；竞品自研解析器必然是不完备子集，不可参考解析逻辑。PTY 经真 pty 建子进程、退出写回 `[Process completed - press Enter]`（`TerminalSession.kt:764`）与本项目保留现场一致，可参考行为。渲染走 Compose Canvas，不可参考。会话规约分命令与文件两种、支持工作目录与 root 切换，其中相对路径与脚本路径写法违反本项目启动入口规范，不可参考。terminfo 随包 12 种（`assets/terminfo`）与本项目固定 `TERM=xterm-256color` 冲突，不可参考。透明、模糊、自定义主题、分屏广播、链接白名单、通知音等均在禁止列或未声明范围，不可参考。

## 可参考点

- P0：真 PTY 加前台会话组加 SIGWINCH 语义注释（`TerminalSession.kt:54-70`）；退出保留现场文案与异步投递（`markExited:752-776`）；历史追加单流常开避每块开关（`TerminalSession.kt:95-130`）；seccomp fork 变通思路（Rust 侧如遇同类拒绝可借鉴排查方向）。
- P1：虚拟键长按重复时序（`VirtualKeyBar.kt:113-150`）；OSC52 读写门控加设置开关（`TerminalEmulator.kt:49-76`）；OSC4/10/11/12 查询回写语义注释（`:85-110`）；OSC7 仅取可用形式的克制处理（`:117-126`）。
- P2：捆绑 terminfo 的离线思路（仅思路，TERM 值仍须固定）；会话导出 txt 的落盘方式。

## 禁止项勿抄

自研 VT 解析器、Canvas 渲染、可配 TERM 与多 terminfo、相对路径与脚本路径启动入口、分屏与广播、透明与模糊与背景图、自定义主题与调色、链接白名单外自定义 schemes、通知音与 OSC 777/9 通知、local echo 开关、keymapper 自定义映射、purge 语义开关。

证据：`pty.c:23-233`、`NativePty.kt:29-75`、`TerminalSession.kt:35-786`、`TerminalEmulator.kt:27-160`、`TerminalView.kt:23-387`、`MouseGestureTracker.kt:617`、`FontResolver.kt:59`、`VirtualKeyBar.kt:113-250`、`SessionForegroundService.kt:368`。
