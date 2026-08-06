# 深度研究：warp JNI/IME/Input/会话 — 亲自逐文件阅读补充

> 研究日期：2026-08-06 | 项目链接：https://github.com/ImL1s/warp-mobile-android
> 前置：`research-warp.md`（pty.rs/bootstrap.rs 亲自读）；本文补充 **lib.rs PTY/IME 段、ime.rs 159 行全、input.rs 506 行全、terminal_model.rs 302 行全**

## 1. lib.rs JNI 导出结构（2394 行，分段扫描 + 关键段精读）

**55 个 JNI 导出，11 组**：

| 组 | 行号 | 导出 | torvox 对照 |
|----|------|------|------------|
| ping | :85 | 1 | 等价 |
| AI ghost-text | :114-427 | 7（complete/stream start/poll/cancel/free） | torvox 无（MCP 是不同形态） |
| AI agent 会话 | :455-736 | 8（create/send/poll/control/free/retry/edit） | torvox 无 |
| **PTY** | :743-956 | 9（spawn/count/exit/acquire/release/read/write/resize/kill） | **同构** |
| render | :968-1148 | 5（attach/detach/clear/capture） | torvox 等价（JNI 形态不同） |
| glyph grid | :1167-1323 | 5 | torvox 无（wgpu CellData 替代） |
| **IME** | :1343-1427 | 5（commit/setComposing/finish/stats/reset） | **torvox 无 Rust 侧 IME** |
| input | :1444-1564 | 7（down/up/cancel/tap/longPress/scroll/stats/reset） | torvox 无 |
| insets | :1594 | 1 | torvox 无 |
| terminal model | :1615+ | ~7 | torvox 用 libghostty-vt |
| 多会话 | :2001+ | 若干 | torvox SessionRegistry 等价 |

### 1.1 PTY 绑定（:743-956，亲自精读）

- **`ptySpawn`（:743-798）**：JString program + JObjectArray args + 扁平 `["KEY=VALUE"]` env 数组 → `pty::spawn_pty` → `Arc::into_raw` 返回 jlong。**env 扁平数组在 JNI 解析时按第一个 `=` 切分**（:779-781）
- **`ptyAcquire/ptyRelease`（:835-855）**：**Arc 引用计数显式管理**——`Arc::increment_strong_count`/`decrement_strong_count`。注释（:832）："Called under PtyManager map lock before ptyRead"。**跨 JNI 边界的 Arc 生命周期协议**——torvox 用 SessionRegistry + Mutex 管理，warp 用原始 Arc 计数，**torvox 更安全**
- **`ptyRead`（:859-897）**：读入临时 Vec → `byte_array_from_slice`。**错误 errno 日志**（:881-886）：区分 EBADF（post-kill 正常）/ EIO（slave 关闭——**v1-prep blocker #3 正在追查**）/ EINTR
- **`ptyWrite`（:901-919）**：失败返回 `-errno`（负值编码）
- **`ptyKill`（:942-956）**：`kill_eager()`（关 fd 让并发读立即 EBADF）+ kill + **decrement Arc**（Java map 持有的那份）

**torvox 对比**：torvox 的 ffi.rs feedPty/writeKey 有 lock_or_recover + session 锁；warp 的裸 Arc 指针 + 手动计数更脆弱（但文档化明确）。**torvox 更优**。

### 1.2 IME 绑定（:1343-1427，亲自精读）

- `imeCommitText(text, new_cursor_position)`（:1343-1358）：空 text 是 no-op（部分 IME 发空 commit）
- `imeSetComposingText`（:1366-1381）：空 text + 活动区 = finish（清区不插入）；空 + 无活动区 = no-op
- `imeFinishComposingText`（:1390-1396）：**空 composing 区发 EmptyFinish 标记而非双提交**（Gboard 已知问题：setComposing 和 commit 之间空 finish）
- `imeStats`（:1407-1415）：**逗号分隔诊断字符串**（11 字段）供设备驱动 grep——**测试契约模式**（torvox 可用 JNI 诊断字符串做集成测试）
- `imeReset`（:1421-1427）

**torvox 对比**：torvox 的 IME 全在 Kotlin（InputCoalescer/BaseInputConnection），Rust 只收 feedPty 字节。warp 的 **Rust 侧 IME 状态机**（composing region 跟踪）是不同架构。**torvox 的 Kotlin 侧方案更灵活**（可直接操作 Compose 状态），但 warp 的"空 finish 防双提交"经验值得记录。

## 2. ime.rs（159 行，完整阅读）

- `global_ime()`（:47-50）：`OnceLock<Mutex<AndroidIme>>` 单例——**OnceLock + Mutex 是 JNI 全局状态的惯用模式**（torvox 用 OnceLock/static Mutex 同款）
- 4 个入口（commit/setComposing/finish/reset）+ stats_string（:77-97，11 字段）
- 单测（:121-158）：`singleton_reset_clears_counters` + `stats_string_schema_matches_driver_grep`（**测试锁定驱动 grep 契约**——schema 变更会被测试捕获）
- 核心状态机在共享 rlib `warp_mobile_android_link::ime`（12 测试含 Gboard `setComposing → finish → commit` defer-flush）

**关键设计**：JNI 层只做"单例 + 转发"，状态机在共享 crate——**测试可无 JNI 直接测状态机**。

## 3. input.rs（506 行，完整阅读）

### 3.1 InputEvent 模型（:44-85）

```
TouchDown/TouchUp/TouchCancel/Tap/LongPress/Scroll{x,y,dx,dy,vx,vy}
```
- **TouchCancel**（:49-54）：ACTION_CANCEL 关闭打开的 down 序列——"没有它，下游消费者会相信手指仍按下"（状态机完整性）
- Scroll 的 vx/vy 注释（:64-83）：**Android 屏幕坐标**（Y 向下），"终端滚动约定 M3 再定（可能反转）"——warp 未定，zelland 已定为 distanceY>0=scroll_down

### 3.2 AndroidInput（:167-287）

- 事件 Vec（capacity 32）+ 计数 + last 坐标 + push 时**单调递增 events_total 日志**（:268-286，驱动靠 monotonic-break 重建窗口）
- `drain_events`（:242-244）：`std::mem::take`——**消费式读取**（torvox pollEvent 同款）

### 3.3 单测（:390-505）

7 个测试：down/up 记录、tap、long_press、scroll 速度、drain、singleton reset、**cancel_after_down_emits_touch_cancel**（状态机完整性验证）、stats_string 格式。

**torvox 对比**：torvox 触摸全在 Kotlin（TerminalSurface 手势 + SelectionHandles），Rust 只收 key/scroll 数据。warp 的 Rust 侧触摸事件模型（含 cancel 完整性）对 torvox 无直接移植价值（架构不同），但 **TouchCancel 处理**是通用教训（torvox Kotlin 侧 selection 拖拽是否处理 ACTION_CANCEL？需查）。

## 4. terminal_model.rs（302 行，完整阅读）

### 4.1 定位（:1-10）

**facade 委托层**——消除 2289 行重复终端模型/解析器，全部集中在 `warp_terminal_mobile_facade` 共享 crate。JNI 层只 re-export + 全局单例 + 转发。

### 4.2 多会话（:26-49）

```rust
pub fn active_model() -> Arc<TerminalModel> {
    if let Some(session) = SessionManager::global().active_session() {
        session.model().clone()
    } else {
        SessionManager::global().create_session("default", ...).ok();
        ...
    }
}
```
**SessionManager::global() 单例 + active_session 路由**。`ingest_pty_bytes_for_session(cmd_id, bytes)`（:55-64）：**关闭/未知会话的字节丢弃不泄漏进活跃视口**（:61-62 注释）。

### 4.3 委托函数集（:51-116）

ingest/take_dirty/peek_dirty/snapshot_text/snapshot_cells/cursor/dims/resize/blocks_dump_json/set_scroll_offset/scrollback_len/is_alt_screen。

### 4.4 测试（:118-301）

- **3 个 `#[ignore]`**（:156/:167/:202/:284）：UTF-8 三字节汉字组装、Block dump JSON、alt-screen 跟踪——**facade 解析器的已知缺陷被显式标记**（诚实标注）
- Block 模型：DCS 帧（`\x1bP$d{hex json}\x1b\\`）解析 Precmd/Preexec/CommandFinished hooks → command/exit_code/output 块
- 多会话路由 + 关闭会话字节丢弃测试（:258-281）

**torvox 对比**：torvox 用 libghostty-vt（完整 VT）替代自研 parser——**warp 的 facade 解析器是"半成品 + 3 个已知缺陷"**，反证 torvox 的 vendored ghostty 决策正确。但 **Block 模型（命令块 JSON）**是独特概念（zellij/agent 生态），torvox MCP 的 terminal_info 可借鉴"命令/退出码/输出"块结构。

## 5. 与 torvox 功能对比总表

| 功能 | warp | torvox | 结论 |
|------|------|--------|------|
| JNI 句柄协议 | Arc 裸指针 + 手动计数（acquire/release） | SessionRegistry + Mutex | **torvox 更安全** |
| PTY | 同构（spawn/read/write/resize/kill） | 同构 | 等价 |
| IME | Rust 侧状态机（composing region） | Kotlin 侧 InputCoalescer | 架构不同，torvox 更灵活 |
| 触摸 | Rust 侧事件模型（含 cancel） | Kotlin 侧手势 | 架构不同 |
| 终端解析 | facade 自研（3 已知缺陷） | libghostty-vt | **torvox 更优** |
| 多会话 | SessionManager 单例 | SessionRegistry | 等价 |
| Block 模型 | DCS hooks → 命令块 JSON | 无 | **warp 独有**（MCP 可借鉴） |
| 诊断字符串 | stats_string 驱动 grep 契约 | 无 | **可借鉴**（测试契约） |
| 测试 | 状态机共享 rlib 可无 JNI 测试 | 单测 + JNI 集成测试 | 等价 |

## 6. 可吸收到 torvox 的具体内容

1. **stats_string 驱动契约（P1）**：JNI 暴露逗号分隔诊断串 + 测试锁定 schema——torvox 集成测试可借鉴（imeStats/inputStats 模式）
2. **IME 空 finish 防双提交（P1 记录）**：Gboard setComposing→finish→commit 之间的空 finish——torvox Kotlin InputCoalescer 是否处理？（需查）
3. **TouchCancel 完整性（P1 记录）**：torvox Kotlin selection 拖拽是否处理 ACTION_CANCEL？
4. **Block 模型 JSON（P2）**：MCP terminal_info 扩展候选（命令/退出码/输出块）
5. **关闭会话字节丢弃（P2 记录）**：torvox 已等价（session 清理）
6. **OnceLock+Mutex 单例模式（P2）**：torvox 已用同款

## 7. 结论

warp 的 JNI 层与 torvox 同构但更脆弱（裸 Arc 计数）；终端解析是半成品（反证 torvox 决策）；**可吸收的是测试方法论**（stats_string 驱动契约、状态机共享 rlib）和 3 个防御性经验（IME 空 finish、TouchCancel、关闭会话字节丢弃）。
