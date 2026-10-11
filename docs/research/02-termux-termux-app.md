# 02 termux/termux-app

上游：<https://github.com/termux/termux-app>
本地：/tmp/competitor-study/termux__termux-app
版本：2026-09 主线，约 4.8 万行。多模块 Java 加 JNI，自研 VT 与 Canvas 渲染。全部源码已通读。

## 架构

`app/` 管 Activity 与 Service，`terminal-emulator/` 管自研 VT 与 PTY 会话，`terminal-view/` 管 View 渲染与选择，`termux-shared/` 管环境与通知等。`TermuxService` 持有 `TermuxShellManager` 与会话列表，Activity 仅绑定显示。`TermuxSession` 桥接执行命令与 `TerminalSession`。会话客户端回调为最小事件集：结束、shell pid、bell、颜色变化、标题变化。

## 与本项目对照

本项目 Rust 拥有会话、Ghostty 解析与 wgpu Vulkan 渲染，Kotlin 仅收轻量事件；竞品 Java 持有全部状态并经 Canvas 绘制。VT 不可参考其解析器，必须用 Ghostty。渲染不可参考 Canvas。回滚默认 2000 与本项目 2K 对齐。选择、修饰键栏 2x7 与粘滞、会话抽屉、崩溃保留现场、bootstrap 原子替换等行为可参考，但须用 Compose 与 Rust 重写。

## 可参考点

- P0：PTY 双线程加队列加主线程泵模型；`[Process completed - press Enter]` 非零退出保留现场；默认键矩阵 `[ESC,/,(-|),HOME,UP,END,PGUP]/[TAB,CTRL,ALT,LEFT,DOWN,RIGHT,PGDN]` 加粘滞与长按重复；staging 解包后 rename 原子替换；回滚默认 2000；会话事件最小集。
- P1：topRow 滚动与缩放字体手势划分；抽屉行格式 `[n] 名称 标题` 与切换关闭；前台通知与 WakeLock 策略；输入连接掩码配置。
- P2：VT 单测向量转写为 Ghostty 集成测试；符号链接清单延迟建链；配置文件落盘后移动避免半读。

## 禁止项勿抄

会话持久化与恢复、`termux-exec` 与 `LD_PRELOAD` 劫持、API 插件包与外部调用、自定义环境与配色与键栏、透明背景、全屏规避、Shizuku 与提权、Ghostty 之外的图像扩展。

证据：`TermuxService.java:55`、`TermuxActivity.java:80`、`TermuxSession.java:32`、`TerminalSessionClient.java:17`、`TerminalEmulator.java:228-230`、`TerminalBuffer.java:72`、`TerminalSession.java:82-367`、`JNI.java:26-36`、`termux.c:117`、`TerminalView.java:308-1429`、`TextSelectionCursorController.java`、`ExtraKeysView.java:76`、`ExtraKeysInfo.java:139`、`TermuxPropertyConstants.java:329`、`TerminalToolbarViewPager.java:30`、`TermuxSessionsListViewController.java:30`、`TermuxInstaller.java:65-277`。
