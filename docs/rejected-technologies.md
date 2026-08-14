# Rejected Technologies & Decisions

> 本文档是**决策状态登记处**：调研中发现但未采用的候选技术、特性与依赖，按状态归入三个分区——
> **§1 Rejected（明确拒绝）**、**§2 Deferred（暂缓/低优先）**、**§3 Absorbed（已吸收/已实现）**。
> 分区标题即语义，条目编号（R/D/A/S 前缀与纯数字）是稳定 ID，跨文档引用以此为准。
>
> 维护规则：
>
> - 新增拒绝项 → 记入 **§1**，注明出处（原 `docs/reference/` 内文件+章节或调研文档）与拒绝原因（优先"用户裁决"级绝对理由）。
> - 暂缓项 → 记入 **§2**，注明触发条件（满足后重新评估）。
> - 已实现/已吸收 → 移入 **§3**，注明吸收位置（代码注释或目标文档）。
> - 出处文件（`docs/reference/` 47 个 research/analysis 文件等）已删除，原文保留在 git 历史提交
>   `493fad5`（`git show 493fad5:docs/reference/<file>`）。

---

## 1. Rejected — 明确拒绝

决策已定、不重新评估的条目。理由以用户裁决或架构冲突（ADR）为绝对依据。

### 1.1 终端引擎/状态机层

| # | 拒绝项 | 出处 | 原因 |
|---|--------|------|------|
| 1 | termux Java 终端模拟器、termux-kotlin 的 Kotlin VT 状态机 | `research-termux-app-extra.md` §6、`research-termux-kotlin.md` §9 | libghostty-vt 已全面超越，维护两份状态机是纯负担 |
| 2 | 手写 VT 解析器（terminator/termx/onecode） | `research-mid-repos-a.md` §5.2 | 重复造轮子，正确性/性能均不如 libghostty-vt |
| 3 | zed 的 gpui 平台层、alacritty_terminal 引擎、Zed settings 体系 | `research-zed-port.md` §9.4 | 平台层与终端引擎均与 torvox 架构冲突 |
| 4 | fission 全家桶（widget 树/IR/布局引擎）、Bevy/Slint、winit/egui/OpenXR | `research-fission.md` §5.2、`research-mid-repos-a.md` §1.5、`research-wgpu-example.md` §8 | 通用 UI 框架引入巨大复杂度和依赖面；torvox 自研渲染管线更贴合终端需求 |
| 5 | Split Panes / Block 模型 | `research-all-projects.md` §P2、`research-warp-extra.md` §11 | Block 模型与 libghostty-vt 冲突，不建议 |

### 1.2 图形/渲染层

| # | 拒绝项 | 出处 | 原因 |
|---|--------|------|------|
| 6 | ash 直接 Vulkan（wgpu 30 已封装） | `research-warp.md` §7、`research-warp-extra.md` §11 | wgpu 30 已封装实例/适配器/管线，ash 直接操作收益低且易错 |
| 7 | 无限 LOD 网格、程序化几何生成器（raytracing）用于**生产渲染** | `research-wgpu-example.md` §6.1/§6.2 | 终端渲染不需要，需先加深度纹理等基础设施，纯娱乐。注：`native/src/render/procedural_geometry.rs` 已实现但 `render/mod.rs:29` 明确标注 `crate-test-only`（FR-056 off-screen render-verification 路径），生产管线 `pipeline.rs` 从不使用 → 拒绝的是**生产接入**，test-only 实现保留作为渲染验证工具 |
| 8 | CI sccache | `research-wgpu-example.md` §6.3 | 当前 CI 规模下收益不明显，且引入缓存失效调试成本 |

### 1.3 部署/安装层

| # | 拒绝项 | 出处 | 原因 |
|---|--------|------|------|
| 9 | bootstrap zip 的 sha256 sidecar 校验 | `research-warp-extra.md` §9.3、`research-warp.md` §5 | 用户明确不需要；bootstrap 通过 HTTP 下载，安装路径已有状态机保证原子性（BootstrapInstaller staging），sha256 校验增加部署复杂度且不解决核心风险 |
| 10 | bootstrap zip 内嵌离线安装 | `research-termux-app-extra.md` §5.7、`research-warp-extra.md` §11 | 用户明确不需要；禁止内嵌，bootstrap 必须支持**外部下载**和本地文件安装两种来源。注：下载通道为 **https-only**（`BootstrapDownloader.kt` 拒绝明文 http + redirect 跨协议守卫）——zip 会被 `.postinst` 执行，明文 http 可被 MITM 篡改；用户"支持 http 下载"的裁决语义为"支持网络下载（区别于内嵌）"，https 即其安全实现，不接受明文 http |
| 11 | ply 的 `curl \| sh` 无校验安装（反模式） | `research-small-repos.md` §2.5 | 安全反模式；torvox bootstrap 走独立安装器 |
| 12 | ~~多用户检查~~ **（误判，已实现）** | `research-termux-app-extra.md` §5.7 | 原裁决"与 torvox 单用户终端定位冲突"系误读：reference 指 TermuxInstaller.java:80-90 的**主用户检查**（防止 secondary user 安装破坏主用户数据），非"多用户支持"功能。torvox 已实现：`UserGuard.kt`（`installer/UserGuard.kt:17-47`，isSystemUser + UID 算术回退） |
| 13 | 跨仓库 path 依赖结构 | `research-warp-extra.md` §11 | torvox 单仓库 + generated-patches 更优，跨仓库破坏原子提交 |

### 1.4 网络/SSH 层

| # | 拒绝项 | 出处 | 原因 |
|---|--------|------|------|
| 14 | SSH + TOFU 全栈（russh/sshj） | `research-mid-repos-b.md` §2.5、§5.3 | 用户明确不需要（当前立项范围外）；TOFU 主机密钥管理复杂且易被绕过 |
| 15 | TermX 的 X11/VNC/SSH/SFTP 服务器、Cron、HTTP 服务器 | `research-mid-repos-a.md` §4.7、§5.2 | 反面教材：重复造轮子，游离于终端核心价值 |
| 16 | proot 用户态方案 | `research-other-repos.md` §3、`research-small-repos.md` §2.3 | torvox native ELF + linker 性能更优 |
| 17 | proot 发行版 | `research-mid-repos-a.md` §2.6、§5.2 | 当前不建议，与 Termux bootstrap 定位冲突；若做则复用 DistroRegistry→init.sh 骨架 |

### 1.5 安全/隐私层

| # | 拒绝项 | 出处 | 原因 |
|---|--------|------|------|
| 18 | MCP 同意门控弹窗（AgentConsentManager 模型） | `research-haven-extra.md` §2、§19（:333-364,:425） | 用户明确不需要；torvox 的 MCP server 开关即足够，弹窗打断流水线操作 |
| 19 | 隐私黑屏覆盖（切后台黑层防截屏） | `research-small-repos.md` §3.5、§5（:413,:427,:601,:610） | 用户明确不需要；与终端应用"切后台保持可见状态"的体验冲突 |
| 20 | 指纹锁（AppLock） | `research-mid-repos-a.md` §2.6 | 用户明确不需要；与终端快速切换体验冲突 |
| 21 | 指纹/隐私（悬浮窗终端、开机脚本） | `research-mid-repos-a.md` §5.2 | 用户明确不需要 |
| 22 | jni_fn 宏消除手写导出名风险 | `research-wgpu-in-app.md` §6-2（:101,:156,:176,:184,:188） | 用户明确不需要；torvox 手写导出名已有测试覆盖，宏引入第 3 方代码生成依赖 |
| 22b | TORVOX_BACKEND 环境变量 GPU 覆盖 | `research-wgpu-in-app.md` §6-2（app-surface_use_winit.rs:68） | 用户明确不需要；FR-010 强制 Vulkan 为唯一后端，不接受 GL/GLES/CPU 降级。`wgpu_backend.rs` 仅允许 `vulkan`/`primary` 且**显式拒绝 `gl`**，env 仅是 Vulkan 兜底 override，不作为可配置特性暴露 |
| 22c | TORVOX_POWER 环境变量功率偏好 | `research-wgpu-in-app.md` §6-3（lib.rs:371-372） | 用户明确不需要；功率偏好不影响 Vulkan 强制后端，不暴露为可配置特性 |

### 1.6 凭据/会话层

| # | 拒绝项 | 出处 | 原因 |
|---|--------|------|------|
| 23 | 会话元数据持久化/重启恢复 | `research-mid-repos-a.md` §3.6 | 用户明确不需要；终端会话重启恢复价值低，状态丢失风险高 |
| 24 | 输出导出到文件 | `research-mid-repos-a.md` §2.6 | 用户明确不需要；终端输出导出可通过重定向自行完成。审计发现代码已实现（SAF CreateDocument），经用户裁决后删除 |
| 25 | 粘贴确认对话框 | `research-gnome-console.md` §4 | 用户明确不需要；打断粘贴流水线。审计发现代码已实现（PasteChipOverlay + 多行对话框），经用户裁决后删除 |

### 1.7 UI/功能层

| # | 拒绝项 | 出处 | 原因 |
|---|--------|------|------|
| 26 | 标签条 / 顶部栏、主题编辑器、extra keys 宽度/副键/编辑器 | `research-ghostty-android-extra.md` §5-6/3.7、§7 | 用户明确不需要标签条/顶部栏；主题编辑器与 extra keys 编辑器属大工程，当前不做 |
| 27 | tmux 集成 | `research-mid-repos-b.md` §2.5 | 用户明确不需要；tmux 是外部工具，集成收益低且易碎 |
| 28 | SFTP 断点续传 | `research-mid-repos-b.md` §2.5 | 用户明确不需要；与 SSH 层一同拒绝 |
| 29 | 悬浮窗终端、开机脚本 | `research-mid-repos-a.md` §5.2 | 用户明确不需要 |
| 30 | AI 集成 | `research-warp-extra.md` §9.7、§11 | 用户明确不需要；warp_ai_mobile 为架构基准但当前不实现 |
| 31 | 自定义字体文件导入（TerminalFontStore 4 槽位文件选择） | `research-ghostty-android-extra.md` §5-1、§7 | **文件导入/私有存储复制**（TerminalFontStore.java）拒绝 —— Rust 侧渲染不接受 Android Typeface，文件导入需 native 存储管理，成本高收益低。**同族 bold/italic 面查找 + 像素合成**已吸收（见 §3.2 A8） |
| 32 | warp `stats_string` 逗号诊断串 + 驱动 grep 契约 | `research-warp-extra.md` §9、`research-warp-jni-ime.md` | 架构差异：torvox 用结构化 log + 截图测试 + Kotlin 单测锁定契约，不依赖 greppable 诊断串；引入会污染日志并耦合测试 |
| 33 | IME 空 finish 防双提交（warp 状态机） | `research-warp-extra.md` §11、`research-warp-jni-ime.md` | 架构差异：torvox Kotlin `InputCoalescer` 已在 `ComposingDiff.reconcile` 层去重，无需 Rust 侧空 finish 状态机 |
| 34 | TouchCancel 显式闭合状态机（warp lib.rs） | `research-warp-extra.md` §11、`research-warp-jni-ime.md` | torvox 触摸在 Compose/View 层，已处理 `ACTION_CANCEL`（`TerminalSurface.kt`）；Rust 侧无独立手势状态机需求 |
| 35 | Block 模型命令块 JSON（warp/zed-port） | `research-warp-extra.md`、`research-zed-port.md` §9 | torvox `terminal_info` 为元数据模型（exit_code/pid/cwd），无命令/输出块结构；非 MCP 需求，不引入 |
| 36 | `WGPU_BACKEND` 环境变量运行时后端切换 | `research-wgpu-in-app.md` §6-2 | FR-010 强制 Vulkan 为唯一后端；torvox 固定 `vulkan`/`primary` 兜底，不接受运行时 GL/GLES/CPU 切换（同 #22b 精神） |
| 37 | `log_panics` crate 引入 | `research-wgpu-in-app.md` §6 | 架构差异：torvox 自研 `set_hook` + `catch_unwind` 双保险（`android/logging.rs`）已覆盖 panic→logcat，不引第 3 方 crate |
| 38 | Popup 内 `startActionMode(TYPE_FLOATING)` 菜单（Haven FloatingTextInputDialog） | `research-haven-floating-input.md` §1.1 | 架构差异：Haven 在 `PopupWindow` 内调 `startActionMode` 会**静默 no-op**（:202-226）；torvox 选择菜单位于 View 层级（`TerminalSurface.kt` `startActionMode`），不受 Popup 限制，无此坑。记录防误用 |

### 1.8 Rust 依赖不引入（依赖选型研究，出处 `docs/dependency-research-*-aggressive.md`，原文已删除）

| # | 不引入项 | 原因 |
|---|---------|------|
| R1 | `tracing` | 全库 200+ 处 `log::*` 调用 + 自研 logcat `Log` 实现（`android/logging.rs`）已工作；上游 libghostty-vt/wgpu emit `log`；热路径日志已有纪律（`pty.rs` "DO NOT add log::debug! in hot path"）；tracing 仅作 Cargo.lock 传递依赖（axum/tower-mcp） |
| R2 | `dashmap` | 仅 2 个低频并发 map（SESSION_REGISTRY / REQUEST_REGISTRY），`RwLock<HashMap>` 无竞争；dashmap shard 内存开销且 `entry` API 与现有风格不兼容 |
| R3 | `bytes` | 字节流是单消费者线性管道（PTY reader → flume bounded(128) → VT），无共享切片需求；渲染侧 `Vec<CellInstance>` → bytemuck cast_slice 已零拷贝 |
| R4 | `loom` | shuttle 0.9 已选定（async 原生、无需 `cfg(loom)` 双写、活跃迭代）；loom 发版停滞且 async 支持有限 |
| R5 | `rayon` | 无适用并行点：`build_instances_from_cell_data` 单线程逐行、行间共享字形缓存；渲染瓶颈在 GPU 上传/字形缓存；Android 多核受散热约束 |
| R6 | `crossbeam` | flume 0.12 已覆盖全部通道需求（PTY 输出、ghostty 命令/查询、MCP 线程控制）；crossbeam 其余组件（epoch/deque）无使用场景 |
| R7 | `simdutf8` | Rust 侧无 UTF-8 校验热路径：PTY 输出不经 Rust 解码（VT 解析在 vendored Zig），生产 UTF-8 处理仅 OSC 52/8 与 DECRQSS 低频处 |
| R8 | `arc-swap` | 架构不匹配：网格快照走 flume bounded(1) 通道 + Arc 消息（`public_api.rs` take_snapshot_with_scroll），一次一读无多读者共享读；`ACTIVE_SESSION_ID` 已是原子镜像 |
| R9 | `zerocopy` 迁移 | 零拷贝结构已全部 bytemuck Pod+Zeroable（CellInstance/CellData/GpuUniforms），`cast_slice` 边界已有 wgpu buffer 大小断言；zerocopy 0.9 alpha 停滞一年路线不明 |
| R10 | `ahash` | foldhash 0.2 更轻（0 依赖、hashbrown 0.15+ 官方默认 hasher）且已在依赖树；ahash 0.8.12 一年未更新 |
| R11 | `hashbrown` 显式引入 | std HashMap 自 1.36 起内部即 hashbrown SwissTable，显式引入无收益 |
| R12 | `tokio-console` | 需 `tokio_unstable` + tracing + console-subscriber；MCP 服务器低频路径；Android 无终端查看 console |
| R13 | `metrics-rs` / `simd-json` | 无采集端/消费者；事件 JSON 负载 <1KB 且 MCP 低频，收益不可测 |
| R14 | `sentry` / `human-panic` / `log-panics` / `android_logger` 回归 | 自研 panic hook（`logging.rs` `install_panic_hook`）+ 自研 logcat logger 已覆盖；log-panics 2022 停滞；human-panic 面向 CLI；sentry 接入成本高且无上报需求 |
| R15 | `panic="abort"` | 与 JNI `catch_unwind`（`jni_export_guard`）直接冲突，跨 FFI unwind 属 UB |
| R16 | parking_lot 不稳定特性（`deadlock_detection`/`hardware_lock_elision`） | 生产禁用；parking_lot 0.12.5 仅以稳定特性引入（已替换 5 处 std 锁并删除 lock_util.rs poison 层） |
| R17 | `mimalloc` 全局分配器 | 高风险实验项：NDK 行为差异、与 jemalloc 观测工具不兼容、回归难定位；需先有基准再决定（未做，维持 std 分配器） |
| R18 | `memchr` 顺手用（osc_handler/action_parser 分隔符） | 激进版列为"顺手用（零新增成本）"，但实际未引入：低频路径（OSC 52/8、DECRQSS 参数切分）std `find`/`split` 已够，避免为微优化引入直依赖 |
| R19 | EVENT_QUEUE → flume unbounded | 激进版列为可选（"parking_lot 保底、flume 可选"）；parking_lot 已落地覆盖锁路径，自研 `EventQueue`（`event.rs`，Mutex + VecDeque，上限 1024 且 Exit 永不丢）语义保留，未迁移 |

### 1.9 Kotlin/Android 依赖不引入（出处同上）

| # | 不引入项 | 原因 |
|---|---------|------|
| K1 | accompanist（permissions / systemuicontroller） | SystemUI Controller 模块已废弃移除；权限仅 POST_NOTIFICATIONS 一处且官方 `ActivityResultContracts` 已覆盖；edge-to-edge 已落地 |
| K2 | kotlinx-datetime | 官方 experimental；8 处日期用法全是本地纯展示时间戳（`SimpleDateFormat` → 已替换为 java.time `DateTimeFormatter`，minSdk 33 全量可用且线程安全） |
| K3 | okio（单独引入） | OkHttp 5.4.0 传递依赖自带；FFI 数据搬运 `java.io` + `ByteArrayOutputStream` 已覆盖 |
| K4 | kotlinx-io | 0.x experimental（0.8→0.9 breaking）；InputBatchBuffer 手写核心是帧调度/executor/背压语义非字节缓冲；LogcatFileWriter 仅 ~30 行收益 |
| K5 | Decompose / Navigation 3 | 导航实为手写布尔 overlay（`showSettings` + Box，~50 行）两屏应用；navigation-compose 显式声明已删除（零 import，hilt-navigation-compose 传递依赖仍满足 `hiltViewModel()`） |
| K6 | App Startup | TerminalApp.onCreate 是强顺序安全语义（BootGuard/崩溃处理器必须先注册），Initializer 化不减少代码反而分散；1.2.0 两年未更新 |
| K7 | termux terminal-view / termux-shared | Java 终端渲染器与 Rust 引擎架构冲突（引入等于换引擎）；termux-shared 未作为独立 Maven artifact 发布 |
| K8 | Material3 1.5.0-alpha25 | 项目组件全为稳定 API 无过时用法；alpha 无必须特性（BOM 2026.06.01 即最新稳定） |
| K9 | coil-network-okhttp | 背景图为本地 content URI，不需要网络模块（避免连带 okhttp）；Coil 3.5.0 core 已引入 |
| K10 | Molecule 试点 | 激进版建议试点（25 个 stateIn 设置流 → 声明式聚合），但 2.2.0 近一年无新 release、社区小众、命令式事件（20+ 处 `_state.update`）无法接管；未实施，仅记录 |
| K11 | 手写 EXIF 修正 | 背景图解码已换 Coil 3.5.0（`coil-core`），EXIF 方向由 Coil 自动校正，无需手写 ExifInterface |

### 1.10 评估后不采用 / 条件性不适用（吸收自原 `docs/reference-deferred-items.md` D 项，原文已删除）

| 原 D | 项 | 决策 | 原因 / 处置 |
|------|----|------|------------|
| D1 | termlib 网格自适应字号二分 | 不实现 | 设计差异：用户显式字号（dp 步进）+ 捏合缩放（zelland 路径） |
| D4 | kitty APC 块尾 NUL 剥离 | 条件性不适用 | KGP 走外部解码 raw-RGBA（无 base64 图像流）；**未来启用 RustPngDecoder 时须在 feed 前补 memchr NUL 剥离**（`internal.rs` PNG 注释） |
| D7/D12 | mosh 协议文档化、root 会话 | 不实现 | mosh 与 SSH 族拒绝（§1.4 #14 精神）；单用户终端定位与越权能力冲突 |
| D8-D11/D13/D17 | 触摸 repr(C)、组合 trait、NVRAM 节流、livery 优先级、enabledWhen 门控 | 不采用 | 架构差异：Kotlin 手势层 / 单 crate cfg 分支 / DataStore / 主题链路已独立实现等价物 |
| D20/D32 | nix-on-droid zip / guix-bwrap 链 | 不实施 | termux bootstrap 定位（用户裁决禁止内嵌 zip），无用户需求 |
| D21 | MCP Tee 双消费 | 不实施 | 单消费者架构；MCP 读屏走 `dump_grid` 快照 |
| D25 | `:integration-tests` Gradle 模块 | 评估后不拆分 | androidTest 已承担跨模块用例（331 instrumented + Cucumber + Maestro） |
| D33 | 子像素渲染 | 已决策不实现 | 灰度 AA + FreeType hinting + raster_scale 超采样已对齐 Termux/Ghostty 观感 |
| D36 | MCP 工具集扩展（Haven 130+） | 不实施（P3） | 11 工具覆盖核心；扩展无用户需求，按需增量 |
| D37 | 主题批量同步、备份/恢复加密导出 | 不实施（P3） | 主题编辑器 + 256 色链路已有；备份与会话持久化拒绝（§1.6 #23 精神） |
| D38 | 插件架构、多通道分发、绑定生成器 | 不实施（P3） | 超出终端核心价值（§1.4 #15 精神）；手工 libghostty-vt 包装已稳定 |
| D39 | zed 方法论模板、warp vsync/DECSET1049、kgx needs-attention | 评估后不采用 | 已有 ADR/文档体系/thiserror；vsync 升级路径注释在 `TerminalRuntime.kt`；无多标签 UI |

## 2. Deferred — 暂缓 / 低优先 / 未实施

暂缓≠拒绝：满足触发条件后重新评估。P2/P3 为优先级标注。

### 2.1 功能暂缓（出处 `research-*.md`，原文已删除）

| # | 项 | 出处 | 说明 / 触发条件 |
|---|----|------|----------------|
| S1 | DECCKM 预推（应用启动前向 PTY 发送 `\e[?1h\e=`） | `research-zed-port.md` §9、`research-haven.md` §7 | P2：仅在直连 SSH/mosh 时减少首次方向键错位；torvox 当前无 SSH/mosh 直连，且 kitty 等远端已自行处理。未来接流式传输时再评估。**注：方向键本身的双套编码已实现**（`getMode` JNI 查 DEC private mode 1 + `TerminalInputEncoder.arrowSequence` SS3/CSI，2026 轮次修复 research-haven.md:141 P2 缺口） |
| S2 | OSC 133 语义段 Kotlin 侧消费（shell 集成提示） | `research-termlib.md`、`osc_handler.rs` | P2：Rust 已解析（`osc_handler.rs`/`output_processor.rs`）。**部分实现**：`getLastCommandOutput` JNI 导出 + MCP `last_command_output` 工具已接线（2026 轮次）；UI 的 prompt/input 高亮仍暂缓 |
| S3 | CellRun cell 级游程合并 | `research-termlib.md` | P2：已有行级脏缓存，cell 级合并为中等改动，性能收益有限 |
| S4 | Compose 键序列模式（Android IME 不适用） | `research-supplement-4.md` | P2：Android 软键盘无 Compose 键场景，低优先 |
| S5 | O(1) 滑动窗口 FPS 计数器（shashlik fps.rs） | `research-shashlik-extra.md` | P2：性能诊断工具；torvox 已有 `Instant::now` + 硬阈值基准（`render/` bench 测试），无需常驻 FPS 计数器 |
| S6 | emoji 位图渲染（warp Samsung>Noto 优先级 / NotoColorEmoji CBDT 解析） | `research-warp.md` §9、`research-mid-repos-a.md` §1.6 | 更小：`font/cjk.rs` `find_emoji_fallback_fonts` 已收集 NotoColorEmoji 族（`is_emoji_candidate_family` + 加分）但 swash 无法 outline color 字体，渲染时跳过 → 落到 db scan/.notdef。真位图渲染需 CBDT/CDT 解析或第二渲染路径，收益有限（终端 emoji 场景少） |
| S7 | 粘贴预览 8000 字符上限（gnome-console kgx-paste-dialog） | `research-gnome-console-extra.md` §2 | P3：torvox 多行/长内容确认对话框预览为 `take(200)+"…"`（`TerminalScreen.kt`）；kgx 用 8000 上限。200 已足够展示意图 |
| S8 | OSC 8 URI → 本地 path 解码（zed `try_osc8_url_to_path`） | `research-zed-port.md` §9.3、`research-zed-port-personal.md` | P2：OSC 8 超链接**打开**已实现（`ffi.rs hyperlinkAt` → `TerminalSurface.kt openLinkAt`）；URI→本地文件系统的 path 解码（`file://` + URL 解码 + 存在性检查）无 UI 承载点（点击文件打开编辑器不在路线图） |
| S9 | OSC 133 SemanticSegment 列范围 / promptId（termlib） | `research-termlib.md`、`04-termlib-features-adoption.md` | 更小：`getLastCommandOutput` JNI + MCP `last_command_output` 工具已实现字符串捕获（B/C 段文本+exit_code）；逐段列范围/promptId 结构无消费需求 |
| S10 | rin `ENV`/mkshrc 初始化 | `research-rin.md` §7、`research-mid-repos-a.md` | 更小/未核实：审计未定位对应符号；torvox 引导为 termux bootstrap + `HOME`/`PWD`/`LINES`/`COLUMNS` 注入，无 mkshrc 依赖 |
| S11 | `MouseModeTracker` 参考实现未接入生产输入路径 | `research-haven.md`、`research-mid-repos-a.md` | P3：`ui/MouseModeTracker.kt` 保留为 DECSET 参考实现（`activeMouseMode` 区分 1000/1002/1003、`altScreen` 字段），已写好但零生产调用点——生产路径用 JNI `Bridge.isAltScreenActive()`/`isAppCursorMode()`（`getMode`）直接查 libghostty 状态。保留作对照，不删除 |

### 2.2 未实施决策记录（原 D 项，暂缓/部分实现/未接入 CI）

| 原 D | 项 | 状态 | 说明 / 触发条件 |
|------|----|------|------------|
| D2 | termux 路径修复白名单 / apt wrapper 预案 | P2 预案 | 自有镜像与包名无此问题；未来复用 termux 官方仓库前先评估路径假设 |
| D3 | MCP `terminal_info` transcript 字段 | P2 未实现 | 需 `handle_exit` 打包 screen_contents；`screenshot` 工具可替代 |
| D6 | `$TMPDIR` 清理策略 | P2 未实现 | TMPDIR 已设置（`pty.rs` bootstrap/tmp 或 /data/local/tmp）；按天清理非紧急 |
| D18 | 依赖边界检查脚本 | P2 未实现 | 单 crate + 2 薄 workspace 成员，边界天然简单；`check-rust.nu` 覆盖 fmt/clippy/test，cargo tree 人工审查 |
| D22 | GMD aosp-atd CI | 未实施 | ATD 镜像缺系统应用，UiAutomator 跨应用用例仍需完整镜像 |
| D23 | cargo-llvm-cov 覆盖率自动化 | 未实施 | wgpu/GPU 分支噪音，手工审计（TESTING.md 基线）足够 |
| D26 | PIT 变异测试接 CI | 已注册未接 CI | `build.gradle.kts` pitestClasspath 1.25.8（AGP 9.x 兼容）；耗时未入常规 gate |
| D27 | `maestro/suites/` 3 文件零引用 | 待清理 | `scripts/*.nu` 零引用（grep 实证）；保留作人工套件或清理均非阻断 |
| D29 | DocumentProvider 完整 CRUD 全链验证 | 部分实现 | rename 已实现；delete/create/write 需 instrumented（SAF 系统 UI 调用链） |
| D30 | 480×854/360dp 小屏全矩阵验证 | 部分验证 | 布局适配 + 菜单 clamp 已做；设置页/搜索/抽屉/对话框全矩阵未逐项跑 |
| D31 | 手柄与 ActionMode 菜单时序 | 部分实现 | `TYPE_FLOATING` + `onGetContentRect` 已实现；hide/延迟 show 时序未逐条对齐 termux |

## 3. Absorbed — 已吸收 / 已实现 / 处置索引

已落地为代码或目标文档的条目，保留登记供对照，不再重新评估。

### 3.1 实现核对更正（曾标"未实现/待办"，代码核实已解决）

| 项 | 更正 | 证据 |
|----|------|------|
| D5 | getCwd /proc 实时刷新 | **本轮实现**：`session.rs` `cwd()` 优先 `read_proc_cwd(pid)`（`/proc/<pid>/cwd`，chdir 后立即可见），回退 OSC 7 缓存与 ghostty 查询；新增 2 测试（`read_proc_cwd_live_pid_returns_cwd` / `read_proc_cwd_invalid_pid_returns_none`） |
| D16 | 背景图"复制私有存储 + 失效自愈" | 代码已实现（文档失实）：`TerminalViewModel.setBackgroundImagePath` 将 `content://` 拷贝至 `filesDir/terminal_background` 后存储私有路径；`applyBackgroundImageFromPath` 加载失败且文件缺失时清空设置自愈（含 ghostty-android BackgroundImageStore 注释） |
| D15 | StreamGobbler 死锁警告 | 已记录：PTY 读路径全走 channel 背压（`session.rs` ReaderTask），无裸管道风险 |
| D19 | SmartTerminalClipboard 完整代理 | 有意采用 hook：`ClipboardAccess.kt:30` `smartCopyProcessor` + OSC 52 verbatim 直通（注释引用 Haven:407-430） |
| D28 | Lavapipe 颜色精度放宽 | 已文档化：`docs/standards/TESTING.md` Benchmarks 节（0.9→0.8，真机通过） |
| D34 | 动画逐帧视频验证 | 已用替代方案：日志级逐帧（`animateDpAsState` 内打印）+ 截图前后对比 |
| D35 | 菜单绝不遮挡严格度 | 已实现 termux 级语义：FloatingActionMode 锚定 + 边缘翻转 |
| D14 | warp 合成提交防抖 | 已覆盖：`ComposingDiff.reconcile` 已去重（§1.7 #33） |
| D24 | maestro 门槛集收敛 | 已覆盖：`test-emulator.nu --include-tags smoke,e2e` 22 flows 即门槛 |

### 3.2 已吸收功能（原 §8 A 系列，保留对照）

| # | 条目 | 吸收位置 |
|---|------|---------|
| A1 | wgpu_hal/naga 日志降噪 | `native/src/android/mod.rs` `module_filtered`（已实现） |
| A2 | 行级脏缓存 | `native/src/render/invalidation.rs`（已实现） |
| A3 | 捏合缩放 | `android/.../TerminalSurface.kt`（已实现） |
| A4 | 自定义主题链路 | `android/.../TerminalTheme.kt` / `SettingsRepository.kt`（已实现） |
| A5 | 多击选择（双击选词/三击选行） | `android/.../TerminalSurface.kt`（已实现） |
| A6 | 背景图（设置路径 + 复制私有存储 + 失效自愈，全部已实现） | `android/.../SettingsScreen.kt`（设置路径）；`TerminalViewModel.setBackgroundImagePath`（`content://` → `filesDir/terminal_background` 私有拷贝，见 ghostty-android BackgroundImageStore 注释）+ `applyBackgroundImageFromPath`（文件缺失时清空设置自愈）。复读曾误标"仅设置路径实现"，代码核实为完整实现（§3.1 D16） |
| A7 | 搜索覆盖层 + 防抖 | `android/.../TextSearchBar.kt`（已实现） |
| A8 | 同族 bold/italic 面查找 + 像素合成 | `native/src/render/font/pipeline.rs` `glyph_information_styled` + `resolve_style_face` |
| A9 | log_panics hook（panic → logcat） | `native/src/android/logging.rs` `install_panic_hook`（吸收自 wgpu-in-app） |
| A10 | 独立 bold/italic 族槽（族名级多族设置，ghostty-android TerminalFontStore 4 槽族名设计） | `native/src/render/font/pipeline.rs` `styled_font_ids` + `set_font_family_for_style`；`ffi.rs` `setFontFamilyForStyle`；`SettingsScreen.kt` `FontFamilySelectors`（regular/bold/italic 三选择器）；`FontUtils.kt` `FONT_SLOT_*`（+ 补全；**拒绝的仅是 §1.7 #31 文件导入形态**，族名级设置已实现） |

### 3.3 文档处置索引（吸收自原 `docs/review-status.md` §5.2/§5.3，原文已删除）

`docs/` 下历代**过程性文档**均已删除，内容逐份核对后吸收（无有用知识丢失）。
本文档（rejected-technologies）承载"拒绝/不实现/暂缓"决策；其余去向见下表：

| 内容类别 | 去向（现状即真相） |
|---------|------------------|
| 已实现功能 | 代码注释（各模块来源引用，含项目名+文件名+行号）；`docs/architecture.md` §5 关键决策表；ADR-0001…0012 |
| 明确拒绝 / 用户裁决不要 | 本文档 §1（编号 1–38）+ R/K 系列 + D 不采用系列 |
| 暂缓 / 低优先 / 未实施 | 本文档 §2（S1–S11 + D 暂缓系列） |
| 测试策略 / 覆盖率基线 / 性能基准 / 已知测试缺口 | `docs/standards/TESTING.md`（Test Pyramid & Coverage Snapshot + Benchmarks 节） |
| 可选/必选依赖评估（含版本与理由） | `docs/dependencies.md` §1 |
| 轮次验证证据 | 已删除（`*.png`/`*.mp4` 遵循 FR-055 与 .gitignore 从 git 历史清除）；修复点已进代码（如 `TerminalSurface.kt` `DRAWER_CLOSE_TAP_GRACE_NANOS`） |
| 评审历史 / 项目状态 | git log（52 thematic commits）+ 本文 §3.3 注；状态事实以代码与测试为准 |

已删除目录/文档：`docs/reference/`（47 个 research/analysis 文件）、openspec 工作区
（15 change + 43 spec）、`docs/progress/`、`docs/_audit/`、`docs/` 顶层 11 个临时文档、
`tasks/`、`docs/reference-projects.md`、`docs/reference-absorption-summary.md`、
`docs/performance.md`、`docs/project-health.md`、`docs/charter.md`、
`docs/reference-deferred-items.md`、`docs/review-status.md`、
`docs/optional-dependencies-evaluation.md`（内容已并入 `docs/dependencies.md`）、
`docs/tech-stack.md`（内容已并入 `docs/dependencies.md`）、
`docs/lessons/`（教训已并入 standards/与代码注释）、
OpenSpec 归档去向见注 2。

> 注 1：`docs/reference/` 47 个文件完整保留在 git 历史提交 `493fad5`（`git show
> 493fad5:docs/reference/<file>`）；其余已删文档保留在各自删除提交的父提交，可随时恢复对照。
> 注 2：OpenSpec 工作区（15 changes）已全部实现并归档，删除前逐项核对的吸收位置表
> 已随原文档删除——各 change 主题均已落在对应 ADR 与代码注释（ADR-0002…0009、
> `ffi.rs`/`mcp.rs`/`ComposingDiff.kt` 等），不再单独维护去向表。

---

## 变更记录

- 2026-08：**三区分区重构**——原 §1-§7（拒绝）/§7b（暂缓）/§7c（未实施）/§8b（依赖不引入）/§8（已吸收）/§9（处置索引）按决策状态合并为 §1 Rejected / §2 Deferred / §3 Absorbed；条目编号与 D 编号作为稳定 ID 保留；修 TESTING.md 对 "§4 LibAFL" 的断链引用。
- 历史变更（归档节选）：初版建立汇总 docs/reference 全部明确拒绝项；多轮措辞修正（#31/#39 字体面查找、"多字体族名设置"→"同族 bold/italic 面查找 + 像素合成"）；§8 已吸收条目改用 A 前缀独立编号；§8b 吸收四份依赖研究（R1-R19 + K1-K11）；TORVOX_BACKEND/TORVOX_POWER 经用户裁决登记为"明确不需要"（§1.5 #22b/#22c）。
