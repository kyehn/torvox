# 15 RohitKushvaha01/ReTerminal

上游：<https://github.com/RohitKushvaha01/ReTerminal>
本地：/tmp/competitor-study/RohitKushvaha01__ReTerminal
版本：主线 2026-09-26 取样，368 文件。Termux View 复用加 Compose 壳，proot Alpine，已停更。核心已通读。

## 架构

`core/main` 为应用主体：会话创建、终端屏、抽屉、虚拟键、主题、设置、文档提供器、崩溃处理；`core/proot` 与资源模块管运行时；`core/components` 为 Compose 偏好组件库。终端仍复用 Termux `TerminalSession`，外层 Compose 化。

## 机制

会话经 `MkSession.createSession` 组装环境后调 Termux 会话：宿主变量透传、HOME 指向存储、工作目录为 Alpine 家、可配登录 shell 经 `RETERM_LOGIN_SHELL`。proot 与加载器走 native 库目录，另有 chroot 模式切换。初始化脚本首次从 assets 落盘。文档提供器暴露 Alpine 家目录，可读写建删。登录 shell 可配，Alpine 首次启动装配。

## 与本项目对照

复用 Termux 会话与 Canvas 的路线不可抄。环境组装设置 `LD_LIBRARY_PATH` 与本项目禁止设置该变量直接冲突，不参考。可配登录 shell 的方向与本项目 shell 启动入口设置框一致，但本项目须为绝对路径二进制、不支持脚本路径、不检查存在，竞品的自定义会话对话框超出此范围。文档提供器暴露 home 可读写的形态与本项目要求一致，但本项目根为 `/data/data/com.termux/files/home` 且可读写，实现须按本项目规范重写，不照搬其 Alpine 根。proot 与 chroot 与预装 Alpine 均撞禁止项。

## 可参考点

- P1：文档提供器根、建删、缩略、搜索标志的形态（根路径按本项目规范）；宿主变量透传名单（取交集，禁 `LD_LIBRARY_PATH`）；初始化脚本首次落盘。
- P2：Compose 偏好组件库组织；登录 shell 设置项形态（收敛到本项目启动入口框）。

## 禁止项勿抄

复用 Termux 会话与视图、`LD_LIBRARY_PATH` 设置、proot 与 chroot 双模式、预装 Alpine、外部命令服务、自定义环境变量扩展。

证据：`MkSession.kt:24-202`、`Rootfs.kt:12-35`、`AlpineDocumentProvider.kt:25-100`、`CustomSessionDialog.kt`、`Settings.login_shell`。
