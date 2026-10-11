# 13 imlunixdev/puppy

上游：<https://github.com/imlunixdev/puppy>
本地：/tmp/competitor-study/imlunixdev__puppy
版本：主线 2026-10-04 取样。仓库混入大量 `build/` 与 `.cxx/` 构建产物，真实源码仅 7 文件约 1200 行：Kotlin 加 C++ PTY，proot Alpine，自研 VT 子集。真实源码已全部通读，构建产物未纳入结论。

## 架构

`pty.cpp:151` 经 native 启 proot 会话并给读写与尺寸句柄；`PtySession.kt:86` 单线程池读、主线程 16ms 合并投递、UTF-8 粘包解码；`TerminalBuffer.kt:170` 为有界 cell 缓冲加 CSI 子集与 OSC133 shell 标记；`RootfsManager.kt:427` 管 rootfs 与 proot 运行时；`TerminalView.kt:85` 展示；`MainActivity.kt:167` 编排。另有缓冲单测 129 行。

## 与本项目对照

自研 VT 仅覆盖常见 shell 子集、无宽字符语义，不可参考解析。本项目 Ghostty 真源、Rust 持有会话、wgpu 渲染三条主线均与之相反。proot 内嵌与预装 rootfs 撞禁止项。16ms 合并投递与粘包解码的 IO 思路可参考行为，但须落在 Rust 侧。OSC133 标记驱动 shell 模式（提示符、执行中、全屏、原始输入）并记录退出码的克制处理可参考，但本项目规范未声明 shell 集成语义，引入前须先立规范。

## 可参考点

- P1：读线程合并投递节流；UTF-8 粘包保留解码；OSC133 显式标记才消费、普通输入不动。
- P2：有界历史环形槽；缓冲单测形态。

## 禁止项勿抄

自研 VT 缓冲、proot 内嵌与 rootfs 管理、预装 Alpine、shell 模式扩展语义（未立规范前）。

证据：`PtySession.kt:86`、`TerminalBuffer.kt:170`、`pty.cpp:151`、`RootfsManager.kt:427`、`TerminalBufferTest.kt:129`。
