# 可扩展性设计研究

## 结论

优先把项目做成**可被外部引用的库**，而不是在应用内做插件系统；外部复用只允许通过一个**窄而稳定的会话传输接口**进入。二者是同一件事的两面：库化决定交付形态，传输接口决定扩展点。

如果必须只选一个 remote 后端先做，选 **Herdr**：它有公开的 manifest + CLI + socket API、Apache-2.0 的客户端/协议 crate 与 OSC 7501 状态协议；Rex 的客户端协议虽可对接，但产品仍是 beta、接口 undocumented/未稳定、无公开 Rust 客户端库。若“支持 rex/herdr”指的是**支持其扩展机制**而不是连接其服务，那么只需要实现一个传输后端接口，Herdr 的 plugin manifest / CLI 约定与 Rex 的 Lua `rex.action` / `rex.on` 都不应搬进本应用——`PROHIBITED.md` 已禁止插件体系、MCP、termux-api，且禁止执行用户脚本。

## 本仓库现状（扩展性的真实边界）

- `native/Cargo.toml` 是**单个 crate** `native`，`crate-type = ["cdylib", "lib"]`，同时包含 `terminal/`（Ghostty VT、PTY、Session）、`render/`（wgpu/Vulkan）、`android/`（JNI FFI）。`lib.rs` 把三个模块无条件公开，没有 feature 开关；`STYLE.md` 也明确“不得定义任何 crate features”。
- Kotlin 只有 `:app` 一个业务模块（`settings.gradle.kts` 只额外包含 `:benchmark`、`:baselineprofile`），没有可发布的 library 模块。
- `NativeBridge.kt` 是纯 `external fun` 直连 JNI：`initSession` 传 shell 路径与主目录，经 `feedPty`/`encodeKeyText` 送输入，`pollEvent` 排空事件，`attachWindow` 直接传 Surface 指针。设计规范规定“Kotlin 只接收轻量事件与查询结果，不持有逐帧网格快照”，渲染完全在 Rust 侧完成。
- 会话与传输强绑定：`initSession` 的参数是本地 shell + 工作目录，PTY 由 `native/src/terminal/pty.rs` 创建。要接入远程终端，唯一需要松动的就是“数据来源”这一点。
- 已有产品变体机制：`build.gradle.kts` 的 `nix` build type 用 `applicationIdSuffix = ".nix"` 与 `TERMUX_BOOTSTRAP_PRESET=false` 生成可与主包共存的产品。这说明“一套引擎、多个产品”在本仓库已经成立，可沿用。
- 规范硬约束：`PROHIBITED.md` 禁止插件体系、MCP、termux-api、内建命令、持久化会话恢复；`Cargo.toml`、`build.gradle.kts`、`settings.gradle.kts` 属受保护文件，任何库化改造都需用户明确同意。

结论：当前外部项目若想复用，只能 fork 整个 app 或照抄 JNI 接口；没有“嵌入本引擎”的入口。

## 五个候选项目的实际形态

| 项目 | 目标 | 技术栈 | 扩展/复用方式 | 与本项目的契合度 |
| --- | --- | --- | --- | --- |
| [rexpocket](https://github.com/leizhiyuan/rexpocket) | Rex 的 Android 客户端 | Kotlin/Compose + `core` + `native`(libghostty-vt) + Go `tailnet` | 一个控制 WebSocket `rex.control.v1` + 一个数据 WebSocket `rex.data.terminal.v1`；先收 `GHOSTSNP` 整屏快照再收增量；SSH 主机用每条连接一个 exec channel 跑 `rex __internal dial-stdio`；内置 tsnet 节点 | 中高。终端核心可复用，但必须提供“外部字节流/GHOSTSNP → VT → 帧”入口，且不能依赖 Surface |
| [rexling](https://github.com/shulhi/rexling) | 非官方 Rex 客户端 | `rexling-android` + Rust `rexling-core` + `rexling-network` | 同一 Rex API；Kitty 图形、图片上传、端口转发要求 API revision ≥ 11/14；自带 Tailscale 与 SSH | 中高。分层方式最接近“库化”，但网络/认证层庞大，不应整体搬入 |
| [bellterm](https://github.com/nimrc/bellterm) | Herdr daemon 的 Android 客户端 | Rust core + JNI + Kotlin/Compose；`herdr-client`/`herdr-protocol` crate；`russh` 进程内 SSH | Herdr 发**渲染好的单元格网格**，先转成 VT 再喂 libghostty；画面从 Ghostty render state 读回；screen 线程独占非 `Send` 的 libghostty，只发布最新帧 | **最高**。JNI + Kotlin + Rust + Ghostty 的分层与本仓库几乎同构，是最现实的“以本项目为底座”对象 |
| [whip](https://github.com/kosumic/whip) | 跨 Android/iOS 的 SSH + Herdr 客户端 | React Native/Expo + Rust core（uniffi）+ WebView/xterm.js | 终端输出由 Rust 直送 WebView 的 xterm.js，写完成信用回流绕过 JS 线程；`AppCore`/`HostRuntime` 持有权威状态；插件式 host hook `whip-notify` | 低。渲染栈完全不同（xterm.js、非 Ghostty/wgpu），不适合直接复用；其“薄 UI + 强 core + 版本协商”值得借鉴 |
| [herdr-mobile-relay](https://github.com/0cv/herdr-mobile-relay) | 手机审批/监控 Herdr agent | Go relay + 前端 PWA；作为 **Herdr 主机插件**安装 | `herdr plugin install 0cv/herdr-mobile-relay`，`herdr plugin pane open --entrypoint setup` 打开配置面板；事件 hook；Cloudflare/WebRTC/Tailscale 传输；E2E 加密、设备配对 | 最低。它是**主机侧扩展**，手机端是 Web PWA，不消耗本地 PTY/VT 引擎；只能作为“远端协议客户端”对接，不能作为本 app 的插件 |

补充观察：这些项目全都是 2026 年新建、各自独立实现终端栈，没有一方复用另一方。原因不是“插件不够用”，而是各自绑定的**远程协议模型**不同（Rex 的快照 + 独占窗格、Herdr 的单元格网格 + agent 语义、SSH 的裸字节流）。

## 上游扩展机制对比

**Herdr（TUI 线）**：插件是任意 argv 命令，契约在 `herdr-plugin.toml`：`[[build]]`、`[[startup]]`、`[[actions]]`、`[[events]]`、`[[panes]]`、`[[link_handlers]]`。官方明确“没有独立 SDK、没有受限命令集，整个 CLI 就是插件 API”；运行时注入 `HERDR_BIN_PATH`、`HERDR_SOCKET_PATH`、`HERDR_PLUGIN_*` 上下文。socket API 是本地 Unix socket / Windows named pipe 上的**换行分隔 JSON**，`session.snapshot` + `events.subscribe` 构成缓存恢复协议，并明确客户端要忽略未知字段、把不支持的方法当普通错误处理。marketplace 靠 GitHub topic `herdr-plugin` 自动发现。Herdr app/core 线另外提供配置文件 + `mods/*.js` + 布局文件，并明确“可以在 herdr-core 之上构建自己的 app”。

**Rex**：扩展在 Lua（`init.lua`、`rex.action`、`rex.on`、`rex.ui.pick`）、CLI、server API 与 agent skill；程序可用状态协议上报 working/blocked/done。第三方客户端通过公开 WebSocket API + SSH `dial-stdio` 接入。没有进程内插件运行时——这恰好说明“外部客户端复用协议”才是它的扩展方式。

**termux-app**：四模块 `terminal-emulator` / `terminal-view` / `termux-shared` / `app`，库发布到 JitPack 供插件 app 引用；插件通过 `sharedUserId` + `RUN_COMMAND` intent + 自定义权限协作。代价是巨大的契约面（Ten 类对外契约、双门禁、`allow-external-apps`、逗号替代字符等历史包袱），本仓库的 persistence 文档已逐条标注其中大部分为**禁止项**。

## 可落地的最小设计

**唯一扩展点：会话数据来源。** 在 Rust 侧把现有 PTY 会话的“输入/输出/尺寸/退出”抽成一个窄接口，本地 PTY 作为第一个实现；远程后端实现同一接口：把远端字节流或单元格快照转成 Ghostty 能吃的 VT，再走既有 VT 线程与帧路径。Kotlin/JNI 侧只增加一个“远端会话”入口，复用 `feedPty`/`pollEvent` 的语义，不新增逐帧网格回传（否则违反设计规范）。

**唯一交付形态：库。** 把 `terminal/` 从 `native` 中拆成不含 Android/JNI/wgpu Surface 依赖的 crate，保留 `native` 作为 Android 绑定层；Kotlin 侧把可复用部分（bridge、输入编码、会话状态）收进一个 `com.android.library` 模块，`app` 只保留 Activity/服务/设置。发布方式参照 termux-app 的 JitPack 库，而非发布完整 APK。这样 rex/bellterm 类项目是**引用**本引擎，而不是 fork 它。

**不做的三件事**：不做应用内插件运行时/脚本加载；不做 MCP 与 termux-api 兼容层；不做 `sharedUserId` + `RUN_COMMAND` 那套 Android 插件契约。它们与 `PROHIBITED.md` 直接冲突，且 termux 的历史包袱证明成本远高于收益。

**先做一个后端**：Herdr。理由是它有稳定的本地 socket JSON API + `herdr-client`/`herdr-protocol` Rust crate 可直接使用，OSC 7501 让 agent 状态无需解析屏幕文本；Rex 放在第二步，且只接 `rex.data.terminal.v1` 数据流，不碰其控制面/窗格所有权语义。

## 逐项目落地评估

- **bellterm**：最佳。可直接用本引擎替换其 `vt.rs`/`screen.rs`/JNI 数据打包层，保留其 `ssh/`、`keys.rs`、`herdr-client`；风险是其单元格→VT 转换与“Kotlin Canvas 绘制”假设与本项目“Rust 直接绘制 Surface”相反，需要先确认能否只复用 VT/会话层。
- **rexpocket / rexling**：中等。可复用 VT + 快照解析，但二者都自有 Kotlin UI、SSH/Tailscale 栈与窗格所有权逻辑；复用范围限于引擎库，不能期待它们“作为扩展跑在本 app 里”。
- **whip**：不适配为扩展；只借鉴其分层与测量方式（UI 不碰核心状态、按 capability 协商协议版本、性能有基准断言）。
- **herdr-mobile-relay**：作为**主机侧 Herdr 插件**保留原位；与本 app 的关系是“远端能力提供者”，不是插件宿主。若要做同类功能，照它的 `herdr-plugin.toml` + pane entrypoint + 事件 hook 模式，而不是塞进本 app。

## 可复用的最佳实践（来自上述项目的一致做法）

1. 核心保持权威状态，UI 只消费**类型化投影**与事件；不要用 UI 自己的模型再存一份状态（whip、herdr、本项目设计规范一致）。
2. 接口窄且版本化：协商端点/能力，未知字段忽略、不支持的方法当普通错误，别把客户端和服务端编译期绑定（herdr、whip）。
3. 缓存用 `snapshot + events.subscribe` 重建，事件只作为失效信号，不假设可重放（herdr socket API）。
4. 插件/扩展与核心解耦：核心只提供稳定 CLI/API，扩展自选语言、自管状态；不内置 SDK、不沙箱（herdr）。这是最省代码、也最符合本项目“不做不必要功能”的做法。
5. 专注一个生态优于通用：whip 明确拒绝 tmux/zellij 一类一等支持，换来一致体验；本项目也不应同时追 Rex + Herdr + 通用 SSH。
6. 扩展点放在**数据源**而非渲染器：渲染与 VT 是本项目最强的资产，但耦合 Surface，外部难以复用；把可复用边界划在 VT 之前。
7. 文档与接口同步：本仓库 `AGENTS.md` 要求 openspec/specs 随实现更新，新增传输接口必须同时补规范与测试，避免接口漂移。

## 风险与需确认项

- 拆分 crate、新增 Gradle library 模块需要修改受保护的 `Cargo.toml`、`settings.gradle.kts`、`build.gradle.kts`；**本轮未改动任何文件**，需用户明确同意后才能实施。
- 传输接口意味着接受远程数据进入 VT；本项目规范声明“基本不考虑安全性、效率优先”，远程路径的主机密钥/凭据处理需单独决定边界（参考 rexpocket 的密钥固定策略，但不要引入其 Keystore 加密体系作为前提）。
- 选择 Herdr 还是 Rex 作为第一个后端属于产品决策；本报告给出建议顺序，最终需用户确认。
- 未做的工作：没有验证 Herdr 单元格网格转 VT 在本引擎上的实际帧一致性，也没有实测 Rex 快照流与 `GHOSTSNP` 分片的兼容成本；这两项需要在选定后端后用原型验证。
