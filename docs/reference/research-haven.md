# 深度研究：GlassHaven/Haven

> 研究日期：2026-08-06 | 克隆位置：`repositories/refs/haven`（depth 1）
> 语言：Kotlin（多模块 Gradle）+ Rust（RDP 等）；Compose Material3
> 定位：大型远程桌面/终端/agent 集合应用。核心价值在 **智能复制（smartCopy）** 与 **跨行换行 URL 选择**——终端选择系统的增强方向

## 1. 项目结构

```
haven/
├── feature/terminal/ # 终端功能模块（核心研究目标）
│ ├── TerminalViewModel.kt (3311) # 终端状态
│ ├── TerminalScreen.kt (2140) # Compose 屏幕
│ ├── SelectionToolbar.kt (606) # 选择工具栏 + 智能复制 ← 核心
│ ├── FloatingTextInputDialog.kt (759)
│ ├── OscHandler.kt (411)
│ └── agent/TerminalSessionRegistry.kt (288)
├── termlib/ # connectbot termlib 集成（Gradle 模块）
├── rdp-kotlin/rust/ # Rust RDP 客户端（egfx/surface.rs）
├── et-kotlin/ mosh-kotlin/ rnsh-kt/ spice-kotlin/ # 多种协议
├── wayland-android/ build-proot/
└── core/ dev/ feature/ # 其他功能模块
```

## 2. 选择系统：`SelectionToolbar.kt`

### 2.1 长按选词：`expandSelectionToWord`

```kotlin
internal fun expandSelectionToWord(controller, emulator) {
 val range = controller.getSelectionRange() ?: return
 // 空白不扩展
 if (text[col].isWhitespace()) return
 // 扩展到连续非空白 token（路径、URL 等）
 var startCol = col
 while (startCol > 0 && !text[startCol - 1].isWhitespace()) startCol--
 var endCol = col
 while (endCol < text.length - 1 && !text[endCol + 1].isWhitespace()) endCol++
 // token 到达行边界时尝试跨行 URL 扩展
 val urlRange = expandAcrossUrlWrap(lines, row, startCol, endCol, columns)
 if (urlRange != null) {
 controller.updateSelectionStart(urlRange.startRow, urlRange.startCol)
 controller.updateSelectionEnd(urlRange.endRow, urlRange.endCol)
 return
 }
 controller.updateSelectionStart(row, startCol)
 controller.updateSelectionEnd(row, endCol)
}
```

要点：**非空白 token 扩展**（`!isWhitespace()` 判定）——与 termux 的"扩展到空字符"语义一致但更简单（不用 `getSelectedText` 逐字符查）。**空格判定** = `Char.isWhitespace()`。

### 2.2 跨行换行 URL 选择：`expandAcrossUrlWrap`（torvox 缺失功能）

算法（`SelectionToolbar.kt:120-214`）：

1. **向后走**：当前行单词是行首（column-0 软换行或悬挂缩进延续）且前一行尾部是 URL-safe 字符 → 扩展到前一行
2. **向前走**：当前行到末尾且下一行开头是 URL-safe 字符 → 扩展到下一行（**缩进行只在"单独短 run 在空白行"时视为 wrap tail**——区分悬挂缩进 vs 缩进散文 vs 表格单元）
3. **验证 joined 结果**像完整 URL（`looksLikeFullUrl` 正则：`^(?:https?://|www\.)[\w-]+(?:\.[\w-]+)+...`）——防止把相邻散文卷入选择

URL-safe 字符集：`isLetterOrDigit() || this in "/:@!$&'()*+,;=-._~%?#[]"`

### 2.3 智能复制：`smartCopy`（torvox 缺失功能）

```kotlin
internal fun smartCopy(controller, emulator): String? {
 // 1. TUI 边框剥离：检测垂直 box-drawing 字符（│┃║|┆┇┊┋）
 // 列在 >=60% 非空行出现 → 视为面板边界，提取面板内容
 val borderCols = findConsistentBorderColumns(fullTexts)
 if (borderCols.isNotEmpty()) {
 return extractPanelContent(fullTexts, borderCols, sel.startCol)
 }
 // 2. 多行硬换行且重组成完整 URL → 用 rebuildWrappedUrl
 if (sel.endRow > sel.startRow) {
 rebuildWrappedUrl(...)?.let { return it }
 }
 // 3. 默认：controller.getSelectedText()（libvterm softWrapped flag 解包软换行）
 return controller.getSelectedText().ifEmpty { null }
}
```

`SmartTerminalClipboard`（:407-430）：`ClipboardManager by delegate` 代理 + `CompositionLocalProvider` 注入——**所有复制路径**（工具栏按钮 + 库内 popup）都经过 smartCopy 处理。空结果回退原文本（防"选区行已滚出快照"时清空剪贴板）。

### 2.4 工具栏 UI

- Compose `Surface(tonalElevation=2.dp)` + `FilledTonalButton`/`IconButton`
- 锚点移动按钮（`AnchorMover.moveStart/moveEnd`）—— d-pad 控制选区端点
- 支持 hyperlinkUri（OSC 8）/ bracketPasteMode 参数

**对比 torvox**：torvox 用系统 ActionMode（用户要求"系统样式"，正确）；Haven 用 Compose 自绘工具栏。Haven 的锚点移动按钮（上下左右移 start/end）是 torvox 缺失的。

## 3. 其他参考点

- `MouseModeTracker.kt` (153)：鼠标报告模式跟踪（torvox 有 `MouseModeTrackerTest` 同功能）
- `OscHandler.kt` (411)：OSC 序列处理
- `FloatingTextInputDialog.kt` (759)：浮动文本输入（无硬件键盘场景）
- `rdp-kotlin/rust/src/egfx/surface.rs`：Rust 侧 EGL surface 管理（RDP 渲染）

## 4. 依赖清单

| 依赖 | 用途 | 本项目适用性 |
|------|------|--------------|
| connectbot/termlib | 终端核心 | 不适用（libghostty-vt 更完整） |
| Material3 Compose | UI | **已用** |
| Rust（RDP/mosh） | 协议 | 不适用（无这些协议需求） |

## 5. 项目文档吸收价值

- `VISION.md` / `CHANGELOG.md`：产品愿景与变更日志
- 测试命名：`SmartCopyTest`、`FloatingInputTextContextMenuProviderTest`、`MouseModeTrackerTest` —— torvox 可借鉴为智能复制写专项测试

## 6. 代码注释引用（待加入 torvox 代码）

```
SelectionExpander.kt (torvox):
// 跨行换行 URL 选择参考 Haven SelectionToolbar.kt:120-214 (expandAcrossUrlWrap)
// URL-safe 字符集 + 缩进散文区分 + looksLikeFullUrl 验证
// 智能复制参考 Haven smartCopy :357-405 (TUI 边框剥离 + soft-wrap 解包)
// ClipboardManager 代理模式参考 SmartTerminalClipboard :407-430
```

## 7. 结论

Haven 是**智能复制的唯一参考**：

1. `expandAcrossUrlWrap`：跨行 URL 选择算法（可移植到 torvox 的 SelectionExpander）
2. `smartCopy`：TUI 边框剥离（复制 `htop` 类面板时自动去边框）
3. `SmartTerminalClipboard`：剪贴板代理拦截模式
4. 锚点移动按钮（d-pad 选择导航）

## deep-v1 增量（2026-08-07 全文件精读轮：feature/terminal 生产文件 8338 行）

### 本次精读文件

- MouseModeTracker.kt（153 行）、OscHandler.kt（411 行）、SelectionToolbar.kt（606 行）、FloatingTextInputDialog.kt（759 行）、SaveConnectionFromSession.kt（231 行）、SshTerminalEmulatorOwner.kt（262 行）、TerminalNotifications.kt（62 行）、TerminalScreen.kt（2140 行）、TerminalViewModel.kt（3311 行）、agent/TerminalSessionRegistry.kt（288 行）、attach/AttachOptionsSheet.kt（115 行）

### MouseModeTracker 对照（torvox 已有 174 行同款）

| 特性 | hav en | torvox | 差异 |
|------|--------|--------|------|
| mouse 模式 1000/1002/1003 | ✅ activeMouseMode StateFlow（最高模式） | ✅（但 torvox 只跟踪布尔） | **P2：torvox 未区分 1002/1003**（button-event/any-event 触摸行为不同） |
| 括号粘贴 2004 | ✅ bracketPasteMode | ✅ bracketPasteMode | 等价 |
| **备用屏幕 1049/1047/47** | ✅ altScreen——**消费方：alt 屏时滑动转发滚轮给远程应用，普通屏滚动本地 scrollback（#175）** | ✅ 跟踪但**零消费方** | **P2：torvox 备用屏幕时滑动仍滚动本地 scrollback——vim/less 内滑动行为错误** |
| **DECCKM（application cursor keys）** | ✅ cursorKeyAppMode——**alt 屏滑动转方向键时 SS3（ESC O A）vs CSI（ESC [ A）（#255）** | ❌ 未跟踪 | **P2：torvox 若实现 alt 屏滑动转方向键需 SS3 编码** |
| 状态机 | 5 态（GROUND/ESC/BRACKET/QUESTION/DIGITS） | 同款 | 等价（torvox 系移植自 hav en） |
| 多模式 `;` 分隔 | ✅ pendingModes 列表 | 需核对 | - |

### OscHandler.kt（411 行）——新增精读

hav en 的 OSC 处理（OSC 0/1/2 标题、OSC 7 cwd、OSC 8 超链接、OSC 9 通知、OSC 52 剪贴板、OSC 133 提示）。**torvox OSC 由 Rust 侧 ghostty_terminal 处理（更底层）**——hav en 是 Kotlin 侧解析（SSH 场景无法用 ghostty）。torvox 无借鉴（ghostty 已覆盖）。

### SelectionToolbar.kt（606 行）——已有 deep-v0 覆盖（expandSelectionToWord/expandAcrossUrlWrap/smartCopy）

本轮确认：**smartCopy 与 expandAcrossUrlWrap 仍是 torvox 缺失功能**（P1 候选，已记录）。

### FloatingTextInputDialog.kt（759 行）——新增精读

hav en 的浮动文本输入对话框（SSH 终端需要显式输入框）。torvox 用系统 IME——**不适用**。但其 **FloatingInputTextContextMenuProvider**（自定义上下文菜单 provider）有测试——torvox 选择菜单可参考其测试结构（P3）。

### TerminalSessionRegistry.kt（288 行）——新增精读

hav en 的会话注册表（agent 会话）。与 torvox SESSION_REGISTRY 同构（HashMap + id）。确认 torvox 模式标准。

### TerminalScreen/TerminalViewModel（2140+3311 行）——结构确认

hav en 终端屏幕：SSH 通道 + 本地渲染（tmux 终端绘制）。**无 GPU 渲染**（Canvas 绘制）。torvox wgpu 渲染领先。

### 新发现汇总

| # | 发现 | 级别 | 落点 |
|---|------|------|------|
| 1 | **altScreen 消费逻辑缺失**：torvox MouseModeTracker 跟踪 altScreen 但无人使用——备用屏幕（vim/less/htop）内滑动应转发滚轮，非滚动本地 scrollback | **P2** | hav en MouseModeTracker.kt:22-28, torvox 同文件 |
| 2 | **DECCKM 未跟踪**：torvox 无 cursorKeyAppMode——alt 屏滑动转方向键路径若实现需 SS3/CSI 区分 | **P2** | hav en MouseModeTracker.kt:30-34 |
| 3 | **mouse 模式未区分 1002/1003**：torvox 只布尔——button-event/any-event 触摸语义不同 | P3 | hav en MouseModeTracker.kt:12-16 |

## deep-v1 增量 B（2026-08-07：haven MCP Agent 系统——33 文件 18101 行）

### 精读文件

McpServer.kt（1725 行）、McpTools.kt（8012 行结构）、GuestMcpClient.kt（138 行）、TerminalInputQueue.kt（238 行）、SchemaDsl.kt（78 行）、ToolProvider.kt（19 行）、ToolContext.kt（33 行）、HavenUiBridge.kt（554 行结构）

### McpServer 架构（与 torvox tower-mcp 对照）

| 特性 | haven | torvox | 差异/价值 |
|------|-------|--------|----------|
| 传输 | **2025-06-18 Streamable HTTP**（单 POST /mcp + keep-alive + Mcp-Session-Id） | tower-mcp Unix socket（+ Stdio/HTTP） | 传输不同，协议同代 |
| **信任分级** | **McpOrigin：DEVICE/TUNNELED/LAN/WIREGUARD**——bind 时由 listener 标记，绝不从 peer 地址推断（reverse tunnel -R 也到 127.0.0.1，loopback ≠ 设备用户）；仅 DEVICE + 显式 opt-in 可跳过配对 | Unix socket 文件权限（本地） | **P2 参考**：torvox 若未来支持 TCP/隧道，需 origin 分级；当前 socket 路径方案足够 |
| 认证 | **配对 token（Bearer）+ session-id 双通道**；`lastClientHint` 仅用于审计/consent 记忆，**绝不作为认证**（旧实现被废弃 #mcp-backbone Stage 3） | socket 文件 + 进程权限 | P3（torvox 无远程场景） |
| 会话失效 | 不认识的 session-id → **HTTP 404**（spec 定义的重初始化信号），非 JSON-RPC -32001（Claude Code 会卡死） | tower-mcp 内建 | P3 |
| consent | 55s 等待（8s 不可用）+ 90% 超时余量 + "Allow for N min" + 每工具 consent 级别 + 会话级 bypass | torvox dialog/pick_file 回调 | P3 |
| 并发 | MAX_CONCURRENT_CONNECTIONS=64（keep-alive 后线程复用需上限，超限 503） | tower-mcp 管理 | P3 |
| 审计 | AgentAuditRecorder（每次调用审计行 + DENIED 记录） | 无 | **P2 候选**：torvox MCP 无审计 |
| 工具拆分 | **ToolProvider 接口**：域提供器拆分 11.7k 行 God file（JVM 64KiB 方法上限） | mcp.rs 单文件 723 行 | torvox 规模不需要 |
| Schema | **objectSchema DSL**（SchemaBuilder：string/integer/boolean/number/stringArray/objectArray + required 跟踪） | **schemars 自动生成**（更优，零样板） | torvox 已领先 |
| UI 资源 | `ui://haven/screen` resources/read（agent 读当前屏幕） | 无 | P3 |

### McpTools 工具集（对照 torvox 9 工具）

- get_app_info / list_paired_clients / unpair_mcp_client（客户端配对管理）
- list_connections / read_exited_session / connect_profile（SSH 连接）
- capture_haven_ui（截屏自渲染 UI）
- **queue_terminal_input（#161）**：**轮询式 prompt 匹配输入注入**——200ms 轮询 scrollback 尾部 + 正则匹配 + **baselineTotalBytes 防已显示 prompt 误触发** + submitKey（\r/\n/空）+ 投递前 consent（toolName 区分 `queue_terminal_input:deliver` 防 memo 碰撞）
- 大量 SSH/邮件/USB/RDP/VNC/密钥工具（torvox 无对应功能）

### TerminalInputQueue（238 行完整）——torvox 最大缺口

torvox MCP 工具无**终端输入注入**（只有 send_signal）。haven 的 queue_terminal_input 模式：

- enqueue(sessionId, text, promptPattern, timeout, submitKey) → UUID
- 轮询：deadline 过期删除、baseline 字节计数防旧 prompt、`TAIL_MATCH_CHARS=512` 只看尾部、ANSI strip 后匹配
- 投递前 consent（用户可能正在输入）
- **对 torvox 价值：P2 候选**——torvox 终端内的 agent（通过 socket 的 CLI）可等待 prompt 后注入输入（如 apt 安装确认）。torvox 已有 scrollback 查询能力（search_all_in_scrollback），实现该工具需：scrollback 尾部读取 + 正则 + 写入队列

### GuestMcpClient（138 行完整）——MCP 客户端参考

haven 聚合 guest MCP 服务器（proot 内 KiCad 等）：阻塞客户端 + session-id 404 重初始化 + **SSE 只读首帧**（FastMCP 保持 SSE 流打开也不挂起）+ 超时。torvox 无 guest 聚合需求（P3）。

### HavenUiBridge（554 行结构）

capture_haven_ui 工具后端：UI 树转 JSON（bounds/text/clickable）——agent 看屏。torvox 无（P3，且 torvox 有真实渲染可截屏）。

## deep-v1 增量 C（2026-08-07：core/terminal-haven + ScrollbackRing）

### HavenTerminal.kt（109 行）+ HavenKeyboardMode.kt（86 行）

- HavenTerminal 是 termlib Terminal composable 的包装（把 HavenKeyboardMode 映射到 termlib 布尔对）
- **HavenKeyboardMode 4 模式**：Secure（TYPE_NULL 类 + NO_SUGGESTIONS + NO_PERSONALIZED_LEARNING，Gboard mic/建议条/书写辅助全关，CJK 组合仍可用）、Standard（完整 IME）、Raw（无 InputConnection，最硬锁定）、Custom（ImeFlagSet 每 bit 可调：noSuggestions/visiblePassword/autoCorrect/fullEditor/noExtractUi/noPersonalizedLearning）
- **torvox 对照**：**torvox input/KeyboardMode.kt 已实现同款 Secure 配置**（VISIBLE_PASSWORD | NO_SUGGESTIONS | NO_PERSONALIZED_LEARNING，Bridge.kt:699 注释）——IME 配置等价。torvox 用户曾要求删除键盘模式选择器 UI，但保留 enum——**确认这是正确折中**（IME 配置逻辑保留，选择 UI 删除）
- Custom 模式（每 flag 可调）torvox 无——用户已删选择器，不适用

### ScrollbackRing.kt（92 行完整）

- 固定容量字节环（capacity 构造）+ 双 arraycopy 无锁逐字节 + snapshot() 返回副本
- **`totalBytesAppended` 单调 Long 计数（@Volatile，永不重置）**——agent 队列检测"自 enqueue 以来有新字节"（size 饱和后不可用，total 计数可用）
- **torvox 对照**：torvox scrollback 是 ghostty CellData 网格（渲染用），hav en 环是 SSH 字节镜像（agent 用）——用途不同。**但 totalBytesAppended 模式对 torvox 未来 queue_terminal_input 类工具是关键**（scrollback 行数可作基线，需注意饱和）——P2 记录

### 新增汇总

| # | 发现 | 级别 |
|---|------|------|
| 1 | torvox IME 配置与 haven Secure 等价（已实现） | 确认 |
| 2 | totalBytesAppended 单调计数模式（queue_terminal_input 基线检测） | P2 |
| 3 | HavenUiBridge（UI 树→JSON 供 agent 看屏）torvox 无 | P3 |
