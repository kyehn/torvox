# 08 pavelc4/Rin

上游：<https://github.com/pavelc4/Rin>
本地：/tmp/competitor-study/pavelc4__Rin
版本：主线 2026-10-07 取样，86 文件，Rust 约 2700 行。Rust 引擎加 Compose 壳，手写 VT 子集，自带 rpkg 包管理器。全部源码已通读。

## 架构

Rust 分 core（cell/grid/buffer/cursor/alternate/screen）、parser（545 行手写 ANSI 加颜色）、input（按键转 ANSI）、pty（portable-pty 封装）、renderer（经 Canvas 指针进 Rust 绘制）、platform/android（JNI 525 行加会话）。Kotlin 侧 Compose 屏、会话管理、前台服务、自绘 Canvas 视图、虚拟键栏。另有 rpkg 包管理子 crate 与文档。

## 机制

PTY 经 portable-pty 建伪终端，固定 `TERM=xterm-256color`，HOME 下设工作目录与 `ENV` 指向 `.mkshrc`，支持 su 路径切换 shell。读线程 16K 缓冲，EOF 与错误均写回红色提示并标记死亡。应答经 buffer 排出回写 pty。渲染经 Canvas 指针传 Rust 绘制矩形与文本。会话新建即定宽高与字号，resize 同步引擎与 pty。手写解析器仅覆盖常用 CSI 与 SGR 子集，鼠标模式枚举具备但远非 Ghostty 完备。

## 与本项目对照

本项目 Ghostty 为唯一真源、Rust 经 wgpu Vulkan 渲染、Kotlin 仅收轻量事件、无沉重网格过 FFI；竞品手写解析器加 Canvas 指针绘制，两条路线均不可抄。固定 TERM、ENV 指向 `.mkshrc`、EOF 保留现场的方向与本项目一致，但文案须用本项目规范的 `[Process completed (code 255) - press Enter]` 形态。portable-pty 属外部库封装，本项目 Rust 直接管理 fork/exec，不引入。rpkg 包管理器超出范围。su/root 切换撞低安全限制但无权限管理设计的边界，不引入。

## 可参考点

- P1：读写线程与应答回写分离；resize 同步引擎与 pty；ENV 指向 home 下 mkshrc。
- P2：解析器按命令枚举组织的代码结构（仅结构思路，解析仍用 Ghostty）。
- 反面价值 P0：手写 545 行仍是不完备子集，证明 Ghostty 单源决策正确，须在汇总中引用。

## 禁止项勿抄

手写 VT 解析器、Canvas 指针进 Rust 绘制、自带包管理器、su 提权路径、红色染色退出文案、自定义环境变量扩展。

证据：`src/parser/ansi.rs:545`、`src/pty.rs:1-87`、`src/platform/android/session.rs:1-129`、`src/renderer/screen.rs:1-100`、`src/platform/android/jni.rs:525`、`TerminalSurface.kt:123-623`。
