# Rust 依赖研究：日志 / 并发 / 数据路径 / 测试与基准

> 范围：`native` crate（终端引擎 + GPU 渲染 + JNI 桥）的依赖选型。
> 方法：本地代码核查（`native/src` grep + `Cargo.toml`/`Cargo.lock` 事实核对）+ 库官方主页与 README（parking_lot、dashmap、criterion、tracing、bytes、smallvec、rayon、loom）+ 既有决策文档（`docs/test-strategy-research.md`、`docs/dependencies.md`、`docs/tech-stack.md`）。
> 关联文档：`docs/test-strategy-research.md`（测试策略，本文第 7/8 类与其保持一致）。

## 0. 项目现状速览（本地核查结论）

| 类别 | 现状 | 证据 |
|---|---|---|
| 日志 | `log 0.4`，200+ 处 `log::*` 调用；自定义 `log::Log` 实现写 logcat + 文件 | `native/src/android/logging.rs`；`tracing` 仅作 axum/tower 传递依赖（Cargo.lock:3043） |
| 互斥锁 | 全 `std::sync::Mutex/RwLock`（20+ 处），并有整套 **poison 恢复层** | `lock_util.rs`（91 行，含测试）+ `ffi.rs` 手写 `match Err(poisoned)`（session 注册表、事件队列、MCP 回调等） |
| 并发 map | 仅 2 个：`RwLock<HashMap<u64, SessionEntry>>`（SESSION_REGISTRY）、`Mutex<HashMap<(u64,u64), oneshot::Sender>>`（REQUEST_REGISTRY，MCP 专用） | `native/src/android/ffi.rs:140,190` |
| LRU | **已有** `lru 0.18`：glyph 10k / shape 1k / glyph_id 10k / cjk 10k 四个缓存 | `render/font/glyph_cache.rs`、`GLYPH_CACHE_CAPACITY = 10_000` |
| 错误处理 | **已有** `thiserror 2`（4 处 `derive(Error)`）；无 anyhow（项目约束） | workspace deps + `render/font/mod.rs`、`pty.rs` 等；`docs/tech-stack.md` |
| 测试 | `proptest 1.11`、`quickcheck 1.1+macros`、`shuttle 0.9` 均已声明，但 **零使用**（历史遗留） | `native/Cargo.toml` dev-deps；`docs/test-coverage-audit.md:128`、`docs/project-health.md:42` |
| 基准 | 10 个 `bench_*` 测试用 `Instant::now` + 硬阈值断言（fps/MB/s），GPU 基准带全局串行锁 | `render/tests.rs`（6 个）、`ghostty_terminal/tests.rs`（4 个）；`GPU_BENCH_LOCK` |
| 通道 | **已有** `flume 0.12`：PTY→VT 输出通道 `bounded::<Vec<u8>>(128)`、ghostty 命令/查询通道、事件队列 | `terminal/session.rs:138,366` |
| 渲染数据路径 | 已零拷贝：`Vec<CellInstance>` → `bytemuck::cast_slice` → `queue.write_buffer` | `render/pass.rs:226,241` |

## 1. tracing vs log —— 不推荐升级，保持 log

- **候选**：`tracing 0.1`（tokio 维护，6.8k stars；结构化 span/event，官方明确"不依赖 tokio 运行时"；`tracing-log` 可桥接 log 生态）。
- **结论**：**不推荐**（暂不升级）。
- **理由**：
  1. 全库 200+ 处 `log::*` 调用 + 自研 logcat `Log` 实现（`android/logging.rs` 约 30 行，含文件日志开关）都已工作；换 tracing 需重写调用点并自写 Android subscriber，成本高、收益集中在对 span 的需求上，而本项目诊断场景是"单会话文本流"（logcat），结构化价值有限。
  2. 上游 `libghostty-vt`、wgpu、nix 等库 emit 的是 `log`；tracing 若接入须经 `tracing-log` 桥接层，多一层间接。
  3. 热路径已有纪律：`pty.rs:104` 明确"**DO NOT add `log::debug!`** in the hot path"——日志量本身受控，不是待解决问题。
  4. tracing 目前只是 Cargo.lock 里的传递依赖（axum/tower-mcp 用），`native` 不直接依赖；引入即新增直接依赖，需过 cargo-machete（会被要求真正使用）。
- **若未来需要**：per-session span 诊断（MCP 会话排障）时再评估，方案为 tracing + `tracing-log` + 自定义 Android subscriber，保留 `log` 宏不动（tracing 兼容 log）。

## 2. parking_lot —— 推荐引入（P1）

- **候选**：`parking_lot 0.12`（Amanieu，3.4k stars，MSRV 1.84 < 项目 1.97）。
- **结论**：**推荐引入**，直接依赖加到 `[workspace.dependencies]`。
- **理由**：
  1. 官方基准（x86_64 Linux）：无竞争快 ~1.5x、高竞争快 ~5x；`Mutex` 仅 1 字节；RwLock 任务公平、避免读写饥饿。本项目锁竞争点在 JNI 线程 ↔ 渲染线程 ↔ VT 线程之间（session 锁、事件队列、字体缓存），属于典型微竞争场景。
  2. **无 poison 机制**：可整体删除现有 poison 恢复层——`lock_util.rs` 全部删除，`ffi.rs` 的 `rlock_session_registry`/`wlock_session_registry` 与各 `match Err(poisoned)` 分支、`event.rs`/`mcp.rs`/`logging.rs` 的 poison 告警全部简化，代码净减 ~150 行且消除一类"恢复后数据是否可信"的隐性风险。
  3. 纯 Rust + futex 实现，Android bionic 上正常工作（Termux、Signal 等移动端 Rust 项目广泛使用）；ARM64 同样受益于自旋 + 原子快速路径。
  4. Cargo.lock 中 **已有 parking_lot 0.12**（wgpu 等带入，Cargo.lock:1797），转为直接依赖零新增依赖树，cargo-audit 面不变。
- **用在哪**：替换全部 `std::sync::{Mutex, RwLock}`（session 注册表、事件队列、字体缓存锁、MCP 回调表、日志文件锁、测试用锁），删除 `lock_util.rs`。
- **注意事项**：
  - JNI 边界语义要保留：`ffi.rs` 现用 poison 检测向 Kotlin 抛异常；parking_lot 无 poison，改为依赖已有的 `catch_unwind`（`ffi.rs:94` "JNI export panicked"路径）即可，需在替换时核对。
  - `lock_util.rs` 的 poison 恢复测试（`lock_or_recover_after_poison` 等）随文件删除。
  - 改动面 20+ 处，建议独立 PR、分模块替换。

## 3. dashmap —— 不推荐引入

- **候选**：`dashmap 5.x`（4.1k stars，sharded 并发 map，`&self` API，可作 `RwLock<HashMap>` 直接替代）。
- **结论**：**不推荐**。
- **理由**：
  1. 项目仅 2 个并发 map，且都是**低频访问**：SESSION_REGISTRY 只在用户建/切/毁会话时读写；REQUEST_REGISTRY 只在 MCP 对话框期间（300s 超时窗口内）操作。`RwLock<HashMap>` 无竞争问题，dashmap 的收益（高并发吞吐）用不上。
  2. dashmap 每条目有 shard 间接与更高内存占用；`entry` API 与现有 `lock_or_recover` 风格不兼容，替换需重写逻辑。
  3. 引入即新增直接依赖（当前不在依赖树中）。
- **若未来需要**：渲染线程每帧查询 per-session 共享状态（目前渲染走快照，不查注册表）时再评估。

## 4. lru —— 已有，保持

- **结论**：**已有**（`lru 0.18`，`render/font/glyph_cache.rs`），无动作。
- 容量合理：glyph 10k / shape 1k / glyph_id 10k / cjk 10k，均有 `clear()` 联动（字体切换时整组失效），并有 LRU 逐出测试（`glyph_atlas_lru_eviction`）。

## 5. bytes / smallvec —— 均不推荐（smallvec 仅可选微优化）

- **候选 A**：`bytes 1`（tokio 生态，refcount 共享字节切片）。
- **结论 A**：**不推荐**。
- **理由 A**：`Bytes` 的价值在"多消费者/部分消费共享同一块内存"。本项目的字节流是**单消费者线性管道**：PTY reader 线程读一块 `Vec<u8>` → flume `bounded(128)` 通道（所有权转移）→ ghostty VT 解析，全程无需共享切片；渲染侧 `Vec<CellInstance>` → `bytemuck::cast_slice` → `queue.write_buffer` **已零拷贝**（bytemuck 是既有方案，`docs/tech-stack.md:21`）。引入 bytes 只会增加间接层。
- **候选 B**：`smallvec 1`（栈内小向量）。
- **结论 B**：**不推荐**（如基准证明 shape 路径分配是热点，可降级为可选）。
- **理由 B**：smallvec 已是传递依赖（swash/cosmic-text 带入，Cargo.lock:2673）。潜在用点：每行 `ShapedGlyphInfo` 结果（通常 < 百项）。但每帧主导分配是 `Vec<CellInstance>`（1920 项量级），shaped-run 临时 Vec 占比小；项目已手写栈数组优化（`ascii_glyph_ids: [Option<GlyphId>; 128]`），说明该处优化意识已在。无基准证据前不值得引入直接依赖。

## 6. thiserror 2 —— 已有，保持

- **结论**：**已有**（workspace `thiserror = { version = "2", default-features = false }`），4 处 `derive(Error)`（render/font、render/mod、pty、session）。
- 与项目约束一致：**无 anyhow**（`docs/tech-stack.md:43` "no `anyhow` in libraries"），无需动作。

## 7. proptest / loom / shuttle —— proptest、shuttle 已有（应启用而非新引入）；loom 明确不引入

- **现状**：三者均已声明（`native/Cargo.toml` dev-deps），但**零使用**（`docs/test-coverage-audit.md:128` 记录为历史遗留，Phase 3 删除 fuzz workspace 时一并移除）。
- **proptest**：**已有，推荐启用**为属性测试主力（策略化生成 VT 序列/网格状态机不变量），与 `docs/test-strategy-research.md:1.1` 结论一致。
- **shuttle vs loom**：shuttle 0.9 **已有且已被选定**（`test-strategy-research.md:1.2`：async 原生支持、无需把 `std::sync` 替换成 `loom::` 类型、活跃迭代；loom 需 `cfg(loom)` 双写、async 支持有限、发版停滞）。loom README 亦确认：须用 `loom::sync` 替换被测代码、不支持 `SeqCst`。**结论：不引入 loom**，为 session 通道竞争补 shuttle 用例（P1 计划项）。
- 补充：`docs/acceptance.md` 引用过 `sgr_proptest` / `shuttle_concurrent` 测试名，说明这些工具曾有用例，属"恢复启用"而非新引入。

## 8. criterion —— 可选引入（P2，dev-deps），不替换现有断言基准

- **候选**：`criterion 0.5`（bheisler，5.5k stars；开发已迁移至 criterion-rs 组织，README 明示）。统计驱动：置信区间、回归检测、HTML 报告。
- **结论**：**可选引入（低优先级）**，与既有 `bench_*` 测试**互补不替代**。
- **理由**：
  1. 现有 10 个 `bench_*` 是**回归守卫型**基准：`Instant::now` + 硬阈值断言（如 `fps > 200`、`MB/s > 500`）+ GPU 全局串行锁（Lavapipe 争抢保护），跑在 `cargo test` 里、CI 强制执行——这是特性而非缺陷。criterion 不跑在 `cargo test` 中、不产生可断言单值，**不能**替代它们。
  2. `docs/test-strategy-research.md` P1 已计划"criterion bench 落地 VT 解析与网格重排"——作为**开发期趋势分析**（检测回归、对比优化前后）合理。
  3. 代价：增加编译时间（criterion 及其依赖较重）；Android 目标不跑 bench，仅桌面。
- **用在哪**：`native/benches/`（dev-dependencies），测 VT 解析吞吐、网格重排、字形整形；产出 HTML 报告人工分析；CI 不强制阈值。

## 9. rayon —— 不推荐引入

- **候选**：`rayon 1.x`（数据并行，MSRV 1.85；官方支持自定义线程池与 join/scope）。
- **结论**：**不推荐**（现阶段）。
- **理由**：
  1. 无适用并行点：`build_instances_from_cell_data` 单线程逐行构建，行间共享字形缓存（未命中触发同步光栅化），并行化会引入缓存锁竞争与实例顺序依赖；渲染瓶颈在 GPU 上传/字形缓存，现有基准已 200+ fps 达标。
  2. VT 解析已在独立线程（ghostty VT 线程），事件/渲染/PTY 三线程模型已定，无"大数组单线程计算"可并行的块。
  3. rayon 全局线程池在 Android 上与渲染线程、JVM 线程争 CPU；移动端多核受散热约束，收益存疑。
  4. rayon 目前仅作传递依赖（image→rav1e→rayon，Cargo.lock:143），引入即新增直接依赖。
- **若未来需要**：CPU 侧 cosmic-text 整形成为瓶颈时，先基准验证再并行。

## 10. flume / crossbeam —— flume 已有，不引入 crossbeam

- **结论**：flume 0.12 **已有**（PTY 输出通道 `bounded(128)`、ghostty 命令/查询通道、MCP 线程控制），覆盖全部通道需求；**不引入 crossbeam**。
- **理由**：flume 设计目标即 crossbeam-channel 的性能 + 更简洁 API；crossbeam 其余组件（epoch、deque、utils）本项目无使用场景。迁移无收益。

---

## 值得引入清单（按优先级）

| 优先级 | 项 | 类型 | 动作 | 收益 |
|---|---|---|---|---|
| **P1** | `parking_lot 0.12` | 新直接依赖（已在依赖树） | 替换全部 `std Mutex/RwLock`；**删除 `lock_util.rs` 及全部 poison 处理**；核对 JNI 边界 `catch_unwind` | 锁竞争提升 + 净删 ~150 行 poison 恢复代码；Android futex 路径可靠 |
| **P1** | `proptest 1.11` / `shuttle 0.9` | 已有依赖（零使用） | 恢复启用：网格/滚动/光标状态机属性测试；session 通道并发用例 | 兑现已声明依赖，补齐属性与并发覆盖（对齐 `test-strategy-research.md` P0/P1） |
| **P2** | `criterion 0.5` | 新 dev-dependency | `native/benches/`：VT 解析、网格重排、字形整形趋势基准 | 开发期回归检测与优化对比（HTML 报告）；不碰 CI 断言基准 |
| P3（可选） | `smallvec 1` | 新直接依赖（已在依赖树） | 仅当基准证明 shaped-run 临时 Vec 分配是热点 | 消除热路径小分配（当前无证据） |

## 明确不引入清单

| 库 | 原因 |
|---|---|
| **tracing** | 200+ 处 `log` 调用 + 自研 logcat 实现已工作；上游库 emit `log`；热路径日志已有纪律；收益（span 结构化）与项目诊断场景（单会话文本流）不匹配。保持 log。 |
| **dashmap** | 仅 2 个低频并发 map，`RwLock<HashMap>` 无竞争问题；dashmap 有 shard 内存开销且需重写逻辑。 |
| **bytes** | 单消费者线性管道（PTY→VT→GPU）无共享切片需求；渲染已 bytemuck 零拷贝。 |
| **loom** | shuttle 已选定（async 原生、无需替换并发原语、活跃迭代）；loom 需 `cfg(loom)` 双写且发版停滞。 |
| **rayon** | 无适用并行点；渲染瓶颈在 GPU/缓存；Android 线程池与渲染线程争 CPU；收益未证实。 |
| **crossbeam** | flume 已覆盖通道需求；其余组件（epoch/deque）无使用场景。 |
| **anyhow** | 项目约束禁止用于库（`docs/tech-stack.md:43`）。 |
| **lru / thiserror / flume** | 已有且用得恰当，无动作。 |

## 来源

- parking_lot：https://github.com/Amanieu/parking_lot （README：无竞争 1.5x / 高竞争 5x、Mutex 1 字节、任务公平 RwLock、MSRV 1.84）
- dashmap：https://github.com/xacrimon/dashmap （README：sharded、`&self` API、`RwLock<HashMap>` 直接替代定位、MSRV 1.70）
- criterion：https://github.com/bheisler/criterion.rs （README：已迁至 criterion-rs 组织）
- tracing：https://github.com/tokio-rs/tracing （README：不依赖 tokio 运行时、tracing-log 桥接、MSRV 1.65）
- bytes：https://github.com/tokio-rs/bytes （README：no_std、serde 可选）
- smallvec：https://github.com/servo/rust-smallvec （README：栈上小向量）
- rayon：https://github.com/rayon-rs/rayon （README：par_iter、自定义线程池、MSRV 1.85）
- loom：https://github.com/tokio-rs/loom （README：C11 模型排列、`cfg(loom)` 双写、SeqCst 不支持）
- 项目内：`docs/test-strategy-research.md`、`docs/test-coverage-audit.md`、`docs/dependencies.md`、`docs/tech-stack.md`、`native/Cargo.toml`、`Cargo.lock`
