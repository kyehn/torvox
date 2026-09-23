# 参考仓库深度研究（B 组）：sushi-ssh · moke · NeoTermux · ReTerminal

> 研究范围：4 个 Android 终端/SSH 相关仓库的**全部源文件**逐文件精读（函数/结构体级别，含精确行号）。
> 对比基准：torvox（Android 终端，Kotlin Compose + Rust native + wgpu + libghostty-vt + MCP 服务 + Termux bootstrap 本地环境）。
> 研究方法：逐个读取 `repositories/refs/` 下 4 个仓库的每个源文件；行号以研究时仓库 HEAD 为准。
> 结论摘要见第 5 章吸收优先级表。

---

## 0. 总览

| 仓库 | 定位 | 技术栈 | 完成度 | 与 torvox 的核心交集 |
|---|---|---|---|---|
| `refs/sushi-ssh`（hlan-net/sushi-ssh-client） | SSH 客户端 + 终端 + Gemini AI 助手 | Kotlin + JSch + JNI C（本地 PTY）+ 自绘 TextView 终端 | 高（可运行，功能丰富） | 终端渲染、PTY、前台服务、AI/脚本自动化（torvox 有 MCP，可对照） |
| `refs/moke`（briqt/moke） | SSH / mosh / SFTP 客户端，多会话 + tmux 集成 | Kotlin + sshj + termux 移植 terminal-emulator/view + mosh 原生客户端 | 高（代码质量最好的一个） | 传输抽象层、会话管理、SFTP、known_hosts/TOFU、前台服务 |
| `refs/neotermux`（developer-mahabbat/NeoTermux） | Termux 的 Compose 重写（多工具屏幕） | Kotlin Compose + Hilt + termlib C（PTY + 极简 VT） | **极低（原型/占位）** | 工具型屏幕 UI 模式（文件/包/进程/Git/SSH/编辑器） |
| `refs/reterminal`（RohitKushvaha01/ReTerminal） | 多模块终端：Compose UI + termux 终端库 + 自研 PRoot 运行 Alpine rootfs | Kotlin Compose + termux v0.118.3 官方库 + PRoot（C 移植） | 中高（可运行） | 虚拟键盘全套、快捷键录制、rootfs/PRoot 启动链、自定义背景、preferences 组件库 |

**一句话结论**：moke 的 SSH/传输/SFTP/tmux 工程质量最高、最值得 torvox 吸收；sushi-ssh 的 AI 对话→命令执行→安全确认闭环与 torvox 的 MCP 设计互补；reterminal 的虚拟键盘/快捷键录制/preferences 组件可直接借鉴；neotermux 仅 UI 骨架有参考价值（功能全是模拟数据）。

---

## 1. sushi-ssh（hlan-net/sushi-ssh-client）

### 1.1 项目定位与完整架构

单模块 Android 应用（`app/`，Kotlin + 一个 JNI C 文件 `sushi-pty.c`），包 `net.hlan.sushi`。定位：**面向云服务器的 SSH 终端**，核心卖点是内置 Gemini AI 助手（云端 Gemini API + 本地 GeminiNano）与"对话即运维"工作流：用户用自然语言下指令 → AI 生成命令 → `CommandSafety` 分级 → 用户确认 → 在 SSH 会话或本地 shell 上执行 → 回传结果 → AI 总结。另有 Play（可复用运维脚本）、短语库、日志上传 Google Drive、GitHub 反馈等外围功能。

数据流：
```
Activity 层（MainActivity / TerminalActivity / SettingsActivity / HostEdit / Keys / Phrases / Plays / GeminiHistory / Share）
   │
   ▼
Backend 抽象 TerminalBackend（接口）──────────────┐
   ├── SshClient（JSch：密码/密钥/跳板机/端口转发）  ├── TerminalSessionHolder（单例持有当前 backend）
   └── LocalShellBackend（JNI sushi-pty.c 本地 PTY）┘
   │
   ├── SshConnectionService（前台服务，保持连接）
   ├── ConversationManager ── GeminiClient（云端）/ GeminiNanoClient（本地）→ 解析命令 → PlayRunner → backend.execCommand
   ├── SecurePrefs（EncryptedSharedPreferences 存密钥/口令）
   ├── GeminiTranscriptDatabaseHelper / PlayDatabaseHelper / PhraseDatabaseHelper（SQLite）
   └── DriveLogUploader / GitHubAuthManager / GitHubIssueClient（云服务）
```

### 1.2 逐文件功能说明（文件:行号，函数级）

**连接与后端层**

| 文件 | 核心类/函数 | 职责 |
|---|---|---|
| `app/src/main/java/net/hlan/sushi/SshClient.kt`（24KB） | `SshClient.kt:14 HostKind`（SSH/LOCAL）；`:16 SshAuthPreference`；`:28 SshConnectionConfig`（主机/端口/用户/密钥/跳板）；`:62 ConnectFailure`（错误分类枚举）；`:75 SshConnectResult`；`:97 SshClient : TerminalBackend`；`:105 AuthPlan`；`:115 connect()`（总入口，启动 shell 读线程）；`:180 addPrivateKeyIdentity()`；`:190 establishJumpSession()`（跳板机 + 本地端口转发 `setPortForwardingL`）；`:228 ConnectedSessionPair`；`:294 configureSession()`（超时/keepalive）；`:305 disconnect()`；`:325 resizePty()`（`setPtySize`）；`:350 execCommand()`（一次性命令 + 超时 + 退出码）；`:461 sendText()`；`:482 sendCtrlC()`；`:491 sendCtrlD()`；`:500 sftpUpload()`；`:559 readShellOutputStream()`（原始字节回调）；`:576 readShellOutput()`（逐行读取，`\r\n` 归一化） | JSch 封装：认证计划（密钥优先/密码回退）、跳板机、PTY 通道、exec 通道、SFTP 上传；行模式输出回调 |
| `app/src/main/java/net/hlan/sushi/LocalShellBackend.kt`（9KB） | `LocalShellBackend.kt:10 LocalShellBackend : TerminalBackend`；`:22 connect()`（调 JNI `sushi_pty_open`）；`:34 sendText()`；`:50 sendCtrlC()`；`:60 resizePty()`；`:85 execCommand()`（带超时）；`:145 readLoop()`（UTF-8 `CharsetDecoder` 增量解码）；`:184 dispatchOutput()`；`:210 flushDecoder()` | 本地 PTY 后端：JNI 打开 `forkpty`，字节流 UTF-8 解码、行缓冲、超时执行 |
| `app/src/main/java/net/hlan/sushi/TerminalBackend.kt`（1.4KB） | `TerminalBackend` 接口 + `TerminalBackendFactory` | 后端抽象：connect/disconnect/sendText/sendCtrlC/sendCtrlD/resizePty/execCommand/sftpUpload |
| `app/src/main/java/net/hlan/sushi/TerminalSessionHolder.kt`（1.8KB） | 单例持有当前 `TerminalBackend` 与 `TerminalActivity` 引用 | 跨 Activity 共享活动连接 |
| `app/src/main/cpp/sushi-pty.c`（5.8KB） | `Java_net_hlan_sushi_SushiPty_*`（open/write/setWindowSize/close/isRunning）、`pty_read_thread`（select 循环）、`pty_write` | JNI 本地 PTY：`posix_openpt`/`forkpty` 风格实现、slave 端 exec `/system/bin/sh -` 环境、读线程回调 Java |
| `app/src/main/java/net/hlan/sushi/SshConnectionService.kt`（4.8KB） | 前台服务（`onStartCommand` START_STICKY + 通知） | 后台保持 SSH 连接存活 |
| `app/src/main/java/net/hlan/sushi/SshSettings.kt`（5.5KB） | 默认端口/超时/keepalive 常量 + 设置读写 | SSH 默认行为配置 |
| `app/src/main/java/net/hlan/sushi/SecurePrefs.kt:8` | `SecurePrefs.get()`：`EncryptedSharedPreferences`（AES256_SIV 键 + AES256_GCM 值，`MasterKeys.AES256_GCM_SPEC`） | 加密存储口令/密钥口令 |

**终端渲染层**

| 文件 | 核心类/函数 | 职责 |
|---|---|---|
| `app/src/main/java/net/hlan/sushi/TerminalView.kt`（15KB） | `TerminalView.kt:21 TerminalView`（TextView 子类 + SpannableStringBuilder）；`:36 OscState`（OSC 状态机）；`:65 onEditorAction`（Enter/Ctrl 处理）；`:104 onKeyDown`（物理键盘）；`:128 onSizeChanged`（行列数计算）；`:145 appendLog()`；`:166 updateText()`（渲染 spannable）；`:206 trimBuffer()`（MAX_CHARS/MAX_LINES 裁剪）；`:245 processChar()`（ESC/OSC/`\r`/`\n`/退格 状态机）；`:284 appendChar()`；`:313 eraseLastPrintableChar()`；`:334 parseAnsi()`（`SGR_PATTERN:46` 正则解析 `\x1b[...m` SGR）；`:339-398` 颜色 span 应用 | **迷你自绘终端**：TextView + ANSI 颜色解析 + OSC 52 剪贴板提取 + URL 正则着色 + 物理键盘输入编码 |
| `app/src/main/java/net/hlan/sushi/TerminalActivity.kt`（17.5KB） | 工具栏（Ctrl/Alt/Tab/Esc 发送）、输入框与 `TerminalView` 绑定、`TerminalSessionHolder` 接入、复制粘贴 | 终端页：把用户输入转成 `sendText`/`sendCtrlC` 等 |

**AI / 自动化层（sushi-ssh 最独特部分）**

| 文件 | 核心类/函数 | 职责 |
|---|---|---|
| `app/src/main/java/net/hlan/sushi/ConversationManager.kt`（22KB） | `ConversationManager.kt:15 ConversationManager`；`:38 initialize()`（检测 Gemini/Nano 可用性 + 日志文件）；`:97 sendUserMessage()`（LLM 调用 → 提取命令 → `CommandSafety.classify` → 确认/直执行）；`:196 executeCommandAndRespond()`（执行 + 输出截断 500 字 + 二次 LLM 总结）；`:280 runRawCommand()`；`:361 generateLlmResponse()`（云端/本地路由）；`:384 addToHistory()`（超 10 轮裁剪）；`:414 persistTurn()`（SQLite）；`:465 initializeLogFile()`；`:497 writeToLog()`（文本日志，输出截断 500）；`:548 ConversationInitResult`；`:558 ConversationResult` | 对话→命令→执行→总结 的完整状态机；历史持久化；日志落盘 |
| `app/src/main/java/net/hlan/sushi/GeminiClient.kt`（11KB） | Gemini REST API 客户端（`generateContent` 流式/非流式、历史消息构造、超时重试） | 云端 Gemini 推理 |
| `app/src/main/java/net/hlan/sushi/GeminiNanoClient.kt`（8.4KB） | 本地 nano 模型：模型文件下载（远程 URL → filesDir）、加载、`generate` 推理封装 | 无网络可用时的本地 LLM |
| `app/src/main/java/net/hlan/sushi/PersonaClient.kt`（8.6KB） | 系统提示词模板（DevOps/网络/通用等 persona）+ 指令注入 | 人格化对话 |
| `app/src/main/java/net/hlan/sushi/CommandSafety.kt`（7.5KB） | `CommandSafety.classify()` 三档：SAFE / CONFIRM / BLOCKED（危险命令黑名单：`rm -rf /`、`mkfs`、`shutdown` 等 + 白名单启发） | 执行前安全闸门 |
| `app/src/main/java/net/hlan/sushi/ManagedPlays.kt`（7KB） | 内置 Play 模板库（如 ssh-setup、user-setup、server-setup） | 常用运维流程复用 |
| `app/src/main/java/net/hlan/sushi/PlayRunner.kt`（3.2KB） | `PlayRunner.run()`：逐步骤执行（每步：命令 + 期望输出正则 + 超时 + 失败中止） | Play 脚本解释执行 |
| `app/src/main/java/net/hlan/sushi/Play.kt`（3KB） | `Play` 数据类（步骤列表、变量替换） | Play 模型 |
| `app/src/main/java/net/hlan/sushi/GeminiTranscriptDatabaseHelper.kt`（8.7KB） | SQLite：对话轮次 CRUD | 对话历史持久化 |
| `app/src/main/java/net/hlan/sushi/GeminiHistoryActivity.kt`（9.5KB） | 历史对话列表 + 详情 | 历史回看 |
| `app/src/main/java/net/hlan/sushi/ConversationTurn.kt` / `GeminiTranscriptEntry.kt` / `GeminiTranscriptRecord.kt` / `GeminiTranscriptAdapter.kt` / `GeminiSettings.kt` / `GeminiTranscriptRecord` | 数据类与适配器 | 支撑上述功能 |

**主机/密钥/短语/Plays UI**

| 文件 | 职责 |
|---|---|
| `MainActivity.kt`（57KB，`MainActivity.kt:1 onCreate`、主机列表 RecyclerView、AI 面板、`initializeConversation` 等） | 主界面：会话列表 + 新建/编辑入口 + Gemini 聊天面板 |
| `HostEditActivity.kt`（9.3KB） | 主机编辑表单（地址/端口/用户/认证方式/密钥选择/跳板） |
| `HostsActivity.kt`（2KB）/ `HostAdapter.kt`（2.8KB） | 主机列表页 + RecyclerView adapter |
| `KeysActivity.kt`（5KB） | 密钥管理：生成 RSA/Ed25519（JSch `KeyPair`）、列表、删除 |
| `PhrasesActivity.kt`（8.7KB）+ `PhraseDatabaseHelper.kt`（5.7KB）+ `PhraseAdapter.kt` + `PhrasePickerHelper.kt` + `Phrase.kt` | 快捷短语（SQLite 存储、终端页插入） |
| `PlaysActivity.kt`（6.1KB）+ `PlayAdapter.kt` + `PlayDatabaseHelper.kt`（7.9KB） | 自建 Play 脚本管理 |
| `SettingsActivity.kt`（38KB） | 四 Tab 设置：General（外观/字体/终端）、SSH（默认参数/密钥）、Gemini（API Key/模型/Persona）、Drive（备份）+ GitHub 反馈入口 |
| `ShareActivity.kt`（7.6KB） | 接收系统分享文本 → 追加到活动会话/存入短语 |
| `SetupChecklist.kt` | 首次启动设置清单弹层 |

**云服务/日志**

| 文件 | 职责 |
|---|---|
| `DriveLogUploader.kt`（4.1KB） | 本地日志打包上传 Google Drive（`DriveService`） |
| `DriveAuthManager.kt`（2.3KB） | Google 账号授权（`GoogleSignIn`/OAuth token） |
| `GitHubAuthManager.kt`（5.9KB） | GitHub **device flow** 认证（获取 `device_code` → 轮询 token） |
| `GitHubIssueClient.kt`（3.6KB） | 以认证用户身份创建 issue（附设备信息） |
| `ConsoleLogRepository.kt` / `TerminalLogRepository.kt` / `ShellUtils.kt:3 shellQuote()` / `AppUtils.kt` / `AppThemeSettings.kt` / `AboutActivity.kt` / `SushiApplication.kt` / `FeedbackSettings.kt` / `DriveLogSettings.kt` | 日志收集、shell 引号转义、主题/关于等杂项 |

`app/build.gradle.kts` 依赖：`com.jcraft:jsch`、`androidx.security:security-crypto`、`com.google.ai.client.generativeai:generativeai`、GMS auth/drive、`androidx.sqlite`、RecyclerView 等（经典 View 体系 + 少量 Compose 未用）。

### 1.3 与 torvox 功能对比

| sushi-ssh 功能 | torvox 有没有 | 对比结论 |
|---|---|---|
| SSH 客户端（JSch：密码/密钥/跳板/端口转发） | **没有** | torvox 只有本地 PTY。SSH 是 torvox 最可能的高价值增量（见 1.5） |
| 主机管理（CRUD + 编辑表单） | 没有 | 空白项；torvox 的会话抽屉是本地会话，无主机概念 |
| 密钥管理（生成 RSA/Ed25519） | 没有 | 空白项 |
| known_hosts / TOFU | 没有 | 空白项（moke 的实现更好，见 2.3） |
| 本地 PTY（JNI） | **有** | torvox 用 Rust `libtermpty` + `LD_PRELOAD` fork/execve 方案，比 sushi-ssh 的 `sushi-pty.c`（`posix_openpt`+`exec /system/bin/sh`）更先进（更接近 Termux）；sushi-pty.c 仅作参考 |
| 终端渲染 | **有（远超）** | sushi-ssh 是 TextView + 正则 SGR 解析（`TerminalView.kt:336`），torvox 是 wgpu GPU + ghostty VT 全状态机。**不建议**吸收其渲染 |
| AI 对话→命令→确认→执行闭环 | **部分有** | torvox 有 9 个 MCP 工具（含 `run_command`），但没有"自然语言生成命令 + 安全分级 + 用户确认"的对话式 UI；sushi-ssh 的 `ConversationManager` + `CommandSafety` 三档分级模型（SAFE/CONFIRM/BLOCKED）与 torvox 的 MCP 权限模型可对照借鉴 |
| 脚本自动化（Play + PlayRunner） | 没有 | torvox 的 MCP 工具可视为"无脚本"版本；Play 的"步骤 = 命令 + 期望输出 + 超时"模型很适合 torvox 做可复用的 MCP 批处理 |
| 日志云上传（Drive）/ GitHub 反馈 | 没有 | 低优先级空白项 |
| 前台服务保活 | **有** | torvox 的 TerminalService 更强（会话在服务内）；sushi-ssh 的服务仅持连接引用 |
| 加密存储 | 部分 | torvox 无凭据需求；`SecurePrefs` 的 EncryptedSharedPreferences 用法可直接照搬（若 torvox 将来存密钥） |
| URL 分享（ShareActivity 接收文本） | 没有 | 空白项，低成本小功能 |

### 1.4 依赖分析

- **JSch（0.2.x）**：成熟、MIT、纯 Java、体积小；但维护缓慢（2024 年才恢复活跃），无 SSH 密钥交换新算法（curve25519 需 jsch-agent-proxy 或 fork）。对 torvox：**不推荐**。备选：`sshj`（moke 使用，活跃、Apache-2.0、原生支持 ed25519/curve25519）或 Rust 侧 `russh`（若走 JNI 直桥，与 torvox"Rust 核心"哲学最一致，零 Java 中间层）。结论：**若吸收 SSH，优先 sshj 或 russh，而不是 JSch**。
- `security-crypto`：torvox 可放心用（Google 官方，EncryptedSharedPreferences）。
- `generativeai`（Gemini SDK）、GMS auth/drive：与 torvox 无关（torvox 用 MCP 而非 Google AI），不吸收。
- 激进程度：中规中矩，无激进依赖。

### 1.5 可吸收到 torvox 的具体内容

1. **SSH 后端（最高优先级）**：以 moke 的 `SshTransport` 为蓝本（1.2 见 moke 章节），但**在 Rust 侧用 russh 实现**，通过现有 JNI 桥暴露 `ssh_connect/host/port/user/auth` 通道，Kotlin 侧仅做 UI。参考 `SshClient.kt:105-257` 的认证计划（`AuthPlan`：密钥优先、密码回退、跳板先连再端口转发）与 `:294 configureSession`（超时/keepalive）。注释建议：
   ```kotlin
   // AuthPlan：认证策略决策点——先试密钥，失败回退密码，
   // 跳板机必须先于目标机建立（SshClient.kt:105-110 同款思路）
   ```
2. **命令安全分级（AI 时代刚需）**：把 `CommandSafety` 的三档分类（SAFE/CONFIRM/BLOCKED）移植为 torvox MCP 工具 `run_command` 的 gate：Rust 侧维护危险命令正则表（`rm -rf /`、`mkfs.*`、`dd if=.*of=/dev/`、`shutdown`、`reboot`、`chmod -R 777 /`），命中 BLOCKED 直接拒绝，命中 CONFIRM 返回 `needs_confirmation` 让 Compose 弹确认框。参考 `CommandSafety.kt` 与 `ConversationManager.kt:115-128`（执行前检查点）。注释建议：
   ```rust
   // 安全闸门：与 sushi-ssh CommandSafety 同构——BLOCKED 直拒，
   // CONFIRM 走 UI 确认；命令输出截断 500 字符后回传 LLM（ConversationManager.kt:512）
   ```
3. **Play 脚本模型**：PlayRunner 的"步骤 = 命令 + 期望输出正则 + 超时 + 失败中止"（`PlayRunner.kt`）可做 torvox 的 MCP 批处理工具 `run_playbook`，JSON 描述即可，无需 DSL。注释建议：
   ```json
   // {"steps":[{"cmd":"apt update","expect":"Reading package lists","timeout_ms":60000}]}
   ```
4. **对话式运维 UI 模式**：`MainActivity` 的"聊天面板 + 命令卡片（显示将执行的命令 + 确认/拒绝按钮）"布局，可移植为 torvox 的 MCP 面板（替代/补充纯工具调用），参考 `MainActivity.kt` 的 chat 区与 `ConversationManager.kt:115-128` 的执行流。
5. **EncryptedSharedPreferences 模式**：若 torvox 存任何密钥（SSH 私钥/口令），直接照抄 `SecurePrefs.kt:8-21`（AES256_SIV 键 + AES256_GCM 值）。
6. **shellQuote**：`ShellUtils.kt:3-9` 的 POSIX 单引号转义（`'` → `'\''`），Rust 侧 `Command::arg` 天然等价，但若拼 shell 字符串可用此函数。
7. **不吸收**：`TerminalView.kt` 渲染（torvox 已远超）、Gemini 客户端（torvox 走 MCP 协议）、GMS 依赖全家桶。

### 1.6 项目文档吸收价值

- `docs/concepts/`：概念文档（架构/安全/自动化设计）对 torvox 的"AI 执行安全"章节有直接引用价值——特别是 CommandSafety 的威胁模型讨论。
- `docs/features/`：功能规划页（AI 对话、Play 脚本）可作 torvox MCP 增强路线的需求清单。
- `docs/process/`：开发流程文档一般，无特殊价值。
- `README.md`：产品定位（"云服务器终端 + AI"）与宣传话术可参考。

---

## 2. moke（briqt/moke）

### 2.1 项目定位与完整架构

单模块 app（`app/`）+ 两个本地库模块（`terminal-emulator/`、`terminal-view/`，均为 termux-app 的 Java 移植）。包 `com.briqt.moke`。定位：**极简但高质量的 SSH / mosh / SFTP 客户端**，UI 全 Compose，核心创新：

1. **Transport 抽象**：`SessionTransport` 接口，`SshTransport` / `MoshTransport` / `PreviewTransport` 三实现，统一接入 termux 的 `TerminalSession`（`terminal-emulator` 模块），传输层只负责"字节进、字节出 + resize"，VT 解析交给 TerminalEmulator。
2. **mosh 原生集成**：自带 mosh Android 客户端（`MoshPty` JNI，由 `scripts/build-mosh-native.sh` 交叉编译），经 SSH 通道 bootstrap 远端 mosh-server，UDP 直连。
3. **tmux 会话管理**：`Tmux.kt` 通过 SSH 执行 `tmux` 命令并解析输出，实现"tmux 会话列表 → 附加 → pane 切换"。
4. **SFTP 文件管理**：独立 SFTP 连接（不占终端通道）+ 串行传输队列 + 断点续传 + 可持久化任务。
5. **TOFU 安全**：`MokeHostKeyVerifier` 首次接受、变更告警。

### 2.2 逐文件功能说明

**传输层（moke 最值得吸收的部分）**

| 文件 | 核心类/函数 | 职责 |
|---|---|---|
| `terminal/SshConnector.kt`（5.2KB） | `SshConnector`：sshj `SSHClient` 封装；`use(host, jumpHost, block)` 扩展；认证（密码/密钥/`~/.ssh` 目录加载）；跳板 `newConnectedSession` 链 | 连接工厂：建连、认证、跳板、关闭 |
| `terminal/SshTransport.kt`（11KB） | `SshTransport.kt:19 SshTransport : SessionTransport`；`:47 start()`（开 shell 通道 + `session.setTerminal`）；`:55` 跳板提示；`:89` 启动命令/登录命令处理；`:102` 读循环（`session.processToEmulator(buf,n)`）；`:134 feed()`（向终端写提示文本）；`:140 startLatencyProbe()`（每 1s `exec("printf ''")` 测 RTT → `onLatency` 状态栏显示延迟）；`:158 write()`；`:170 updateSize()`（`cmd.changeWindowDimensions`）；`:203 close()` | SSH 终端通道传输：字节流桥接 + RTT 探测 + resize |
| `terminal/MoshTransport.kt`（13KB） | `MoshTransport.kt:29 MoshTransport : SessionTransport`；`:57 start()`（校验 mosh/terminfo 存在 → `bootstrapMosh` → `MoshPty.exec` 起 `mosh-client`）；`:64` 缺二进制提示；`:138` 读循环；`:153/170` 退出码处理；`:213` SSH 连接用于 bootstrap；`:215 write()`（`MoshPty.write`）；`:223 updateSize()`；`:253` terminfo assets 复制（`copyAsset` 递归）；`:281` `isMoshUnavailableError`（错误链匹配） | mosh 客户端传输：JNI 直连 mosh 协议、UDP、预测回显 |
| `terminal/MoshBootstrap.kt`（1.9KB） | `MoshBootstrap.bootstrap()`：经 SSH `exec` 探测远程 `mosh-server`、建立 mosh 会话、回传端口/密钥 | mosh 会话建立协议（SSH 握手部分） |
| `terminal/PreviewTransport.kt`（3KB） | 假传输（回显演示） | 演示/测试用 |
| `terminal/TerminalController.kt`（8.9KB） | `TerminalController`：绑定 `TerminalSession` + transport；`updateSize` 转发；`onLine` 回调桥；`isClosed` 判断 | transport ↔ TerminalSession 粘合 |
| `terminal/SessionManager.kt`（15KB） | `SessionManager`：会话集合管理；创建 `TerminalSession`（`TerminalEmulator` 初始化）；保存/恢复会话列表；`getSession`/`removeSession`；前台服务绑定 | 多会话生命周期中枢 |
| `terminal/TerminalThemes.kt`（7.3KB） | 内置配色（`TerminalColors.COLOR_SCHEME` 填充） | 主题 |

**SFTP 层**

| 文件 | 核心类/函数 | 职责 |
|---|---|---|
| `terminal/sftp/SftpSession.kt`（8KB） | `SftpSession`：**独立 sshj SFTP 连接**；`ls/cd/pwd/mkdir/rm/rename/stat`；`download(remote, local, resume)`；`upload(local, remote, resume)`（`sftp.get/resume` 断点续传） | 文件操作 + 续传 |
| `terminal/sftp/TransferManager.kt`（17.8KB） | `TransferManager`：全局**串行队列**（`ConcurrentLinkedQueue` + 单 worker）；`enqueue`；进度回调（分块大小、剩余字节）；失败重试（3 次）；`TransferStore` 持久化；前台服务通知更新 | 传输调度中枢 |
| `terminal/sftp/Transfer.kt`（6.1KB） | `TransferTask`（可序列化：方向/路径/大小/状态/重试次数）、`TransferStatus` 枚举 | 任务模型 |
| `terminal/sftp/TransferStore.kt` | JSON 持久化任务列表（kotlinx.serialization） | 断点任务恢复 |
| `terminal/sftp/RemotePath.kt`（5.1KB） | `RemotePath`：`~/`、`.`、`..`、绝对/相对路径规范化、`parent/child` 运算 | 路径工具 |

**数据层**

| 文件 | 核心类/函数 | 职责 |
|---|---|---|
| `data/Host.kt`（5.6KB） | `Host` 数据类（id/名称/地址/端口/用户/认证类型/密钥路径/口令/跳板/启动命令/主题色） | 主机模型 |
| `data/HostStore.kt`（2.3KB） | `HostStore`：SharedPreferences JSON 列表 CRUD | 主机持久化 |
| `data/CredentialCrypto.kt`（2.8KB） | `CredentialCrypto`：AndroidKeyStore 派生 AES-GCM 密钥，`encrypt/decrypt`（口令/私钥口令） | 凭据加密 |
| `data/SettingsStore.kt`（18KB） | 大量设置键（字体/主题/键盘/隐私/通知/ssh 默认参数），DataStore + 内存缓存 | 全局设置 |
| `data/UiPrefs.kt` | Compose UI 偏好（会话排序等） | UI 状态 |
| `data/ListOrder.kt` | 排序方向枚举 | 会话排序 |
| `terminal/KnownHosts.kt`（1.6KB） | known_hosts 文件读写（`~/.ssh/known_hosts`） | TOFU 存储 |
| `terminal/MokeHostKeyVerifier.kt`（1.7KB） | sshj `HostKeyVerifier`：首次接受并记录；再次连接指纹不匹配 → 拒绝并提示 | TOFU 验证器 |

**服务 / 杂项**

| 文件 | 核心类/函数 | 职责 |
|---|---|---|
| `terminal/MokeSessionService.kt`（4.6KB） | 前台服务（会话通知 + stop 按钮） | SSH 会话保活 |
| `terminal/MokeTransferService.kt`（5.5KB） | 前台服务（传输进度通知） | 传输保活 |
| `update/UpdateChecker.kt`（8.3KB） | GitHub Releases API 检查 + SemVer 比较（`kotlinx.serialization`） | 应用更新 |
| `terminal/FontCatalog.kt` | 字体清单（Fira Code/JetBrains Mono/Maple 等元数据） | 字体商店 |
| `terminal/FontRepository.kt`（13.1KB） | 从 Google Fonts GitHub 下载 TTF；`CustomFallbackBuilder` 合成缺字回退字体；缓存 | 字体下载与合成 |
| `MainActivity.kt` / `MokeApplication.kt` / `LocaleManager.kt` | 入口 / 初始化 / 语言 | — |

**UI 层（`ui/`）**：`MokeApp.kt`（导航：Home/Terminal/Files/Tmux 四路由）、`HomeScreen.kt`（43KB：主机卡片列表、连接/编辑/删除、拖拽排序 `Reorderable.kt`）、`TerminalScreen.kt`（32KB：`AndroidView(TerminalView)` 嵌入 Compose + ExtraKeys 行 + 状态栏显示 RTT）、`FilesScreen.kt`（29KB：SFTP 双栏文件浏览器 + 传输队列 UI）、`TmuxPanel.kt`（tmux 会话面板）、`SessionsSheet.kt`（会话切换底部弹层）、`SettingsSheet.kt`、`AppearanceScreen.kt`（26KB：主题/字体预览）、`FontsScreen.kt`、`ExtraKeys.kt`、`RichDropdown.kt`、`Components.kt`（13KB 通用组件）、`AboutScreen.kt`、`MokeViewModel.kt`（36KB：连接状态机、主机 CRUD、传输状态）。

`gradle/libs.versions.toml` 依赖：`com.hierynomus:sshj`、termux terminal-emulator/view（本地模块）、kotlinx-serialization、core-splashscreen、Compose BOM 等。

### 2.3 与 torvox 功能对比

| moke 功能 | torvox 有没有 | 对比结论 |
|---|---|---|
| SSH（sshj：密码/密钥/跳板/TOFU） | **没有** | 最大空白项。moke 的 TOFU（`MokeHostKeyVerifier` 首次接受 + 变更告警）比 sushi-ssh 的 JSch 方案更现代、更安全，**直接可移植** |
| mosh 客户端（原生 JNI + bootstrap） | 没有 | 空白项。mosh 依赖 UDP + 原生库 + 远端 mosh-server；torvox 若做远程终端，mosh 是"弱网救星"，但工程量大（需交叉编译 mosh），建议排在 SSH 之后 |
| SFTP 文件传输（独立通道 + 断点续传 + 队列） | 没有 | 空白项。`TransferManager` 串行队列 + 任务持久化 + 通知 的模型可整体吸收；若 torvox 用 russh（Rust），SFTP 也可在 Rust 侧实现 |
| tmux 集成（会话列表/附加/pane） | 没有 | 空白项。`Tmux.kt` 的"SSH exec tmux 命令 + 解析输出"思路简单有效，与具体 SSH 库解耦，吸收成本低 |
| 传输抽象（SessionTransport 接口） | 部分 | torvox 的 Rust 核心本身就是传输层；moke 的接口设计（start/write/updateSize/close + onLatency）值得在 Rust trait 设计时参考 |
| 延迟探测（RTT 状态栏） | 没有 | 小功能；SSH 场景体验利器（`SshTransport.kt:140`） |
| 多会话（TerminalSession 集合 + 前台服务） | **有** | torvox 的 SessionManager（4 会话 + 抽屉）同构；moke 的 `SessionManager` 支持任意数量 + 通知栏会话数，torvox 可参考其服务端会话注册表设计 |
| 字体商店（Google Fonts 下载 + 合成回退） | 部分 | torvox 有字体设置（内置字体 + 自定义 TTF）；moke 的 `CustomFallbackBuilder` 缺字合成（中文/emoji 回退）是 torvox 目前没有的，**值得吸收**（Compose 侧同样可用） |
| 文件管理器 UI（SFTP 双栏） | 没有 | 空白项（本地文件管理器也没有；见 neotermux 3.5） |
| 更新检查（GitHub Releases + SemVer） | 没有 | 低成本空白项 |
| 终端渲染/模拟器 | **有（远超）** | moke 用 termux Java 移植（`terminal-emulator` 模块 131KB `TerminalEmulator.java`）；torvox 的 ghostty-vt 是全状态机 + GPU，不吸收 |

### 2.4 依赖分析

- **sshj（hierynomus，0.39+）**：活跃维护、Apache-2.0、纯 Java（无 native）、支持 ed25519/curve25519 密钥交换、SFTP 完整实现、API 清爽。**对 torvox：推荐**——若 SSH 走 Kotlin/Java 侧，sshj 是当前最优解；若坚持"Rust 核心"路线，用 `russh`（Rust 原生，与 JNI 桥一致），两者 API 语义相近（`Session`/`Channel` 模型）。
- **mosh**：GPL-2.0（**许可证风险**：torvox 若闭源/专有，需规避或单独模块隔离）；依赖 terminfo assets + 原生二进制。激进程度：高（罕见 Android mosh 客户端，参考价值大）。
- **terminal-emulator/terminal-view（termux 移植）**：Apache-2.0，torvox 不需要（已有 ghostty）。
- 整体激进程度：中等——依赖选择保守（sshj、serialization），但 mosh 集成属于激进创新。

### 2.5 可吸收到 torvox 的具体内容

1. **SSH + TOFU 全栈（最高优先级）**：吸收 `SshConnector`（认证）+ `SshTransport`（通道/读写/resize/延迟）+ `MokeHostKeyVerifier`（TOFU）+ `KnownHosts`（存储）。若走 Kotlin 侧：sshj 直接搬；若走 Rust 侧：russh 对应实现，JNI 暴露 `connect(host,port,user,auth)`/`write`/`resize`/`latency`。注释建议：
   ```kotlin
   // TOFU：首次连接记录主机指纹（KnownHosts.kt），
   // 指纹变更时拒绝连接并提示用户核实——防中间人（MokeHostKeyVerifier.kt:1-40）
   ```
2. **传输接口契约**：`SessionTransport` 的 `start/write/updateSize/close + onLatency` 语义（`SshTransport.kt:19-203`）作为 torvox Rust `Transport` trait 的参照，未来可插 SSH/mosh/serial 传输。
3. **延迟探测**：`SshTransport.kt:140-152` 的 1s RTT 探测 + 状态栏展示（<4500ms 才显示，防抖动），SSH 功能落地时直接照搬。
4. **SFTP 传输队列**：`TransferManager` 串行队列 + `TransferTask` 序列化 + `TransferStore` 恢复 + 前台服务通知（`MokeTransferService`）四件套整体移植；断点续传语义见 `SftpSession.download/upload(resume)`。注释建议：
   ```kotlin
   // 串行队列：同一时间只跑一个传输，避免多通道争用 SSH 带宽；
   // 任务落盘，进程被杀后重启可恢复（TransferManager.kt）
   ```
5. **tmux 集成**：`Tmux.kt:52-156` 的"exec + 解析"模式（`tmux ls -F #{session_name}:#{session_windows}` 解析、attach、switch pane、kill）与传输层无关，torvox 在 SSH 之上加一个 `TmuxController` 即可，UI 用 `TmuxPanel.kt` 的弹层。
6. **字体合成回退**：`FontRepository` 的 `CustomFallbackBuilder` 用法（多 TTF 合成一个 Typeface，中文字体/emoji 回退），torvox 的 Compose `FontFamily` 同样适用——解决"终端字体缺 CJK 字形"问题。
7. **更新检查**：`UpdateChecker`（GitHub Releases + SemVer 三段比较）15 分钟即可移植。
8. **不吸收**：termux 移植模块（渲染已被 torvox 超越）、`PreviewTransport`。

### 2.6 项目文档吸收价值

- `README.md`：功能矩阵（SSH/mosh/SFTP/tmux）可作为 torvox 远程能力的需求基线。
- `THIRD_PARTY_NOTICES` / `NOTICE`：mosh GPL 组件的隔离声明方式值得 torvox 借鉴（若引入 GPL 组件）。
- `scripts/build-mosh-native.sh`：**高价值**——Android NDK 交叉编译 mosh 的完整流程（toolchain、termux-packages 源、静态链接），torvox 若做 mosh 可省数天踩坑。
- `CHANGELOG.md`：版本演进记录可窥见其功能优先级（先 SSH → 再 mosh/SFTP → 再 tmux），印证"先 SSH 后扩展"的路线。

---

## 3. neotermux（developer-mahabbat/NeoTermux）

### 3.1 项目定位与完整架构

多模块 Gradle 工程（`app` + `editor`/`filemanager`/`gitlib`/`sshclient`/`terminal-emulator`/`termlib` 六个库模块），包 `com.neotermux.app`。定位：**Termux 的 Compose 重写**——单 Activity + Navigation Compose + Hilt DI + Material3 深色主题，提供 8 个屏幕：终端、文件管理器、包管理器、进程管理器、Git、SSH、编辑器、设置。

**关键事实（完成度极低）**：
- `TerminalViewModel.kt:97-125 processCommand()` 是**硬编码模拟**（`ls`/`pwd`/`date`/`cat` 全部返回写死的字符串），**没有真实 shell**；`TerminalScreen` 的会话 buffer 只是 `List<String>`。
- `PackageManagerViewModel.kt:17-55`、`GitViewModel.kt:39-46`、`SshViewModel.kt:34-39`、`ProcessManagerViewModel.kt:15-16`、`EditorViewModel.kt:46-51` 全部是**模拟数据或空函数**。
- `termlib` 有真实 C 代码（`pty.c` 完整 PTY + `terminal.c` 极简 VT），但**未接入任何 UI**（Kotlin 侧没有任何调用 termlib 的类——`TerminalNative`/`PtyNative` 无调用方）。
- `editor/`、`filemanager/`、`gitlib/`、`sshclient/`、`terminal-emulator/` 模块**只有 build.gradle.kts，没有 src**。

因此其价值 = **UI 骨架与屏幕组织模式**，而非功能实现。

### 3.2 逐文件功能说明

| 文件 | 核心类/函数 | 职责 |
|---|---|---|
| `MainActivity.kt:30 MainActivity`；`:44 NavHost`（startDestination="terminal"，8 条路由） | 单 Activity + NavHost 注册全部屏幕 | 导航宿主 |
| `viewmodel/MainViewModel.kt:15`；`:23 toggleTheme`；`:30 setThemeMode`；`:36 addSession/removeSession/switchSession`；`:57 TerminalSessionInfo` | 主题 + 会话列表（内存态） | 全局状态 |
| `navigation/Screen.kt:3` sealed class（Terminal/Settings/FileManager/PackageManager/ProcessManager/Git/Ssh/Editor） | 路由常量 | 导航类型安全 |
| `di/AppModule.kt:20`（`provideDataStore`、`provideCoroutineContext`） | Hilt 模块 | DI |
| `NeoTermuxApplication.kt:13`（`:28 createNotificationChannels` 3 个通道） | Application 初始化 | 通知通道 |
| `service/TerminalService.kt:15`（`:17 onStartCommand` 前台通知 START_STICKY） | 前台服务（占位） | 保活占位 |
| `service/SshService.kt:9` | 空 Service | 占位 |
| `ui/screens/terminal/TerminalScreen.kt:43`（`:64` 横屏 NavigationRail 会话栏；`:111` 顶栏；`:326` DrawerItem） | 终端页 UI（会话 rail + 顶栏 + 输入行 + extra keys 行） | **UI 模式参考** |
| `ui/screens/terminal/TerminalViewModel.kt:23`（`:41 addSession`、`:48 closeSession`、`:55 switchSession`、`:61 setFontSize`、`:69 executeCommand`→`:97 processCommand` 模拟命令、`:87 sendKey`） | 终端状态（纯内存模拟） | 模拟终端 |
| `ui/screens/filemanager/FileManagerViewModel.kt:22`（`:28 navigateTo` 真实 `File.listFiles`、`:43 navigateUp`、`:58 navigateToSegment`、`:73 setSearchQuery`、`:81 toggleShowHidden`、`:83 refresh`）+ `FileManagerScreen.kt:43`（面包屑 `:94`、网格/列表切换 `:106`） | **唯一真实实现**：本地文件浏览（Java File API）+ 搜索 + 面包屑 | 文件管理器（真实） |
| `ui/screens/packagemanager/PackageManagerViewModel.kt:11`（`:17 refresh` 硬编码 7 个包、`:43 searchPackages` 硬编码 3 个）+ `PackageManagerScreen.kt:48`（TabRow：Installed/Updates/Search） | 包管理（模拟） | 包管理 UI |
| `ui/screens/processmanager/ProcessManagerViewModel.kt:11`（`:15 killProcess`、`:16 refresh` 全空）+ Screen | 进程管理（空） | 进程 UI |
| `ui/screens/git/GitViewModel.kt:35`（`:39 refresh` 空、`:40 switchBranch`、`:41-46 commit/push/pull/fetch/stage/unstage` 空）+ Screen（`:47` 分支/暂存区/提交列表三块） | Git 客户端（模拟） | Git UI |
| `ui/screens/ssh/SshViewModel.kt:30`（`:34 connect/disconnect/addConnection/removeConnection/generateKey` 全空）+ Screen（连接列表 + 添加表单 + 密钥生成按钮） | SSH（空壳） | SSH UI |
| `ui/screens/editor/EditorViewModel.kt:23`（`:27 updateContent`、`:31 openFile` 读文件、`:42 switchFile`、`:46 save` 仅改标记、`:50 undo/redo` 空）+ `EditorScreen.kt:38`（保存/撤销/重做按钮、`:59` 行号列、`:90` 代码区、`:111` 状态栏 Modified/UTF-8/Ln,Cn） | 编辑器（半成品：能读不能写） | 编辑器 UI |
| `ui/screens/settings/SettingsViewModel.kt:27`（`:35-40` toggle 系列 + `:40 resetSettings`）+ `SettingsScreen.kt`（分组设置项） | 设置（内存态） | 设置 UI |
| `ui/theme/Theme.kt` / `util/Constants.kt:3`（字体/滚动/PTY 常量、`SUPPORTED_ARCHIVES:33`、`SUPPORTED_LANGUAGES:34`） | 主题 / 常量 | 支撑 |
| `termlib/src/main/cpp/pty.c` | `pty_open:73`（`posix_openpt`→`grantpt`→`unlockpt`→`fork`→`dup2`→`execve`，`pty_read_thread:33` select 循环，`pty_write`，`pty_close:196`，`pty_is_running:204`） | 真实 PTY（未接线） |
| `termlib/src/main/cpp/terminal.c` | `terminal_init:36`、`terminal_write:54`（**只处理 `\n`/`\r`/32-127 可打印字符，无 ANSI 转义**）、`terminal_resize:86`、`terminal_get_buffer:113`、`terminal_destroy:131` | 极简 VT（玩具级，未接线） |
| `termlib/src/main/cpp/jni_bridge.c` | `JNI_OnLoad:35`、`Java_com_neotermux_termlib_TerminalNative_*`（`:41` init、`:46` write、`:55` resize）、`Java_com_neotermux_termlib_PtyNative_*`（`:90` write、`:99` resizeWindow、`:105` close、`:110` isRunning）、`on_pty_output:23`（回调 Java） | JNI 导出（未接线） |

### 3.3 与 torvox 功能对比

| neotermux 功能 | torvox 有没有 | 对比结论 |
|---|---|---|
| 终端（Compose 直绘 + 会话 rail） | **有（远超）** | neotermux 是模拟终端；torvox 是 GPU 真终端。**对比意义：torvox 的会话抽屉 + 横屏 NavigationRail 思路可对照其 `TerminalScreen.kt:64-107`**（横屏 rail 会话切换） |
| 文件管理器（真实） | **没有** | torvox 有 DocumentsProvider（系统文件选择器接入），但没有应用内文件浏览 UI。neotermux 的 `FileManagerViewModel`（面包屑 + 搜索 + 网格/列表 + 隐藏文件）是干净的参考骨架 |
| 包管理器 UI（apt 封装预期） | 没有 | torvox bootstrap 后无 GUI 包管理；neotermux 是模拟数据，仅 UI 布局（统计卡 + TabRow）可参考 |
| 进程管理器 UI | 没有 | torvox 有监控（内存/热/ANR）但无 `/proc` 进程列表 UI；neotermux 为空壳，UI 布局可参考 |
| Git 客户端 UI | 没有 | 模拟实现；但"分支 + 暂存区 + 提交列表"三段布局（`GitScreen`）是标准 Git GUI 信息架构，可参考 |
| SSH 屏幕 | 没有 | 空壳（连接列表 + 表单 + 密钥生成按钮）；moke/sushi-ssh 的实现远超它，不参考 neotermux 的 |
| 编辑器 | 没有 | 半成品（能读不能写）；`EditorScreen` 的行号 + 状态栏（Ln/Cn、UTF-8）布局可参考 |
| 前台服务/通知通道 | **有** | 无新意 |
| termlib PTY（C） | **有（Rust 版）** | `pty.c` 与 sushi-ssh 的 `sushi-pty.c` 同构；torvox 的 Rust PTY 更完整（信号/环境/超时）。不吸收 |

### 3.4 依赖分析

- Hilt + DataStore + Navigation Compose + Material3（`libs.versions.toml`）：全部主流稳定；torvox 用 Hilt + 自有 DI，无冲突。
- termlib：纯 C 无第三方依赖。
- 激进程度：**不激进**，依赖面保守；**但完成度是四仓库最低**——`editor` 等五个模块连 src 都没有，说明项目停在原型期。对 torvox 的启示：**不要照搬其"模块先行"的组织**（空模块是负债）。

### 3.5 可吸收到 torvox 的具体内容

1. **文件管理器屏幕（真实价值）**：以 `FileManagerViewModel.kt:22-84` 为骨架（`FileItem` 列表 + `navigateTo/navigateUp/navigateToSegment` 面包屑 + 搜索过滤 + 隐藏文件开关），把文件操作接到 torvox 的 DocumentsProvider/`filesDir` 上，UI 用 `FileManagerScreen.kt` 的面包屑（`:94-101`）与网格/列表切换（`:106`）。注释建议：
   ```kotlin
   // 面包屑导航：currentSegments 驱动，点击段即 navigateToSegment(index)，
   // 优于纯 up 栈——在深层目录快速跳回任意祖先（FileManagerViewModel.kt:58-69）
   ```
2. **横屏会话 rail**：`TerminalScreen.kt:64-107` 的 NavigationRail（新建 FAB + 会话图标列表）适合 torvox 横屏模式下的会话切换（当前 torvox 用抽屉），低成本增强。
3. **编辑器状态栏**：`EditorScreen.kt:111-117`（Modified 状态 + UTF-8 + 行/列）若 torvox 未来加 MCP 文件编辑工具，UI 信息架构可复用。
4. **`SUPPORTED_ARCHIVES/LANGUAGES` 常量表**（`Constants.kt:33-38`）：torvox 做文件操作/语法高亮时的枚举起点。
5. **不吸收**：模拟命令的 processCommand、空服务、空模块、termlib 的 `terminal.c`（VT 解析水平远低于 ghostty-vt）。

### 3.6 项目文档吸收价值

- `README.md`：功能截图与定位描述（Termux Compose 重写愿景）可用于 torvox 竞品分析。
- `strings.xml`：8 个屏幕的完整 UI 文案（中英）可作 torvox 功能命名参考。
- 无架构/设计文档（项目太早期）。

---

## 4. reterminal（RohitKushvaha01/ReTerminal）

### 4.1 项目定位与完整架构

多模块工程：`core/components`（共享 Compose 组件 + preferences 组件库 + 拉伸滚动效果）、`core/main`（终端 + rootfs + 服务）、`core/proot`（**自研 PRoot 移植**，C 代码约 700KB）、`core/resources`（字符串/图标）、`app`（打包）。定位：**"手机上跑完整 Linux"**——自带 Alpine rootfs（assets 内置 `alpine-<arch>.tar.gz.rootfs`），用 PRoot 无 root 运行，终端用 termux 官方库（`com.github.termux.termux-app:terminal-view/emulator:v0.118.3`）。

启动链：
```
SessionService.createSession → MkSession.createSession（构造环境变量 + init 脚本落盘）
  → TerminalSession(shell=/system/bin/sh, args=["-c", init-host.sh])
  → init-host.sh：解压/校验 alpine rootfs → 拼 PRoot 参数（-r rootfs -b 系统目录绑定 -0 --link2symlink --sysvipc -L）
  → exec $PROOT $ARGS sh init.sh → init.sh：写 resolv.conf、source /etc/profile、exec /bin/ash
```

### 4.2 逐文件功能说明

**终端与会话**

| 文件 | 核心类/函数 | 职责 |
|---|---|---|
| `core/main/.../terminal/TerminalBackEnd.kt:28` | `TerminalBackEnd : TerminalViewClient, TerminalSessionClient`；`:36 onTextChanged`；`:43 onCopyTextToClipboard`；`:47 onPasteTextFromClipboard`（`mEmulator.paste`）；`:54 onBell`（assets `bell.oga` 播放）；`:96 onScale`（双指缩放 11-45f）；`:105 onSingleTapUp`（硬件键盘连接时隐藏软键）；`:117 onKeyDown`（先走 `KeyShortcutHandler`）；`:160 showSoftInput` | termux 终端客户端适配：剪贴板/铃/缩放/快捷键分发 |
| `terminal/MkSession.kt:18` | `MkSession.createSession()`：`:27` 环境变量（`ANDROID_*`/`BOOTCLASSPATH`/`LINKER`/`PROOT_LOADER`/`PROOT`/`PROOT_TMP_DIR`）；`:41` init-host/init 脚本落盘；`:87` stat/vmstat 假文件；`:116` 构造 `TerminalSession`；`:128 PendingCommand` | 会话工厂：PRoot 启动参数全在这 |
| `terminal/TerminalViewModel.kt:21` | `:22` WeakReference 持有 `TerminalView`/`VirtualKeysView`；`:31 bitmap`（背景图）；`:35-37` showToolbar/showVirtualKeys/showHorizontalToolbar；`:59-79` `changeSession`（重新 attach session + 颜色注入 `mColors.set(256..258)`） | Compose 状态层（持有 View 弱引用，供 Compose 读写） |
| `terminal/TerminalScreen.kt:45` | `:61` 背景图加载；`:79 BackHandler`；`:86 AddSessionDialog`（工作模式选择）；`:100 ModalNavigationDrawer`（`TerminalDrawer`）；`:116 Box(BackgroundImage)`；`:202 VIRTUAL_KEYS` 布局 JSON 常量 | 终端主屏：Compose 包 View |
| `terminal/TerminalViewLayout.kt:30` | `:37 AndroidView(factory)` 创建 `TerminalView`（`setTextSize/attachSession/setTerminalViewClient`）；`:66` 颜色索引 256/257/258 注入（前景/背景/光标）；`:72` `colors.properties` 加载（`TerminalColors.COLOR_SCHEME.updateWith`）；`:101 VirtualKeysPager`（HorizontalPager 双页：虚拟键 + 命令编辑行 `:150`） | **Compose 嵌入 termux TerminalView 的标准姿势** |
| `terminal/TerminalDrawer.kt:24` | 会话抽屉：`SelectableCard`（`:103`，选中动画）+ 删除按钮 + 设置/新建入口 | 会话列表抽屉 |
| `terminal/TerminalUtils.kt:10` | `:14 init`（font.ttf 加载）；`:21 getViewColor`；`:23 getBackgroundColor`（30% alpha）；`:37 stat`/`:55 vmstat`（**硬编码假 /proc 内容**，注入 PRoot 内程序读取） | 工具 + 假系统文件 |
| `terminal/Rootfs.kt:9` | `:12 checkInstallation`、`:16 isRootfsInstalled`（alpine 目录已解压或 tar 存在） | rootfs 安装状态 |
| `service/SessionService.kt:24` | `:25 sessions` HashMap；`:26 sessionList`（Compose 可观察）；`:29 SessionBinder`（`:32 terminateAllSessions`、`:39 createSession`、`:56 getSession`、`:58 terminateSession`）；`:86 onCreate` 前台服务（API 34+ `FOREGROUND_SERVICE_TYPE_SPECIAL_USE`）；`:100 onStartCommand`（ACTION_EXIT）；`:108 createNotification`（会话数 + 退出按钮） | 会话服务：绑定模式 + Compose 状态暴露 |
| `service/RunCommandService.kt:7` | 空壳（`TODO("Not yet implemented")`） | 未完成 |

**快捷键系统（reterminal 原创度最高的部分）**

| 文件 | 核心类/函数 | 职责 |
|---|---|---|
| `terminal/ShortcutBinding.kt:10` | `ShortcutBinding(ctrl,shift,alt,keyCode)`；`:20 matches()`；`:29 serialize()`（"CTRL\|SHIFT\|54"）；`:40 toDisplayString()`；`:56 deserialize()`；`:75 fromKeyEvent()`；`:85 RESERVED_KEY_CODES`（HOME/BACK/POWER/音量…）；`:97 MODIFIER_KEY_CODES`；`:116 ShortcutAction` 枚举（PASTE/NEW_SESSION/CLOSE_SESSION/SWITCH_PREV/NEXT，带默认键） | 快捷键绑定模型（SharedPreferences 字符串存储） |
| `terminal/KeyShortcutHandler.kt:9` | `:11 handle()`（遍历 `ShortcutAction.entries` 匹配 → `dispatch`）；`:23 dispatch()`（PASTE→`mEmulator.paste`、NEW_SESSION/CLOSE_SESSION→服务操作、SWITCH→`changeSession` 循环切换）；`:100 generateUniqueSessionId`（main1/main2…） | 快捷键分发中枢（硬件键盘） |
| `terminal/ShortcutCaptureDialog.kt:47` | `:66 onPreviewKeyEvent` 捕获组合键（实时显示修饰键、忽略重复事件、拒绝系统保留键、检测与其他动作冲突）；`:186` window focus 等待后请求焦点（修复首次按键丢失） | **快捷键录制 UI**（Compose Dialog） |
| `settings/Settings.kt`（rk.settings 包 `:12`） | 大量设置属性（seccomp/amoled/monet/font_size/wallTransparency/working_Mode/input_mode/…）；`:105 getShortcutBinding/setShortcutBinding`；`:118 Preference`（内存缓存 + SharedPreferences 读写） | 设置数据层 |

**虚拟键盘（termux 移植 + 定制）**

| 文件 | 核心类/函数 | 职责 |
|---|---|---|
| `terminal/virtualkeys/VirtualKeysView.java:70` | termux 移植：`GridLayout` 动态生成按键（`reload(VirtualKeysInfo)`）、`SpecialButton`（CTRL/ALT/SHIFT 按下状态机 `SpecialButtonState`）、长按重复（`mRepetitiveKeys` + 定时器）、popup 键、颜色/触觉配置 | 虚拟键 View 引擎 |
| `terminal/virtualkeys/VirtualKeysListener.kt:7` | 按键→`session.write(转义序列)`（UP=`\u001B[A`、PGUP=`\u001B[5~`…） | 按键动作 |
| `terminal/virtualkeys/VirtualKeyClient.kt:8` | 同上的另一实现（含 ESC/TAB/HOME/END 映射） | 备用客户端 |
| `VirtualKeysConstants.java` / `VirtualKeysInfo.java` / `SpecialButton.java` | 常量（`CONTROL_CHARS_ALIASES`）、布局 JSON 解析、特殊键状态 | 支撑 |

**rootfs / 资源 / 其他**

| 文件 | 职责 |
|---|---|
| `core/main/assets/init.sh`（`:1-28`：PATH/HOME/resolv.conf/PS1/PIP_BREAK_SYSTEM_PACKAGES/linkerconfig 修复/exec ash） | rootfs 内启动脚本 |
| `core/main/assets/init-host.sh`（`:1-66`：解压 rootfs → 拼 PRoot 参数：`-b /apex,/odm,/product,/system,…,-b /sdcard,-b /dev,-b /data,-b /proc,-r $ALPINE_DIR,-0,--link2symlink,--sysvipc,-L`） | host 侧 PRoot 启动器（torvox 可整篇借鉴） |
| `core/main/assets/alpine-{aarch64,armhf,x86_64}.tar.gz.rootfs` | 三架构 Alpine rootfs 内置包 |
| `AlpineDocumentProvider.kt:25` | DocumentsProvider：root = `alpineHomeDir`；`:29 queryRoots`、`:56 queryChildDocuments`、`:75 openDocument`、`:99 createDocument`（重名加 " (2)"）、`:122 deleteDocument`、`:134 renameDocument`、`:156 querySearchDocuments`、`:249 getMimeType` | **rootfs 文件通过系统文件 App 访问** |
| `terminal/App.kt:19`（`:22 getTempDir`；`:32 onCreate` ANRWatchDog + StrictMode） | Application |
| `update/UpdateManager.kt:9`（`:10 onUpdate` 重写 init 脚本） | 应用更新时刷新 init |
| `crashhandler/CrashHandler.kt:9`（crash.log 落盘） | 崩溃记录 |
| `theme/ThemeManager.kt:17`（AMOLED/Monet 切换）、`Theme.kt`/`Color.kt`/`Type.kt` | 主题 |
| `libcommons/`（Utils.kt:24 runOnUiThread/toast/isDarkMode、FileUtil.kt、ApplicationContext.kt、ApplicationBackground.kt、ActionPopup.kt、LoadingPopup.kt） | 工具与全局弹层 |
| `ui/animations/NavigationAnimationTransitions.kt`、`ui/components/`（BottomSheetContent/InputDialog/RadioBottomSheet/ScrollableTabLayout/SelectableCard/SettingsToggle/SetStatusBarTextColor/InfoBlock）、`routes/MainActivityRoutes.kt` | 通用 UI 件 |
| `core/components/compose/preferences/`（base：PreferenceGroup/PreferenceLayout/PreferenceDivider/ExpandAndShrink/NestedScrollStretch/ScrollContainers；category：PreferenceCategory；normal：Preference；switch：PreferenceSwitch） | **设置页声明式组件库**（与 Karbon 同源） |
| `core/components/compose/edges/StretchEdgeEffect.java` + `EdgeEffectCompat.java` | 列表滚动**拉伸回弹**效果（Compose 无法原生实现的 edge effect 注入） |
| `core/proot/src/main/cpp/` | **PRoot 移植**（vendored 第三方 C）：`cli/`（参数解析）、`tracee/`（进程跟踪）、`syscall/`（syscall 翻译）、`path/`（路径绑定/规范化）、`execve/`、`ptrace/`（ptrace 管理）、`extension/`（fake_id0/fs/bind/…）、`loader/`（loader 注入）、`talloc/`（内存分配）；入口 `cli/proot.c`、`tracee/tracee.c`、`syscall/enter.c`、`extension/fake_id0/fake_id0.c` | 无 root 用户态系统调用翻译（ptrace 拦截） |
| `app/build.gradle.kts`、`core/main/build.gradle.kts`（依赖：termux terminal-view/emulator v0.118.3、utilcode、anrwatchdog、palette）、`libs.versions.toml`（AGP 9.2.1/Kotlin 2.3.20/Compose BOM 2025.11——非常激进的新版本） | 构建配置 |

### 4.3 与 torvox 功能对比

| reterminal 功能 | torvox 有没有 | 对比结论 |
|---|---|---|
| PRoot 完整 Linux rootfs（Alpine 三架构） | **没有** | torvox 用 Termux bootstrap（proot 目录结构 + LD_PRELOAD 方案）。reterminal 的 PRoot 是**完整 ptrace 翻译器**（能跑 `apk add`、`docker` 级 syscall），torvox 的 bootstrap 更轻但兼容性差。**结论：torvox 无需换 PRoot**（bootstrap 已满足 apt 场景），但 `init-host.sh` 的绑定参数清单（`-b /apex,/odm,/product,/system,/sdcard,/dev,/data,/proc` + `-0 --link2symlink --sysvipc`）对 torvox 改进 bootstrap 绑定有直接参考价值 |
| 虚拟键盘面板（VirtualKeys 全套：特殊键状态机/长按重复/popup/双页 pager） | **部分** | torvox 有 ModifierBar（Ctrl/Alt/Esc 行）。reterminal 是 termux 级完整实现：**SpecialButton 按下状态机（CTRL/ALT/SHIFT 点亮保持）、长按重复（`mRepetitiveKeys`）、popup 键（长按出备选字符）** 是 torvox 可吸收的交互增强（`VirtualKeysView.java` 全文件） |
| 快捷键录制（ShortcutCaptureDialog + ShortcutBinding + KeyShortcutHandler） | 没有 | **torvox 的硬件键盘体验空白**。reterminal 的三件套（绑定模型 `ShortcutBinding.kt:10` + 捕获 UI `ShortcutCaptureDialog.kt:47` + 分发 `KeyShortcutHandler.kt:11`）可整体移植到 Compose（torvox 有硬件键盘场景） |
| 自定义背景（图片 + 透明度 + 模糊） | 没有 | torvox 终端是纯色背景；reterminal 的 `BackgroundImage` + `wallAlpha` + `background_blur`（`TerminalScreen.kt:61-71`、`Customization.kt:65-107`）是低成本视觉功能 |
| rootfs DocumentsProvider | **部分** | torvox 已有 DocumentsProvider（bootstrap 文件）。reterminal 的 `AlpineDocumentProvider` 暴露整个 rootfs 且支持 create/rename/delete/search，torvox 的 provider 若只读可对照补全（`:99 createDocument` 重名策略 `:106-110`） |
| 会话服务（绑定 + Compose 可观察 sessionList + 通知会话数 + ACTION_EXIT） | **有** | torvox 的 TerminalService 同构且更完整；reterminal 的 `mutableStateMapOf` 暴露给 Compose 的方式（`SessionService.kt:26`）值得 torvox 参考（Compose 直接观察服务状态，免回调） |
| 拉伸滚动（StretchEdgeEffect） | 没有 | 小功能：列表滚动过界回弹（`StretchEdgeEffect.java`），torvox 的文件/会话列表可用 |
| preferences 声明式设置组件库 | 部分 | torvox 设置屏手写；reterminal 的 `PreferenceGroup/PreferenceSwitch/PreferenceCategory` 声明式 DSL（与 Karbon 同源）可显著减少 torvox 设置页样板代码 |
| 终端渲染/模拟器 | **有（远超）** | termux v0.118.3 官方库；torvox 不吸收 |
| 主题（AMOLED/Monet/动态色） | **有** | torvox 已有主题系统；reterminal 的 `ThemeManager` 无新意 |
| 崩溃日志 | 部分 | torvox 有 ANR WatchDog；reterminal 的 `CrashHandler`（crash.log 落盘）更简单直接 |

### 4.4 依赖分析

- **termux terminal-view/emulator v0.118.3（Maven 官方制品）**：Apache-2.0、稳定。torvox **不需要**（已有 ghostty-vt）。
- **PRoot（GPL-2.0+）**：**许可证与体积双重问题**（~700KB C、GPL 传染）。torvox 若吸收需评估：当前 bootstrap 方案（LD_PRELOAD + fake proot 目录）够用则不动；若追求完整 rootfs 能力，可考虑 Rust 重写轻量版或隔离模块。
- **utilcode（blankj）**：实用工具库（剪贴板/键盘/文件），torvox 用不上（已有自有工具）。
- 版本激进程度：**很高**（AGP 9.2.1 / Kotlin 2.3.20 / Compose BOM 2025.11 / compileSdk 37）——领先 torvox 的构建链，但其激进版本本身对 torvox 无吸收价值（torvox 应按自己的节奏升级）。

### 4.5 可吸收到 torvox 的具体内容

1. **快捷键录制三件套（高优先级）**：`ShortcutBinding.kt`（序列化格式 `"CTRL|SHIFT|54"` 可直接复用）+ `ShortcutCaptureDialog.kt`（组合键捕获、修饰键实时提示、冲突检测、保留键拒绝、window focus 等待修复）+ `KeyShortcutHandler.kt`（分发）。torvox 接入点：`TerminalKeyHandler`（Rust 侧已有 key handling？）——Kotlin 侧捕获后映射到现有动作（粘贴/新会话/切换）。注释建议：
   ```kotlin
   // 捕获对话框：先等 Dialog 窗口真正获得焦点再 requestFocus，
   // 否则硬件键盘首键会被窗口激活吃掉（ShortcutCaptureDialog.kt:182-190）
   ```
2. **SpecialButton 状态机**：把 `SpecialButton`（CTRL/ALT/SHIFT 按下保持、再次按下取消、与其他键组合）与长按重复（`VirtualKeysView.java:80-91` 的 `mLongPressTimeout/mLongPressRepeatDelay` 常量）移植到 torvox 的 ModifierBar。注释建议：
   ```java
   // 长按重复：按住方向键 400ms 后每 80ms 重发一次，
   // 等效 PC 键盘的按键重复（VirtualKeysView.java DEFAULT_LONG_PRESS_*）
   ```
3. **init-host.sh 的 PRoot 绑定参数**：torvox 的 bootstrap 启动脚本对照补齐绑定（`/apex,/odm,/product,/system_ext,/vendor,/linkerconfig,ld.config.txt,/plat_property_contexts`）与参数（`-0 --link2symlink --sysvipc -L --kill-on-exit`），可解决部分应用在 bootstrap 下找不到 linker 配置的问题（reterminal 的 `init.sh:16-19` linkerconfig 修复同理）。
4. **rootfs 假 /proc 文件**：`TerminalUtils.kt:37-234` 的 stat/vmstat 注入思路（PRoot 内无 /proc 时用文件替代）——torvox 若遇到 `ps`/`top` 类工具在 bootstrap 下报错，可参考（注意：torvox 的 LD_PRELOAD 方案通常可绑定真 /proc，此技巧优先级低）。
5. **Compose 可观察服务状态**：`SessionService.kt:26 sessionList = mutableStateMapOf(...)` + `currentSession = mutableStateOf(...)`——服务内直接用 Compose 状态，UI `collectAsState` 免去回调/Flow 样板。torvox 的 SessionManager 可对照简化。
6. **自定义背景（图片 + alpha + blur）**：`TerminalScreen.kt:61-71` + `Customization.kt:65-107` 的交互（透明度滑杆 + 模糊滑杆），torvox 渲染层加背景层即可（wgpu 里画一张纹理，成本低）。
7. **preferences 声明式组件**：`PreferenceGroup/PreferenceCategory/Preference/PreferenceSwitch`（`core/components/.../preferences/`）的 DSL 模式移植到 torvox 设置屏（torvox 有多页设置，样板代码可减半）。
8. **StretchEdgeEffect**：`edges/StretchEdgeEffect.java`（Compose `LazyColumn` 过界回弹）移植为通用 modifier。
9. **不吸收**：termux 渲染库、PRoot 全量 C 代码（除非未来要完整 rootfs，则单独评估 GPL）、RunCommandService 空壳。

### 4.6 项目文档吸收价值

- `README.md`：功能亮点（rootfs 一键运行、虚拟键盘、快捷键）与 UI 截图可作为 torvox 功能对标清单。
- `core/main/assets/init.sh` / `init-host.sh`：**直接可复用**的 rootfs 启动脚本（torvox bootstrap 改进的蓝本）。
- `strings.xml`（core/resources）：功能命名与描述（多语言）可参考。
- 无架构文档（代码即文档，且注释质量一般）。

---

## 5. 横向对比与 torvox 吸收优先级

### 5.1 功能空白矩阵（torvox 视角）

| 功能 | sushi-ssh | moke | neotermux | reterminal | torvox 吸收优先级 |
|---|---|---|---|---|---|
| SSH 客户端 | ✅ JSch | ✅✅ sshj（推荐蓝本） | 空壳 | — | **P0**（配合 russh/sshj） |
| TOFU / known_hosts | — | ✅✅ | — | — | P0（随 SSH） |
| 密钥/主机管理 UI | ✅ | ✅ | 空壳 | — | P1（随 SSH） |
| mosh | — | ✅（GPL，原生） | — | — | P2（隔离模块） |
| SFTP + 断点续传 + 队列 | 上传 only | ✅✅ | — | — | P1 |
| tmux 集成 | — | ✅ | — | — | P1 |
| 延迟探测 | — | ✅ | — | — | P2（随 SSH） |
| AI 对话→命令→确认 | ✅✅ | — | — | — | P1（MCP 增强） |
| 命令安全分级 | ✅✅ | — | — | — | P1（MCP gate） |
| Play 脚本自动化 | ✅ | — | — | — | P2 |
| 快捷键录制 | — | — | — | ✅✅ | P1（硬件键盘） |
| 虚拟键盘增强 | — | — | — | ✅✅ | P2（ModifierBar 增强） |
| 文件管理器 UI | — | ✅（SFTP） | ✅（本地，真实） | — | P2 |
| 字体合成回退 | — | ✅ | — | — | P2 |
| 自定义背景/模糊 | — | — | — | ✅ | P3 |
| PRoot 完整 rootfs | — | — | — | ✅（GPL） | P3（评估） |
| 更新检查 | — | ✅ | — | — | P3 |
| 声明式 preferences | — | — | — | ✅ | P3 |
| 工具屏（包/进程/Git/编辑器） | — | — | UI 骨架 | — | P3 |

### 5.2 关键架构决策建议

1. **SSH 走 Rust 还是 Kotlin？** 三方案对比：JSch（sushi-ssh，维护慢，弃）、sshj（moke，成熟，但绕开 torvox 的 Rust 核心）、russh（Rust 原生，与 JNI 直桥、ghostty 输入管线天然一致，**推荐**）。若短期求快可先 sshj 落地再迁 russh。
2. **mosh 的 GPL 问题**：moke 将 mosh 作为独立 native 组件（`MoshPty` JNI + terminfo assets）隔离，torvox 若引入需保持同样隔离并公开 NOTICE。
3. **AI 能力定位**：torvox 已有 MCP 工具集；sushi-ssh 的对话→命令→安全确认闭环可作为 torvox 的"AI 面板"功能层（UI 层复用其命令卡片交互，安全层复用 CommandSafety 三档模型），不必引入 Gemini SDK。
4. **会话服务状态暴露**：采用 reterminal 的 `mutableStateMapOf` 直曝服务状态模式，减少 torvox SessionManager↔UI 的样板。
5. **不吸收清单**：所有 TextView/Java 自绘终端（sushi-ssh `TerminalView`、neotermux `terminal.c`、reterminal termux 库——torvox 的 ghostty-vt + wgpu 均为全状态机/GPU 方案，代际领先）；空壳模块（neotermux 五个无 src 模块）；PRoot 全量代码（除非 rootfs 完整化立项）。

### 5.3 路线建议（按吸收优先级排序）

1. **SSH 会话（P0）**：russh + TOFU（moke 模型）→ 主机/密钥管理 UI（sushi-ssh `HostEditActivity`/`KeysActivity` 信息架构）→ 延迟探测。
2. **AI 安全闭环（P1）**：CommandSafety 三档 gate + 命令确认卡片 UI + Play 批处理（MCP 扩展）。
3. **SFTP 传输（P1）**：独立通道 + 串行队列 + 断点续传 + 任务持久化（moke 四件套）。
4. **硬件键盘体验（P1）**：快捷键录制三件套（reterminal）+ 虚拟键状态机增强。
5. **tmux 集成（P1）**：SSH exec + 输出解析 + 面板 UI。
6. **体验小件（P2-P3）**：字体合成回退、自定义背景、拉伸滚动、更新检查、声明式设置组件。

## sushi-ssh deep-v1 增量（2026-08-07 精读确认）

### LocalShellBackend.kt（本轮完整 40+ 行）
- 本地 shell 后端：`System.getenv("SHELL") ?: "/system/bin/sh"` + nativeStart(nativeHandle) + **CharsetDecoder + CodingErrorAction（UTF-8 容错解码）**
- `nativeHandle != 0L` 作为连接状态
- **torvox 对照**：torvox PTY 吃原始字节（无需解码）——sushi 的 CharsetDecoder 方案适用于"按行回调"的 SSH 后端模型，torvox 不适用（确认）

### TerminalView.kt（403 行完整结构）
- **EditText 子类 + SpannableStringBuilder**：OSC 状态机（NONE/ESC_SEEN/IN_OSC/IN_OSC_ESC_SEEN）+ applyColors(Spannable) + trimBuffer 上限——**纯 Java Canvas/EditText 方案**
- **torvox 对照**：torvox wgpu GPU 渲染远优于 Spannable 渲染（内存/性能）——确认 torvox 领先，无需借鉴

### GeminiClient.kt（327 行结构确认）
- sushi 的 AI 集成（Gemini API 客户端）——torvox 无 AI 客户端（MCP 是服务端）——P3 记录（若 torvox 未来加 AI 对话工具可参考）

### 新增汇总
| # | 发现 | 级别 |
|---|------|------|
| 1 | CharsetDecoder 容错（LocalShellBackend）——torvox 字节流不需要 | 确认 |
| 2 | EditText+Spannable 渲染 vs torvox wgpu——torvox 领先 | 确认 |
| 3 | Gemini AI 客户端——torvox 无（MCP 服务端） | P3 |

## moke deep-v1 增量（2026-08-07 精读确认）

### TerminalController.kt（159 行完整）
- termux terminal-emulator 的 TerminalSessionClient 实现（onTextChanged/onBell/onColorsChanged/onScale）——**moke 用 termux 内核**（确认）
- `shouldEnforceCharBasedInput() = keyboardMode == SECURE`（:114）、`isTerminalViewSelected() = keyboardMode != IME`（:121）——键盘模式影响输入路径

### TerminalThemes.kt（本轮已读）
- **TermColorScheme：id/name/nameZh/isDark/bg/fg/accent/ansi 16 色**——`applyToTerminal()` 用 Properties 写 `TerminalColors.COLOR_SCHEME`（termux 内核静态调色板）
- **torvox 对照**：torvox setTheme 54 字节 JNI 打包——moke 的 accent + 中文名 + isDark 字段是 torvox 主题模型缺失（P3：torvox 主题可加 accent 色用于 UI 高亮）

### Tmux.kt（246 行结构）
- tmux 集成（attach/list-sessions 命令构造）——torvox 无 tmux 集成（超出终端范围，P3 记录）

### FontRepository.kt（245 行结构）
- 字体仓库（下载/缓存自定义字体）——torvox 字体来自系统扫描 + 自定义路径（已实现等价）
