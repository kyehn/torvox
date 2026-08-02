# Rust 依赖激进调研（最先进/激进版）

> 状态：研究完成（2026-07 时点）。本文是 [`dependency-research-rust.md`](./dependency-research-rust.md)（保守版）的激进补充——**不重复保守版已确认的结论**，只在其基础上评估：最新版本动向、保守版遗漏的激进选项、以及保守版结论在版本演进后的**变化**（尤其 criterion 已恢复活跃，需修正保守版"criterion 停滞"的判断）。
>
> 方法：① 本地代码热点核查（`native/src/`、`Cargo.lock`、`vendor/ghostty`）；② crates.io API 版本核实（2026-07 最新稳定版）；③ 与 Kotlin 侧现状（`TerminalApp.kt` 崩溃处理器、android/benchmark）对照。
>
> 工具链事实：`rust-toolchain.toml` = **stable**（无 nightly），edition 2024，rust-version 1.97；目标含 `aarch64-linux-android`/`x86_64-linux-android`。因此"不稳定特性"只可能来自 crate 本身（如 parking_lot 的 `unstable` feature），nightly-only 方案（tokio-console、`-Zbuild-std` 等）直接出局或需额外成本。

---

## 0. 本地热点核查结论（第一步结果）

| 热点 | 核查结果 | 结论 |
|---|---|---|
| 手写 JSON 序列化 | **不存在**。MCP 事件（`mcp.rs` `json!`）、JNI `pollEvent` 输出（`ffi.rs:1088` `serde_json::to_string`）、会话 ID 列表（`ffi.rs:1482`）全部走 `serde::Serialize` + serde_json（已是直接依赖） | 无改进空间；`simd-json` 不适用（事件负载小、MCP 低频） |
| 手写字节/UTF-8 校验 | PTY 热路径（`session.rs:263` reader 线程）是 `read_buf[..n].to_vec()` 原样送 flume，**无 Rust 侧 UTF-8 校验**；VT/PTY 解析在 `vendor/ghostty`（**Zig 源码**，Rust 侧改不动）；UTF-8 处理仅在低频处（`osc_handler` OSC 52 base64、`action_parser` DECRQSS） | simdutf8 无应用点；memchr 已在依赖树（2.8.3），低频处顺手可用 |
| HashMap/HashSet | 唯一**每帧** map：`cell_builder.rs:158` `HashMap<i32, Vec<&SearchHighlight>>`（每帧构建 + 每格 lookup）。其余低频：`SESSION_REGISTRY`（RwLock）、`pipeline.rs:554` HashSet。注意：std HashMap 自 1.36 起内部就是 hashbrown SwissTable，**hasher（SipHash13）才是真实收益点** | 候选 = foldhash hasher（已在依赖树，0 新增），非 hashbrown |
| 原子/无锁 | 原子用法正确（Acquire/Release）；`ACTIVE_SESSION_ID` 已是原子镜像；网格快照走 **flume bounded(1) 消息传递**（`public_api.rs:302`），一次一读，不是共享读 | **arc-swap 架构不匹配**，不适用 |
| 小向量/栈分配 | 每帧 `Vec::with_capacity(cell_data.len())` 分配 instances（~2000 项 ≈ 每帧 ~100KB 分配流量）；shaping 输出有 cache 复用 | smallvec 已在树但收益小；真正激进 = 跨帧缓冲复用 / bumpalo |
| unsafe 数量 | 集中在 `ffi.rs`（JNI 指针）与 `pty.rs`（fork/poll/termios），**均不可避免**；渲染数据零拷贝已由 bytemuck（Pod+Zeroable）覆盖 | zerocopy 迁移无收益（详见 §6） |
| 错误处理 | thiserror 已用，anyhow 禁用 | 无动作 |
| 日志/崩溃 | `logging.rs` 自研 logcat logger（注释明示曾用 android_logger 后替换）；**缺口：无全局 panic hook**——`session.rs:263` reader 线程等非 JNI 线程 panic 只写 stderr，Android 上不可见、线程静默死亡 | **激进推荐：自研 ~15 行 panic hook**（§10） |

---

## 1. parking_lot 0.12

- **候选**：parking_lot 0.12.5（2025-10-03 发布，crates.io 最新稳定版；rust 1.71+）
- **激进推荐**：✅ 推荐（确认保守版 P1，补版本事实）
- **理由**：已作为传递依赖锁定在 `Cargo.lock`（0.12.5），**新增成本只是把它声明为直接依赖**。本项目 5 处 std 锁：`SESSION_REGISTRY`（RwLock，读多写少）、`EVENT_QUEUE`（Mutex<VecDeque>）、`response_buffer`（Arc<Mutex<Vec<Vec<u8>>>>）、`cell_data` 相关、`pending_rx`。std 锁的 poison 恢复包装（`ffi.rs:145-160` 等 ~40 行）可整层删除。
- **若推荐**：
  - 用在哪：`ffi.rs`（SESSION_REGISTRY + poison 包装）、`event.rs`（EVENT_QUEUE）、`ghostty_terminal/commands.rs`（response_buffer）。
  - 减多少代码：删 poison 恢复层 + `lock_util` 约 **-100~150 行**；锁路径无 poison 检查分支。
  - 提升多少性能：读多写少场景（SESSION_REGISTRY）parking_lot RwLock 快于 std；Mutex 竞争路径无 syscall 级差别但少一次 poison 分支。对渲染帧路径无直接影响（锁均在低频/中频路径），收益主要是**代码量与健壮性**。
  - 风险：极低（0.12.5 已发布 9 个月、生态标配；`hardware_lock_elision`/`deadlock_detection` 等不稳定特性**不要开**）。
- **来源**：https://crates.io/crates/parking_lot （0.12.5，2025-10-03）；https://github.com/Amanieu/parking_lot

---

## 2. 基准框架：divan 0.1.21 vs criterion 0.8.2 vs codspeed 5.0.1

> ⚠️ **保守版重大修正**：保守版写"criterion 停滞（0.5 时代）"。**事实已变**：criterion 0.6/0.7/0.8 于 2025-2026 连续发布，**0.8.2 于 2026-02-04 发布**（criterion-rs 组织维护），rust 1.86+。criterion **已恢复活跃**，且 0.8 新增 `--failure-threshold`（回归断言）。

- **候选**：divan 0.1.21（2025-04-10）；criterion 0.8.2（2026-02-04）；codspeed 5.0.1 + codspeed-criterion-compat 5.x（2026-06-26）
- **激进推荐**：✅ **criterion 0.8.2 为主**；本地快速迭代用 divan 可选；CI 回归用 codspeed 可选（三者可共存，criterion 与 codspeed 通过 compat 层直接互通）
- **理由**：
  - 项目现状：**没有 `native/benches/`**，只有测试内的手写 `Instant` 断言（保守版 P2 项，尚未落地）。criterion 0.8 是比手写 Instant 更激进的正确工具：自动预热/统计/异常值剔除 + 0.8 的失败阈值（`--failure-threshold`）可把基准变成 **CI 防回归门禁**。
  - divan：0 默认依赖、编译快（不拉 rayon/plotters），适合本地快速比较；**但 `codspeed-divan` 不存在（crates.io 404）**——divan 无法上 CodSpeed 云端。criterion + `codspeed-criterion-compat` 是 CI 路径唯一现成组合。
  - 保守版"criterion 停滞"的否决理由已失效。
- **若推荐**：
  - 用在哪：新增 `native/benches/`，覆盖三处热路径：① `build_cell_instances`（cell_builder 全帧构建，`throughput` 按 cell 数）；② shaping（glyph cache 命中/未命中两组）；③ ghostty 命令通道往返（`send_and_wait` 类，基准已有此类测试可迁移）。手写 Instant 断言测试保留（功能断言），criterion 管性能趋势。
  - 减多少代码：手写基准样板（预热循环、多次采样取中位数）约 30-60 行/处 → criterion 自动。
  - 提升多少性能：不直接提速，但 `--failure-threshold` 让回归在 CI 现形（如 cell_builder 每帧 map 改动导致 5% 退化时门禁拦截）。
  - 风险：低。criterion 0.8 默认特性含 rayon/plotters（编译变重，~1-2 分钟增量）；建议 `default-features = false, features = ["cargo_bench_support"]`（dev-dep，不影响发布构建）。codspeed 需要 CodSpeed 账号 + GitHub App，属可选增强。
- **来源**：https://crates.io/crates/criterion （0.8.2，2026-02-04）；https://github.com/criterion-rs/criterion.rs ；https://crates.io/crates/divan ；https://crates.io/crates/codspeed ；https://crates.io/crates/codspeed-criterion-compat

---

## 3. simdutf8 0.1.5

- **候选**：simdutf8 0.1.5（2024-09 发布后停滞，`aarch64_neon` 特性可用）
- **激进推荐**：❌ 不推荐
- **理由**：**本项目 Rust 侧没有 UTF-8 校验热路径**。PTY 输出不经 Rust UTF-8 解码——VT 解析在 vendored **Zig** 代码（`vendor/ghostty`）内，Rust 侧改不动；`from_utf8_lossy` 大量出现在测试代码（非热路径）；生产代码 UTF-8 处理只在 OSC 52/8（`osc_handler`，剪贴板粘贴/超链接）与 DECRQSS 查询参数，低频。强行接入等于为不存在的问题引入依赖。
- **来源**：https://crates.io/crates/simdutf8 ；https://github.com/rusticstuff/simdutf8

---

## 4. memchr 2.8.3

- **候选**：memchr 2.8.3（2026-07-08，crates.io 最新；**Cargo.lock 已锁定同版本**，aarch64 SIMD 内置）
- **激进推荐**：✅ 顺手用（**零新增成本**——已在依赖树，由 regex/ahash 等带入）
- **理由**：低频但真实存在的字节扫描点：`osc_handler` 内 payload 分隔符查找、`action_parser` 的查询参数切分。现用 `find(b';')`/`split` 等 std 方法（分支预测尚可但无 SIMD）。**注意边界**：不要试图用它加速 PTY 数据路径——那里无扫描需求（`session.rs:300` 直接 `to_vec()` 送通道）。
- **若推荐**：用在哪：`osc_handler.rs`/`action_parser.rs` 的分隔符定位（`memchr(b';', payload)`）。减多少代码：0（替换调用点）。提升多少性能：微（低频路径，非帧热路径）；收益是**顺手消除潜在慢路径**，不单独立项。风险：无。
- **来源**：https://crates.io/crates/memchr （2.8.3，2026-07-08）；https://github.com/BurntSushi/memchr

---

## 5. hashbrown 0.17 / ahash 0.8 / foldhash 0.2（hasher 替换）

- **候选**：hashbrown 0.17.1（2026-05，rust-lang 组织维护，已是 std 内部实现）；ahash 0.8.12（2025-05 后停滞）；foldhash 0.2.0（**已在 Cargo.lock，两处**：0.1.5 + 0.2.0）
- **激进推荐**：✅ **foldhash**（替代 SipHash hasher），**不引入** hashbrown/ahash
- **理由**：
  - std HashMap 自 1.36 起内部即 hashbrown SwissTable → 显式引入 hashbrown **无收益**。
  - 真实收益点是 hasher：`cell_builder.rs:158` 每帧 `HashMap<i32, Vec<&SearchHighlight>>` 构建 + 每格 lookup（~1920 次 hash/帧 @60fps ≈ 每秒 ~11 万次 SipHash13）。foldhash 是 hashbrown 0.15+ 的官方默认 hasher（**0 依赖、0 初始化开销**），比 ahash 更轻（ahash 需 getrandom 或 AES 路径，且 0.8.12 已一年未更新）。
- **若推荐**：
  - 用在哪：`cell_builder.rs` 的 `highlights_by_row` 一处（`HashMap<i32, Vec<&SearchHighlight>, FoldBuildHasher>`），及将来任何每帧/每事件 map。
  - 减多少代码：0（仅声明直接依赖 + 类型别名一行）。
  - 提升多少性能：该 map 单次操作从 SipHash13（i32 key ~10-20ns）降到 foldhash（~2-5ns），每帧节省 ~10-30µs（**帧时间 ~0.5-1%**，量级小但零成本）；真正的意义是**防止该热点随搜索高亮数量增长放大**。
  - 风险：极低（foldhash 已在本项目依赖树中实际运行——hashbrown 0.16/0.17 的默认 hasher 就是它）。
- **来源**：https://crates.io/crates/foldhash ；https://crates.io/crates/hashbrown （0.17.1）；https://crates.io/crates/ahash （0.8.12）；https://github.com/rust-lang/hashbrown

---

## 6. zerocopy 0.8.55 / 0.9.0-alpha

- **候选**：zerocopy 0.8.55（2026-07，活跃）；0.9.0-alpha.0（2024-10 后停滞）；**Cargo.lock 已锁定 0.8.54**（wgpu 30 带入，另有 zerocopy-derive）
- **激进推荐**：❌ 不迁移（bytemuck 已覆盖；0.9 alpha 无进展）
- **理由**：本项目零拷贝数据（`CellInstance`、`CellData`、`GpuUniforms`、`types.rs` 布局结构）已全部 bytemuck Pod+Zeroable，且 `ffi.rs` 的 `cast_slice` 边界已由 wgpu buffer 大小断言防护。zerocopy 相对 bytemuck 的增量是编译期布局验证（`Immutable+IntoBytes` derive），但：① 收益是防未来回归，不是修现存问题；② 迁移 = 所有 Pod derive 换皮 + 双栈共存期；③ 0.9 alpha 停滞一年，路线不明。**保留传递依赖即可，不主动用**。
- **来源**：https://crates.io/crates/zerocopy ；https://github.com/google/zerocopy

---

## 7. smallvec 1.15.2 / arrayvec / bumpalo（栈/帧分配）

- **候选**：smallvec 1.15.2（已在 Cargo.lock，cosmic-text 带入）；arrayvec（已在树）；bumpalo（已在树，wgpu 带入）
- **激进推荐**：⚠️ 部分推荐——**真正的激进项是 cell_builder 跨帧缓冲复用（零依赖）**；smallvec/bumpalo 为可选
- **理由**：
  - 每帧分配真实存在：`build_cell_instances` 每帧 `Vec::with_capacity`（~2000 项，~100KB/帧 @60fps ≈ 6MB/s 分配流量）——但这是 **with_capacity 的单次分配**，smallvec 不适用（容量远超栈内联上限）。
  - **激进方案 A（推荐，零依赖）**：把 `instances` 改为调用方传入的 `&mut Vec<CellInstance>`，`clear()` + `extend()` 复用跨帧 buffer——消除每帧分配，改动集中在 `cell_builder.rs` + 一处调用点（`render/mod.rs` 或 `renderer.rs`），约 10-20 行。
  - **激进方案 B（可选）**：bumpalo 帧竞技场（每帧 `reset`），彻底消除渲染帧内全部临时分配；但生命周期借用贯穿整个 cell_builder 函数，重构成本与心智负担最高。
  - smallvec：仅适用于 `shape_run` 结果（行内 shaping 输出通常 <32 项）等小向量；shaping 已有缓存复用，收益边际。
- **来源**：https://crates.io/crates/smallvec ；https://crates.io/crates/bumpalo

---

## 8. arc-swap 1.9.2

- **候选**：arc-swap 1.9.2（2026-06）
- **激进推荐**：❌ 不推荐（架构不匹配）
- **理由**：arc-swap 的适用场景是"多读者无锁读共享状态"（如渲染线程每帧读会话标题）。本项目网格快照走 **flume bounded(1) 通道 + Arc 消息**（`public_api.rs:302` `take_snapshot_with_scroll`），每帧**一次 recv、一次消费**，不存在多读者共享读；`ACTIVE_SESSION_ID` 已是原子镜像。引入 arc-swap 需要重写快照交付机制（通道 → 共享槽位），收益为 0（通道本身是零拷贝 Arc 传递），风险是破坏现有背压/超时语义（`internal.rs:299-323` 的 drain 逻辑依赖通道）。**不适用**。
- **来源**：https://crates.io/crates/arc-swap ；https://github.com/vorner/arc-swap

---

## 9. EVENT_QUEUE 激进化：Mutex<VecDeque> → flume unbounded

- **候选**：flume（已在直接依赖，保守版已用于其它通道）
- **激进推荐**：✅ 可选（收益小但改动小）
- **理由**：`event.rs:94` `Mutex<VecDeque<Event>>` 是全局锁（UI 线程 pollEvent pop，MCP/VT 线程 push）。换成 `flume::unbounded()`：删掉 Mutex + `last_overflow_warn` 逻辑可保留（flume 有 `len()`）。收益：去掉一个全局锁 + ~20 行锁样板；风险：Event 是无界队列（现 VecDeque 也是无界，语义等价）、需保持 `try_recv`/drain 行为（pollEvent 目前 drain 到 Vec 返回，flume 的 `drain()` 支持）。**注意保守版对 flume 通道的既有结论**——此处只是把"已接受的 flume"用于最后一个手写队列。优先级低于 §1（parking_lot 已覆盖该锁，两者取其一；激进建议 **parking_lot 保底、flume 可选**）。
- **来源**：https://crates.io/crates/flume ；https://github.com/zesterer/flume

---

## 10. 崩溃/日志激进项：panic hook（自研）vs log-panics 2.1.0 vs human-panic 2.0.8 vs sentry 0.49.0 vs android_logger 0.15.1

- **候选**：log-panics 2.1.0（2022 停滞）；human-panic 2.0.8（2026-04）；sentry 0.49.0（2026-07）；android_logger 0.15.1（2025-06）；**自研 panic hook（零依赖）**
- **激进推荐**：✅ **自研 ~15 行 panic hook**（`std::panic::set_hook` + `std::backtrace::Backtrace::force_capture()`，Rust 1.65+ 稳定，无需 RUST_BACKTRACE）；其余 ❌
- **理由（基于真实缺口）**：
  - **缺口确认**：`session.rs:263` reader 线程 `std::thread::spawn` **无 catch_unwind**；VT 线程/渲染线程有 catch_unwind 但只记录消息**无 backtrace**。panic 默认写 stderr，Android 上 **logcat 不可见** → 崩溃现场丢失。JNI 线程有 `jni_export_guard`，非 JNI 线程裸奔。
  - Kotlin 侧已有 `Thread.setDefaultUncaughtExceptionHandler`（`TerminalApp.kt:110`）但**只覆盖 Java/Kotlin 线程**，管不到 Rust 线程。
  - log-panics：功能正是"panic → log"，但 2022 年后停滞、自带 backtrace 需额外 feature；自研 15 行即可等值实现（项目已有自研 logger 先例，`logging.rs` 注释明示曾弃用 android_logger 自研——**延续该风格**）。
  - human-panic：面向 CLI 崩溃报告（打印报告 + 崩溃文件），Android 无控制台，不适用。
  - sentry：接入成本高（DSN、符号上传、native crash 配置），且 Kotlin 侧已自行处理崩溃日志；Rust 侧当前无"必须上报"的崩溃数据需求。
  - android_logger 回归：无必要——自研 logger 已实现 logcat+文件双写，回归仅为了 RUST_LOG 过滤，收益低。
- **若推荐**：用在哪：`logging.rs` 底部安装 hook（panic 消息 + force_capture backtrace → `log::error!` → logcat）。减多少代码：+15 行。提升多少性能：N/A（可观测性）；**价值**：所有 Rust 线程 panic 有现场（现为空白）。风险：无（hook 内只用 log + std，无递归风险）。
- **来源**：https://crates.io/crates/log-panics ；https://crates.io/crates/human-panic ；https://crates.io/crates/sentry ；https://crates.io/crates/android_logger

---

## 11. shuttle 0.9.1 / proptest 1.11.0 启用（已有依赖，零使用）

- **候选**：shuttle 0.9.1（2026-04-21，Cargo.lock 已锁即最新）；proptest 1.11.0（2026-03）
- **激进推荐**：✅ 启用（确认保守版 P1，补具体测试清单）
- **理由**：已有 dev-dependencies 声明却零使用 = 纯浪费。shuttle 并发测试能覆盖**本项目真实的锁序纪律**（`ffi.rs:52-54` 文档：SESSION_REGISTRY → Session → exit_code；EVENT_QUEUE 顺序），这种纪律目前只靠文档与代码审查维护。
- **若推荐——具体测试**：
  - proptest：
    1. `osc_52_payload_roundtrip`：任意字节序列 → OSC 52 base64 编码 → 解码 → 恒等（`osc_handler`）。
    2. `escape_sequence_no_panic`：随机生成 escape 字节流（`\x1b[...` 混合 UTF-8 边界）灌入 ghostty parser（`feed` + `take_snapshot`），断言不 panic、快照 CellData 的 codepoint 均为合法 `char`（配合现有 `debug_assert`）。
    3. `scroll_and_dimension_invariants`：随机 resize + 滚动序列后，网格行数/列数不变式（对照 `types.rs` 不变量）。
    4. `utf8_boundary_input`：以 1-3 字节 UTF-8 尾片边界切割输入流，断言无 panic、无数据丢失（对照 PTY 任意切块语义，`session.rs` reader 按 8KB 切块）。
  - shuttle：`event_queue_concurrent_push_pop`（多 push 线程 + drain 线程，断言事件不丢不重、无死锁）；`session_registry_lock_order`（模拟 SESSION_REGISTRY → Session 双锁顺序，验证无逆序死锁——shuttle 会随机调度暴露）。
  - 风险：低（仅测试依赖；host 上运行，Android 目标不受影响）。
- **来源**：https://crates.io/crates/shuttle （0.9.1）；https://crates.io/crates/proptest （1.11.0）

---

## 12. 其余激进选项复核

| 候选 | 版本/现状 | 激进结论 | 理由 |
|---|---|---|---|
| tracing 0.1.44 | 已在树（传递） | ❌ 维持保守版拒绝 | 无新论据；`log` + 自研 logger 已满足可观测性；tracing 订阅器在 Android 无查看端 |
| tokio-console 0.1.14 | 2025-10 | ❌ 不推荐 | 需 `tokio_unstable` + tracing + console-subscriber；MCP 服务器是低频路径；Android 上无终端查看 console |
| metrics-rs 0.24.6 | 2026-05 | ❌ 不推荐 | 无采集端（无 Prometheus/无统计消费者）；渲染帧时间等指标用现有 log 计数器即可 |
| simd-json 3.x | 活跃 | ❌ 不推荐 | 事件 JSON 负载 <1KB、低频（MCP 会话级），收益不可测 |
| mimalloc | ghostty 桌面同款 | ⚠️ 激进可选（高风险） | Android 上替换全局分配器（`#[global_allocator]`）可减少碎片/提升小对象分配；风险：NDK 行为差异、与 jemalloc 观测工具（如 perfetto 内存轨道）不兼容、回归难定位。**建议先跑 §2 基准再决定** |
| panic=abort / LTO profile | Cargo 配置 | ⚠️ LTO 可开；**panic=abort 禁止** | `panic="abort"` 与 JNI `catch_unwind`（`jni_export_guard`）**直接冲突**，会导致跨 FFI unwind UB；`lto="thin"`/`codegen-units=1` 对渲染热路径（cell_builder、shaping）有真实收益，非依赖项 |
| hashbrown 显式引入 | 0.17.1 | ❌ | std 已内置等价实现（1.36+） |

---

## 13. 激进引入清单（按收益/风险比排序）

| # | 项 | 收益 | 成本/风险 | 与保守版关系 |
|---|---|---|---|---|
| 1 | **自研 panic hook（零依赖，~15 行）** | 补上真实可观测性空白：reader 等非 JNI 线程 panic 现为静默丢失 | 无风险；+15 行 | 新增（激进） |
| 2 | **parking_lot 声明为直接依赖并替换 5 处 std 锁** | -100~150 行（删 poison 层）；锁路径更快 | 极低；0.12.5 已在树 | 保守版 P1 确认 |
| 3 | **criterion 0.8.2 基准框架**（dev-dep，default-features=false） | 手写 Instant 基准升级为统计基准 + `--failure-threshold` CI 防回归 | 编译时间 +1-2 分钟（dev-only）；迁移现有手写基准 | **修正保守版**（criterion 已恢复活跃） |
| 4 | **proptest/shuttle 启用**（具体测试清单见 §11） | 锁序纪律 + 解析器鲁棒性由测试固化 | 仅测试依赖 | 保守版 P1 确认 + 补清单 |
| 5 | **foldhash hasher**（cell_builder 每帧 map） | 每帧 ~2000 次 hash 提速 ~5-10×（帧时间 ~0.5-1%）；已在依赖树 | 2 行改动；零新增依赖 | 新增（激进微优化） |
| 6 | **cell_builder 跨帧缓冲复用**（`&mut Vec` 传入，零依赖） | 消除每帧 ~100KB 分配 | 重构 10-20 行；需回归渲染测试 | 新增（激进，性能） |
| 7 | **EVENT_QUEUE → flume unbounded** | 去掉最后一个全局 Mutex；-20 行 | 语义等价（现即无界）；与 #2 二选一优先 #2 | 新增（激进） |
| 8 | **codspeed-criterion-compat（CI 基准）** | 云端历史趋势 + PR 对比 | 需 CodSpeed 账号/App；CI 时间 | 新增（激进，可选） |
| 9 | **memchr 顺手用**（osc_handler/action_parser 分隔符） | 消除低频慢路径；零新增成本 | 无 | 新增（顺手） |
| 10 | **bumpalo 帧竞技场** | 彻底消除渲染帧内临时分配 | 生命周期重构成本最高 | 新增（激进，可选） |
| 11 | **mimalloc 全局分配器** | 碎片/小对象分配改善（桌面 ghostty 同款） | 高风险：NDK 兼容、观测差异；需基准验证 | 新增（激进，实验） |

## 14. 明确不引入

- **simdutf8** —— 无应用点（Rust 侧无 UTF-8 校验热路径；VT 解析在 vendored Zig）
- **arc-swap** —— 架构不匹配（快照走 flume 消息，非共享读）
- **zerocopy 迁移** —— bytemuck 已覆盖；0.9 alpha 停滞一年
- **ahash** —— foldhash 更轻（0 依赖）且已在树；ahash 0.8.12 一年未更新
- **hashbrown 显式引入** —— std 已内置等价 SwissTable
- **tracing / tokio-console / metrics-rs** —— 无消费者/查看端；维持保守版拒绝
- **sentry / human-panic / log-panics / android_logger 回归** —— 自研 panic hook + 既有自研 logger 更优（log-panics 2022 停滞）
- **simd-json** —— 负载小、低频
- **panic="abort"** —— 与 JNI catch_unwind 冲突（FFI UB）
- **dashmap / bytes / loom / rayon / crossbeam / tracing** —— 保守版已拒，无新论据
- **parking_lot 不稳定特性**（`deadlock_detection`/`hardware_lock_elision`）—— 生产禁用

## 15. 保守版结论需要更新的点（两版差异速查）

| 保守版结论 | 激进版修正 |
|---|---|
| criterion 停滞，P2 缓推 | **已修正**：criterion 0.8.2（2026-02）恢复活跃 + 新增 `--failure-threshold`；升级为激进推荐 #3 |
| parking_lot P1 | 确认，版本事实 0.12.5（2025-10），已在 Cargo.lock |
| proptest/shuttle P1 | 确认，补具体测试清单（§11） |
| 未覆盖崩溃可观测性 | **补**：panic hook 缺口（reader 线程无 catch_unwind、Android stderr 不可见） |
| 未覆盖 hasher/分配器 | **补**：foldhash（已在树）、cell_builder 跨帧复用、mimalloc（实验） |
| 拒绝 tracing/dashmap/bytes/loom/rayon/crossbeam | 维持（无新论据），并新增拒绝：tokio-console、metrics、simd-json、simdutf8、arc-swap、zerocopy 迁移 |

---

## 附：来源 URL

- parking_lot 0.12.5：https://crates.io/crates/parking_lot · https://github.com/Amanieu/parking_lot
- criterion 0.8.2：https://crates.io/crates/criterion · https://github.com/criterion-rs/criterion.rs
- divan 0.1.21：https://crates.io/crates/divan · https://github.com/nvzqz/divan
- codspeed 5.0.1 / codspeed-criterion-compat：https://crates.io/crates/codspeed · https://crates.io/crates/codspeed-criterion-compat · https://github.com/CodSpeedHQ/codspeed-rust
- simdutf8 0.1.5：https://crates.io/crates/simdutf8 · https://github.com/rusticstuff/simdutf8
- memchr 2.8.3：https://crates.io/crates/memchr · https://github.com/BurntSushi/memchr
- foldhash 0.2.0：https://crates.io/crates/foldhash
- hashbrown 0.17.1：https://crates.io/crates/hashbrown · https://github.com/rust-lang/hashbrown
- ahash 0.8.12：https://crates.io/crates/ahash
- zerocopy 0.8.55：https://crates.io/crates/zerocopy · https://github.com/google/zerocopy
- arc-swap 1.9.2：https://crates.io/crates/arc-swap
- smallvec 1.15.2：https://crates.io/crates/smallvec
- bumpalo：https://crates.io/crates/bumpalo
- shuttle 0.9.1：https://crates.io/crates/shuttle · https://github.com/awslabs/shuttle
- proptest 1.11.0：https://crates.io/crates/proptest · https://github.com/proptest-rs/proptest
- log-panics 2.1.0：https://crates.io/crates/log-panics
- human-panic 2.0.8：https://crates.io/crates/human-panic · https://github.com/rust-cli/human-panic
- sentry 0.49.0：https://crates.io/crates/sentry · https://github.com/getsentry/sentry-rust
- android_logger 0.15.1：https://crates.io/crates/android_logger
- tokio-console 0.1.14：https://crates.io/crates/tokio-console
- metrics 0.24.6：https://crates.io/crates/metrics
- flume：https://crates.io/crates/flume
