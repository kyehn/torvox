# 07 rafaelmeloreisnovo/termux-app-rafacodephi

上游：<https://github.com/rafaelmeloreisnovo/termux-app-rafacodephi>
本地：/tmp/competitor-study/rafaelmeloreisnovo__termux-app-rafacodephi
版本：主线 2026-10-08 取样，2610 文件。termux 重型 fork：终端核心未换但多处修改，外加 bootstrap 治理、文档与工作流膨胀。与上游 `diff -rq` 逐模块比对已做，核心改动已通读。

## 血缘与改动

终端核心修改：`TerminalSession.java` 会话创建失败早退并回调结束、读写循环响应中断、可重入资源清理、finish 先杀进程组再杀 shell、防 fd 复用；`termux.c` 加几何校验、数组长度校验、失败路径释放已分配内存与局部引用、补关闭泄漏 fd；`KeyHandler.java` 交换 kP/kN 翻页映射；`TerminalBuffer/Row/Emulator/TextStyle/WcWidth` 有改动；上游 `AndroidUtils/TerminalBitmap/TerminalSixel/ITermImage` 缺失即剥离图像协议。`TerminalRenderer/View` 有改动。`termux-shared` 加路径处理、错误码、运行时路径、质量管理等。`app/` 加引导门控、完整性校验、审计、基准、传感器桥等大批 Activity。另有自研 bootstrap 工具链（`bootstrap*/`、receipt、回滚、看门狗）与海量治理文档工作流。

## 与本项目对照

本项目 Rust 拥有会话、Ghostty 解析、wgpu 渲染；竞品仍是 Java 持有加 Canvas。会话加固思路可参考，JNI 校验与清理语义可参考。kP/kN 交换属上游行为变更，无 Ghostty 依据，不参考。剥离图像协议与本项目 kitty 支持方向相反，不参考。bootstrap 包的 sha256 校验文件、自研 bootstrap 工具链、治理工作流膨胀，均撞禁止或超出范围，不参考。`finish()` 先杀进程组再杀 shell 的做法与 zed 参考（先 killpg 前台组再 kill shell）方向一致，但本项目会话归属 Rust 且规范未声明前台进程组语义，须先立规范再决定是否引入。

## 可参考点

- P0：会话创建失败即清理并结束、防资源重复清理、读写循环可中断；JNI 几何与数组校验、失败路径内存与引用清理。
- P1：先进程组后单进程的关闭顺序（须先补规范）；退出文案 StringBuilder 化等小优化无价值不引入。
- P2：启动路径纯函数化测试（`PathTreatmentUtilsTests`）的测试思路。

## 禁止项勿抄

bootstrap 包的 sha256 校验文件、自研 bootstrap 与回滚与收据、图像协议剥离、kP/kN 重映射、传感器桥与审计膨胀、会话持久化（如有）、任何治理工作流。

证据：`TerminalSession.java` 全 diff（创建失败早退、中断循环、进程组关闭、清理幂等、接收缓冲 64K 改 4K）、`termux.c` diff（校验与清理）、`KeyHandler.java:123-124`、`BootstrapIntegrityVerifier.java`、`bootstrap_rafaelia/`、`raf_bootstrap_linear_flat_v2/`。
