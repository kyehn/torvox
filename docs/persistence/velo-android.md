# velo-android 上游审查

- 上游：https://github.com/yhzion/velo-android @ `5ed7304`（2026-11-27）；Rex 的 unofficial Android 客户端。
- 定位：终端、渲染与远端协议全在 Zig 核心，Kotlin 只管界面与平台服务；与本地“Rust 核心 + Kotlin UI”的分层最可比。

## 架构与数据流

- `native/` 生成 `libvelocore.so`：Zig 0.17.1，静态链接 libghostty-vt 1.0.1 C ABI，负责 TLS、WebSocket、Rex 控制/数据协议、会话/窗口/布局/块状态、终端仿真、Kitty 图片、OpenGL ES 3 渲染、键编码、选择与复制文本。
- `net/TlsClient.zig` 是 `std.crypto.tls.Client` 的修补副本，补齐 CA path length、nameConstraints、extendedKeyUsage、未知关键扩展检查与逐长度边界比较；根证书来自 `AndroidCAStore`，App 启动时复制到 `no_backup/user-cas`，5 秒超时避免首连卡死。
- WebSocket 自行实现 RFC 6455（upgrade、掩码客户端帧、分片、ping/pong/close，无压缩）；控制通道是 JSON one-object-per-message，数据通道是二进制 HELLO/WELCOME/RESUME 与 Rex APC 事件。
- JNI 契约很细：`start/connect/selectSession/command/key/paneSurface/paneScale/panePan/panePreview/paneResume/paneSuspend/paneTouch/paneCopy/paneBracketedPaste/forwardOpen`；handle 是不透明 `Long`，调用快速返回，工作发生在 core 线程。`ANativeWindow_fromSurface` 与 `ANativeWindow_release` 成对管理。
- `Pane.zig` 把表面生命周期与终端所有权分开：`panePreview` 供标签总览复用 surface 但不抢终端，`paneSuspend`/`paneResume` 处理离开屏幕/被遮挡；`fit` 与 `touch` 分别表达尺寸落定与手指按下。缩放分 live 与最终态，捏合中拉伸字形，结束时才按最终字号重绘一次。
- `Renderer.zig`/`Atlas.zig` 用 GLES3 实例化绘制，字形键含精确栅格参数；`TermEditor` 管理 composing、trusted click、bracketed paste 与剪贴板确认。

## 已核实边界

- `Pane.detachSurface` 对 `setSurface(null, 0, 0)` 用 `catch unreachable`，依赖“只有启动 renderer 会失败”的不变式；该假设变更后会变成进程崩溃。
- 依赖 Rex 服务端协议 revision 18 与 Tailscale MagicDNS/HTTPS 证书；自实现 TLS/WebSocket 与修补标准库意味着升级 Zig 时必须重审补丁。
- 远程客户端没有本地 PTY；把其键编码/表面生命周期直接套到本地会引入不存在的远端状态机。`unsafe` 计数为 0 只是 Zig 不用该关键字，不能推断内存安全。

## 对 torvox 的取舍

- 逐条对照 `paneSurface/panePreview/paneSuspend/paneResume/fit/touch` 的语义与本地 `android/app/src/main/java/terminal/emulator/ui/TerminalScreen.kt` 的 Surface 暂停账本；这是本仓最值得借的生命周期协议。
- 对照 `native/src/android/ffi/input.rs`：本地 key/paste 仍是单句柄 JNI 边界，不引入多 pane 所有权。
- TLS、WebSocket、Rex 协议与 Tailscale 不采纳；本地没有远端客户端状态机。
- 跳过：未运行 Zig/Kotlin 测试与 GPU 路径（需 Zig 0.17.1、Android SDK 与设备）；风险是渲染与生命周期只按源码和 `docs/native-architecture.md` 核对。
