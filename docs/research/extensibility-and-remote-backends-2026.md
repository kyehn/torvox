# 可扩展性与远程后端研究报告

## 摘要

本报告回答四个问题：支持 Rex／Herdr 时应走「应用内扩展程序」还是「让本项目更容易被外部引用」；五个外部项目作为本项目扩展或基于本项目开发的实际效果；能否对标 termlib 那样做成组件库；在「大概率没有外部库使用者」前提下的成本收益选择。

结论先行：

- 三个问题彼此独立。插件框架不会自动产生 Rex／Herdr 支持能力，组件库也不会自动产生远程终端能力，真正的技术前置是**会话字节传输与生命周期契约**。
- 本仓当前没有任何扩展面：没有插件运行时，没有可发布的库产物，`DESIGN.md` 也未声明远程／网络终端形态；`Session` 的注入点被 `pub(super)` 封死，`Pty` trait 又把字节读写与本地 OS 能力（pid、winsize ioctl、fd）混在一起。
- 推荐顺序是：先在 `docs/specification/DESIGN.md` 声明会话传输抽象面，再在 `native/src/terminal/` 内做受限重构（字节源 + 生命周期契约），然后把 Herdr／Rex 作为独立适配器挂到该接口；库化交付与插件运行时暂缓。重构的前置条件是补规范声明（`STYLE.md` 禁止实现未声明的功能），库化的前置条件是取得受保护构建文件的明确同意。

## 研究方法

原则：只用一手资料。对外部项目一律直接克隆仓库并阅读源码、构建脚本与协议定义，不引用项目自述以外的二手描述；对本仓结论一律回到本仓源码与 `docs/specification/` 规范核对。

| 外部仓库 | 研究坐标 | 形态 |
| --- | --- | --- |
| `leizhiyuan/rexpocket` | `v0.1.3` | Android 远程 shell 客户端（`core`／`app` 分层 + Ghostty JNI） |
| `shulhi/rexling` | master 快照（无发布标签） | Rust 协议核心 + Android 壳 |
| `nimrc/bellterm` | `v1.1.3` | Android Herdr 客户端（Rust JNI + 独立协议 crate） |
| `0cv/herdr-mobile-relay` | master 快照（无发布标签） | Go 中继（协议与动作目录） |
| `kosumic/whip` | `v2.4.1` | 远端主机客户端（Rust 核心 + React Native 壳） |
| `termux/termux-app` | master 快照（`versionName` 为 `0.118.0`） | Android 终端应用（多个 library 模块） |
| `connectbot/termlib` | `v0.3.16-SNAPSHOT` | Compose 终端组件库 |

本仓约定文档不写 git 提交号，因此上表用标签或快照标识研究基线；精确提交号记录在本次变更的归档记录里（归档台账是历史证据，允许保留提交号）。

## 本仓当前的扩展接缝

### 分层与模块公开度

链路是 Kotlin UI → JNI → Rust 会话 → PTY → Ghostty VT → wgpu 渲染。`native/src/lib.rs` 中 `terminal`、`render`、`android` 都是 `pub mod`；`native/src/terminal/mod.rs` 中 `ghostty_terminal`、`output_processor`、`pty`、`session`、`shell_env`、`theme` 均为公开模块，并再导出 `Pty`、`PtyError`、`PtyPair`、`MockPty`、`MockPtyHandle`、`ShellEnv`、`CellData`、`CursorInfo`、`CursorStyle`。

结论：Rust 侧模块边界本身已经公开，库化的阻碍不在「模块不可见」，而在构建产物形态与发布元数据。

### 会话创建与注入点

`native/src/terminal/session/spawn.rs` 的 `Session::spawn` 走 `PtyPair::spawn`；同文件的 `Session::spawn_with_pty` 接受 `Box<dyn Pty>`，但可见性是 `pub(super)`。`native/src/terminal/session/pty_master.rs` 的 `PtyMaster::new` 同样是 `pub(super)`。`Session` 字段 `pty: PtyMaster`，而 `PtyMaster` 持有 `Arc<dyn Pty>`。

结论：外部代码无法注入自定义传输。`Pty` 是公开 trait，但当前只有本地 PTY 一个实现，`mock_pty` 仅在测试内使用。

### 输出推进与背压

`native/src/android/ffi/render/pump.rs` 调 `Session::process_output`；缺失推进即死锁。字节进入 VT 有两条语义不同的路径：`GhosttyTerminal::try_pty_write` 做 NUL 剔除与裸 LF→CRLF 成形，通道满时原样返还、绝不丢字节；`GhosttyTerminal::vt_write` 只做输入清洗直透。背压单槽的契约见 `openspec/specs/pty-output-backpressure/spec.md`。

结论：下游「字节→VT」管线本身与传输无关，字节源是天然的替换点；但两条写入路径的成形语义不同，接缝设计必须明确哪一条用于远程后端。

### 生命周期与退出

`native/src/terminal/session/lifecycle.rs` 的 `Session::request_exit` 先断输出通道，再对本地进程组执行挂断→唤醒→宽限→强杀；退出码由 `take_reported_exit` 取出。

结论：生命周期契约与「本地进程」强耦合。远程后端必须同时替换「请求退出」与「取退出码」两件事，不能沿用 pid／信号路径。

### Kotlin 侧链路

输入：UI → `TerminalViewModel.writeToPty` → `TerminalRuntime.writeToPty` → `Bridge.writeToPty` → `NativeBridge.feedPty` → JNI `feed_pty_inner`。批量输入缓冲已有接缝：`Bridge.ptyWriteQueue` 与 `InputBatchBuffer`。输出：`RenderSupervisor` → `Bridge.waitOutput` → `Bridge.renderWithNewOutput`；渲染暂停时由 `TerminalRuntime.startSessionPump` 调 `NativeBridge.pumpSessions` 维持推进。尺寸：`TerminalRuntime.resize` 与 `TerminalRuntime.setPixelSize` → `NativeBridge.resize` 与 `NativeBridge.setPixelSize`。

结论：Kotlin 侧已把「写入字节」与「读取网格」分成两条独立通道，远程后端不需要改动 Kotlin 层的数据通路。

### 不可用作接缝的位置

- 渲染与 Surface 层：与字节源无关，替换它得不到远程能力，只会重写 wgpu 管线。
- JNI 层：Android 专属边界，不是可复用面。

## 三种形态的评估

### 应用内扩展程序

指在应用内提供插件注册、动态加载或脚本执行能力，让第三方在运行时扩展终端行为。

- 收益：不产生任何 Rex／Herdr 能力。五个外部项目没有一个是「可加载插件」，它们都是独立客户端或协议库。
- 成本：需要新的运行时、权限与安全面；脚本加载、事件总线、多路复用此前已被判为未声明不立项。
- 结论：在当前形态下不采纳。若日后确需「运行时扩展」，更现实的是**编译期适配器**（把 Herdr／Rex 后端作为仓内一个 Rust 模块接入），而不是插件运行时。

### 库化交付

指让本项目可被外部引用：Rust crate 发布，或 Android library（AAR）交付。

- 现状：`native/Cargo.toml` 已有 `crate-type = ["cdylib", "lib"]`，但缺 `license`、`repository`、`readme` 等发布元数据，且依赖 git 源 `libghostty-vt`（固定 rev），直接发布 crates.io 会把上游 rev 约束带给下游。Android 侧只有 `com.android.application` 模块（`:app`、`:benchmark`、`:baselineprofile`），根构建脚本虽声明了 `com.android.library` 插件版本但 `apply false`，没有可发布的 library 模块，也没有 `maven-publish` 配置。
- 障碍：Rust crate 发布需改 `Cargo.toml`；Android library 拆分需改 `settings.gradle.kts` 与 `build.gradle.kts`。三者都是受保护文件，须取得明确同意。
- 收益：取决于外部使用者数量。在「大概率没有使用者」的前提下，收益接近零，而代价是长期维护公开 API 稳定性的义务，与「代码量最小」「不实现未声明功能」两条现行约束相悖。
- 结论：**暂缓**。若日后要做，第一步应是 Rust crate 的发布元数据（改动面最小、对外承诺最小），而不是先拆 Android 模块。

### 独立传输适配器

指把「会话」与「本地 PTY」解耦，让字节源可替换，Herdr／Rex 作为独立后端接入。

- 需要的改动：拆 `Pty` trait（字节读写 + 尺寸协商）与本地 PTY 能力（pid、winsize ioctl、fd 克隆）；把 `Session::spawn_with_pty` 与 `PtyMaster::new` 的可见性提到 `pub(crate)`；把读取线程从「fd + poll」泛化为「字节源」；把生命周期抽象为「请求退出 + 取退出码」。VT、渲染、JNI 契约不变，背压语义按现有规范逐字节保持。
- 前置条件：这是新能力面，`DESIGN.md` 未声明远程／网络终端形态，须先补声明再实现（`STYLE.md` 明禁未声明功能）。
- 收益：这是唯一能同时服务于「支持 Herdr」「支持 Rex」「未来任何非本地后端」的前置投入，且与 Termux 分层、termlib 回调契约的方向一致。
- 结论：**推荐**，但按「先声明、后重构、再接后端」三步推进。

## 外部项目逐仓评估

### rexpocket

结构为独立 `core/` Kotlin 模块 + `app/` 模块 + native Ghostty JNI 层。`core` 侧有 `RexClient`、`StreamDecoder`、`TerminalStream`、`PocketSession`、`SshTunnel`；`app` 侧有 `GhosttyVt`、`TerminalSession`、`TerminalFrame`。协议分控制面与数据面（`rex.control.v1`、`rex.data.terminal.v1`），并有 `GHOSTSNP`、`dial-stdio`、`own_block`、`watch_pane` 等机制。

可复用资产：协议与终端渲染分层的**形态**（客户端协议层与 VT 层各自独立）。不可直接复用：Kotlin 侧承担了较多协议与流状态；本仓若照搬，会把协议真值放到 Kotlin，与本仓「会话归 Rust」的既有分层相反。

结论：作为**形态参照**有价值；作为「基于本项目开发」的起点不可行——它的协议栈与本仓会话模型不兼容，且它的终端层本身也走 Ghostty JNI。

### rexling

Rust workspace，核心 crate `rexling-core`（Apache-2.0，edition 2024），公开模块 `control`、`data`、`ghostty`、`snapshot`、`transport`、`upload`、`graphics`、`forward`、`network`。`transport.rs` 定义两个 WebSocket 路径常量 `CONTROL`（`rex.control.v1`）与 `TERMINAL`（`rex.data.terminal.v1`）；`snapshot.rs` 实现有界 `SnapshotBuffer`，按步骤推进快照解析并区分 ready 与 finish；`network.rs` 提供 Unix stream 连接与外部命令执行。Android 侧只有 `:app` 与 `:benchmark`，没有可发布的 library 模块。

可复用资产：控制流与终端字节流**分离**的双通道设计、有界快照缓冲、协议 crate 与传输实现的分层。这三点与 Whip、Bellterm 相互印证，是本仓接缝设计最值得吸收的三条。

结论：作为「Rust 侧拥有协议真值」的参照最贴近本仓；代码本身不可直接引入（依赖树、协议版本与本仓无关联）。

### bellterm

原生 Android Herdr 客户端，分层为 `app`（Kotlin／Compose UI、JNI wrapper、Android services）、`native`（Rust JNI 核心：`russh` 传输、`libghostty-vt` 屏幕、输入路由）、`crates/herdr-client`、`crates/herdr-protocol`，依赖方向 `native` → `herdr-client` → `herdr-protocol`。

`herdr-protocol` 用 generation-1 wire 声明与有界 bincode 分帧，做原子 surface 校验与 `PaneSurfaceFrame::apply_patch`，用 boot ID、base projection／surface revision、next surface revision 做 fencing。`herdr-client` 是独立 crate，不链接 Herdr、GPUI、ratatui、crossterm、Tokio 或 PTY 实现；它有 boot ID fencing、有序命令队列、单一 in-flight API 请求、事件背压；**没有自动重连或 replay**；boot 变化、regressing snapshot 或畸形帧都会断连；snapshot 与 surface 必须按 `(boot_id, projection_revision)` 匹配。

关键事实：其 README 明确写出 **Herdr 发送渲染后的 cell grid，而不是字节流**。它把 Herdr surface 帧编码成 VT 字节喂给本地的 `libghostty-vt`，再读回渲染状态。

结论：这是最完整的 Herdr 客户端参考，但它与本仓**存在模式冲突**——本仓自己就是 Ghostty 模拟器，直接接 Herdr 的语义 surface 会绕过或重复本仓的终端状态。采纳它的协议 crate 分层、revision fencing、事件背压与后台阻塞操作纪律；不采纳它的 surface 模式，除非本仓明确改为「服务端权威渲染」。

### herdr-mobile-relay

Go 实现的 Herdr 与手机之间的中继，不是终端模拟器。`internal/protocol/protocol.go` 定义协议版本 3、加密 WebSocket 子协议 `herdr-e2ee-v2`、混合传输能力标记；`TargetRef` 带 `server_session_id`／`pane_id`／`terminal_id`／`generation`／`agent_session_id`；动作有 `ActionReceipt` 与阶段（`not_started` 等）、错误码（`invalid_request`、`unknown_action`、`incompatible_protocol`、`reader_denied`）、设备角色（`reader`／`controller`）与动作分类（`read_only`／`mutating`）；`Inbound` 结构带 `Generation`、`SourceRevision`、`ExpectedRevision` 等乐观并发字段；动作目录含 `watch_pane`、`send_input`、`send_keys`、`lease_pane_size`、`pane_applied` 等终端相关项以及大量工作区、代理、推送、语音项。帧契约测试确认：非 JSON object 的帧被拒；加密连接遇非法帧直接关闭，普通连接只拒该帧；`ping` 只接受裸 `{"type":"ping"}`；有序入口队列满时返回「未开始」阶段而不是假装已发。

结论：它提供的是**协议治理**经验（世代号、乐观并发、动作分类与审计、确定性错误处理），不是终端能力。对本仓的价值限于「若日后做远程后端，状态同步必须带世代号」这一条纪律；其庞大的动作目录与本仓无关。

### whip

远端 Herdr server 的移动客户端，不在设备上启动 Herdr runtime、shell 或 PTY。三平面：Transport（SSH、凭据、known-host、连接事务）、Control（Rust `AppCore`／`HostRuntime`／`HostState`）、Terminal（每个终端独立 Herdr stream-local channel，二进制终端帧）。

其 `ARCHITECTURE.md` 写明的纪律：Rust 是状态与协议真值拥有者，UI 层只做展示与平台适配；`AppCore` 管应用级 session／选择／terminal rail，`HostRuntime` 管单 host 连接、重连、快照／事件与终端传输；控制流与终端字节流分离，失败互不影响；终端热路径保持二进制字节、不转 JSON、不破坏 partial UTF-8；用 generation／epoch fencing 防止旧连接、旧回调、旧快照污染新状态；快照与事件做 reconciliation，事件在快照请求期间缓冲并重放；新服务端能力作为中立 API 方法而非移动专用端点；协议 crate 独立于 SSH、UI 框架、网络、计时器与全局状态；认证、取消、重连、凭据事务、known-host 信任事务由 Rust 队列／协调器拥有。

结论：这是本次研究中最有价值的**方法论**来源。它证明「远程终端」的正确切分是控制面与字节面分离 + 状态由协议层拥有 + 世代隔离，而不是在 UI 里拼装网络连接。本仓若做远程后端，应把协议真值放在 Rust 侧，与本仓既有分层一致；其具体状态机不宜整体照搬。

## 对标参照

### Termux 的分层与发布

Termux 的可复用性来自**同一仓库内按职责拆 Android library 模块并各自 Maven 发布**：`terminal-emulator`（自研 VT + JNI）、`terminal-view`（Compose 视图，依赖 `terminal-emulator`）、`termux-shared`（会话与终端服务，依赖 `terminal-view`）。三个模块都是 `com.android.library` + `maven-publish`，发布坐标为 `com.termux:terminal-emulator`、`com.termux:terminal-view`、`com.termux:termux-shared`。

对照本仓：`android/settings.gradle.kts` 只有 `:app`、`:benchmark`、`:baselineprofile` 三个模块，应用是 `com.android.application`，没有任何 library 模块与 `maven-publish` 配置。结论：Termux 的「分层」靠的是**模块边界 + 发布流程**两件事，不是应用内插件运行时；本仓连模块边界都未建立，因此「照 Termux 做成库」不是一步到位的事，而是一次结构性改造。

### termlib 的库化形态

termlib 是 ConnectBot 的 Compose 终端组件库（Kotlin + C++／libvterm，MIT）。`lib/build.gradle.kts` 核验：`com.android.library`、`kotlin.compose`、`kotlin-parcelize`、roborazzi、`com.vanniktech.maven.publish`、metalava、dokka、kover、sonarqube；`namespace = "org.connectbot.terminal"`、compileSdk 37、minSdk 24、Compose 特性开启、CMake externalNativeBuild、consumerProguardFiles；`mavenPublishing { publishToMavenCentral(automaticRelease = true, ...); signAllPublications() }`，坐标 `org.connectbot:termlib`；POM 填写名称、描述、起始年份、URL、许可、开发者与 SCM；另有 Dokka 文档、`metalava` API 签名检查、`kover`／`sonarqube`／`roborazzi` 质量门禁。

其内部回调契约 `TerminalCallbacks` 是 `internal`：`onKeyboardInput(data: ByteArray)` 的注释明确要求调用方把数据写到 PTY／传输；回调在 native Terminal 的非重入 mutex 保护下触发，实现方不得在回调里同步调用 `writeInput()`、`resize()`、`dispatchKey()`、`dispatchCharacter()`、`getCells()`。

结论：termlib 已经把「终端模拟」与「传输」分开，这是本仓最值得借鉴的一点——它印证了接缝应该落在**字节流边界**，而不是让终端库自己管进程。但它的 API 形态（Compose 组件 + 内部回调）依赖 C++／libvterm 栈，与本仓 Rust／wgpu／Ghostty 栈不可直接对齐。

## 能力模型对照

termlib 现有能力与本仓现状：

| 能力 | termlib | 本仓现状 |
| --- | --- | --- |
| 256 色与真彩色 | 有 | 有（上游 Ghostty 解析） |
| 东亚双宽字符 | 有 | 有（`cjk-rendering` 能力） |
| 组合字符与复杂脚本整形 | 有 | 由上游与字形图集承担 |
| 触摸选择（放大镜） | 有 | 有（`text-selection` 能力） |
| 滚动与回滚 | 有 | 有（行数固定 2000） |
| 缩放 | 有 | 有（`font-selection` 能力） |
| 动态尺寸调整 | 有 | 有（`startup-grid-resize`、resize 契约） |
| 多字体 | 有 | 有（`font-selection` 能力） |
| 内联图像（iTerm2 `imgcat`、Kitty graphics） | 有 | 有（`kitty-graphics` 能力） |
| 强制尺寸终端与像素尺寸回调 | 计划中未完成 | 有（`setPixelSize` 已接线） |
| Pasting、shell prompt 集成、URL／编译错误扫描 | 计划中未完成 | 未声明 |

接口一致性的现实边界：

- **不可对齐**：API 形态（termlib 是 Compose 组件加内部回调；本仓是 Rust 渲染加 JNI 批量查询）、VT 核（libvterm 对 libghostty-vt）、图形栈（C++ Canvas 对 wgpu／Vulkan）、构建形态（C++ 静态库加 CMake 对 Rust cdylib 加 cargo ndk）。
- **可对齐**：能力模型（网格、选择、滚动、颜色、宽字符、图像）与少量稳定数据契约——尤其是「终端只产出字节与网格，传输由调用方负责」这一分工，以及「回调内不得反向调用终端」这一重入纪律。
- **若走库化路线，最小可交付**是：渲染帧数据契约、输入字节流外送、尺寸协商三项，而不是整个终端视图。

## 成本收益矩阵

| 形态 | 前置条件 | 改动面 | 持续维护成本 | 直接收益 | 无外部使用者前提下的性价比 | 结论 |
| --- | --- | --- | --- | --- | --- | --- |
| 应用内插件运行时 | 新增运行时与安全面声明 | 运行时、注册表、脚本宿主 | 高（安全面、兼容性） | 零（五个外部项目都不是插件） | 负 | 不采纳 |
| Rust crate 发布 | 改 `Cargo.toml` 发布元数据；上游 git 依赖需改坐标 | 单个受保护文件 | 中（API 稳定性承诺） | 低（依赖者少） | 低 | 暂缓 |
| Android library 交付 | 改 `settings.gradle.kts`、`build.gradle.kts`，拆模块 | 两个受保护文件 + 模块重组 | 高（双构建体系、API 冻结） | 低 | 低 | 暂缓 |
| 会话传输抽象（字节源 + 生命周期） | `DESIGN.md` 补声明 | `native/src/terminal/**` 内部重构 | 中（接缝一旦稳定就不再变） | 高（远程后端的唯一前置） | 高 | 推荐 |
| Herdr／Rex 适配器 | 上一项完成 + 产品决策 + 协议取舍 | 新增后端模块 | 中高（协议演进、fencing、重连） | 高（直接满足目标） | 高（目标本身） | 目标态 |

## 结论与建议

### 事实

- 本仓没有插件运行时、没有库化交付物、没有传输抽象面；`DESIGN.md` 未声明远程／网络终端形态。
- `Session::spawn_with_pty` 与 `PtyMaster::new` 的可见性是 `pub(super)`，外部无法注入自定义传输。
- `Pty` trait 把字节读写与本地 OS 能力混在一起；`Session::request_exit` 走本地进程组信号路径。
- VT、渲染、JNI 三层的契约与本仓传输无关；输出推进链路是「fd 读取线程 → 有界通道 → `process_output` → VT」。
- `native/Cargo.toml` 无发布元数据且依赖 git 源上游；Android 侧无 library 模块与 `maven-publish` 配置。
- `Cargo.toml`、`build.gradle.kts`、`settings.gradle.kts` 是受保护文件；`STYLE.md` 禁止实现未声明功能。
- 五个外部项目里没有可加载插件；其中三个（Rexling、Whip、Bellterm）以 Rust 侧持有协议真值，一个（Herdr Mobile Relay）只提供协议治理，一个（RexPocket）把协议状态放在 Kotlin 侧。

### 推论

- 三种形态互不替代。若目标是「支持 Rex／Herdr」，插件框架与组件库都不产生该能力；必要且充分的前置是字节传输与生命周期接缝。
- 在无外部使用者的前提下，先做库化是把有限精力投在收益最不确定的一侧；先做接缝则收益确定且对未来的所有后端通用。
- 接缝的最小形态是：字节源（写、读、尺寸协商）+ 生命周期（请求退出、取退出码）两组契约，本地 PTY 能力（pid、winsize ioctl、fd）单独留在本地实现侧。
- 远程后端必须自带世代隔离与状态对账；Bellterm 与 Herdr Mobile Relay 在这一点上完全一致，可直接作为设计输入。
- 若选 Herdr，必须先裁决「字节流模式」与「服务端渲染 surface 模式」：本仓自带 Ghostty 模拟器，直接接 surface 会重复终端状态。
- 若选 Rex，Rexling 与 RexPocket 的协议分层可直接借鉴，但须把协议真值放在 Rust 侧，而不是照搬 RexPocket 的 Kotlin 侧状态组织。
- termlib 式的组件库方向可行但成本最高，且须与本仓 Rust 渲染栈做接口设计，不是把现有代码打包即可。

### 未知

- 本项目外部使用者的实际规模与形态（决定库化收益的真实值）。
- Herdr 与 Rex 协议的稳定性、版本演进策略与许可条款。
- 产品层是否要支持远程终端、以何种形态支持（单一远程会话还是与本地会话并存）。
- 远程会话是否需要断线重连与状态恢复；若需要，`PROHIBITED.md` 的会话持久化边界如何界定。

### 推荐执行顺序

- 补 `docs/specification/DESIGN.md`：声明「会话传输面」这一最小抽象（字节源与生命周期契约），不声明具体远程产品形态。这是其余步骤的前提。
- 在 `native/src/terminal/` 内做受限重构：拆分字节读写与本地 PTY 能力；把注入点可见性提到 `pub(crate)`；读取线程泛化为字节源；生命周期改为「请求退出 + 取退出码」。VT、渲染、JNI 契约与背压语义保持不变。
- 选定后端并实现为独立适配器：先裁决字节流模式与 surface 模式，再落地；协议 crate 与传输实现分层，状态真值在 Rust 侧，世代号与快照对账按 Bellterm／Whip 的纪律实现。
- 库化与插件运行时暂缓。若日后确有外部使用者，先做 Rust crate 发布元数据，再评估 Android library 拆分；两者都需先取得受保护构建文件的明确同意。

### 不做清单

应用内插件运行时与脚本加载、事件总线与多路复用、外部服务与 AI 助手集成、Termux API 兼容层、自定义环境变量、跨设备剪贴板同步、会话持久化与恢复、后台图与自定义主题——这些此前已被判为明禁或未声明，本研究不重开。

## 复现方式

- 本仓证据：`native/src/terminal/pty.rs` 的 `Pty` trait；`native/src/terminal/session/spawn.rs` 的 `Session::spawn` 与 `Session::spawn_with_pty`；`native/src/terminal/session/pty_master.rs` 的 `PtyMaster`；`native/src/terminal/session/lifecycle.rs` 的 `Session::request_exit`；`native/src/terminal/session/mod.rs` 的 `Session::process_output`；`native/src/android/ffi/render/pump.rs`；`native/src/android/ffi/render/frame.rs`；`android/app/src/main/java/terminal/emulator/bridge/Bridge.kt` 的 `writeToPty`、`waitOutput`、`renderWithNewOutput`；`android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt` 的 `startSessionPump`、`resize`、`setPixelSize`；`native/Cargo.toml`；`android/settings.gradle.kts`；`android/app/build.gradle.kts`。
- 外部证据：上表七个仓库的对应源码路径——`rexpocket/core/src/main/kotlin/me/leaver/rexpocket/core/`、`rexling/rexling-core/src/transport.rs` 与 `snapshot.rs`、`bellterm/crates/herdr-protocol/`、`bellterm/crates/herdr-client/`、`bellterm/native/src/`、`herdr-mobile-relay/internal/protocol/protocol.go` 与 `internal/transport/frame_contract_test.go`、`whip/ARCHITECTURE.md`、`termux-app/terminal-view/build.gradle` 等三个模块构建脚本、`termlib/lib/build.gradle.kts` 与 `termlib/lib/src/main/java/org/connectbot/terminal/TerminalCallbacks.kt`。
- 精确提交号见本次变更的归档记录（`openspec/changes/archive/2026-11-20-research-remote-backends/`）。
