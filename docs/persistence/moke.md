# moke 研究留档

## 定位

Moke 是远程 SSH/mosh 终端：内置 userspace Tailscale（不占系统 VPN 槽）、代理、多会话、远程 tmux、SFTP 与加密 WebDAV 备份。终端核心是 vendored 的 Termux 模块 `terminal-emulator` 与 `terminal-view`（Java），产品层是 Kotlin Compose。应用代码 GPL-3.0，vendored 终端模块 Apache-2.0。

## 终端相关实现

`terminal-emulator` 保留 Termux 的 `TerminalEmulator`、`TerminalBuffer`、`JNI`、`WcWidth`、`KeyHandler`、`ByteQueue` 等结构，并加入 `TerminalScrollToken` 与 `MokeIgnoreAltScreenTest` 等 Moke 专用行为；`terminal-view` 有 `MokeScroll`、`ScrollViewport`、`ScrollMotion`、`GestureAndScaleRecognizer` 与光标/选区控制器。`GestureAndScaleRecognizer` 与 `ScrollMotion` 承担触摸与阻尼，测试里有 `MokeScrollPolicyTest`、`ScrollMotionTest`、`ScrollViewportTest`。

`TerminalSizeState` 解决引导期 resize 与输入队列的顺序问题：初始化保留首个尺寸，后续 `update` 只更新 `latest`；`attach` 时持有元数据锁把当前尺寸排入与终端输入同一 FIFO，注释明确“持锁入队以免被更新后的尺寸超越”。`TerminalController` 管理一次性/锁定修饰键、捏合字号、链接命中（`getColumnAndRow` + `linkAt`）、键盘请求与滚到底部。`KeySeq` 用 `KeyId`/`KeyStep`/`Combo` 描述组合键，`encode` 产出 CSI、tilde、function 序列，`ctrlChar` 处理 Ctrl 码，`describeBytes` 把字节序列转成可读文本。

测试密度很高：仪器测试覆盖 `TerminalRendererTest`、`TerminalInputConnectionTest`、`TerminalScrollGestureTest`，单元测试覆盖 Tmux 协调、SSH 键、剪贴板粘贴、滚动策略等，`TmuxCoordinatorTest` 体量尤其大。

## 已核实局限

- 核心是远程终端的 Java 实现，本地没有 Rust VT 状态；本仓的 VT、PTY 与 GPU 绘制归 Rust，不能采用它的分层。
- Tailscale、代理、SFTP、tmux 编排与 WebDAV 备份属于本仓规范未声明且明确不实现的功能，不移植。
- vendored 模块意味着上游修复要手工同步；本仓规范禁止 vendor 可自动下载处理的文件，也不接受这种重复实现路径。

## 与本仓对照

- 尺寸变更与输入共用 FIFO、attach 时持元数据锁入队，是“控制流与字节流分离但顺序一致”的可取做法；本仓的 resize 与输入也要防止旧尺寸覆盖新尺寸。
- 修饰键一次性消费、链接命中用终端坐标而非像素扫描、不可信 IME 长度上界，这些都能转化为本仓的回归用例。
- 大量仪器测试依赖真实设备或模拟器；本仓规范要求在模拟器上可复现、不跳过，借鉴测试组织方式时不能照搬其环境依赖。
