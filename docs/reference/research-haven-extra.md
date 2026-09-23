# 深度研究补充：GlassHaven/Haven（除 feature/terminal 外全部模块）

> 研究日期：2026-08-06 | 克隆位置：`repositories/refs/haven`（depth 1）
> 语言：Kotlin（约 35 个 Gradle 模块，Hilt + KSP + Room）+ Rust（RDP）+ Go（rclone/wireguard）
> 前篇：`research-haven.md` 已覆盖 `feature/terminal/SelectionToolbar.kt`（智能复制/跨行 URL 选择）
> 本文件覆盖：`feature/editor`、`feature/connections`、`feature/keys`、`feature/settings`、`feature/tunnel`、`feature/rdp`、`feature/mail`、`core/*`（全部 30 个子模块）、`dev/`、`et-kotlin/`、`mosh-kotlin/`、`rnsh-kt/`、`spice-kotlin/`、`wayland-android/`、`build-proot/`、`scripts/`
> 对比基准：torvox（Rust 终端核心 libghostty-vt + Kotlin/Compose 壳，纯本地终端，无 SSH）

## 0. 重要前置说明：子模块全部未初始化

`.gitmodules`（`.gitmodules:1-27`）声明 8 个子模块：`termlib`（GlassOnTin/termlib）、`mosh-kotlin`（GlassOnTin/ssp-transport）、`et-kotlin`（GlassOnTin/et-kotlin）、`build-proot/proot-termux`（GlassOnTin/proot，分支 `haven-299-fd-fix`）、`wayland-android`、`reticulum-kt`、`rnsh-kt`、`spice-kotlin`。**本次克隆中这些目录全部为空**，无法读到库源码本身。但主仓库 `core/mosh`、`core/et`、`core/reticulum` 等模块内含有对它们的调用与完整协议注释（尤其 `core/mosh/MOSH.md`），足以还原协议全貌。`rdp-kotlin/`、`rclone-android/` 不是子模块，内容在仓库内（Rust/Go 源码）。

## 1. 模块总览

| 模块 | 定位 | 规模 | torvox 有无对应 |
|------|------|------|----------------|
| `feature/editor` | 远程/本地文本编辑器（Sora Editor + TextMate） | 2 文件 + assets | 无 |
| `feature/connections` | 连接管理主屏（SSH/mosh/ET/Reticulum/本地/VNC/RDP/SPICE/SMB/rclone） | 25 文件，ViewModel 280KB | 无 |
| `feature/keys` | **SSH 密钥管理**（生成/导入/FIDO2/step-ca/TOTP） | KeysViewModel 1.1K 行 | 无 |
| `feature/settings` | 设置（12 个折叠分区） | SettingsScreen 185KB | 有（torvox SettingsScreen） |
| `feature/tunnel` | 隧道管理 UI（WireGuard/Cloudflare/Tailscale） | TunnelViewModel 282 行 | 无 |
| `feature/rdp` | RDP 客户端屏幕 | RdpScreen 68KB | 无 |
| `feature/vnc` | VNC 客户端屏幕 | VncScreen 97KB | 无 |
| `feature/mail` | IMAP 邮件（规则/多账户） | 10 文件 | 无 |
| `feature/sftp` | 统一文件浏览器（SFTP/SMB/rclone/本地/proot） | SftpViewModel 268KB | 无（torvox 有 TerminalDocumentsProvider） |
| `feature/imagetools` | 图片裁剪/透视/旋转 | 6 文件 | 无 |
| `core/ssh` | SSH 双后端（jsch + sshlib）+ 端口转发 L/R/Dynamic | 大 | 无 |
| `core/tunnel` | Cloudflare Access WebSocket 隧道、SOCKS5、TunneledSocket | 中 | 无 |
| `core/mosh` | **纯 Kotlin mosh 客户端**（AES-OCB + SSP + 手写 protobuf） | 2 文件 + MOSH.md | 无 |
| `core/et` | Eternal Terminal（TCP 2022）客户端 | 2 文件 | 无 |
| `core/reticulum` | Reticulum 网状网络（rnsh）传输 | 中 | 无 |
| `core/local` | proot 本地 Linux + 音频桥 + 桌面管理器 | 大（DesktopManager 1.5K 行） | 无（torvox 用 Termux bootstrap） |
| `core/wayland` | Wayland 合成器渲染桥（JNI） | 中 | 无 |
| `core/scan` | ZXing 条码 + Tesseract OCR | 中 | 无 |
| `core/security` | age 加密、Keystore、Tink、生物识别、OtpAuthUri | 中 | 无 |
| `core/data` | Room + DataStore + AgentConsentManager（MCP 同意门控） | 大 | 无（torvox 有 SettingsRepository） |
| `core/knock` + `core/spa` | TCP 端口敲门 + fwknop SPA 单包授权 | 小 | 无 |
| `core/stepca` / `core/mcp` / `core/ffmpeg` / `core/fido` | step-ca 证书 / MCP agent / FFmpeg 封装 / FIDO2 | 小/中 | torvox 仅有 MCP server 设置开关 |
| `core/vnc` `core/rdp` `core/spice` `core/smb` `core/mail` `core/rclone` | 各协议客户端封装 | 中 | 无 |
| `core/usb` `usbserial` `btserial` `bleserial` | USB/蓝牙串口终端 | 小 | 无 |
| `core/terminal-haven` `core/toolbar` | terminal 依赖的公共件 | 小 | — |
| `rdp-kotlin/` | IronRDP + UniFFI Kotlin 绑定（**有内容**） | Rust lib.rs 147KB | 无 |
| `rclone-android/` | gomobile 单 .so：rcbridge + wgbridge + tsbridge + socks5 | Go 源码 | 无 |
| `et-kotlin/` `mosh-kotlin/` `rnsh-kt/` `spice-kotlin/` `wayland-android/` `termlib/` | 空子模块（见 §0） | — | — |
| `build-proot/` | proot 交叉编译脚本 + vendored talloc | build.sh 157 行 | 无 |
| `scripts/` `dev/` | CI/质量检查 + mosh 故障注入 | 13 个脚本 | torvox 有 scripts/（可对比） |

## 2. feature/editor —— 文本编辑器（Sora Editor + TextMate）

### 2.1 定位与架构

`feature/editor` 是**终端外挂的文本编辑器**：从 SFTP/文件浏览器/agent 打开远端文件（`EditorState.Loaded` 携带 `content/fileName/filePath/charset`），编辑后回写。核心是 **Rosemoe Sora Editor**（`io.github.rosemoe.sora`，Android 上最成熟的代码编辑器控件）通过 `AndroidView` 嵌进 Compose，语法高亮用 **tm4e（Eclipse TextMate）** 引擎加载 assets 里的 TextMate 语法。

### 2.2 核心文件

**`EditorScreen.kt`（430 行）**
- `EditorState` sealed interface `:63`：`Idle/Loading/Loaded(content, fileName, filePath, charset)/Error` —— 编辑器三态模型
- `EditorScreen()` `:77`：Compose 壳。顶栏 `:106` 用 `"\u2022 ${fileName}"` 显示脏标记；`LaunchedEffect(state)` `:96` 载入新内容时重置 `isDirty`；光标行列/undo/redo 状态 `:87-93`
- `FindReplaceBar` `:245`（约）：搜索/替换面板——`doSearch()` `:251` 调 `editor.searcher.search(...)`，支持**正则**（`EditorSearcher.SearchOptions.TYPE_REGULAR_EXPRESSION` `:259`）、`matchIndex/matchCount` 计数 `:264-265`、`gotoNext/gotoPrevious` `:281-304`、`replaceThis/replaceAll` `:337-348`；`LaunchedEffect(searchText, useRegex)` `:268` 输入即搜
- `EditorContent()` `:360`：`AndroidView(factory/update)` 包装 CodeEditor `:410-429`；`update` 里 `if (currentText != content) editor.setText(content)` `:425` 防回写风暴；`onEditorCreated(this)` `:420` 把实例上抛给顶层做查找/撤销引用

**`TextMateSupport.kt`（282 行）**
- `ANSI` 16 色数组 `:20-37`：**注释明确对齐 termlib 的 `ColorCache.standardAnsiColor()`** —— 编辑器配色与终端配色同源
- `extensionToScope` 映射 `:39-91`：40+ 扩展名 → TextMate scope（`"kt"→"source.kotlin"`、`"md"→"text.html.markdown"`、`"toml"→"source.yaml"` 等）
- `init(context)` `:93`：双检锁单例，注册 `AssetsFileResolver` 并 `loadGrammars("textmate/languages.json")`
- `scopeForFileName()` `:105`：Dockerfile 特判 + 扩展名查表
- `applyLanguage()` `:111`：`TextMateLanguage.create(scope, true)` 失败静默（`catch (_: Exception)` `:116`）
- `applyTheme()` `:120`：用终端背景/前景色构造 `TextMateColorScheme`，**推导**注释/字符串/关键字色（亮度判定 `:272` 的 `(r*299+g*587+b*114)/1000` 相对亮度公式 + `blendColor` `:275` 按比例混合背景前景）——不依赖主题文件，随终端配色联动

### 2.3 torvox 对比

**torvox 没有**任何文本编辑器（终端内 `vi`/`nano` 是唯一编辑途径）。torvox 的 `TerminalDocumentsProvider.kt` 只提供 SAF 文档浏览。

### 2.4 依赖分析与吸收建议

- Sora Editor（`io.github.rosemoe.sora:editor`）+ tm4e 是成熟稳定方案，非激进（Termux 也用同类方案）；体积约 1-2MB AAR。
- 吸收价值**中等偏高**：若 torvox 未来加"本地文件编辑"（bootstrap 环境内编辑配置文件），`TextMateSupport` 的**主题联动设计**（编辑器配色 = 终端 ANSI 色推导，`TextMateSupport.kt:120-156`、`ANSI :20`）和**输入即搜的正则查找栏**（`EditorScreen.kt:251-268`）可直接移植；`AndroidView` 包 Sora 的模式（`EditorScreen.kt:410-429`）是现成样板。
- 纯本地场景不需要 `EditorState` 的远端 `filePath/charset` 字段，可简化为 `Loaded(content, fileName)`。

## 3. feature/connections —— 连接管理中枢

### 3.1 定位与架构

Haven 的"主屏"。`ConnectionsViewModel.kt`（280KB，约 2800 行）聚合全部连接类型的生命周期：SSH、mosh、ET、Reticulum、本地（proot）、VNC、RDP、SPICE、SMB、rclone、USB。`ConnectionsScreen.kt`（119KB）是分组可拖拽排序的连接列表。`ConnectionEditDialog.kt`（255KB）是单文件巨型表单（每种协议一套字段）。

### 3.2 核心文件（文件:行号）

- `ConnectionsViewModel.kt`：`ProfileStatus` 枚举 `:191`（CONNECTING/CONNECTED/RECONNECTING/DISCONNECTED/ERROR）；mosh 重连策略 `MoshReconnectDecision` `:129` 与 `scheduleMoshReconnect` `:1148`（指数退避、`MOSH_RECONNECT_MAX_ATTEMPTS` 上限、失败后单次静默重连 `connectMoshSilent` `:1178`，**刻意无重试循环**）；HostKey 验证协程桥 `HostKeyPrompt` `:788` + `awaitHostKeyDecision` `:921`；跳板连接 `VncNavigation/RdpNavigation/SpiceNavigation` `:967-995`；Reticulum 节点扫描 `scanReticulumDestinations` `:1248`、子网 SMB 扫描 `scanSubnetSmb` `:1344`；分组 `createGroup` `:1456`、`saveConnection` `:1390`；电池优化提示文案按厂商定制 `:347-368`
- `ConnectionsScreen.kt`：拖拽排序（`MoveDraggedRowTest`/`MoveGroupBlockTest` 测试配套）
- `NetworkDiscovery.kt`（20KB）：局域网服务发现
- `PortForwardDialog.kt`（26KB）：连接级端口转发规则编辑（`TunnelDraft.kt` 草稿模型）
- `usb/UsbipConnectionForwarder.kt`（7.7KB）：**usbip over SSH**——把远端 USB 设备挂到手机
- `MoshEtBootstrap.kt`（4.4KB）：mosh/ET 的 SSH bootstrap 编排
- `HostRediscovery.kt`（7.4KB）：SSH 主机重发现（密钥变更检测）

### 3.3 torvox 对比

**torvox 没有**连接管理（无 SSH/多协议/分组）。torvox 单屏即终端。

### 3.4 吸收建议

整个模块对 torvox 不适用（torvox 无远程连接）。可借鉴的只有**模式**而非代码：`MoshReconnectDecision` 的"单次静默重连 + 明确上限 + 指数退避"策略（`ConnectionsViewModel.kt:1120-1190`）是移动端重连的教科书实现，若 torvox 未来做 SSH 会话可参考；厂商定制电池提示文案（`ConnectionsViewModel.kt:347-368`）体现的"诊断文案要具体"思路可用于 torvox 的错误提示。

## 4. feature/keys —— SSH 密钥管理（**不是键盘配置**）

### 4.1 定位纠正

模块名 `keys` 容易被误读为"键盘配置"。**实际是密钥库**：SSH 私钥的生成/导入/导出/排序/生物识别保护、FIDO2 安全密钥（`sk-ssh-ed25519`）、OpenKeychain 外部 provider、**step-ca 证书签名**、TOTP 2FA 码（扫码导入 + OCR）。键盘相关设置在 `feature/settings` 的 keyboard 分区和 `feature/terminal`。

### 4.2 核心文件（KeysViewModel.kt，约 1140 行）

- `generateKey(label, keyType)` `:344`：调用 `SshKeyGenerator` 生成（ed25519/rsa/ecdsa）
- `importFromUri(context, uri)` `:488` / `startImport(fileBytes)` `:505`：导入流程含密码短语重试 `retryImportWithPassphrase` `:527`、`handleSkKeyImport` `:567`（FIDO2 密钥特殊路径）
- `regenerateViaStepCa(keyId)` `:437`：**step-ca 短期证书再签名**（SSH CA 工作流）
- `setBiometricProtected(keyId, protected)` `:305`：Keystore 生物识别门禁开关
- `addProviderKey(providerPackage)` `:599`：集成 OpenKeychain（`OpenKeychainApi.providers` `:587`）
- `addTotpFromText` `:1073` / `addTotpFromImage` `:1083`：TOTP 导入（URI 解析走 `core/security/OtpAuthUri`，图片走 OCR）
- `setKeyStoredPassphrase` `:685`：加密存储的密码短语（Tink 信封）
- `setSortMode/moveKey` `:170-201`：密钥尝试顺序（对应 `~/.ssh/config` IdentityFile 顺序）

### 4.3 torvox 对比

**torvox 没有**（无 SSH 密钥概念）。torvox 的 `SettingsScreen` 无密钥管理。

### 4.4 吸收建议

torvox 当前不可用（无 SSH）。若 torvox 未来支持 SSH，`KeysViewModel` 是**完整的功能清单**：生成（多类型）、导入（含 passphrase 重试与 FIDO2 分支）、排序（尝试顺序）、生物识别门禁、step-ca 签名、OpenKeychain 集成、TOTP。其中**排序 + 生物识别门禁**（`KeysViewModel.kt:196-201, 305-308`）是最容易被低估的两个细节。当前阶段仅记录，不吸收。

## 5. feature/settings —— 设置（12 分区）

### 5.1 架构

`SettingsScreen.kt`（185KB）+ `SettingsViewModel.kt`（约 950 行）+ `UserPreferencesRepository`（DataStore）。屏幕用 `CollapsibleSettingsSection`（折叠分区）组织 12 个分区，`SettingsScreen.kt:424/457/580/627/715/766/819/895/984/1260/1308` 依次为：安全隐私 / 外观 / 键盘输入 / 终端 / 桌面 / 连接屏 / 诊断 / 高级 / MCP / 备份 / 关于。每个开关都是 `viewModel.setXxx()` 直写 DataStore。

### 5.2 核心文件

**SettingsViewModel.kt**
- 备份：`exportBackup(uri, password)` `:167` / `importBackup` `:187`（**age 加密 + JSON**，见 core:security）；`pushBackupToRemote` `:283` / `pullBackupFromRemote` `:303`（经 SSH/rclone 远端同步）；自动同步 `setBackupAutoSync` `:253`
- 字体：`installTerminalFontFromContentUri` `:93`、`clearCustomTerminalFont` `:109`
- 终端：`setTerminalFontSize` `:889`、`setTerminalScrollbackRows` `:895`、`setReflowTerminalOnKeyboard` `:877`、`setShowTerminalTabBar` `:883`、`setInterceptCtrlShiftV` `:871`、`setSessionCommandOverride` `:559`
- 安全：`setBiometricEnabled` `:664`、`setScreenSecurity` `:682`、`setLockTimeout` `:552`
- 桌面/RDP：`setRdpProgressiveUpgrade` `:351`、`setRdpAvcEnabled` `:359`、`setDesktopInputMode` `:463`、`setRdpDesktopSize` `:473`、`setGpuUseVenus` `:480`、`setBandwidthAutoSuggest` `:487`
- MCP：`setMcpClientConsentBypass` `:145`、`unpairMcpClient` `:150`、`setMcpWireguardEnabled` `:408`、`setTrustLoopbackMcpClients` `:428`
- 本地：`setProotIdleTimeoutMinutes` `:904`

**SettingsScreen.kt**：`showRecommendedFontsDialog` `:391`（内置推荐字体 URL 列表）、`ProotInstallLogScreen` `:360`、`AuditLogScreen` `:356`、`AgentActivityScreen` `:364`、`PairedClientsScreen` `:368`、语言/主题/键盘模式/屏幕顺序/Wayland shell/媒体扩展名对话框 `:1347-1581`

### 5.3 torvox 对比（torvox 有 SettingsScreen）

| 设置项 | torvox（SettingsRepository.kt） | Haven |
|--------|-------------------------------|-------|
| 字体大小/族 | `setFontSize/FontFamily` :141-143 | `setTerminalFontSize` :889（+自定义字体文件/URL 安装） |
| 主题 | `setThemeName/Day/Night/Mode` :145-151 | `setTheme` :910 + 自动昼夜切换 |
| shell/scrollback | `setShell` :155 / `setScrollbackLines` :157 | `setTerminalScrollbackRows` :895 + 会话命令覆盖 :559 |
| 触摸行为 | `setTouchBehavior` :159 | 无直接对应 |
| 键盘模式 | `setKeyboardMode` :167 | 键盘输入分区（含 raw 模式、Ctrl 键拦截） |
| MCP | `setMcpServerEnabled` :169 | **MCP 全套**：同意门控、配对、反向隧道、WireGuard |
| 光标 | `setCursorBlink/Style/Speed` :177-181 | 无（由 terminal 内部处理） |
| 背景图 | `setBackgroundImagePath/Blur/Alpha` :171-175 | 无 |
| 备份 | 无 | **age 加密备份 + 远端同步** |
| 安全锁 | 无 | 生物识别/锁屏超时/截屏防护 |

**结论**：torvox 的设置更"终端向"（光标、背景、语义选择）；Haven 更"连接器向"（备份、安全、MCP、桌面）。重叠区（字体、主题、键盘）Haven 的 UI 模式 `CollapsibleSettingsSection` 与"每个开关一个 `viewModel.setX()`"的直写模式值得参考。

### 5.4 吸收建议（重点模块之一）

- **`CollapsibleSettingsSection` 分区模式**（`SettingsScreen.kt:424` 起）：torvox 设置项已有分区但可借鉴其"可折叠 + 记忆展开状态"交互
- **自定义字体安装**（`SettingsViewModel.kt:93-109` + 推荐字体列表 `SettingsScreen.kt:391`）：torvox 有字体切换（Nerd Font），可加"从 URL/文件安装字体"能力
- **设置直写模式**：`SettingsViewModel` 每个设置一个 suspend setter、UI 层 `LaunchedEffect` 收集——与 torvox 现有 `SettingsRepository`（DataStore）同构，无需改动
- 备份（`exportBackup :167`）对 torvox 价值低（无敏感连接数据），但**导出配置为 age 加密 JSON** 的格式思路可留作 future

## 6. feature/tunnel + core/tunnel —— 隧道（SSH 隧道之外的网络通路）

### 6.1 定位澄清

**`feature/tunnel` 不是 SSH 端口转发**（那是 `core/ssh/SshSessionManager` 的职责，见 §7）。它管理**独立于 SSH 的隧道配置**：WireGuard、Tailscale（tsbridge）、**Cloudflare Access（cloudflared 的 WebSocket 隧道协议，无需隧道守护进程）**。SSH 端口转发 L/R/Dynamic 在 `feature/connections/PortForwardDialog.kt` + `core/ssh`。

### 6.2 core/tunnel 核心文件

- **`CloudflareAccessTunnel.kt`（338 行）**：`class CloudflareAccessTunnel` `:61` —— 把 cloudflared 的 WebSocket 隧道协议用 OkHttp 实现；`WebSocketTunneledConnection` `:159` 把 `InputStream/OutputStream` 桥到 WebSocket 二进制帧（`write` `:175-185`）；`mapFailure` `:279` 把 HTTP 状态/`cf-ray` 头/body 摘要转成诊断文案（302→`/cdn-cgi/access/login` 映射为"需要 Access 登录"）；`dial()` `:239` 超时控制。**JWT 可选**——公开路由只需 hostname
- **`ProxySocketFactory.kt`（约 140 行）**：**手写 SOCKS5 客户端**——握手 `:91-110`（版本协商、用户名密码认证）、CONNECT `:120-132`（IPv4/IPv6/域名 ATYP 解析）
- **`TunneledSocket.kt`（约 220 行）**：`java.net.Socket` 子类，把 TCP 流量经隧道（WireGuard/Tailscale/Cloudflare）转发；`getLocalAddress()` `:182` 返回 loopback
- `AuthenticatedProxy.kt` `:26`：jsch `Proxy` 接口适配（SSH 走 HTTP 代理）
- `TunnelResolver.kt`：按 profile 选隧道（Tailscale/WireGuard/Cloudflare）
- `TunneledDatagramSocket`：**UDP over tunnel**（供 mosh 用，`MoshSession.kt:47` 注释 #164）
- `CloudflareAccessConfigBlob.kt` `:35`：配置序列化（hostname/teamDomain/jwt/jumpDestination）

### 6.3 feature/tunnel

`TunnelViewModel.kt`：`testCloudflareAccess(hostname, jwt, jumpDestination)` `:81` —— 构建瞬时隧道做一次 `dial(host, 22, 8_000)` 探测（`forTest` `:98`），成功/失败写 `ConnectionLog`（合成 profileId `cf-tunnel-test:<hostname>` `:94` 避免污染 profile 日志）；`delete` `:273`。**"测试连接"按钮内联结果**的模式（`CloudflareTunnelTestResult`）值得抄。

### 6.4 torvox 对比

**torvox 没有**任何隧道/代理能力（无 SSH、无 VPN）。

### 6.5 依赖分析与吸收建议（重点模块之一）

- Cloudflare Access 隧道是**新颖但不激进**的方案：无 root、无守护进程、纯 WebSocket，比 WireGuard 实现简单一个数量级（后者要内核 tun 或 gVisor netstack）。
- 对 torvox 的直接价值：**无**（torvox 不需要连内网主机）。但两个模式可移植：
  1. `testCloudflareAccess` 的**"连接测试"内联 UX**（`TunnelViewModel.kt:81-120`）：测试结果即时显示在表单旁、成功/失败都落日志——适用于 torvox 未来任何"输入地址→验证可达性"的场景（如自定义 bootstrap URL 校验）
  2. `mapFailure` 的诊断文案工程（`CloudflareAccessTunnel.kt:279-301`）：错误信息带状态码 + `cf-ray` + body 摘要——torvox 的错误提示应达到同样具体程度
- **手写 SOCKS5 客户端**（`ProxySocketFactory.kt:91-132`，约 50 行核心逻辑）是低依赖实现代理的样板，若 torvox 需要"通过代理下载 bootstrap"可直接抄。

## 7. core/ssh —— SSH 双后端与端口转发

### 7.1 架构

`core/ssh` 是 Haven 的网络心脏：**jsch + sshlib 双后端**（`build.gradle.kts:46,49`），统一接口 `SshConnection`。sshlib 后端在 `core/ssh/src/main/kotlin/sh/haven/core/ssh/sshlib/`（`SshlibConnection.kt`、`SshlibPortForwarders.kt`、`JschProxyTransport.kt`）。`SshClient.kt` 是 jsch 封装。还包括 OpenKeychain 兼容 API（`org/openintents/ssh/authentication/SshAuthenticationApiError.java`）。

### 7.2 端口转发（torvox 未来做 SSH 时的核心参考）

- `SshConnection.kt:105-110`：接口含 `setPortForwardingL/R`、`setPortForwardingDynamic`
- `SshClient.kt:850-905`：jsch 实现（`sess.setPortForwardingL(bindAddress, localPort, remoteHost, remotePort)`）
- `SshSessionManager.kt`（约 1600 行）：
  - `PortForwardInfo` `:131` + `PortForwardType { LOCAL, REMOTE, DYNAMIC }` `:161` + **`critical` 标记**（关键转发失败则整条连接失败）
  - `applyPortForwards(sessionId, rules)` `:1464`：批量应用规则，**部分失败回滚** `:1466-1490`
  - `bindLocalWithRetry` `:1517`：本地端口占用时自动换端口重试
  - `bindRemoteWithSelfHeal` `:1546`：远端 bind 失败后**自动降级端口再试**（`selfHealOnBindFailure`），并记录 SSH PID `recordTunnelSshPid` `:1594`（供后续清理孤儿转发）
  - **TunnelLease 生命周期** `:1333-1399`：`acquireTunnelLease` / `attachTunnelDependent` / `releaseTunnelDependent`——"以 SSH 连接为父资源的租约"，父会话断开时自动通知并拆除依赖（RDP-over-SSH、VNC-over-SSH、mosh-over-tunnel #164 都挂在这个机制上）
- `DynamicForwardServer.kt:37`：SOCKS 动态转发服务端
- `BackgroundDisconnectDetector.kt:29`：**后台被杀检测**——按厂商（`when (manufacturer.lowercase())` `:74`）给出定制提示，解释"连接被系统杀掉 vs Haven 主动断开"
- `HostKeyVerifier.kt`：known_hosts 管理（含 **CA 签名主机证书信任** `isTrustedCaSignedHostCert` `:42`）
- `CertificateWrappedIdentity.kt:23`：step-ca 证书包装的 Identity
- `DropbearKeyConverter.kt:16`：dropbear 密钥格式转换

### 7.3 torvox 对比

**torvox 没有**（纯本地）。torvox 的 `TerminalRuntime` 只管理本地进程。

### 7.4 吸收建议

torvox 若引入 SSH（roadmap 级），`core/ssh` 是**最完整可抄的模块**，但依赖 jsch/sshlib 双后端是过度设计——单后端即可。真正值得抄的三个点：
1. **端口转发规则模型 + 批量应用/回滚**（`SshSessionManager.kt:1464-1516`）：规则带 `critical`、失败回滚、`selfHealOnBindFailure`
2. **TunnelLease 租约**（`SshSessionManager.kt:1333-1399`）：多资源共享一个 SSH 会话的引用计数生命周期，避免"RDP 关了 SSH 还活着"或反之
3. **HostKeyVerifier 的 known_hosts + CA 证书信任**（`HostKeyVerifier.kt:42-105`）：比 torvox 现有任何校验都完整

## 8. core/mosh —— 纯 Kotlin mosh（重点：UDP 会话）

### 8.1 定位

**mosh 客户端全协议纯 Kotlin 实现**，替代 C++ `libmoshclient.so`：无 PTY、无 JNI、无 native。mosh server 发出的 VT100 序列直接喂 termlib。传输库本体是子模块 `mosh-kotlin`（ssp-transport，未检出），主仓库 `core/mosh` 是会话层；**`MOSH.md` 完整记录了协议细节**（这是本次研究最值钱的文档）。

### 8.2 协议要点（`core/mosh/MOSH.md:27-118`）

- **包格式**：`[8-byte nonce][AES-128-OCB(plaintext) + 16B tag]`；plaintext = `[2B ts][2B ts_reply][10B fragment header][payload]`；fragment header = `[8B fragment_id BE][2B combined(bit15=final, bits0-14=frag_num)]`；payload zlib 解压后是手写 protobuf
- **nonce**：12B OCB nonce = `[4 零字节][8B nonce]`；bit63=方向（0=客户端→服务器），bits0-62=单调序号
- **手写 protobuf**（`WireFormat.kt`）：字段号对齐上游 `src/protobufs/`；**关键坑**：proto2 `has_xxx()` 对缺省字段返回 false，服务端用 `has_old_num()` 决定是否应用 diff，**所以所有字段必须显式写出即使为 0**（`MOSH.md:72-74`）
- **SSP 双状态空间**：客户端→服务器（UserStream：每个按键一个 action）；服务器→客户端（完整终端：VT100 帧）。**invariant：`throwawayNum` 必须来自 UserStream 状态空间**（= serverAckedOurNum），混用会导致服务端丢弃状态（`MOSH.md:113-115`）
- **发送节奏**：按键 20ms 内发送、ack 20ms 内、重传指数退避 100→200→400→800ms、keepalive 3s、轮询 ≤100ms
- **Android 特定坑**：**不能用 connected UDP socket**（ICMP 错误从过期会话以 `PortUnreachableException` 冒上来）→ 用 unconnected socket（`MOSH.md:116-117`）；`DatagramSocket` 构造含 DNS 会触发 StrictMode → 延迟到 IO 协程（`MOSH.md:133-134`）
- **未实现**：本地预测回显（~100ms 感知延迟）、漫游 UI、RTT 自适应

### 8.3 会话层（`MoshSessionManager.kt`、`MoshSession.kt`）

- `MoshSessionManager`：`hasNetwork()` `:50`——**用 `NET_CAPABILITY_INTERNET + NET_CAPABILITY_VALIDATED` 门控 #421 死会话升级**（设备真正离线时绝不放弃会话，只有"在线但静默"才判死）；`SessionState` `:63` 含 `socketProvider`（mosh-over-tunnel #164）；`onSessionDied` `:42` 单次静默重连
- `MoshSession`：`start()` `:93` 先推 **`DECCKM_ON`（`ESC [ ? 1 h`）` :242`** 到本地模拟器再连——修复 #73：libvterm 初始在普通光标键模式，mosh 不重发 DECCKM 会导致方向键输出 CSI 而非 SS3，破坏 mutt/emacs/less；`stallSeconds` StateFlow `:82`（停滞指示器而非倒计时）
- `MoshSessionDecckmTest.kt`：DECCKM 修复的回归测试

### 8.4 torvox 对比

**torvox 没有** mosh（无远程协议）。但注意：torvox 终端核心是 libghostty-vt，**DECCKM 缺陷 #73 是模拟器无关的协议层问题**——任何 mosh 客户端（含 Rust 版）都会遇到，Haven 的"本地预推 `ESC [?1h`"方案（`MoshSession.kt:242`）是通用修复。

### 8.5 依赖分析与吸收建议（重点模块）

- **先进激进度：高但务实**。AES-128-OCB（Bouncy Castle）+ 手写 protobuf 是 mosh 协议原样，纯 Kotlin 消除 native 依赖。对 torvox（Rust 核心）而言，**mosh 应直接用 Rust 生态的 mosh 实现**（上游 mosh-client 协议库是 C++，Rust 移植见社区项目），不需要抄 Kotlin 代码。
- **真正可吸收的是协议知识**（写进 torvox 文档而非代码）：
  1. UDP 会话密钥协商：SSH bootstrap `exec "mosh-server new"` → 解析 `MOSH CONNECT <port> <key>`
  2. 无连接 UDP + 非单调 nonce 方向位 → 漫游安全
  3. 停滞检测 vs 连接死亡的判定：**网络验证（VALIDATED）+ 停滞时长双条件**（`MoshSessionManager.kt:50-61`）
- **DECCKM 预推修复**（`MoshSession.kt:93-107, 242`）是唯一可直移植入 torvox 的代码级内容——若 torvox 未来接任何"非 SSH 的流式传输"（mosh/ET/自研），在会话开始向本地模拟器推 `ESC [?1h` 即可。代码注释建议：

```
// MoshSession.kt:93-107 移植注释（torvox 侧）
// 会话启动时向本地模拟器预推 DECCKM ON（ESC [ ? 1 h），
// 参考 Haven MoshSession.kt:242。上游 mosh-client 在会话开始时
// 也会这样做：libvterm 初始为普通光标键模式，若不预推，
// 方向键将输出 CSI (ESC [ A) 而非 SS3 (ESC O A)，
// 破坏 mutt/emacs/less 等依赖 tput smkx 的程序（Haven #73）。
```

## 9. core/et —— Eternal Terminal

`EtSessionManager.kt`：与 `MoshSessionManager` 同构的会话管理（`SessionState` `:30`、`registerSession` `:66`）。ET 特性：SSH bootstrap 后**持久 TCP 连接（默认端口 2022）**，`clientId + passkey` 恢复会话（`SessionState` 字段 `:37-38`），断线重连后**会话在服务端存活**（ET 与 mosh 的定位差异：mosh 是 UDP 实时同步，ET 是 TCP 全缓冲恢复）。`EtSession.kt` 桥接终端。**torvox 无对应**；ET 是 TCP 实现比 mosh 简单，若 torvox 要"断线恢复"可优先考虑 ET 协议而非 mosh，但同为远程协议，当前不吸收。

## 10. core/reticulum —— 网状网络（rnsh）

纯 Kotlin 的 Reticulum 网络栈（子模块 `reticulum-kt` + `rnsh-kt`，替代原 Python/Chaquopy 栈）。**唯一在完全无互联网时仍能工作的传输**（`README.md:66`）。核心：
- `ReticulumTransport.kt`：Coroutines-first 接口（`:8`），`openShellSession` `:46`、`execCommand` `:63`、announce 发现 `:158`
- `ReticulumSessionManager.kt`：会话生命周期（detach 不关 rnsh 会话 `:152`）
- `ReticulumForwardServer.kt:28`：**`-L`/`-D` 转发 over rnsh**——把 Reticulum 变成可隧道的端口
- `sftp/SftpV3Client.kt` + `SftpV3Codec.kt`：**在 rnsh 上跑 SFTP-v3 子集**，注意 markqvist/rnsh listener 的怪癖：stdin 回显到 stdout（`SftpV3Client.kt:33`）、无 stdin-EOF（发 EOF 50ms 后杀进程 `:43-45`）、退出码不可靠（`:38`）
- `feature/sftp/transport/ReticulumFileBackend.kt`：文件操作经 rnsh 单 Link 流式上传（八进制编码 + `printf` 命令喂交互 sh，`:26-45`）

**torvox 无对应**。Reticulum 对 torvox 无现实价值（需网状网络基础设施），但 `SftpV3Client` 处理"不可靠传输上跑可靠协议"的韧性（回显剥离、哨兵退出码 `ReticulumFileBackend.kt:254`）是通用工程智慧，仅记录。

## 11. core/local + core/wayland —— proot 本地环境与桌面

### 11.1 core/local

本地"Linux 发行版"体验（类似 Termux + proot-distro）：
- **ProotManager**：rootfs 管理、`runCommandInProot()`（`AudioBridge.kt:123` 调用例）、空闲超时（`setProotIdleTimeoutMinutes` 设置项）
- `AudioBridge.kt`：**pulseaudio 音频桥**——proot 内 pulseaudio 经 Unix socket 转发到 Android AudioTrack；`start()` `:115` 检测 `command -v pulseaudio` `:123`、孤儿 pulse 回收 `reapOrphanPulse` `:135`、socket 重连循环 `:204-210`
- `DesktopManager.kt`（约 1460 行）：Wayland 桌面会话管理——嵌套 compositor 启动 `launchNestedWayland` `:1375`、display 分配/释放 `:1377,1391`、端口监听轮询 `:1413-1415`、缩放 `setAppWindowScale` `:1457`
- `build.gradle.kts:65-138`：Gradle 任务链 `buildProot → buildWayvncShim → buildHavenUsb → fetchQemuLoaders`（`dependsOn` `:138`）——**proot/wayvnc/USB/qemu 全部走 bash 脚本 + jniLibs 注入**
- `cpp/haven-usb/libhaven_usb.c`（约 530 行）：**LD_PRELOAD 风格的 libc 拦截库**——拦截 `open/ioctl/read/write/poll`（`:418-529`），把 USB 访问代理给 Android USB 服务（usbip over SSH 的本地端），还 stub 掉 udev（`:359-405`）让 proot 内工具以为有真实 USB 子系统

### 11.2 core/wayland

- `WaylandBridge.kt`：JNI 桥 `nativeSetSurface(Surface)` `:68`、`nativeSendTouch/Key/Scroll` `:71-77`、`nativeResize` `:80`、`nativeSetZoom` `:84`、`nativeStartVirglServer` `:93`（**virgl GPU 加速**）
- `WaylandDesktopView.kt`：Compose 视图——Android `Surface` 直渲合成器输出 `:107`；**IME 合成文本→evdev 按键**转换 `:325-378`（`sendCharAsEvdev` `:378`，即"输入法候选→键盘事件"的完整实现）；捏合缩放/平移 `:439`
- `build.gradle.kts`：NDK 构建 wayland 原生库（git rev 记录 `:80`）

### 11.3 torvox 对比与吸收

torvox 用 **Termux bootstrap**（直接 exec 的 chroot 风格）跑本地 shell，与 Haven 的 proot 方案**定位相同、技术路线不同**：
- proot：用户态 ptrace 重定向路径，无需 root，但慢 ~30-50%、对 seccomp 敏感；Android 14+ 禁 exec 数据目录 → **libproot.so 命名技巧**（见 §14）
- torvox bootstrap：chroot/直接运行，快，但需要 root 或特定环境
- **可吸收**：`AudioBridge` 的 pulseaudio→AudioTrack 桥思路（`AudioBridge.kt:115-236`）——torvox bootstrap 环境若想出声，这是现成架构；`libhaven_usb.c` 的 LD_PRELOAD 拦截模式（`:418-529`）是"让容器内工具看到真实硬件"的通用技巧，torvox 的 USB 串口场景可参考。这两项均属**可选增强**，非核心。

## 12. core/scan、core/security、core/data、core/knock、core/spa 等

### 12.1 core/scan（OCR + 条码）
- `BarcodeDecoder.kt:25`：ZXing，`TRY_HARDER` `:62`、解码前缩放 `:48`
- `TextRecognizer.kt:28`：Tesseract4Android（`build.gradle.kts:42` 从 **JitPack** 拉 adaptech-cz 的 AAR，`settings.gradle.kts:23-33` 限定 group）
- `TrainedDataManager.kt`：tessdata 下载/校验（`ensureLanguage` `:47`，Gradle 任务带 checksum 重试 `build.gradle.kts:98-145`）
- 用途：TOTP 二维码扫描、图片转文本。**torvox 无 OCR**（grep 确认主代码无 OCR 引用）。吸收价值低（torvox 无扫码场景），但 `TrainedDataManager` 的"运行时下载模型 + Gradle 预取 + checksum 验证"三态模式（`build.gradle.kts:98-164`）对 torvox 未来任何模型/资产下载有参考价值。

### 12.2 core/security
- `AgeFile.kt`（约 290 行）：**纯 Kotlin 实现 age 加密格式**（`AgeIdentity` `:45`、X25519 + HKDF + ChaCha20-Poly1305 流加密 `streamEncrypt` `:227`、HMAC 头校验 `:141-152`、全零共享密钥拒绝 `:287`）。用于备份文件加密。**torvox 无**。这是"加密备份/导出"功能的完整实现，吸收价值取决于 torvox 是否做配置导出（可留作 future）。
- Keystore 封装（生物识别门禁）、Tink（`build.gradle.kts:27`）、`OtpAuthUri`（TOTP URI 解析）

### 12.3 core/data
- Room（`build.gradle.kts:15` schema 导出）+ DataStore + `AgentConsentManager.kt`（MCP 同意门控：`ConsentLevel { NEVER, ONCE_PER_SESSION, EVERY_CALL }` `:32`、前台才弹窗 `:351`、超时默认 DENY `:364`、配对黑名单 `:415`）
- `AgentUiCommand.kt`：agent→UI 命令总线（`OpenInEditor :154`、`ConnectProfile :179`、`OpenRemoteDesktop :102`、`OpenUsbDrive :125`、`RegenerateStepCaCert :140`）
- `AgentActivityHolder.kt`：前台活跃度跟踪
- **torvox 对比**：torvox 的 `SettingsRepository` 只有 DataStore，无 Room/无 agent。`AgentConsentManager` 的"超时默认拒绝 + 前台门控"模型（`:351-364`）是 MCP/agent 场景的安全基线，torvox 有 MCP server 设置（`SettingsRepository.kt:169`），**未来做工具调用时应抄这个同意模型**。

### 12.4 core/knock + core/spa（连接前敲门）
- `core/knock`：`PortKnocker.kt` + `KnockSequence.kt`（TCP 端口序列敲门）
- `core/spa`：**fwknop SPA 单包授权**——`FwknopPacket.kt`（HMAC 包构造）、`SpaConfig.kt`、`SpaSender.kt`（UDP 单包发送）
- 用途：SSH 前敲门（连接序列含 `portKnockSequence`，`ConnectionsViewModel.kt:990`）。**torvox 无**；纯安全外围功能，不吸收（除非未来做 SSH）。

### 12.5 其余 core 小模块
- `core/stepca`：step-ca 证书签发客户端（配合 keys 模块）
- `core/mcp`：agent 的 HTTP/JSON-RPC 传输（`HttpConnection.kt`、`HttpFraming.kt`、`JsonRpc.kt`）
- `core/vnc`：**纯 Kotlin VNC 客户端**（`VncClient.kt:23`——`start` `:36`、`moveMouse` `:78`、`charToKeyEvent` 字符→keysym 表 `:169`、`typeText` `:121`、双事件循环 `:209-281`）
- `core/rdp`：RDP 解码（`Avc420MediaCodecDecoder.kt:29`——**H.264 AVC420 走 MediaCodec 硬解**，SPS/PPS 注入 `:103-111`、低延迟模式 `:118`、YUV I420 打包 `:201-218`）
- `core/spice`：SPICE 客户端（UniFFI 绑定 spice-kotlin）
- `core/smb`：**SmbClient（smbj）**（`SmbFileEntry` `:24`、`openShare` `:70`、`ensureShare` 自愈重连 `:98`、隧道 socket 工厂 `:78`）
- `core/mail`：IMAP 邮件后端（`MailBackend.kt`、`RfcMailBackend.kt`）
- `core/rclone`：rclone 云存储后端封装
- `core/fido`：FIDO2/CTAP（配合 keys）
- `core/usb`/`usbserial`/`btserial`/`bleserial`：串口终端（USB CDC-ACM、CH34x/FTDI/CP21xx 驱动在 `settings.gradle.kts:31` 注释）
- `core/ffmpeg`：FFmpeg 封装（`build-ffmpeg/` 目录）
- `core/terminal-haven`/`core/toolbar`：terminal 模块的公共件

**torvox 全部无对应**（唯一可注意点：`core/vnc/VncClient.kt:169` 的字符→keysym 映射表与 `feature/rdp/RdpKeyMapping.kt:29-52` 的字符→扫描码映射，是"软键盘字符→远程按键"的两份现成字典，torvox 的 `TerminalInputEncoder` 已有类似工作，无需吸收）。

## 13. feature/rdp、feature/vnc、feature/mail、feature/sftp、feature/imagetools

- `feature/rdp/RdpScreen.kt`：RDP 全屏/输入模式（`TOUCHPAD` 虚拟光标 `:532-542`、overlay 自动隐藏 `:574`、手势 `:626`）；`RdpKeyMapping.kt:29-52` 字符→RDP 扫描码（Shift 组合 `:31`、Unicode 兜底 `:36`）
- `feature/vnc/VncScreen.kt`（97KB）+ `VncViewModel.kt`
- `feature/mail`：`MimeParser.kt:29`（**Jakarta Mail 树遍历**：`collect` `:44` 拆分 text/plain+html+附件、`isAttachmentPart` `:108`、`CountingOutputStream` `:213` 做附件大小统计）、`MailRulesViewModel.kt`（规则引擎）、`MailViewModel.kt`（29KB）
- `feature/sftp`：`FileBackend` 接口 `:20` + 多实现（`LocalFileBackend.kt:25`、`RcloneFileBackend.kt:26`、`ReticulumFileBackend`、SSH/SMB 后端）——**统一文件抽象**是 Haven 架构亮点；`LocalFileBackend` 的"proot home 与 Android 存储根共存"（`:104-160`）
- `feature/imagetools`：`CropOverlay.kt`、`PerspectiveScreen.kt`、`RotateOverlay.kt`（图片扫描后处理，配合 OCR/TOTP 扫码）

**torvox 对比**：除 `feature/sftp` 与 `TerminalDocumentsProvider`（SAF 文档浏览，功能层级不同：torvox 只读浏览，Haven 是全功能文件管理器）外全部无对应。`FileBackend` 接口抽象（`FileBackend.kt:20-75`：list/delete/mkdir/rename/readBytes/writeBytes/stat）值得记录——torvox 若把 `TerminalDocumentsProvider` 扩展为可写文件管理，此接口可直接作为抽象层。

## 14. rdp-kotlin / rclone-android（仓库内嵌构建）

- **`rdp-kotlin/`**：IronRDP（Rust）+ UniFFI Kotlin 绑定；`rust/src/lib.rs`（147KB）主实现、`egfx/`（EGL/GPU surface，`research-haven.md` 已提 `egfx/surface.rs`）、`redirection.rs`（21KB，RDP 重定向）、`yuv.rs`（12KB，色彩转换）、`bitmap_bridge.rs`。`core/rdp/build.gradle.kts:35` 用 `preBuild dependsOn includedBuild("rdp-kotlin").task(":buildRdpNative")` 链入构建
- **`rclone-android/`**：gomobile 单 `libgojni.so` 打包 **rcbridge（rclone）+ wgbridge（wireguard-go + gVisor netstack）+ tsbridge（Tailscale）+ socks5 + mailbridge**（`settings.gradle.kts:74-78` 注释说明合并原因：避免重复 `go.Seq` 运行时类与重复 `.so`）
- `core/rdp/Avc420MediaCodecDecoder.kt:29`：**AVC420 → MediaCodec 硬解**（RDP 8.x 图形管线，比软解省电一个数量级）

**torvox 对比**：无。这些是"远程桌面/云存储"级功能，torvox 不需要；但**"多 Go 桥合并进单个 gomobile .so"**（`settings.gradle.kts:74-78`）与 **"Rust UniFFI 绑定 + preBuild 钩子"**（`core/rdp/build.gradle.kts:35`）是两个工程模式，torvox 若引入 Rust 侧 native 代码（本身已是 Rust 项目，但 android 壳是纯 Kotlin，未用 UniFFI）可参考。

## 15. build-proot —— proot 交叉编译（重点模块）

`build-proot/build.sh`（157 行）：
- 输出 `core/local/src/main/jniLibs/<ABI>/libproot.so` + `libproot_loader.so`（`build.sh:24`）
- **命名技巧**（`build.sh:17-18` 注释）：PRoot 命名为 `libproot.so` 让 Android 提取到 nativeLibraryDir 从而**可执行**（Android 14+ 禁止从 app 数据目录 exec）——这是绕过 Android 14 exec 限制的关键
- 三 ABI（arm64-v8a / x86_64 / armeabi-v7a）`build.sh:151-153`
- **talloc 绕过 waf 直接编译**（`build.sh:75-98`）：单文件 `$CC -c talloc.c` + `ar rcs`
- **Android 15 ARM64 TLS 对齐修复**（`build.sh:101-106`）：注入 `__thread int __tls_align_fix __attribute__((aligned(64)))` 强制链接器产出 64 字节对齐 TLS 段（NDK r28+ 必需）
- NDK 自动探测（`build.sh:39-47`）、proot-termux 源码补 `#include <string.h>`（`build.sh:114`）

**torvox 对比**：torvox 用 Termux bootstrap（`BootstrapInstaller`/`SecondStageRunner`，纯 Java/Kotlin 解压 + 符号链接），**不走 proot**。两条路线对比：
- bootstrap（torvox）：快、无 ptrace 开销；但 **Android 14+ 的 exec 限制**同样影响它（`BootstrapCompatibilityTest`/`BootstrapSymlinkInstrumentedTest` 就是为此写的）
- proot（Haven）：慢但无需 root 即可跑完整发行版 rootfs

**吸收建议**：
1. `libproot.so` 命名技巧（`build.sh:17-18`）——**torvox 的 bootstrap 可执行文件若遇到 exec 限制，可同法改名为 .so 塞 jniLibs**（torvox 已有 `SecondStageRunner` 处理 exec 兼容，可对比其方案）
2. TLS 对齐修复（`build.sh:101-106`）——若 torvox 未来编译任何含 TLS 的 native 代码给 Android 15 ARM64，这是已知坑与解法
3. Gradle 任务链模式（`core/local/build.gradle.kts:65-138`：`commandLine("bash", "build.sh")` + `environment("PROOT_OUTPUT", ...)`）——native 构建与 Gradle 集成的最小样板

## 16. scripts/ 与 dev/

| 脚本 | 功能 | torvox 借鉴价值 |
|------|------|-----------------|
| `mosh-fault-rig.py` | **UDP 故障注入中继**（`relay` `:52`）：手机↔mosh-server 之间转发 UDP，可 `blackhole`/`pass` 切换（文件轮询控制 `MODE_FILE` `:37`）；关键设计：**黑洞 ≠ 关 Wi-Fi**——设备保持在线，才能触发"在线但静默"的 #421 升级路径（`:8-14`）；客户端源地址每次包刷新（`:82`）以兼容客户端 socket 重绑 | **高**：任何网络会话的故障测试思路；torvox 无网络会话，但"测试装置必须精确复现真实故障形态"的工程哲学值得写进 torvox 测试文化 |
| `check-native-libs.sh` | 校验各 ABI 的 .so 齐全 | 中 |
| `check-no-committed-binaries.sh` | 防二进制入库 | 中（torvox 有 `generated-patches/` 类似关注） |
| `check-r8-kept-classes.sh` + `r8-must-keep-classes.txt` | R8 keep 规则回归 | 中 |
| `check-i18n-hardcoded.sh` | 硬编码字符串检查 | 低 |
| `i18n_drift.py` / `i18n_backfill.py` / `i18n_export.py` | 多语言字符串漂移检测/回填/导出（31KB/16KB/4KB） | **高**：torvox 若做多语言，这是现成工具链 |
| `check-changelog.sh` / `gen-fastlane-changelog.sh` | 变更日志 CI 门禁 | 低 |
| `haven-vm-setup.sh` | 测试 VM 准备 | 低 |
| `dev/agent-smoke.sh` | agent 冒烟 | 低 |

## 17. 功能对比矩阵（torvox 有/无）

| Haven 功能 | torvox | 备注 |
|-----------|--------|------|
| 文本编辑器（Sora+TextMate） | **无** | 可吸收主题联动+查找栏模式 |
| 连接管理（多协议） | **无** | 不适用 |
| SSH 密钥管理 | **无** | 不适用（未来 SSH 时参考） |
| 设置屏幕 | **有**（SettingsScreen.kt） | 重叠项：字体/主题/键盘；Haven 多备份/安全/MCP |
| 隧道（Cloudflare/WG/Tailscale） | **无** | 仅 UX 模式可吸收 |
| SSH 端口转发 L/R/Dynamic | **无** | 未来 SSH 时参考（TunnelLease 租约） |
| mosh UDP 会话 | **无** | 吸收协议知识 + DECCKM 修复 |
| ET / Reticulum | **无** | 不适用 |
| proot 本地环境 | **部分**（Termux bootstrap） | 吸收 libproot.so 命名 + TLS 修复 |
| OCR / 条码 | **无** | 不适用 |
| age 加密备份 | **无** | future 配置导出可参考 |
| MCP/agent | **部分**（MCP server 开关） | 吸收同意门控模型 |
| 邮件 / VNC / RDP / SPICE / SMB / rclone | **无** | 不适用 |
| 文件管理 | **部分**（TerminalDocumentsProvider 只读） | 吸收 FileBackend 接口 |
| 端口敲门 / fwknop SPA | **无** | 不适用 |

## 18. 依赖分析汇总

| Haven 依赖 | 激进度 | 对 torvox 适用性 |
|-----------|--------|-----------------|
| jsch + sshlib 双后端 | 保守（双后端是兼容性妥协） | 不适用（无 SSH） |
| Bouncy Castle（AES-OCB/age） | 保守 | 仅 future |
| Rosemoe Sora + tm4e | 成熟非激进 | **适用**（编辑器） |
| Tesseract4Android（JitPack AAR） | 保守但供应链弱（JitPack 预编译） | 不适用 |
| Rust UniFFI（RDP） | 先进（绑定层标准做法） | 模式适用（torvox 本体已是 Rust） |
| Go gomobile 多桥合一 | 先进但务实 | 模式适用 |
| 纯 Kotlin mosh/ET/Reticulum | **激进**（手写 protobuf + 自研 SSP） | 不适用（Rust 生态更合适） |
| proot ptrace | 保守（老技术） | 模式适用（Android 14 exec 坑） |

## 19. 可吸收内容清单（含代码注释建议）

按优先级：

1. **DECCKM 预推修复**（协议层，最高性价比）——任何"非 PTY 直连的流式会话"开始前向本地模拟器推 `ESC [?1h`；注释建议见 §8.5。

2. **MCP/agent 同意门控**（安全层）——若 torvox 的 MCP server 功能扩展为工具调用：
```
// AgentConsentManager.kt:351-364 移植注释（torvox 侧）
// 同意请求只在应用前台时弹出；后台请求带超时（默认 DENY）。
// 参考 Haven AgentConsentManager.kt:351-364：
//   foregroundState 门控 + withTimeoutOrNull(await) ?: DENY
// 会话级记忆（ONCE_PER_SESSION）避免每次调用都打断用户。
```

3. **Cloudflare 隧道测试的"内联测试" UX**（`TunnelViewModel.kt:81-120`）——表单旁即时测试 + 结果落日志；适用 torvox 的自定义 bootstrap URL 校验。

4. **编辑器主题联动**（若做编辑器）——终端 ANSI 色 → 编辑器语法色推导（`TextMateSupport.kt:120-156`），注释建议：
```
// TextMateSupport.kt:120 移植注释（torvox 侧）
// 编辑器配色不独立配置：从终端背景/前景色推导语法高亮色
// （注释=ANSI 亮黑、字符串=绿、相对亮度阈值判定），
// 参考 Haven TextMateSupport.kt:120-156（ANSI 表 :20 对齐 termlib ColorCache）。
```

5. **端口转发规则模型 + TunnelLease**（未来 SSH）——`SshSessionManager.kt:131-161, 1333-1399, 1464-1516`。

6. **proot 构建三技巧**（若跑本地发行版）——`libproot.so` 命名（`build-proot/build.sh:17-18`）、TLS 对齐修复（`:101-106`）、Gradle 任务链（`core/local/build.gradle.kts:65-138`）。

7. **mosh UDP 会话协议知识**（写文档不写代码）——`core/mosh/MOSH.md` 全文吸收：无连接 UDP、nonce 方向位、proto2 全字段显式写出、双状态空间 throwawayNum 不变式、发送节奏。

8. **FileBackend 接口**（若扩展文件管理）——`feature/sftp/transport/FileBackend.kt:20-75`。

9. **mosh-fault-rig 测试哲学**（测试文化）——"故障装置必须区分'网络静默'与'设备离线'"（`scripts/mosh-fault-rig.py:8-14`）。

10. **i18n_drift.py 工具链**（若做多语言）——`scripts/i18n_drift.py`。

## 20. 项目文档吸收价值

- **`core/mosh/MOSH.md`**（最有价值）：完整 mosh 协议实现笔记（线格式、protobuf 字段表、SSP 状态机、发送节奏、Android 坑、发现的上游 bug 列表 `:119-134`、未实现清单 `:136-142`）——torvox 未来任何 mosh/实时协议工作直接引用
- **`VISION.md`**（60KB）：产品愿景与"肢体/神经"隐喻；`CHANGELOG.md`（226KB）：**按 issue 编号的详尽变更记录**（如 `#421` 死会话、`#164` mosh-over-tunnel、`#73` DECCKM、`#92` 离线回归）——是"变更与 bug 关联"的文档范例
- **`docs/features/`**：`reticulum.md` 展示了"能力文档"写法（含实现约束：busybox 兼容、字节级校验）
- **注释文化**：Haven 的代码注释大量带 issue 编号与因果链（如 `MoshSession.kt:98-103` 的 DECCKM 完整因果分析），这是 torvox 可借鉴的注释标准
- **测试命名**：`MoshSessionDecckmTest`、`MoshReconnectPolicyTest`、`MoshServerMissingClassifierTest`、`SftpV3ClientTest`——"一个协议怪癖一个测试"

## 21. 结论

Haven 是"连接器集合"（SSH/mosh/ET/Reticulum/RDP/VNC/SPICE/SMB/mail + 隧道 + 密钥 + agent），torvox 是"本地终端"（bootstrap + 渲染 + 选择增强）。两者交集有限，但 Haven 提供了 torvox 未来扩展所需的**协议知识库与工程模式**：mosh 的 UDP 会话细节（MOSH.md）、DECCKM 修复（可直接移植）、端口转发规则与租约模型（未来 SSH）、同意门控（MCP 扩展）、proot 构建技巧（Android 14 exec 限制）、以及把"终端外壳"扩展成"连接器"的完整路线图参照。**本次研究确认：除 `SelectionToolbar.kt`（前篇）与 DECCKM/同意门控外，Haven 对 torvox 的代码级吸收点有限，价值主要在文档与模式层面。**

## deep-v4 增量（复核第 1 轮：TerminalFontInstaller + ScrollbackRing）

### TerminalFontInstaller.kt（core/data/font/TerminalFontInstaller.kt:1-259）

完整字体安装流程（设置流 + MCP 工具共用同一例程）：
- `MAX_FONT_BYTES = 32MB` 下载上限（防误 URL 填满 FS）
- 8KiB 流式下载缓冲
- **zip 解包 .ttf/.otf**（Maple 等字体仓库只发 zip）
- **WOFF/WOFF2 明确拒绝**（Android 无法渲染 web 字体）
- `Typeface.decode` 验证——坏输入不污染设置存储
- 清理先前导入的残留文件

**torvox 对照**：Bridge.loadFontFile(:620-629) 仅"本地路径直传 native"——无下载/解包/上限/验证/清理。**功能差异 P2**：若 torvox 支持 URL/zip 安装字体（设置或 MCP 工具），此文件是完整蓝本。

### ScrollbackRing.kt（core/data/terminal/ScrollbackRing.kt:1-92）

代理传输的每会话 stdout 镜像环形缓冲：
- 双 arraycopy 环形写（大 burst 不逐字节锁）
- `totalBytesAppended` 单调计数（agent 检测"新字节到达"）
- 原字节存储（ANSI/OSC 原样保留）

**torvox 对照**：MCP 快照通道是 flume + CellData 快照模式，非字节流镜像——**架构不同，不吸收**；`totalBytesAppended` 式"更新检测"若未来 MCP 需要可参考（P3）。

### HavenTerminal.kt（core/terminal-haven/.../HavenTerminal.kt:1-109）

haven 自研终端核心（109 行，非 libghostty）——之前研究未覆盖；其规模证明"自研终端核心"对 haven 是轻量封装，torvox 用 libghostty-vt（权威完整）仍是最优选择（对照确认）。

## deep-v5 增量（复核第 2 轮：HavenKeyboardMode + ImeFlagSet）

### HavenKeyboardMode.kt（core/terminal-haven/.../HavenKeyboardMode.kt:1-86）

四态键盘模式：Secure（TYPE_NULL + NO_SUGGESTIONS + NO_PERSONALIZED_LEARNING）/ Standard（全 IME 特性）/ Raw（无 InputConnection，物理键 only）/ **Custom（ImeFlagSet 逐位可调）**。

### ImeFlagSet（:48-86）——逐位 IME 标志

6 个可调位：noSuggestions / visiblePassword / autoCorrect / fullEditor / noExtractUi / noPersonalizedLearning。

**金矿注释**：
- `autoCorrect`（:66-71）：**Samsung Honeyboard 的 IMM gate 要求 AUTO_CORRECT 位才派发输入**（#110）——IME 兼容性坑位
- `fullEditor`（:73-77）：给 BaseInputConnection 真实 Editable 才支持合成（语音/滑动）——与 VISIBLE_PASSWORD 互斥

**torvox 对照**：KeyboardMode.kt（Secure/Standard/Raw 三态，:30-67 已处理 VISIBLE_PASSWORD/NO_SUGGESTIONS/AUTO_CORRECT 位组合）——**差异 P2**：torvox 无 Custom 逐位模式（用户已删 KeyboardModeSelector，保留三态是决策）；Samsung Honeyboard 坑位注释值得记入 torvox IME 文档（P2）。
