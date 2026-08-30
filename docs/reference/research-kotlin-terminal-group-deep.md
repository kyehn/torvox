# Kotlin 终端组深度研究（deep-v2 汇总）

> 定位：阶段 2 交付物——10 个 Kotlin 终端仓库亲复查 + 交叉结论 + P0-P2 吸收排序。仓库独立深研见 research-mid-repos-a/b.md 与 research-small-repos.md（本文件只记**增量**与**交叉**）。
> 方式：亲自逐文件核验（refs/ 快照）+ 既有子代理文档对照。

## 0. 一览

| 仓库 | 定位 | 相对 torvox 唯一价值 |
|------|------|---------------------|
| sushi-ssh | SSH+终端+Gemini AI 闭环 | CommandSafety 三档分类器（已注释到 Bridge.kt）、对话→命令→确认→执行状态机 |
| moke | SSH/SFTP/tmux/mosh 终端 | TOFU 主机密钥（MokeHostKeyVerifier:1-41）、sshj 传输、延迟探测、断点续传 SFTP |
| terminator | Compose 终端 | 会话持久化 SessionRepository、ForegroundService 模式 |
| termx | x11/PTY C 深层终端 | 传感器（SensorProvider）、ExtraKeysView（额外键）、x11 显示服务 |
| neotermux | UI 壳 | 仅 UI 骨架（功能模拟）——不吸收 |
| reterminal | 终端+快捷键录制 | AlpineDocumentProvider CRUD、虚拟键盘、快捷键录制、proot |
| onecode | empty submodule | 无（terminal-core 为空） |
| cpmdroid | Z80/CP-M 仿真器（含 VT100 终端） | emu_io_android.cpp（输入映射）、VT100 终端仿真（低相关度） |
| redterm | proot 发行版终端 | init.sh 链、Settings 输出导出 |
| ply | Rust 终端 | proot.rs（未接线） |

## 1. sushi-ssh（亲自复核）

### 1.1 CommandSafety（app/src/main/java/net/hlan/sushi/CommandSafety.kt:1-220 全读）

三级分类器，torvox 无对应物：
- `classify()`（:44-67）：BLOCKED→按 `&&|\|\||[;|]` 分段→段内 isBlocked→首词 shell 解释器（bash/sh/zsh/.../node）→BLOCKED→任一段 !isSafe=CONFIRM→否则 SAFE
- `isBlocked()`（:73-123）：shutdown/reboot/poweroff/rm -rf //mkfs/dd if=/dev//fdisk/:(){ :|:& };:/while true; do; done/apt autoremove
- `isSafe()`（:129-207）：只读命令 set（ls/cat/grep/find/ps/df/...）+ 正则（systemctl status、journalctl、apt list、docker ps...）
- 测试：`CommandSafetyTest.kt`（50 个 @Test：ls/cat 安全、`ls && apt install` →CONFIRM、`curl | bash`→BLOCKED、空串→SAFE）

**torvox 落点**：`Bridge.kt writeToPty` 上方已加注释（见 §4）。

### 1.2 其他增量事实

- `ConversationManager.kt:97`：LLM 生成命令 → `CommandSafety.classify` → 确认/直执行 → 输出截断 500 字
- `sushi-pty.c`：`forkpty`-style + slave 端 `execvp(cmd, argv)`（`$SHELL` 或 fallback /system/bin/sh，参数 -i interactive） + 读线程回调 Java
- `SecurePrefs.kt:8`：EncryptedSharedPreferences（AES256_SIV/GCM）

## 2. moke（亲自复核）

### 2.1 MokoHostKeyVerifier（:1-41 全）

TOFU：首次连接记录指纹放行（终端提示）；之后一致放行、不一致拒绝+告警（字符串 `\n`→`\r\n` 转换）。
→ torvox 若未来 SSH（russh）需同样 TOFU（sshj 的 PromiscuousVerifier 是反模式）。

### 2.2 SshTransport（:1-60 首段）

- `start()` 后台线程连接；`startLatencyProbe`（:140）实时 RTT 回调；写走单线程 executor（避免阻塞 UI）；跳过跳板机 direct-tcpip。
- 对比：torvox 无 SSH 传输（P0 候选 russh 替代 sshj）。

### 2.3 测试资产

- `TmuxAttachTest/MoshPtyTest/RemotePathTest/TransferResumeTest/SessionTitleTest`（app/src/test/.../terminal/）——SFTP 路径/tmux 附加的测试命名基线，torvox SFTP 若加可对照。

## 3. 组 B/C 亲自复核（增量）

### 3.1 terminator — SessionRepository（app/src/main/java/com/terminator/app/session/SessionRepository.kt:1-50）

- DataStore 持久化会话定义（`sessionDataStore` + JSON 编解码），**不变式："至少一个 default 会话"**（delete 时若被删的是 default，自动提升剩余第一个或回退 `defaultAndroidShell`，default 兜底 :38-43 在 delete() :35-46）。
- → torvx SessionDrawer 无会话持久化（只有临时 session 列表）；若加"命名会话/默认会话"功能，此模式直接可用。

### 3.2 termx — SensorProvider（sensor/SensorProvider.kt:1-177 全读）

- termux-sensor 类实现（全 177 行）：`listSensors()` :45-50、`getSensorInfo()` :55-71、`readSensor()` :76-114、`readAllSensors()` :119+。→ torvox MCP 若加 `sensor` 工具可参考（不在优先级前列）。

### 3.3 reterminal — AlpineDocumentProvider（core/main/.../AlpineDocumentProvider.kt:25-259 全读）

- **docId = file.absolutePath 直通**（:238-246）：简单但**根泄露风险**（不校验目录包含）。torvx `TerminalDocumentsProvider`（402 行）用 symlink 编码 + canonicalPath 消毒 + 白名单——**保留 torvox 加强版，不吸收此反模式**。

### 3.4 redterm — SearchHighlightOverlay（ui/SearchHighlightOverlay.kt:1-98 全读）

- 独立 overlay View 叠加到 TerminalView 上；`SearchMatch(row,startCol,endCol)`（:10）；匹配色 0x66CDD6F4 / 当前项 0xCC89B4FA（:22-29，主题色））、`invalidate()` 随 `postOnAnimation` 每帧追踪（:34-63）——但 **每次整 view redraw**（简单但低效）。
- → torvx 搜索高亮在 GPU 渲染（shader 高亮），**代际领先**；overlay 方案仅作参考（不需要）。

### 3.5 cpmdroid / onecode / ply / neotermux（轻量确认）

- cpmdroid：Z80/CP-M 仿真器（含 VT100 终端；`DiskCatalog*` 为磁盘映像管理），终端部分低相关——不吸收（torvox 用 ghostty-vt 全状态机，代际领先）。
- onecode：terminal-core 是空 submodule——无内容。
- ply：Rust 终端 + android/ 旧 build.gradle——反模式记录（Rust 终端却用 Java 壳？不吸收）。
- neotermux：UI 壳 + 真实 termlib PTY 模块（review-1 提示），功能为模拟数据——仅 UI 结构参考。

## 4. torvox 已落注释（本阶段）

1. `native/src/terminal/pty.rs` build_env：Termux 变量/SSL_CERT_FILE 缺口 + kill 链参考（zed-port）
2. `android/app/.../bridge/Bridge.kt` writeToPty 上方：CommandSafety 三段分类器参考（sushi-ssh）

## 5. 吸收优先级（增量更新，非完整清单）

> 本表只列 deep 阶段新增项；完整 P0–P2 表见 `research-mid-repos-b.md §5.3`（含 SSH P0、SFTP/tmux P1、硬件键盘 P1 等，不在此重复、未撤销）。

- P1：MCP run_command + CommandSafety gate（sushi-ssh）
- P1：SSH TOFU（moke，与 mid-b 的 SSH P0 会话项配套）
- P2：reterminal 虚拟键盘/快捷键录制
- P2：会话持久化 SessionRepository 模式（terminator）——若加保存/恢复会话功能
- P3：sensor MCP 工具（termx SensorProvider）
