# 深度研究补充：warp-mobile-android（ImL1s）—— JNI 导出全集 / warp_ai_mobile / Kotlin 宿主 / 测试基建

> 研究日期：2026-08-06 | 克隆位置：`repositories/refs/warp`（depth 1）
> 本文是 `research-warp.md`（已覆盖 `android-host` 的 pty.rs / bootstrap.rs / vulkan.rs / font_render.rs / ime.rs）的**补充篇**，覆盖其余全部源码：
> `crates/android-host/src/lib.rs`（JNI 导出全集）、`input.rs`、`terminal_model.rs`、`font_picker.rs`、`crates/warp_ai_mobile/`（全部 8 个文件 + 集成测试）、
> `android/` Kotlin 宿主（NativeBridge / WarpInputView / WarpTerminalService / MainActivity / 搜索引擎 / AI 工具链）、`spikes/`、`tools/scripts/`、两份 Cargo.toml、项目文档（CLAUDE.md / PROJECT.md / TEST_INFRA.md / TEST_READY.md）。
> 对比对象：torvox（Kotlin + Compose + Rust wgpu 渲染，`android/app/src/main/java/terminal/emulator/` + `native/src/`）。

---

## 1. 仓库全景（两个 crate 的工作区）

```
warp/                                  # workspace（resolver 2，成员仅 2 个 crate）
├── crates/android-host/               # JNI cdylib（libwarp_mobile_android_host.so）— 终端运行时宿主
│   ├── lib.rs (2394)                  # 55 个 JNI 导出 + 测试（AI 流式 / PTY / render / IME / input / terminal / session）
│   ├── pty.rs (413)                   # fork/exec PTY（AS-safe）— 已由 research-warp.md 覆盖
│   ├── bootstrap.rs (788)             # bootstrap zip 原子安装（sha256 sidecar）— 已覆盖
│   ├── vulkan.rs (2263)               # ash 0.38 直接 Vulkan（static/dynamic grid 渲染）— 已覆盖
│   ├── font_render.rs (701)           # cosmic-text 字体发现/栅格化（ASystemFontIterator）— 已覆盖
│   ├── ime.rs (159)                   # IME JNI 单例包装（状态机本体在 link crate）— 已覆盖
│   ├── input.rs (478)                 # 触摸输入状态机（host 可测）— 本文 §3.3
│   ├── terminal_model.rs (302)        # 终端模型全局访问器（host 可测）— 本文 §3.4
│   └── font_picker.rs (145)           # 字体族选择（OEM 命名变体，host 可测）— 本文 §3.5
├── crates/warp_ai_mobile/             # 独立 rlib：Anthropic Claude 异步流式客户端（BYOK ghost-text + agent）— 本文 §4
│   ├── lib.rs / client.rs / session.rs / session_registry.rs / provider.rs / profile.rs / approval.rs / audit.rs
│   └── tests/task2_stress_tests.rs
├── android/app/                       # Kotlin Compose 宿主（77 个 Kotlin 文件、10 个模块）— 本文 §5
└── spikes/  tools/  scripts→tools/scripts/  — 本文 §6
```

**关键前置事实**：`android-host/Cargo.toml:49-50` 通过 path 依赖引用 `../../warp-src/crates/warp_mobile_android_link`（static_grid/dynamic_grid/ime 状态机规范实现）与 `warp_terminal_mobile_facade`（终端模型/解析器规范实现）。`warp-src` 是**独立 git 仓库**（Warp 上游 fork，被 .gitignore 排除，CLAUDE.md:76 明确说明），**本 depth-1 克隆中不存在**——即该克隆单独 `cargo build` 不可行，只能当纯参考。这也是 warp 项目"单仓库 + 跨仓库 path 依赖"的耦合弱点，torvox 单仓库设计（libghostty-vt 经 `generated-patches/` 本地 patch）没有此问题。

---

## 2. crates/android-host/src/lib.rs —— JNI 导出全集（55 个，11 组）

`lib.rs` 结构：前 73 行为模块声明 + 非 unix 平台 stub；`ping:85`；AI ghost-text 同步版 `:114`；**AI 流式四件套** `:261-427`；**多轮 agent 八件套** `:455-736`；PTY 九件套 `:743-966`；render 十二件套 `:968-1342`；IME 五件套 `:1343-1443`；input 八件套 `:1444-1563`；render insets `:1594`；terminal 九件套 `:1643-1999`；session 六件套 `:2001-2153`；测试 `:2155-2394`。

### 2.1 AI ghost-text（同步 + 流式）—— 本文核心之一

| 导出 | 行号 | 功能 |
|---|---|---|
| `aiGhostComplete(apiKey, model, prompt, maxTokens): String` | :114 | **同步**请求：`Runtime::new().block_on(messages_complete)`，成功返回文本、失败返回 `"ERR:<msg>"`（Kotlin 检查前缀区分）。每调用新建 tokio runtime（~10ms 开销，相对 500ms-3s 网络往返可忽略） |
| `aiGhostStreamStart(...): jlong` | :261 | **流式**：`OnceLock<Runtime>` 全局多线程 runtime（:222-235），spawn tokio 任务跑 `messages_stream`（SSE），chunk 推入 `StreamHandle.chunks: Mutex<VecDeque<String>>`（:237-259），返回 `Arc::into_raw(handle) as jlong` |
| `aiGhostStreamPoll(handle): String` | :332 | 非阻塞 poll，协议：`""`=仍在跑、`":CHUNK:<text>"`=新数据（一次排空队列）、`":DONE:"`=正常结束、`":ERR:<msg>"`=失败。Arc 借用用 `from_raw + clone + into_raw` 三连保持 refcount（:342-345） |
| `aiGhostStreamCancel(handle)` | :388 | `CancellationToken::cancel()`，幂等；注释明确"Free 前必须先 Cancel，否则泄漏在途请求"（:403-410） |
| `aiGhostStreamFree(handle)` | :413 | `Arc::from_raw` 消费引用；tokio 任务还持有时对象继续存活到任务结束 |

### 2.2 多轮 agent 会话（Issue #14）

| 导出 | 行号 | 功能 |
|---|---|---|
| `aiAgentSessionCreate(sessionId, model, systemPrompt): Boolean` | :455 | `global_registry().create_or_get_session`（OnceLock 全局注册表） |
| `aiAgentSendTurnStart(sessionId, apiKey, userPrompt, maxTokens): jlong` | :547 | 追加 user message → `spawn_agent_turn_stream`（:480-543）：读会话快照 → 状态置 Connecting → tokio spawn → Streaming → `messages_stream_multi_turn` 回调里**每 50ms 检查 paused 标志**（:509-511，`thread::sleep` 轮询式暂停）→ 事件以 `:EVENT:<json>` 前缀入队 → 完成后写回会话并置 Completed/Cancelled/Error |
| `aiAgentTurnPoll(handle): String` | :579 | 与 ghost poll 同协议，多 `":STATUS:{\"state\":\"PAUSED\"}"` |
| `aiAgentTurnControl(handle, action): Boolean` | :629 | action 1=CANCEL（cancel token + 状态 Cancelled）、2=PAUSE（`paused.store(true)`，**流式回调内 sleep 50ms 实现背压**）、3=RESUME |
| `aiAgentTurnFree(handle)` | :664 | Arc 消费 |
| `aiAgentRetryTurn(sessionId, apiKey, maxTokens)` | :679 | `retry_last_turn()`（弹掉最后一条 assistant 消息，user prompt 保留）后重启流 |
| `aiAgentEditPrompt(sessionId, apiKey, turnIndex, newPrompt, maxTokens)` | :706 | `edit_prompt(idx, new)` 就地替换某轮 user 消息后重启流 |

**架构要点**：Rust 侧持有全部会话状态（`AgentSessionRegistry`），Kotlin 侧只做 **50ms 轮询**——零 JNI 回调对象、零 GlobalRef、无跨线程 Java 调用。这是与 torvox 的 `pollEvent()` 完全同源的设计哲学（JNI 面只做同步拉取），但 warp 把它用于 AI 流式而 torvox 用于终端事件。

### 2.3 PTY 九件套（:743-966）

`ptySpawn(program, args, env): jlong` :743（返回 `Arc::into_raw` 指针，0=失败）；`activePtyCount(): jint` :802 / `totalSpawnedPtys(): jlong` :811（AtomicU32/U64 计数器）；`ptyGetExitStatus(ptr): jint` :820（-2=尚未退出）；`ptyAcquire(ptr): jboolean` :835 / `ptyRelease(ptr)` :848（**Arc 引用计数手工管理**：Kotlin 读线程在锁内 acquire、锁外 ptyRead、finally release，见 §5.5）；`ptyRead(ptr, buf): jint` :859（阻塞 libc::read，**不在 @Synchronized 内调用**）；`ptyWrite(ptr, data): jint` :901；`ptyResize(ptr, rows, cols)` :923（TIOCSWINSZ）；`ptyKill(ptr)` :942（SIGKILL 语义 + kill_eager）。

### 2.4 render 十二件套（:968-1342）

`renderAttachSurface(surface: JObject)` :968 —— `ANativeWindow_fromSurface`（引用计数转移给 vulkan::attach）；`renderDetachSurface` :1001；`renderClearFrame(r,g,b,a)` :1016（vkQueuePresentKHR 清色帧）；`renderFramesPresented(): jlong` :1032；`renderCaptureFrame(path, r,g,b,a)` :1052（**帧回读 → BGRA→RGBA swizzle → png crate 编码**，同步 vkQueueWaitIdle，日志 `capture_ok` 供驱动 grep）；`renderCaptureFrameWithText(path, text, font_size_px, baseline_x, baseline_y)` :1099（capture + cosmic-text 形状化 + swash 栅格化 + alpha 混合，日志 `font_render_ok` 计数）；static_grid 组：`renderInitStaticGrid(text,font_size_px,rows,cols,cell_w,cell_h)` :1167、`renderDrawGridFrame` :1209、`renderStaticGridAttached` :1225、`renderStaticGridStats` :1305；dynamic_grid 组（M3-S08 每格 SGR 渲染）：`renderDrawDynamicGridFrame` :1244、`renderDynamicGridAttached` :1260、`renderDynamicGridStats` :1273。static/dynamic 实现均在 `warp_mobile_android_link`（cfg-gated），lib.rs 只 re-export（:47-48）。

### 2.5 IME 五件套（:1343-1443）

`imeCommitText(text, newCursorPosition)` :1343、`imeSetComposingText` :1366、`imeFinishComposingText` :1390（各自转发到 `ime::*` → `global_ime(): Mutex<AndroidIme>`，状态机在 link crate）、`imeStats(): String` :1407（CSV 诊断串，驱动 grep 用）、`imeReset()` :1421。

### 2.6 input 八件套（:1444-1563）

`inputTouchDown/Up/Cancel(x,y)` :1444/:1458/:1477（**TouchCancel 显式闭合 DOWN 序列**，防父 View 拦截手势后 Rust 侧以为手指仍在）、`inputTap` :1491、`inputLongPress` :1505、`inputScroll(x,y,dx,dy,vx,vy)` :1523（VelocityTracker 的瞬时速度一并传入）、`inputStats(): String` :1543、`inputReset()` :1557。

### 2.7 terminal 九件套（:1643-1999）+ insets

`setRenderInsets(top,left,right,bottom)` :1594 —— 4 个 AtomicI32（物理像素，bottom = max(ime.bottom, sysBars.bottom)），由 `ViewCompat.setOnApplyWindowInsetsListener` 驱动（:1565-1570）。

`terminalInputBytes(cmdId, bytes): jint` :1643 —— **PTY 输出 → 终端模型**：`ingest_pty_bytes_for_session`（按 cmdId 路由，M3 后多会话）；`terminalTakeDirtyAndPushFrame(font_size_px,rows,cols,cell_w,cell_h): jint` :1691 —— **Choreographer 每帧驱动**：dirty bit 检查 → `snapshot_cells()` → `terminal_model::Cell → dynamic_grid::Cell` 翻译（:1730-1742）→ **ACCESSORY_PAD=7 底对齐投影**（:1772-1798，光标始终落在 `total_rows - 7 - 1` 行，内容少时顶部补空行、内容满时丢弃顶部 scrollback 行，为 AccessoryRow 键盘工具栏留白）→ `init_dynamic_grid` + `submit_dynamic_grid_frame`（黑色清屏）。返回 1=已推帧、0=无 dirty、-1=失败。`terminalModelStats` :1832、`terminalSgrSummary` :1860、`terminalBlocksDump(): String` :1884（Block 模型 JSON dump，含 `"output"` 字段）、`terminalResize` :1899、`terminalSetScrollOffset` :1945、`terminalScrollbackInfo` :1965、`terminalIsAltScreen` :1983（**DECSET 1049 状态**，驱动 Kotlin 切换 Block 时间线 ↔ TUI raw grid）。

### 2.8 多会话六件套（:2001-2153）

`createSession(sessionId, envJson): jlong` :2001、`switchSession(sessionId)` :2042、`closeSession(sessionId)` :2068、`activeSessionId(): String` :2094、`saveSessionState(): String` :2109、`restoreSessionState(json)` :2129（会话元数据 + scrollback 持久化）。

### 2.9 lib.rs 内置测试（:2155-2394）

- `m3_s11_emoji_smoke_tests`（:2192-2307）—— 任务点名。**host 可编译的 `classify_char` 镜像**（:2206，CJK 区间 :2208-2221、emoji 区间 :2225-2228，注释强制与 `font_render.rs:105-126` 保持一致），3 个测试：
  - `emoji_codepoints_classify_as_emoji` :2242 —— 8 个码点跨 4 个 Unicode 块（U+1F300/U+1F900/U+1FA00/U+2600-27BF）断言 Emoji；回归 = 设备 tofu
  - `emoji_range_boundaries_are_tight` :2271 —— **边界负例**：U+1F2FF、U+1F700、U+25FF、U+27C0 必须 Latin；「世界」必须 CJK（防 emoji 区间过度扩张把正常字形路由进 Noto Color Emoji）
  - `mixed_string_produces_three_run_kinds` :2296 —— "Hello, 世界 🎉" 三段分类
- `agent_stream_stress_tests`（:2312-2393）—— `AgentStreamHandle` 暂停/恢复缓冲对抗 + 会话注册表状态机遍历。

**测试方法论文档价值**：emoji 测试用"**镜像实现 + 注释钉死与 Android-only 规范实现的对应关系**"，让 host 测试无需交叉编译即可防回归——这是 torvox 可以把字体分类逻辑做成 host 可测的范本。

---

## 3. android-host 其余模块（本次新增覆盖）

### 3.1 input.rs（478 行）—— 触摸输入状态机

`InputEvent` 枚举 :44（TouchDown/TouchUp/TouchCancel/Tap/LongPress/Scroll，含坐标与速度）；`InputStats` :141；`AndroidInput` :167（VecDeque 事件缓冲 + 计数器）；全局单例 `static INPUT: OnceLock<Mutex<AndroidInput>>`（:300-304）+ 自由函数 `touch_down/touch_up/touch_cancel/tap/long_press/scroll`（:306-350）、`stats_string` :352、`reset` :378。9 个 host 单元测试 :391-478。**注意**：这是"记录型"状态机（事件缓冲供驱动 grep），不是手势识别器——识别在 Kotlin `GestureDetector` 完成，Rust 只收语义化事件。torvox 的触摸处理在 Kotlin 侧（TerminalInputEncoder），架构等效。

### 3.2 terminal_model.rs（302 行）—— 终端模型全局访问器

进程级 `TerminalModel`（真实实现是 facade 镜像）的薄封装：`active_model()` :26 / `global_model()` :47、`ingest_pty_bytes` :51、**`ingest_pty_bytes_for_session(cmd_id, bytes)` :55**（多会话路由）、`take_dirty` :66 / `peek_dirty` :70、`snapshot_text` :74、`snapshot_cells` :78、`cursor_position` :82、`dims` :86、`resize` :90、`blocks_dump_json` :94、`set_scroll_offset` :98、`scroll_offset` :102、`scrollback_len` :106、`scrollback_max_lines` :110、`is_alt_screen` :114。测试 :143-302 含 UTF-8 三字节汉字、Block output 捕获（Preexec/CommandFinished 之间 stdout 入 `Block.output`，上限 64KB）、多会话路由、关闭会话字节丢弃、DECSET 1049 切换。**torvox 等价物**：libghostty-vt 的 Session + Kotlin `TerminalViewModel` 的 scrollback 快照。

### 3.3 font_picker.rs（145 行）—— 纯函数字体族选择

`pick_emoji_family(names: &[&str]) -> Option<String>` :31 —— 按优先级挑 emoji 字体族（Samsung Color Emoji > Noto Color Emoji），处理 OEM 命名变体（"SamsungColorEmoji2" 之类），大小写不敏感。9 个 host 测试 :56-145（三星优先、Pixel 回退 Noto、变体名、非 emoji 三星字体跳过）。**这是 host 可测纯逻辑的正确切法**（font_render.rs 保持 android-gated，选择逻辑独立出来）。

---

## 4. crates/warp_ai_mobile/ —— Anthropic 客户端库（8 文件 + 集成测试）

独立 rlib（`crate-type=["rlib"]`），与 android-host 同 workspace，零 NDK 依赖，**纯 host 可测**（45 个 Rust 测试）。定位：M6 的 AI 层，替代早期 Java `HttpsURLConnection` 客户端（AnthropicClient.kt）。

### 4.1 client.rs（884 行）—— 流式 SSE 客户端

- 常量：`API_ENDPOINT` :41（https://api.anthropic.com/v1/messages）、`ANTHROPIC_VERSION` :46（2023-06-01）、`DEFAULT_GHOST_MODEL="claude-haiku-4-5"` :50、`DEFAULT_AGENT_MODEL="claude-sonnet-4-6"` :54
- `AnthropicClient` :61 —— `connect_timeout=8s` / `request_timeout=30s`（:79-80），`new()` 只校验 `sk-ant-` 前缀；`redact()` :87 输出 `Bearer sk-ant-***...xxxx` 日志格式（与 Kotlin `AiKeyStore.redact()` 对齐）
- `messages_stream` / `messages_stream_multi_turn` —— POST stream=true，**所有网络点都包 `tokio::select! { _ = cancel.cancelled() => ... }`**（:397-399、:413-416），SSE 按 `\n\n` 切块
- `SseChunkEvent` :440 —— serde tagged enum：TextDelta / ThinkingDelta / ToolUseStart / InputJsonDelta
- `append_utf8_bytes_safely` :448 —— **跨 chunk 边界的 UTF-8 累积器**：`from_utf8` 失败时 `valid_up_to()` 推进、`error_len()==None`（不完整尾部）保留尾字节、否则 lossy（对 Agent/LLM 字节流正确性至关重要）
- `extract_sse_events` :475 —— 手写 SSE 解析（data: 前缀、content_block_delta/start、tool_use、thinking_delta、input_json_delta），**不依赖 eventsource crate**
- `MessagesError`：Cancelled / Network / HttpStatus（含 `scrub_key` 从错误体剥 API key，:405）

### 4.2 session.rs（297 行）—— 多轮会话模型

`MessageRole` :8（serde lowercase）、`Message` :24（role/content/timestamp）、`TurnState` :57（Idle→Connecting→Streaming→Paused→Completed/Cancelled/Error，serde SCREAMING_SNAKE_CASE）、`AgentSession` :83、`retry_last_turn` :120、`edit_prompt(idx, new)`、`to_anthropic_messages(): Vec<Value>`（转 Anthropic 请求数组）。测试含编辑后 JSON 形状断言 :288-296。

### 4.3 session_registry.rs（80 行）

`global_registry()` :11（`OnceLock<AgentSessionRegistry>`）、`AgentSessionRegistry` :17（`Arc<Mutex<HashMap<String, AgentSession>>>`，锁中毒时 `into_inner()` 恢复 :30）。**torvox 的 NativeQueryPort/Session 注册表与之同构**（见 §8 对比）。

### 4.4 provider.rs（304 行）—— 多提供商抽象

`ProviderDispatcher` :11，`stream_multi_turn` :46 按 `ProviderKind` 分派 Anthropic / OpenAI（`/v1/chat/completions`，choices[].delta.content，`data: [DONE]` 结束）；`endpoint_for_profile` :30 支持自定义 URL（企业代理）。`parse_openai_sse_line` 供 host 测试（:294-303）。

### 4.5 profile.rs（296 行）—— 模型 profile

`ProviderKind` :8（Anthropic/OpenAi/CustomOpenAi）、`ModelProfile` :26（id/name/provider/model_name/endpoint/temperature/max_tokens/top_p/context_window/supports_tools/supports_streaming/is_builtin）、`validate()` :42（temp∈[0,1]、max_tokens>0、CustomOpenAi 必须有 URL）、`builtin_profiles()` :76（Claude 3.5 Sonnet/Haiku、GPT-4o 等 5 个）、`ModelProfileRegistry`（register/set_active/active_profile）。

### 4.6 approval.rs（144 行）—— 高危命令评估

`CommandRiskEvaluator::evaluate` :65 —— **33 条危险子串黑名单** :27-62（rm -rf、dd if=、mkfs、sudo、curl|sh、git push -f、`:(){ :|:& };:` 等）+ 组合规则（curl/wget 管道 shell）。二元 Low/High。测试 :93-143。

### 4.7 audit.rs（112 行）—— CSV 审计

`AuditEntry` :26（timestamp/model/input_tokens/output_tokens/latency_ms/command_string/approval_state）、`to_csv_row` :39、`escape_rfc4180` :46（引号翻倍 + 整体加引号）、`ApprovalState` :8（Approved/Rejected/AutoAllowed）。对应 Kotlin `AiUsageTracker.kt` 写 `warp-ai-usage.csv`。

### 4.8 tests/task2_stress_tests.rs（~300 行）

Challenger 1 对抗测试：OpenAI SSE 解析 10 边界（无空格 `data:`、role-only delta、空 content、`[DONE]`、坏 JSON、空 choices）、profile 校验边界（max_tokens=0、temp=-0.01/1.01、top_p 越界、CustomOpenAi 缺 URL）、审计 CSV 转义（逗号/引号/换行）。

---

## 5. android/ —— Kotlin 宿主（77 文件，10 模块）

### 5.1 NativeBridge.kt（574 行）—— JNI 声明全集

`object NativeBridge`，`init` 块 `System.loadLibrary("warp_mobile_android_host")` 包 try-catch（**host JVM 测试环境无 .so 时静默跳过**，:7-13）——torvox 的 Bridge/NativeBridge 同样处理。55 个 `external fun` 与 §2 一一对应。注释是**协议文档**：ghost stream 生命周期 4 步（start→poll 循环→cancel→finally free，:41-55）、bootstrap 返回码表（0-6，:112-119）、"Free 忘调用 = 每个孤儿 handle ~200 字节泄漏"（:54）。其余声明：`bootstrapInstall(assetManager, dataDir): Int` :119、`ptyRead(ptr, buf): Int`、`terminalInputBytes(cmdId, bytes): Int`、`terminalTakeDirtyAndPushFrame(...): Int`、render 组、`setRenderInsets`、`terminalSetScrollOffset(offsetRows): Int` :578、`terminalScrollbackInfo(): String` :584。

### 5.2 WarpInputView.kt（>800 行）—— 自绘输入 View + 自定义 InputConnection

- `class WarpInputView` :88 —— 覆盖 `onTouchEvent` :458（GestureDetector 语义化 → `inputTouch*` JNI）、`onCheckIsTextEditor()=true` :511、`onCreateInputConnection` :520（inputType=TYPE_CLASS_TEXT|MULTI_LINE，imeOptions=IME_ACTION_NONE|NO_EXTRACT_UI|NO_FULLSCREEN|**NO_PERSONALIZED_LEARNING**）、`getAccessibilityNodeProvider` :375（WarpAccessibilityNodeProvider 供 TalkBack）
- `WarpInputConnection : BaseInputConnection(view, fullEditor=false)` :544 —— **composing 状态全部在 Rust 状态机**（:534-543 注释）；`lineContextBuffer` :551 维护当前行缓冲（getTextBeforeCursor 返回真实行内容而非空，Pinyin 候选需要）；`updateLineContext` :568（换行清空、512 字符上限）
- **`forwardComposingDiff(prev, next)` :587 —— 核心算法**：最长公共前缀 → 公共前缀之后旧文本按 **Unicode code point 数**发 DEL 0x7F（防代理对/emoji 过度删除，:596-605）→ 新后缀按 UTF-8 写入 PTY（:607-614）。`commitText` :617 与 `setComposingText` 共用此 diff，保证 PTY 侧状态与用户所见一致
- 附带 `GhostSuggestController.onTextCommitted` 钩子 :635（AI ghost-text 联动）
- 驱动测试缝：`setCellHeightPx` :132、`resetScroll` :144、`cancelFling` :155

### 5.3 WarpTerminalService.kt（~1100 行）—— 前台服务 + PTY 生命周期

- `onCreate` :70（`ensureForeground` :106 通知）、`extractWarpAssets` :254、`writeWarpZshenv` :326 / `writeAptConfig` :534（**两处调用：onCreate + 每次 spawn**——bootstrap 原子 rename 会清掉先前写入，spawn 时重试幂等，:743-753 注释）
- `installTermuxBinSymlinks` :169（nativeLibraryDir 的 libzsh.so → W^X 执行方案，见 research-warp.md §W^X）
- `handleSpawn` :735 —— 默认 shell 选择链 :770-774：`nativeLibraryDir/libzsh.so`（APK 打包的可执行 .so，SELinux `apk_data_file` 有 execute 权限）→ `$PREFIX/bin/zsh` → `/system/bin/sh`；**spawn 前 `writeWarpZshenv()`+`writeAptConfig()`+`installTermuxBinSymlinks()` 三连重试**（:752-757）；**初始 winsize 先于 read loop 应用** :797-808（否则 zsh 继承 80×24，与 dynamic_grid 计算的行列不符，折行错位——torvox 同款坑，见 §8）
- `handleWrite` :813 —— 三种解码优先级：byte-array extra → `data_b64`（**base64 规避 `am broadcast` 把含 `-l`/`-a` 形状的 token 当 flag 的解析器**，:817-829）→ 字符串（补换行）
- `startReadLoop` :859 —— 协程读循环；**fast-death 恢复** :906-915：进程存活 <1.5s 即退出则重试（MAX_FAST_DEATH_RETRIES=3、500ms 指数退避封顶 5s、回退 /system/bin/sh），重试前清网格防失败 shell 的 stderr 污染 UI
- 消息传递：**broadcast Intent + cmd_id 路由**（`PtyBroadcastReceiver` 541B），与 torvox 的 `TerminalRuntime` 直接持有 Session 的架构不同

### 5.4 MainActivity.kt（~1000 行）—— 渲染循环宿主

- `frameCallback : Choreographer.FrameCallback` :110 —— **每 vsync 驱动**（Choreographer 一次性回调，:156 重新 post）：terminalMode → `terminalTakeDirtyAndPushFrame`（dirty=1 推帧 / 0 时 fallback `renderDrawDynamicGridFrame` 黑清屏保持 swapchain 健康，:141）→ gridMode → static grid → 默认清色帧；失败即跳过等下个 vsync（OUT_OF_DATE 自愈）
- `startStaticGrid` :172 —— `--ez grid_mode true` 启动参数驱动（幂等，surface 未就绪时置标志等 surfaceCreated 重试 :187-189）
- `surfaceCreated/Changed/Destroyed` :826/:837/:904、`attachAndStartRender` :920（`renderAttachSurface` + 初始尺寸）、`dispatchKeyEvent` :987（硬件键盘）、`onDestroy` :812（renderDetachSurface）

### 5.5 PtyManager.kt（91 行）—— 指针生命周期管理

**关键模式**（:35-38 注释）：`readDirect` 不 @Synchronized（阻塞 read 不能持类监视器）——锁内 `ptyAcquire` 增加 Arc 引用 → 释放锁 → 锁外 `ptyRead` → finally `ptyRelease` 减引用，保证 kill 与 read 并发安全。`spawn` :10 先 kill 同 cmdId 旧会话（防孤儿）。

### 5.6 搜索系统（search/ 模块，5 文件 + 8 个测试文件）

- `UnifiedSearchEngine.kt` :14 —— **跨 5 域搜索**：`search` :16 用 `coroutineScope + async` 并发跑 5 个域（sessions/blocks/history/AI conversations/files），内存域走 `Dispatchers.Default`、文件域走 `Dispatchers.IO`，按 score↓ 然后 timestamp↓ 排序（:77-80）；`searchWithCounts` :84 附带每域计数（overlay 徽章）；`searchFiles` 支持 maxDepth=5、maxFileMatches=100、忽略目录
- `BlockSearchEngine.kt` :24 —— 块内搜索：**`stripAnsiCodes` :26 复用 `parseAnsiToAnnotatedString(text).text` 剥离 ANSI**（同 UI 解析器，防搜索命中转义序列本身）；纯文本 indexOf 循环 + 正则模式（Pattern.CASE_INSENSITIVE，编译失败安全返回空）；`highlightSearchMatches` :109 返回 AnnotatedString（黄色高亮 + 橙色当前匹配，Compose SpanStyle）
- `UnifiedSearchModels.kt` —— `SearchDomain` :5（ALL/SESSIONS/BLOCKS/HISTORY/AI/FILES 各带徽章色）、sealed `UnifiedSearchResultItem` :18（5 个 data class）、`UnifiedSearchState` :108
- 测试（8 个文件）：`UnifiedSearchEngineTest` 9 个用例（空查询、各域、ANSI 剥离、忽略目录、域过滤计数）；`BlockSearchEngineAdversarialTest`（50,000 块基准——PROJECT.md:77 记录 **75x 提速**，索引命令+输出剥离 ANSI 后缓存）；`UnifiedSearchAdversarialTest` / `UnifiedSearchEmpiricalStressTest` / `UIStateSafetyChallengerTest` 等

### 5.7 AI 工具链（Kotlin 侧）

- `AiKeyStore.kt` :23 —— `EncryptedSharedPreferences`（MasterKey AES256_GCM、AES256_SIV 键加密），硬件 KeyStore 失败回退普通 prefs（:54-59）；多提供商键 `key_anthropic`/`key_openai`/`key_custom_<id>`；`redact()` 与 Rust 侧格式一致
- `GhostSuggestController.kt` :59 —— 幽灵文本：缓冲区维护 + 300ms debounce（scheduleSuggestion :313）+ 流取消（cancelActiveStream :432，按任意键即 cancel）+ Tab 接受（acceptCurrent :235 返回字节写入 PTY）
- `ai/CommandApprovalDialog.kt` + `CommandApprovalManager.kt` + `CommandRiskEvaluator.kt`（Kotlin 版）—— 高危命令弹窗审批，审批通过才把字节写入 PTY（PROJECT.md:91 接口契约）
- `editor/`：`GhostCompletionEngine.kt`、`SlashCommandRegistry.kt`、`CommandHistoryManager.kt`（9KB，历史 + 对抗测试）
- `clipboard/ChunkedPasteEngine.kt`（分块粘贴防 PTY 背压丢字节）、`HardwareKeyDecoder.kt`、`WarpAccessibilityNodeProvider.kt`

### 5.8 测试基建（642 个 Kotlin JVM 测试）

`app/src/test/java/dev/warp/mobile/`：按功能域组织（ai/、editor/、search/、mcp/、panes/、clipboard/、security/、skills/、ssh/、test/）；`test/Tier1UnitTest.kt`(44KB)/`Tier2UnitTest.kt`(65KB)/`Tier3UnitTest.kt`/`Tier4UnitTest.kt` 是四层金字塔的落地载体；工具类 `WarpTestFixtures.kt`/`WarpAssertHelpers.kt`/`BaseWarpUnitTest.kt`。MockK + JUnit4 + kotlinx-coroutines-test + Turbine + Robolectric（build.gradle:1051-1056）。

---

## 6. spikes/、tools/、scripts/

### 6.1 spikes/

- `vulkan-surface-recreate/` —— M0 spike：swapchain 重建 100 次压测（Adreno 750 p95=18ms、Adreno 660 p95=28ms），产物：生产 `vulkan.rs` 的 per-image present-wait 信号量模式
- `symlink-jnilibs/` —— `app/` + `hello-exec/`：验证"APK lib/ 下 .so 可执行 + 符号链接"的 W^X 绕过 PoC（`tools/run-symlink-test.sh` 4.2KB）

### 6.2 tools/scripts/（~30 个 ADB 驱动脚本）

- `build-bootstrap.sh` 26KB —— 生成 bootstrap-aarch64.zip（`zip -r9 -X` + **SHA256 写入 version.json sidecar**，与 bootstrap.rs 校验对应）；`generate_test_bootstrap.py`、`m4-bootstrap-packages.txt`（包清单）
- `test-all-e2e.sh` —— 四层测试主入口（--unit-only 无硬件可跑 / 传设备序列号跑全量，输出 `.omc/e2e-artifacts/`）；`test-tier1..4.sh` 分层跑；`write-summary.py`/`write-tier-summary.py` 汇总 JSON
- 设备压测系列：`test-frame-capture-stress.sh` 17KB、`test-rotation-stress.sh` 29KB、`test-30min-idle-stress.sh` 11KB、`test-ime.sh` 31KB、`test-touch.sh` 29KB、`test-scroll.sh` 23KB、`test-window-insets.sh` 26KB、`test-ansi-color.sh`、`test-block-model.sh`、`test-dynamic-grid.sh`、`test-static-grid.sh`、`test-bootstrap-install.sh`、`test-pty-reattach.sh`、`test-zsh-asset.sh`、`test-fgs-clean-kill.sh` 等——**模式统一**：`am start/force-stop` + 广播驱动 + logcat grep 断言 + 退出码
- `release.sh` —— 签名/APK + bootstrap + SHA256SUMS + RELEASE_NOTES → dist/；`security-audit.sh`、`keep-awake.sh`（lib/）、`env-setup.sh`、`setup-companion-sources.sh`（拉 warp-src/termux-packages 并校验 sha256）

---

## 7. 依赖清单与激进程度分析

### 7.1 Rust（android-host）

| 依赖 | 版本 | 用途 | 是否适用于 torvox | 激进程度 |
|---|---|---|---|---|
| jni | 0.21 | JNI 绑定 | ✅（torvox 同款） | 保守 |
| android_logger | 0.13 | logcat 桥 | ✅（torvox 有 logging.rs） | 保守 |
| sha2 + zip(defalte only) | 0.10 / 2.2 | bootstrap 校验/解压 | ✅ 值得借鉴（torvox BootstrapInstaller 用 Java zip） | 保守 |
| ash | 0.38 | 直接 Vulkan | ❌ torvox 用 wgpu 30（抽象掉 swapchain） | **激进**（2263 行手写管线，仅 Android 单目标可接受） |
| cosmic-text (git fork rev 锁定) | — | 文本形状化 | ⚠️ torvox 用 crates.io cosmic-text 0.19 | **激进**（warpdotdev fork 未发布，需 git 依赖） |
| png | 0.17 | 帧捕获编码 | ⚠️ torvox 有 image 0.25 | 保守 |
| tokio + reqwest(rustls-webpki-roots) + futures-util + tokio-util | 1/0.12/0.3/0.7 | AI 流式 | ⚠️ torvox 无 AI 客户端；**若加则这是正解**（rustls 而非 OpenSSL，Android 交叉编译干净） | 中等（为 55MB .so 增加 tokio+reqwest 栈） |
| warp_mobile_android_link / warp_terminal_mobile_facade | path | 跨仓库共享 rlib | ❌ 仓库外依赖，克隆即缺 | **激进（架构反模式）** |

### 7.2 Rust（warp_ai_mobile）—— 精简清单

tokio（default-features=false，仅 rt-multi-thread/macros/io-util/time/sync，注释明示"全量 tokio 会往 .so 塞 800+KB"）、reqwest（default-features=false + rustls-tls-webpki-roots/json/stream）、webpki-roots 0.26 显式钉版（**Android 用户证书库 API 24+ 锁定，webpki-roots 给确定性 CA 集**，Cargo.toml:47-50——torvox 若加 AI 必须照抄这条）、serde/serde_json、futures-util、log、tokio-util。

### 7.3 Kotlin

core-ktx/appcompat/coroutines-android、Compose BOM + material3 + icons-extended + activity-compose、**security-crypto 1.1.0-alpha06**（EncryptedSharedPreferences，alpha 版本略激进但这是官方唯一路径）、测试：MockK/Turbine/Robolectric。compileSdk 36、NDK 29。无网络库（AI 全走 Rust）。

### 7.4 workspace profile（激进但值得记录）

`[profile.release] strip="symbols" lto="fat" codegen-units=1 panic="abort"`（Cargo.toml:48-52，注释详列收益：8.2MB→5.8MB .so、-1MB 压缩 APK；panic=abort 在 Android 上 panic 即进程崩、从不跨 FFI catch，可接受）。torvox 的 release profile 可对照评估。

---

## 8. 功能对比：warp-mobile-android vs torvox

### 8.1 总表

| 功能 | warp | torvox | 结论 |
|---|---|---|---|
| PTY fork/exec | Rust pty.rs（AS-safe、CString 预构建、TIOCSWINSZ seed 24×80、write(2) errno 直报） | native 侧 fork/exec + linker64/LD_PRELOAD（termux-exec 风格）、`_exit(100+errno)` 编码 | 等价，细节见 research-warp.md §2 |
| JNI 面 | 55 个命令式导出（指针句柄 + poll 协议） | 48 个导出（sessionId: Long 句柄 + pollEvent 事件队列） | **同构**：都是"句柄 + 轮询"零回调模式；warp 多 AI/IME/input/terminal 统计导出 |
| 终端模型 | facade 镜像 TerminalModel（自研解析器 + Block 聚合） | libghostty-vt（上游 Zig 移植，全 VT 覆盖） | torvox 更强（warp 自研解析器只为 Block 时间线服务） |
| 渲染 | ash 直接 Vulkan（static/dynamic grid、SGR 每格着色） | wgpu 30（TerminalSurface 93KB，着色器在 native/src/shaders/） | torvox 更现代（wgpu 抽象 + 可复用桌面路径）；warp 的 **ACCESSORY_PAD 底对齐投影**（lib.rs:1772-1798）值得借鉴 |
| 渲染驱动 | Choreographer 每 vsync + dirty bit + fallback 清帧 | SurfaceView 独立渲染循环 | 等价；warp 的 dirty-bit + 无变化重绘黑帧保 swapchain 健康模式可参考 |
| 字体 | cosmic-text fork + ASystemFontIterator（NDK 29+） | fontdb + swash + 系统扫描 + SystemFonts.kt | warp 的 ASystemFontIterator 更直接；torvox 已有 setExtraFontPaths |
| emoji | classify_char 三段分类 + **边界负例测试**（lib.rs:2271） | 无显式 emoji 分类测试 | **torvox 可移植**（见 §9） |
| IME | BaseInputConnection + **composing diff 回退**（LCP + DEL code point 数） | BaseInputConnection + InputCoalescer（Kotlin 批处理） | 各有所长；warp 的 forwardComposingDiff 解决"PTY 与 IME 所见不一致"问题，torvox 的 InputCoalescer 可自查同类问题 |
| 输入 | GestureDetector 语义化 → inputTouch* 状态机（事件缓冲） | Kotlin 手势 → TerminalInputEncoder → writeKey | 等价架构 |
| 多会话 | SessionManager + Rust createSession/switchSession + sessions.json 持久化 | TerminalViewModel SessionDrawer + listSessions | 等价；warp 有崩溃恢复（fast-death） |
| 搜索 | **UnifiedSearchEngine 5 域**（sessions/blocks/history/AI/files）+ BlockSearchEngine（ANSI 剥离 + 正则 + 高亮） | TextSearchBar + **Rust 侧 searchAllInScrollback/setSearchHighlights**（scrollback 行级搜索 + 高亮数据） | **方向相反**：warp 搜"语义块"，torvox 搜"原始 scrollback 文本"。warp 的块搜索 = torvox 的行搜索的上位替代候选；torvox 的优势是搜索在 Rust 侧（不重复解析 ANSI） |
| AI agent | **warp_ai_mobile 完整栈**：SSE 流式 + 多轮会话 + ghost text + turn 控制 + 多提供商 + KeyStore BYOK + 风险审批 + CSV 审计 | ❌ **无**（torvox 只有 MCP 通道，无 LLM 客户端） | **torvox 缺失**；若规划 AI 功能，warp 的"Rust 客户端 + poll 协议 + 状态机"架构是直接模板 |
| Block 时间线 | WarpBlockState/DCS hook（Preexec/Preexec/CommandFinished 聚合 Block + 1049 自动切换） | ❌ 无 | torvox 缺失（这是 Warp 产品核心，但依赖自研解析器 hook，与 libghostty-vt 集成成本高） |
| 审批/审计 | CommandRiskEvaluator + CommandApprovalDialog + warp-ai-usage.csv | ❌ 无（MCP 工具有自己的确认流程） | torvox 缺失 |
| MCP | McpMessage/McpTransport/McpToolRegistry（最小 JSON-RPC） | native/src/mcp.rs 42KB（socket + JSON-RPC + setMcpSocketPath/setMcpEnabled） | torvox **更强**（MCP 在 Rust 侧、走本地 socket） |
| SSH | 计划中（russh，PROJECT.md:37 标注） | ❌ 无 | 均未落地 |
| 测试金字塔 | TEST_INFRA.md：4 层 290 用例（Rust 163 + Kotlin 642） | docs/test-strategy-research.md + test-coverage-audit.md（torvox 自己的体系） | 方法论等价；warp 的 **ADB 驱动脚本体系**（30 个 test-*.sh）是 torvox 缺失的实物资产 |
| 部署 | bootstrap zip 原子安装（sha256 sidecar） | BootstrapInstaller（staging 原子安装，无 sha256） | **warp 的 sha256 校验值得吸收**（见 §9） |
| 仓库健康 | 跨仓库 path 依赖（warp-src 缺失即不可构建）、AGPL、单分支 | 单仓库（libghostty-vt 经 generated-patches 本地化）、APACHE？、文档完善 | torvox 的依赖策略更稳健 |

### 8.2 详细对比：JNI 面设计

两者都放弃"Java 回调对象跨 JNI"（GlobalRef 生命周期地狱），改用**句柄 + 轮询**：

- warp：`Arc::into_raw as jlong` + `from_raw/clone/into_raw` 借用三连（lib.rs:342-345），Kotlin 50ms `Handler.postDelayed` 轮询；Rust 持状态（会话、事件队列、dirty bit）
- torvox：`sessionId: Long`（Arc 索引）+ `pollEvent(): String` 事件 JSON 队列，Kotlin `TerminalRuntime` 轮询

差异：warp 每个功能域各写一套 poll（ghost/agent/terminal），torvox 统一一个 pollEvent。**torvox 的收敛更优**；warp 的 AI poll 协议（`:CHUNK:`/`:DONE:`/`:ERR:` 前缀）在需要"流式结果"时是现成模板。

### 8.3 详细对比：搜索

- torvox 现状：`searchAllInScrollback(sessionId, needle, backward): Int`（ffi.rs:2085）+ `setSearchHighlights`（:2299，ByteArray 区间数据）+ Kotlin `TextSearchBar.kt`（11KB：输入、上下跳转、匹配计数、高亮渲染）。搜索在 Rust 侧对**原始行文本**做（scrollbackLine 逐行取），无 ANSI 问题（模型层已剥离）。
- warp 现状：`BlockSearchEngine.search`（对 Block.command/output 剥离 ANSI 后 indexOf/正则，`Dispatchers.Default`，maxMatches=10,000）+ `highlightSearchMatches`（Compose AnnotatedString 高亮）+ `UnifiedSearchEngine` 5 域聚合（async 并发 + score 排序 + domainCounts 徽章）。**50,000 块基准 75x 提速**（PROJECT.md:77）。
- 结论：warp 的 UnifiedSearchEngine 的**多域聚合 + 并发 + 计分排序** UI 模式对 torvox 的 TextSearchBar 升级有参考价值；但 torvox 的 Rust 侧行搜索更干净，**不建议为了"块"重构**（torvox 无 Block 模型）。

---

## 9. 可吸收到 torvox 的具体内容（含代码注释建议）

### 9.1 ⭐ IME composing diff（高价值，直接可吸收）

`WarpInputView.kt:587-615 forwardComposingDiff` 解决真实痛点：`setComposingText` 与 `commitText` 交替时，PTY 侧与 IME 所见不同步（拼音候选替换、emoji 删除）。torvox 的 `InputCoalescer`（Kotlin）可加同款逻辑：

```kotlin
// 参考 warp-mobile-android WarpInputView.kt:587 forwardComposingDiff（ImL1s/warp-mobile-android, AGPL-3.0）
// 问题：IME 组合区从 "nihao" 变为 "你好" 时，直接 commit 会让 PTY 侧多出残留字符。
// 方案：最长公共前缀 → 公共前缀之后的旧文本按 Unicode code point 数发 DEL(0x7F)
// （不能用 String.length，会按 UTF-16 单元误删代理对/emoji）→ 新后缀 UTF-8 写入。
private fun forwardComposingDiff(prev: String, next: String) {
    if (prev == next) return
    var i = 0
    val maxI = minOf(prev.length, next.length)
    while (i < maxI && prev[i] == next[i]) i++
    val toErase = if (prev.length > i) prev.substring(i) else ""
    val cpCount = toErase.codePointCount(0, toErase.length)
    if (cpCount > 0) ptyWrite(ByteArray(cpCount.coerceAtMost(256)) { 0x7F })
    val toAdd = next.substring(i)
    if (toAdd.isNotEmpty()) ptyWrite(toAdd.toByteArray(Charsets.UTF_8))
}
```

### 9.2 ⭐ emoji 分类 + 边界负例测试（直接可移植）

torvox 用 cosmic-text + fontdb + swash，同样需要 Latin/CJK/emoji 分段（warp 的 `classify_char` 区间：CJK :2208-2221、emoji :2225-2228）。把 `m3_s11_emoji_smoke_tests`（lib.rs:2192-2307）的 3 个测试按 torvox 的字形分段函数移植，**特别是边界负例**（U+1F2FF/U+1F700/U+25FF/U+27C0 必须非 emoji），防止 emoji 区间过度扩张导致正常字形被路由进彩色 emoji 字体（tofu）。

```rust
// 参考 warp-mobile-android crates/android-host/src/lib.rs:2206 classify_char（ImL1s/warp-mobile-android, AGPL-3.0）
// 镜像实现必须与渲染侧字形分段保持同一份区间表——warp 用注释钉死对应关系（font_render.rs:105-126）。
fn classify_char(ch: char) -> RunKind { /* CJK 0x1100-0x11FF|0x3000-0x303F|0x3040-0x312F|0x3400-0x4DBF|0x4E00-0x9FFF|0xAC00-0xD7AF|0xF900-0xFAFF|0xFE30-0xFE4F|0xFF00-0xFFEF；emoji 0x1F300-0x1F6FF|0x1F900-0x1F9FF|0x1FA00-0x1FAFF|0x2600-0x27BF */ }
```

### 9.3 ⭐ bootstrap sha256 sidecar（改进现有 BootstrapInstaller）

research-warp.md 已记录（bootstrap.rs:1-48）。补充 torvox 侧落点：`android/app/src/main/java/terminal/emulator/installer/BootstrapInstaller.kt` 的 staging 原子安装**加 version.json 期望 sha256 校验 + 安装标记文件存校验值**，损坏 zip 不再"看起来已安装"。

### 9.4 ⭐ PTY 初始 winsize 先于首读（一行级修复）

`WarpTerminalService.kt:797-808`：spawn 后、read loop 前应用 `rows/cols`，否则 shell 继承内核 80×24，与渲染网格行列不符导致折行错位、光标出界、提示符覆盖输出。torvox 的 `TerminalRuntime` 若 spawn 与首帧 resize 存在竞态，同样适用。

### 9.5 ⭐ fast-death 恢复（可参考实现）

`WarpTerminalService.kt:906-915`：进程存活 <1.5s 即退出 → 最多 3 次重试、500ms 指数退避（封顶 5s）、回退 `/system/bin/sh`、重试前清网格。torvox 的 BootGuard/AnrWatchDog 体系可加同款"shell 启动即崩"检测（torvox 当前主要防 ANR/卡死，防"秒退"是补集）。

### 9.6 ⭐ ACCESSORY_PAD 底对齐渲染投影（可选）

`lib.rs:1772-1798`：渲染时把光标固定投影到 `rows - 7 - 1` 行，内容少时顶部补空行、内容满时丢顶部 scrollback 行，底部留 7 行给键盘工具栏。torvox 若引入固定底部工具栏（ModifierBar 已存在），可在渲染视口计算时做等价处理——但 torvox 的滚动是模型驱动的（scrollOffset），需评估交互冲突。

### 9.7 ⭐ AI agent 架构模板（若 torvox 规划 AI 功能）

warp_ai_mobile 整套是可复制模板：`client.rs` 的 SSE 解析 + UTF-8 边界累积器（:448-472）+ `tokio::select!` 取消（:413-416）+ `CancellationToken` 暂停背压（lib.rs:509-511）+ poll 协议（`:CHUNK:`/`:DONE:`/`:ERR:`）+ `session_registry` 全局会话表 + `redact()` 日志脱敏（client.rs:87）+ webpki-roots 钉版（Android 用户证书库锁定，warp_ai_mobile/Cargo.toml:47-50）。torvox 已有 reqwest 类似的栈吗？没有——但 native crate 加 tokio/reqwest 栈对 APK 体积的影响（warp 实测 .so 8.2→5.8MB 仅是 strip 收益，AI 栈另算）需先评估。

### 9.8 ⭐ 测试驱动脚本模式（工具链吸收）

warp 的 30 个 `tools/scripts/test-*.sh` 是"adb 广播驱动 + logcat grep 断言"的实物模板。torvox 已有 scripts/（含 bootstrap-libghostty.nu 等）但**无设备端 E2E 驱动脚本**；可对照 `test-ime.sh`/`test-touch.sh`/`test-scroll.sh`/`test-rotation-stress.sh` 补 torvox 版（torvox 的 `TestBackdoorReceivers.kt` 已具备广播接收能力，只差驱动脚本层）。

### 9.9 其他可摘录细节

- `imeOptions` 加 `IME_FLAG_NO_PERSONALIZED_LEARNING`（WarpInputView.kt:526）
- `commitTextSynthExpiresMs` 合成提交防抖（WarpInputView.kt:553-554）
- `handleWrite` 的 `data_b64` 通道（规避 adb broadcast 参数解析器把 `-l`/`-a` 形状 token 当 flag）
- 搜索高亮双色（普通匹配黄 0xFFFFD54F / 当前匹配橙 0xFFFF8F00，BlockSearchEngine.kt:114-115）
- `EncryptedSharedPreferences` 失败回退普通 prefs（AiKeyStore.kt:54-59）

---

## 10. 项目文档吸收价值（CLAUDE.md / PROJECT.md / TEST_INFRA.md / TEST_READY.md）

| 文档 | 内容 | 对 torvox 的价值 |
|---|---|---|
| CLAUDE.md（12KB） | AI 入口：里程碑状态表（M0-M6 每个 story 的 PASS 记录 + commit SHA）、**"如何恢复工作"阅读顺序**（:27-40）、用户治理偏好（全自动、只 ping 不可逆操作）、Verifier SOP（codex 审查每份交付物）、项目约定（main 单分支、warp-src gitignore、驱动脚本用 `am force-stop` 不用 `am kill` 的 AOSP 语义 :80、测试设备分级 :81-86） | ⭐⭐ 高：**"读冷启动文档而非翻对话历史"原则**（:40）、里程碑 close-out 文档制度、codex 审查 SOP 都值得 torvox 的 AGENTS.md 借鉴；`am force-stop` vs `am kill` 的坑对所有 adb 驱动脚本通用 |
| PROJECT.md（12KB） | 5 层架构模型（System/Service → Native Engine → App State/Facade → UX/Timeline → AI/Safety）、25 个 feature 的 wave 账本（每条带 issue 号/wave/来源/状态）、**Interface Contracts 章节**（:79-92，如 DECSET 1049 ↔ UI 切换、AI 审批 ↔ shell 执行的契约） | ⭐ 高：torvox docs/architecture.md 可对照补"接口契约"章节（torvox 的 pollEvent/事件协议缺一份契约文档） |
| TEST_INFRA.md（3KB） | 测试哲学（需求驱动不透明盒）、**4-Tier 金字塔**（Feature≥5/Boundary≥5/Pairwise 25/Real-World 15，合计 290）、5 种测试框架分工（Rust 单测/Kotlin JVM/Compose UI/ADB 脚本/发布门禁） | ⭐⭐ 高：torvox 的 docs/test-strategy-research.md 已覆盖方法论，但 warp 的"每 feature 固定 5+5+1+1 配额"式账本（TEST_READY.md 的 25×4 表格）对测试覆盖审计（torvox 已有 test-coverage-audit.md）是现成模板 |
| TEST_READY.md（7.8KB） | 认证清单：日期/commit/版本基线 + 290 用例账本 + **15 个 Tier-4 真实工作负载场景描述**（:56-72，如"AI agent 自主重构 + 工具审批 + 密钥脱敏 + CSV 审计"、"TUI 全屏 raw 模式 + Vulkan 管线 + DECSET 1049"） | ⭐⭐ 高：15 个 workload 场景 = 15 个跨功能验收剧本，torvox 的 integration-tests/ 可对照挑选可复用的场景（多标签并发、崩溃恢复、统一搜索工作流） |
| CONTRIBUTING.md / SECURITY.md / deny.toml / CHANGELOG.md（20KB） | 贡献流程、安全策略（密钥处理）、cargo-deny 配置、逐里程碑 changelog | ⭐ 中：torvox 的 SECURITY.md 已有；deny.toml 可对照补 license/advisory 策略 |

**总结论**：warp 仓库文档体系的精髓不是单篇内容，而是**"每里程碑一份 close-out 文档 + 认证账本 + 读冷启动"的工程制度**。torvox 的 docs/ 体系（architecture/srs/acceptance/traceability）在规范文档上更强，缺的是"设备端测试的实物驱动脚本"与"可验证的测试完成度账本"。

---

## 11. 结论

- **lib.rs JNI 全集**（55 导出）证明"句柄 + 轮询"JNI 协议可支撑完整终端 + AI 栈，与 torvox 的 pollEvent 同源；warp 的 AI poll 协议（`:CHUNK:`/`:DONE:`/`:ERR:`）是 torvox 未来加流式 AI 的现成模板。
- **warp_ai_mobile** 是当前可参考的最完整"移动端终端 + LLM agent"Rust 实现（SSE 流式、多轮会话、暂停背压、审批、审计、KeyStore），torvox 若规划 AI 功能应以此为架构基准而非从零设计。
- **可直接吸收**（按优先级）：① IME composing diff（WarpInputView.kt:587）；② emoji 分类边界测试（lib.rs:2192-2307）；③ bootstrap sha256 sidecar；④ spawn 初始 winsize；⑤ fast-death 恢复；⑥ 5 域搜索的并发聚合 UI 模式（仅 UI 层）。
- **不建议吸收**：ash 直接 Vulkan（wgpu 30 已封装）、cosmic-text git fork 依赖、跨仓库 path 依赖结构（torvox 单仓库 + generated-patches 是更优解）、Block 时间线（依赖自研解析器 hook，与 libghostty-vt 冲突）。
- **文档制度**：close-out 认证账本 + 4-Tier 配额 + 30 个 ADB 驱动脚本是 torvox 测试体系缺失的实物资产。

## deep-v5 增量（复核第 2 轮：approval.rs + vulkan-surface-recreate spike）

### approval.rs（crates/warp_ai_mobile/src/approval.rs:1-144）

高风险命令风险评估器（Issue #15）：`DANGEROUS_SUBSTRINGS` 子串表（rm -rf/shred/wipefs/dd if=/mkfs/fdisk/parted/mkswap/sudo/su -/chmod 777/chown -R/reboot/shutdown/poweroff/curl|sh/wget|bash/git push --force/git reset --hard/apt purge/drop database/:(){ :|:& };:）→ `evaluate()` 返回 RiskLevel::Low/High（二态，:66-85）+ curl/wget | sh 管道检测。

**三方案对比**：sushi-ssh CommandSafety.classify()（三态 BLOCKED/CONFIRM/SAFE + shell 解释器检测 + 只读命令集 + 正则）> warp approval.rs（二态子串表，简单）> torvox（无命令审批）。**torvox MCP 若加命令审批，sushi 方案更完善**（已记录）；warp 提供 Rust 侧实现样例（torvox 若在 native 侧做审批可参考 :66-85）。

### vulkan-surface-recreate spike（spikes/vulkan-surface-recreate/src/lib.rs:1-706）

ash/Vulkan 原生 surface 重建实验：`create_surface_from_native_window`（:153-161）、`find_graphics_queue_family`（:164）、`select_physical_device`（:178-201）、`create_device_and_swapchain`（:204+，格式选择/交换链创建）。**torvox 对照**：wgpu 已封装（attach_surface/reconfigure_swapchain 等价），spike 验证了"销毁→重建"顺序——P3 记录，无直接吸收点。

## deep-v6 增量（复核第 4 轮：font_picker.rs:1-135）

**emoji 字体族选择器** `pick_emoji_family`（:31-54）：优先级 Samsung*Emoji*（CBDT/CBLC bitmap，swash 0.1.x 可解码）→ Noto*Emoji*（Android 标配）→ None（cosmic-text fallback 接管）。含 3 测试（Samsung 优先/Noto 兜底/无 emoji 返回 None）。

**torvox 对照**：cjk.rs:72 检测 emoji 字体 + emoji_glyph_grinning 测试——但**无显式优先级选择**。差异 P2：三星设备上 emoji 渲染（SamsungColorEmoji 位图）torvox 未显式优先——若设备 emoji 显示异常可参考 warp 的选择逻辑（P2 记录）。
