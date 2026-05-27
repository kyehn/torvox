# Torvox

本文件是 AI 编码智能体 (opencode, AI Agent, Cursor 等) 的持久上下文。
每个会话从这里开始。不要跳过。不要略读。

> **核心原则**: 将智能体视为需要清晰上下文的有能力初级工程师，而非魔法代码生成器。
> 它需要明确的指令、已知的约束、和可验证的目标才能正确工作。
> 缺少上下文时，停止并询问 — 不要猜测。

---

# 第一部分: 行为准则

以下准则偏重谨慎而非速度。对于琐碎任务，运用判断力。

## 1. 动手之前先思考

**不要假设。不要隐藏困惑。展示权衡。**

在实现之前:
- 明确陈述你的假设。如果不确定，先问。
- 如果存在多种解读，全部呈现 — 不要默默选择一种。
- 如果有更简单的方法，说出来。适当时予以反驳。
- 如果有什么不清楚的，停下来。指出困惑之处。问。
- 阅读 ADR。它们存在是为了防止错误决策。不要略读。

**危险信号**:
- 你准备写代码但还没读过 ROADMAP 当前阶段
- 你准备修改某个 crate 但没读过相关 ADR
- 你准备引入新依赖但没检查 ARCHITECTURE.md 的技术版本锁定
- 你准备实现功能但 ROADMAP 中该阶段尚不可达

## 2. 简单优先

**解决问题的最少代码。没有投机性代码。**

- 不实现未被要求的功能。
- 不为单次使用的代码创建抽象。
- 不添加未被请求的"灵活性"或"可配置性"。
- 不为不可能发生的场景编写错误处理。
- 不"以防万一"添加配置项、特征门、或泛型参数。
- 如果你写了 200 行而其实 50 行就够了，重写。

自问: "高级工程师会说这过度工程化了吗?" 如果是，简化。

**具体到本项目**:
- `torvox-core` 是 `no_std` — 不要引入需要 `alloc` 的功能除非绝对必要
- 不要提前实现未来阶段的骨架 — ROADMAP 说了何时做
- 不要为了"一致性"给一个 crate 加 feature flag 如果只有一种使用模式

## 3. 外科手术式修改

**只触碰你必须触碰的。只清理你自己造成的混乱。**

编辑已有代码时:
- 不"改进"相邻代码、注释、或格式。
- 不重构没有坏的东西。
- 匹配已有风格，即使你会用不同方式。
- 如果注意到无关的死代码，提及它 — 不要删除它。
- 不要在同一 PR 中混合功能变更和风格变更。

当你的修改产生孤儿代码时:
- 移除 YOUR 修改导致不再使用的 imports/变量/函数。
- 不要移除预先存在的死代码除非被要求。

**检验标准**: 每一行变更都应该能追溯到用户的请求。

**具体到本项目**:
- 修改 `torvox-core` 类型时，同步更新 `bridge.rs` 的桥接类型
- 修改 Rust 公共 API 时，检查 boltffi 生成绑定是否需要重新生成
- 修改 Cargo.toml 依赖版本时，确认与 ARCHITECTURE.md 版本锁定一致

## 4. 目标驱动执行

**定义成功标准。循环直到验证通过。**

将任务转化为可验证的目标:
- "添加校验" → "为无效输入写测试，然后让测试通过"
- "修复 bug" → "写一个能重现它的测试，然后让测试通过"
- "重构 X" → "确保重构前后测试都通过"

多步任务，陈述简要计划:
```
1. [步骤] → 验证: [检查项]
2. [步骤] → 验证: [检查项]
3. [步骤] → 验证: [检查项]
```

强成功标准让你能独立循环。弱标准 ("让它工作") 需要不断确认。

**每步验证流程**:
1. 写代码 → `cargo clippy -- -D warnings` 零警告
2. 写测试 → `cargo nextest -p <crate>` 通过
3. 提交前 → `cargo nextest --workspace` 全量通过
4. 完成阶段 → `nu scripts/quality-gate.nu` 通过

## 5. 不确定时停下

**编造 API 比承认不知道更危险。**

以下情况必须停止并询问:
- 文档之间信息冲突 (如 ARCHITECTURE.md 与代码不一致)
- 不确定某个函数是否应该 `pub` 还是 `pub(crate)`
- 不确定新类型该放在哪个 crate
- 不确定某个依赖是否已被项目批准
- 不确定修改是否会破坏 boltffi 绑定

---

**这些准则有效的标志**: diff 中不必要的变更更少，过度工程化导致的重写更少，
澄清问题出现在实现之前而非错误之后。

---

# 第二部分: 项目标识

- **名称**: Torvox — Android 终端模拟器
- **定位**: 从零构建的现代终端 — GPU 渲染、Rust 引擎、AI 优先
- **语言**: Rust (引擎) + Kotlin (Android UI)
- **架构**: 库优先分层。Rust crate 在 `torvox-*` 下，Kotlin 应用在 `android/app/`
- **目标平台**: Android 13+ (minSdk 33, targetSdk 36)，arm64-v8a + x86_64
- **阅读者**: 仅作者和 AI，无第三方贡献者
- **开发方法**: 规范驱动开发 (SDD) — 先写规范，再 AI 生成实现
- **许可证**: UNLICENSED — 仅作者和 AI 使用，不添加 license 声明

## 设计哲学 (摘要，详见 VISION.md)

1. **库优先** — `torvox-core` 零 Android 依赖，可嵌入任何应用
2. **GPU 从第一天起** — wgpu v29，不存在 CPU 回退路径
3. **用户与像素之间无 VM** — Rust 热路径，无 GC 暂停，无 JIT 预热
4. **AI 代理作为一等公民** — 结构化 JSON-RPC 会话协议
5. **精确优于兼容** — 顶级 VT 标准合规
6. **规范先行，AI 生成其次** — 先写规范再写代码

---

# 第三部分: 关键文件

## 必读文档

每个新会话必读。按重要性排序:

| 文件 | 重要性 | 用途 | 何时阅读 |
|------|--------|------|----------|
| `AGENTS.md` (本文件) | ★★★★★ | AI 智能体首要上下文 | 每个会话开始 |
| `docs/WORKFLOW.md` | ★★★★★ | SDD 工作流、状态管理、提交规范、质量门禁 | 每个会话开始 |
| `docs/ROADMAP.md` | ★★★★★ | 当前阶段、里程碑步骤、退出标准 | 开始任何工作之前 |
| `docs/ARCHITECTURE.md` | ★★★★★ | 完整架构、crate 结构、数据流、线程模型、技术版本锁定表 | 修改任何 crate 时 |
| `docs/SPECIFICATION.md` | ★★★★★ | VT 标准覆盖、性能目标、合规测试 | 实现 VT 功能或优化时 |
| `docs/ADR/004-pty-implementation.md` | ★★★★★ | 为什么 nix crate forkpty、不用 portable-pty、W^X 方案 | 修改 PTY 时 |
| `docs/ADR/006-testing-strategy.md` | ★★★★★ | 五层测试策略、模糊测试、属性测试、MIRI | 写任何测试时 |
| `docs/VISION.md` | ★★★★★ | 项目愿景和设计哲学 | 首次接触项目时 |

## 按需阅读文档

| 文件 | 重要性 | 用途 | 何时阅读 |
|------|--------|------|----------|
| `docs/ADR/001-language-choice.md` | ★★★☆☆ | 为什么 Rust+Kotlin 混合 | 质疑语言选择时 |
| `docs/ADR/002-architecture-pattern.md` | ★★★☆☆ | 为什么库优先分层、事件驱动、crossbeam 而非 tokio | 修改架构时 |
| `docs/ADR/003-rendering-pipeline.md` | ★★★☆☆ | 为什么 wgpu v29 + cosmic-text + swash/skrifa + 实例化四边形 | 修改渲染时 |
| `docs/ADR/005-ai-workflow-and-tooling.md` | ★★★☆☆ | AI 工作流、SDD 方法论 | 规划 AI 协作流程时 |
| `docs/DEVELOPMENT.md` | ★★★☆☆ | 构建步骤、命令、CI/CD、开发工作流 | 构建或调试问题时 |

## 关键代码文件

| 文件 | 用途 | 关键注意 |
|------|------|----------|
| `Cargo.toml` (workspace root) | 所有 Rust crate 依赖 | 版本变更须同步 ARCHITECTURE.md |
| `android/settings.gradle.kts` | 所有 Android 模块 | 新模块须在此注册 |
| `torvox-core/src/cell.rs` | Cell, Attrs (10 个 SGR 字段), Color, DirtyMask | `no_std` 兼容 |
| `torvox-core/src/config.rs` | TerminalConfig, Shell (SystemDefault/Custom(String)), RenderConfig, FontConfig | Shell 是 Clone 不是 Copy |
| `torvox-core/src/grid.rs` | Grid, DirtyMask (Vec<u64> 分区位标志) | 任意行数 |
| `torvox-terminal/src/pty.rs` | PtyPair (spawn, resize, Read/Write, Drop) | 唯一允许 fork unsafe 的位置 |
| `torvox-gui-android/src/bridge.rs` | boltffi 导出类型 + TorvoxBridge | 唯一允许导出的位置 |
| `torvox-gui-android/uniffi.toml` | boltffi 配置文件 | 已移除 (boltffi 不需要配置文件) |
| `scripts/build-android-libs.nu` | cargo-ndk 交叉编译 + torvox-exec 构建 | 替代 rust-android-gradle |
| `scripts/quality-gate.nu` | 8 步质量门 | 提交前必须通过 |
| `flake.nix` | 完整 Nix 开发环境 | 使用 flake-parts，需要 allowUnfree |

---

# 第四部分: 当前状态

- **阶段**: 1 完成 (终端引擎) — P0.1–P0.6, P1.1–P1.6 全部完成
- **下一步**: P2.1 回滚缓冲UI (Grid scrollback已实现, Kotlin触摸滚动UI待完成)

## 阶段 0 完成内容

| 里程碑 | 交付物 | 状态 |
|--------|--------|------|
| P0.1 | Rust workspace 8 个 crate + Cargo.toml | ✅ |
| P0.2 | `torvox-core` 完整类型系统 (9 模块, no_std) | ✅ |
| P0.3 | Android 项目 + Kotlin + Hilt + Compose | ✅ |
| P0.4 | 文档 + CI + 质量门 | ✅ |
| P0.5 | PtyPair (spawn, 原始模式, 非阻塞, kill_on_drop) + W^X 多调用二进制 | ✅ |
| P0.6 | boltffi 桥接验证 (bridge.rs + 生成 Kotlin 绑定) | ✅ |

## 阶段 1 待完成内容 (详见 ROADMAP.md)

| 里程碑 | 交付物 | 状态 |
|--------|--------|------|
| P1.1 | VT 解析器 (libghostty-vt 0.1, SIMD优化) | ✅ 完成 |
| P1.2 | PTY 会话集成 (crossbeam SPSC) | ✅ 完成 |
| P1.3 | 字体管线 (fontdb → cosmic-text → swash/skrifa → etagere) | ✅ 完成 |
| P1.4 | GPU 渲染管线 (实例化四边形, WGSL 着色器) | ✅ 完成 |
| P1.5 | Android Surface 渲染 (wgpu v29 SurfaceView) | ✅ 完成 |
| P1.6 | 输入处理 (触摸/键盘 → VT 转义序列 → PTY 写入) | ✅ 完成 |

## 当前代码状态

| 组件 | 状态 | 说明 |
|------|------|------|
| `torvox-core` (9 模块) | **完整** | Cell, Attrs (10 SGR), Color, DirtyMask (Vec<u64>), Grid (含 scrollback + GridSnapshot trait), Line, Config, Cursor, Selection, Unicode, Event, Ansi |
| `torvox-terminal/pty.rs` | **完整** | PtyPair: spawn, resize, read/write, Drop (增量终止), 非阻塞, 4 个 Linux 测试 |
| `torvox-terminal/parser.rs` | **完整** | VtParser 包装 vte::Parser, advance 方法 |
| `torvox-terminal/terminal.rs` | **完整** | TerminalState + vte::Perform impl, 76 测试 (含 proptest), 就地 insert/delete chars |
| `torvox-terminal/session.rs` | **完整** | Session orchestrator: PtyPair + TerminalState + parser + crossbeam channel + Condvar 事件通知, 5 个集成测试 |
| `torvox-terminal/keyboard.rs` | **完整** | InputEngine: Kitty 协议 + VT 传统编码 + 鼠标 SGR, 43 个测试 |
| `torvox-renderer` | **完整** | FontPipeline (fontdb+cosmic-text+swash+guillotiere, 7 测试), GpuContext (wgpu v29, 缓存 instance_buffer, cell.wgsl, cursor.wgsl, 3 测试), Android Surface 支持 (set_surface_from_native_window), 已移除空壳 atlas.rs/pipeline.rs, 无 torvox-terminal 依赖 |
| `torvox-gui-android/bridge.rs` | **完整** | BridgeCell(+BridgeAttrs), Shell(Enum), TerminalConfig, TerminalEvent(8变体), TerminalError(detail), TorvoxBridge; From/Into 转换 core 类型 |
| `torvox-gui-android/surface.rs` | **完整** | AndroidSurface: GPU 管线 + 字体 + 终端状态, set_native_window, render (使用 GridSnapshot) |
| `torvox-exec` | **完整** | argv[0] 多调用二进制, 符号链接模式 + 直接调用模式 |
| `torvox-fuzz` | **空** | 仅有 src/lib.rs 存根 |
| `torvox-integration-tests` | **空** | 仅有 src/lib.rs 存根 |
| `torvox-bench` | **空** | 仅有 src/lib.rs 存根 |
| Android Kotlin | **壳** | TorvoxApp, MainActivity, TerminalViewModel, TerminalScreen (占位), ForegroundService, ExecInstaller |

## 已删除/合并的组件

- `torvox-bridge-types/` — **已删除**。boltffi 库模式只允许一个导出位置，
  跨 crate derive 会导致 Kotlin 生成重复脚手架。所有 boltffi 类型合并到
  `torvox-gui-android/src/bridge.rs`。文档中如仍引用此 crate 视为过时。

---

# 第五部分: 关键约束

## 绝对禁止

```
┌────────────────────────────────────────────────────────────────────────────┐
│ 以下约束源自 ADR 决策或已验证的技术限制。违反意味着架构错误。              │
│ 不要"只是试试"违反它们。如果你认为某个约束过时了，先提 ADR 再改。        │
├────────────────────────────────────────────────────────────────────────────┤
│                                                                            │
│ 【语言与依赖】                                                             │
│ ✗ 添加 Java 文件 — 仅 Kotlin                                              │
│ ✗ 依赖 Termux — Torvox 是独立项目                                         │
│ ✗ 使用 portable-pty — 不支持 Android，用 nix 0.31 crate                  │
│ ✗ 使用 bincode — 已废弃 (RUSTSEC-2025-0141)，用 serde derive            │
│ ✗ 使用 rust-android-gradle — AGP 9.0+ 移除 AppExtension，不兼容          │
│   用 scripts/build-android-libs.nu (cargo-ndk v4) 代替                    │
│ ✗ 在库 crate 中使用 anyhow — 库用 thiserror 2，仅二进制可用 eyre         │
│ ✗ 在 boltffi Error 枚举中使用 `message` 字段名 — 与 Kotlin               │
│   Throwable.message 冲突。改用 `detail`                                   │
│ ✗ 添加 license 声明 — 本项目 UNLICENSED，仅作者和 AI 使用                 │
│                                                                            │
│ 【架构与安全】                                                             │
│ ✗ 在 VT 解析器中添加 `unsafe` — 需要类型安全解析器                        │
│ ✗ 在 torvox-core 中添加 `unsafe` — 零 unsafe crate                       │
│ ✗ 在多个 crate 中使用 setup_scaffolding!() — boltffi 库模式              │
│   只允许一个。所有 boltffi 类型放 torvox-gui-android/src/bridge.rs       │
│ ✗ 使用 Canvas.drawText 逐单元格 — 仅 GPU 渲染 (wgpu v29)                │
│ ✗ 在 FFI 边界传递原始字节 — 传递结构化事件 (boltffi #[data]/#[error])  │
│ ✗ 使用 /proc/self/exe 确定多调用二进制身份 — 用 argv[0] 的               │
│   file_name()。/proc/self/exe 解引用符号链接，丢失名称                    │
│                                                                            │
│ 【开发流程】                                                               │
│ ✗ 无规范编码 (vibe coding) — 必须先写规范再实现 (SDD)                     │
│ ✗ 忽略测试 — 每个公共函数需要单元测试                                     │
│ ✗ 一次修改 10+ 文件 — 分步修改，每步验证                                  │
│ ✗ 略读 ADR — 阅读它们。它们存在是为了防止错误决策                         │
│ ✗ 使用 torvox-bridge-types crate — 已删除，类型在 bridge.rs 中           │
│                                                                            │
├────────────────────────────────────────────────────────────────────────────┤
│                                                                            │
│ 【必须遵守】                                                               │
│ ✓ 每个有意义步骤后 cargo clippy -- -D warnings                            │
│ ✓ 每个阶段完成后 cargo nextest --workspace                                │
│ ✓ 保持 AGENTS.md 更新 (新约定、新工具、状态变化)                         │
│ ✓ 逐阶段优先于逐函数 — 完成阶段退出标准再往下走                          │
│ ✓ 不确定时询问 — 不要编造 API、版本号、或 crate 结构                     │
│ ✓ 技术版本见 ARCHITECTURE.md 技术版本锁定表 — 不要自行升级               │
│ ✓ 新 crate → 更新 ARCHITECTURE.md + Cargo.toml workspace                 │
│ ✓ 新 Android 模块 → 更新 settings.gradle.kts                             │
│ ✓ 修改 Rust 公共类型 → 检查 bridge.rs 是否需要同步更新                   │
│ ✓ 修改 bridge.rs → 重新生成 Kotlin 绑定                                  │
│ ✓ 修改 Cargo.toml 依赖版本 → 确认与 ARCHITECTURE.md 版本锁定一致         │
│ ✓ 新 unsafe 块 → 在注释中记录安全不变量                                  │
│                                                                            │
└────────────────────────────────────────────────────────────────────────────┘
```

## 已知陷阱与教训

以下是从过往会话中学到的具体技术教训。避免重蹈覆辙:

| # | 陷阱 | 教训 | 来源 |
|---|------|------|------|
| 1 | `Shell::Custom(u8)` 存储路径 | `u8` 太小，改为 `Shell::Custom(String)`。Shell 失去 `Copy`，TerminalConfig 也失去 `Copy` | P0.2 审计 |
| 2 | `DirtyLine` 枚举 vs 位掩码 | ARCHITECTURE.md 指定位掩码。实现为 `DirtyMask { partitions: Vec<u64> }`，每 u64 分区 64 行，支持任意行数 | P0.2 审计 → C4 → 本次修复 |
| 3 | `thiserror 2.x` 与 `no_std` | `torvox-core` 是 `no_std`。`thiserror` 需要 `std` feature。方案: `thiserror` 设为 optional，`std` feature 启用它；`serde/postcard` 也需 `default-features = false` + `alloc` feature | P0.2 审计 |
| 4 | boltffi `setup_scaffolding!()` 多 crate 冲突 | boltffi 库模式只允许一个导出位置。跨 crate derive 导致 Kotlin 重复脚手架 | P0.6 |
| 5 | boltffi Error `message` 字段与 Kotlin 冲突 | Kotlin `Throwable.message` 与 boltffi 生成的 `message` 字段冲突。改用 `detail` | P0.6 |
| 6 | `libtorvox_core.so` 命名冲突 | lib 名与 `torvox-core` Rust crate 冲突。改为 `libtorvox_android.so` | C1 审计 |
| 7 | `/proc/self/exe` 不保留符号链接名 | 解引用到真实二进制。用 `argv[0]` 的 `file_name()` 代替 | ADR 004 |
| 8 | `rust-android-gradle` 与 AGP 9.0 不兼容 | AGP 9.0 移除了 `AppExtension`。用 `scripts/build-android-libs.nu` 代替 | P0.3 |
| 9 | `std::env::set_var` 在 Rust 1.95 是 unsafe | 需要 `unsafe` 块包裹 | 构建 |
| 10 | `nix::fcntl::fcntl` 在 nix 0.31 接受 `AsFd` 而非 `RawFd` | API 变更，注意类型转换 | P0.5 |
| 11 | `OwnedFd::from_raw_fd` + `mem::forget` 模式 | 用于在借用 fd 上操作 termios，防止 drop 关闭 fd | P0.5 |
| 12 | 一个枚举中多个 `#[from] nix::errno::Errno` 冲突 | 产生冲突的 `From` impl。用手动 `From` impl 代替 | P0.5 |
| 13 | `cargo-ndk` 仅支持 cdylib | `torvox-exec` 是 `[[bin]]`，不能通过 cargo-ndk 构建。用 `CARGO_TARGET_*_LINKER` 环境变量直接构建 | P0.5 |
| 14 | `Result<T, String>` 不被 boltffi 支持 | 必须使用 `#[error]` 枚举 | P0.6 |
| 15 | `#[error]` 枚举在 boltffi 中仅适用于枚举 | 不适用于结构体 | P0.6 |
| 16 | `DirtyMask(Vec<u64>)` 不再是 `Copy` | 含 Vec 的类型不能 Copy。DirtyMask 失去 Copy，需 Clone。Grid 的 Clone 仍可用 | P0.2→本次修复 |
| 17 | bridge.rs `shell: String` 无法表达 "系统默认" | 改为 `Shell` 枚举 (SystemDefault/Custom)，与 core 的 Shell 对齐，避免空字符串 hack | 本次修复 |

---

# 第六部分: 构建命令

```bash
# ── Nix 开发环境 (推荐) ──────────────────────────────
nix develop                                        # 进入 devshell
nix develop --command cargo build --workspace      # 直接构建

# ── Rust 构建与测试 (需已安装 Rust stable + cargo-nextest) ──
cargo build                                        # Debug 构建 (workspace)
cargo build -p torvox-core                         # 单 crate 构建
cargo build -p torvox-core --no-default-features --features alloc # no_std 构建 (验证无 std)
cargo nextest --workspace                          # 全部测试
cargo nextest -p torvox-core                       # 单 crate 测试
cargo clippy -- -D warnings                        # 零警告 (必须)
cargo fmt --check                                  # 格式检查

# ── Android 构建 ─────────────────────────────────────
./scripts/build-android-libs.nu                    # 交叉编译 Rust → Android .so + exec
cd android && ./gradlew assembleDebug              # 构建 APK (需要先运行上方脚本)
cd android && ./gradlew lint                       # Kotlin lint
cd android && ./gradlew test                       # Kotlin 测试

# ── boltffi 绑定生成 ──────────────────────────────────
# 先构建 cdylib，再生成 Kotlin 绑定:
cargo build -p torvox-gui-android
boltffi pack android \
  target/debug/libtorvox_android.so \
  --language kotlin \
  --output-dir android/app/src/main/java/io/torvox/bridge/

# ── 质量门 ───────────────────────────────────────────
nu scripts/quality-gate.nu                          # 8 步全量质量门

# ── 安全检查 ─────────────────────────────────────────
cargo geiger                                       # 检查 unsafe 使用
cargo audit                                        # 检查已知漏洞

# ── no_std 验证 ──────────────────────────────────────
cargo build -p torvox-core --target thumbv6m-none-eabi --no-default-features --features alloc
```

完整命令见 `docs/DEVELOPMENT.md`。

## Nix DevShell

项目包含 `flake.nix`，提供完整开发环境:

| 工具 | 版本 | 说明 |
|------|------|------|
| Rust (fenix stable) | 1.95 | rust-toolchain.toml 锁定 |
| cargo-nextest | 0.9.136 | 替代 cargo test |
| cargo-fuzz | 0.13 | 模糊测试 |
| cargo-geiger | 0.13 | unsafe 审计 |
| cargo-audit | 0.22 | 漏洞扫描 |
| cargo-ndk | latest | Android 交叉编译 |
| rust-analyzer | latest | IDE 支持 |
| JDK (Temurin) | 25 | Android 构建 |
| Kotlin | nixpkgs latest | |
| Gradle | 9 | |
| ktfmt | nixpkgs latest | Kotlin 格式化 |
| ktlint | nixpkgs latest | Kotlin lint |
| Android SDK | platform 36/33, build-tools 36.0.0, cmdLineTools 16.0 | |
| Android NDK | 29.0.14206865 (r29) | |
| android-tools | nixpkgs latest | adb 等 |

使用 `flake-parts` (非 `flake-utils`)。Android SDK 需要 `allowUnfree`。

```bash
nix develop                    # 进入 shell
nix develop --command cargo nextest  # 直接运行
```

---

# 第七部分: 技术版本锁定

完整版本表见 `docs/ARCHITECTURE.md`。以下是最关键的锁定项:

| 技术 | 版本 | 注意 |
|------|------|------|
| Rust edition | 2024 | workspace 强制 |
| Rust MSRV | 1.95 | package.rust-version |
| wgpu | 29 | Surface API 变更: InstanceDescriptor.display 是 Option |
| vte | 0.15 | VT 解析器 (Paul Williams 状态机) |
| nix | 0.31 | PTY (openpty/fork/ioctl) |
| cosmic-text | 0.19 | 文本整形 |
| swash | 0.2.7 | 字体光栅化 (内部用 skrifa 做 scaling) |
| etagere | 0.3 | 字形图集打包 |
| boltffi | 0.25 | Kotlin 绑定生成 |
| postcard | 1.1 | 已移除 (原用于测试序列化) |
| thiserror | 2 | 错误类型 (torvox-core 中 optional) |
| tokio | 1.43 | 仅异步运行时 (热路径用 crossbeam) |
| crossbeam | 0.8 | 无锁 SPSC 队列 (PTY→解析器) |
| proptest | 1.11 | 属性测试 |
| AGP | 9.0.1 | |
| Kotlin | 2.3.21 | compose 插件 |
| Compose BOM | 2026.05.00 | |
| Hilt | 2.59.2 | 需 AGP 9.0+ |
| KSP | 2.3.9 | 替代 kapt |
| Gradle | 9 | |
| JDK | 25 (Temurin) | |
| NDK | 29.0.14206865 (r29) | |
| minSdk / targetSdk | 33 / 36 | Android 13+ / 16 |

**不要自行升级版本**。如需变更，先更新 ARCHITECTURE.md 版本锁定表，再改 Cargo.toml / build.gradle.kts。

---

# 第八部分: 约定

## Rust

| 约定 | 规则 | 理由/注意 |
|------|------|-----------|
| Edition | 2024 | workspace 强制 |
| 格式化 | `cargo fmt` 强制 | 不合规不合并 |
| Clippy | `--deny warnings` 每 PR 必需 | 零警告策略 |
| `unsafe` | `torvox-core` 中 **零**。仅 `torvox-gui-android` FFI 桥接和 `torvox-terminal::pty` 中 | 每个 unsafe 块注释安全不变量 |
| 错误处理 | `thiserror 2` (库) + `eyre` (二进制)。**库 crate 中无 `anyhow`** | thiserror 在 torvox-core 中 optional (需 std feature) |
| 序列化 | `serde derive` (不用 bincode/postcard，已废弃) | |
| 测试 | `cargo nextest` 替代 `cargo test`。内联 `#[cfg(test)] mod tests` | 集成测试在 `torvox-integration-tests` |
| 属性测试 | `proptest 1.11` — VT 解析器和 Grid 必须有 | 最少 10K 用例 |
| 命名 | 函数/变量 `snake_case`，类型 `PascalCase`，常量 `SCREAMING_SNAKE` | |
| 导出 | 每个 crate 最小公共 API 表面。用 `pub(crate)` 隐藏内部 | |
| `no_std` | `torvox-core` 必须 `no_std` 兼容。`extern crate alloc` 用于 Vec/String | 验证: `cargo build -p torvox-core --target thumbv6m-none-eabi --no-default-features --features alloc` |
| Copy 语义 | 不要盲目 derive Copy。含 String/Vec 的类型不能 Copy | Shell::Custom(String) 使 TerminalConfig 失去 Copy |

## Kotlin

| 约定 | 规则 | 理由/注意 |
|------|------|-----------|
| Kotlin | 2.3.21+，Compose BOM 2026.05.00 | |
| DI | Hilt 2.59.2 (需 AGP 9.0+) | @HiltAndroidApp + @AndroidEntryPoint |
| 架构 | MVVM with StateFlow/SharedFlow | 单向数据流 |
| UI | Jetpack Compose, Material 3 | |
| 命名 | 函数/变量 `camelCase`，类 `PascalCase` | |
| 可空性 | 默认非空。`?` 仅在真正可空处 | |
| 格式化 | `ktfmt` 强制 | |
| 渲染 | SurfaceView 宿主 Rust wgpu v29 Surface，**不用 Canvas** | ADR 003 |
| 前台服务 | `FOREGROUND_SERVICE_SPECIAL_USE` | AndroidManifest 中声明 foregroundServiceType |
| JNA | `net.java.dev.jna:jna:5.17.0@aar` | boltffi 运行时依赖 |
| AGP 插件 | 不使用 `org.jetbrains.kotlin.android` — AGP 9.0+ 内置 Kotlin | 用 KSP 2.3.9 替代 kapt |

## Git

| 约定 | 规则 | 理由/注意 |
|------|------|-----------|
| 提交 | Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`) | |
| 分支 | `phase-N/*` 用于阶段工作，`fix/*` 用于修复 | |
| PR | Squash merge 到 `main`。每个 PR 一个逻辑提交 | 保持历史清晰 |
| Cargo.lock | 提交到版本控制 | 二进制项目需要可复现构建 |

---

# 第九部分: 测试要求

完整策略见 `docs/ADR/006-testing-strategy.md`。

## 五层测试金字塔

| 层 | 工具 | 频率 | 覆盖目标 |
|----|------|------|----------|
| L0 编译时 | clippy, geiger, MIRI, fmt | 每 PR | unsafe 审计、类型安全、格式 |
| L1 单元 | `cargo nextest` + 内联 `#[test]` | 每 PR | 每个公共函数 |
| L2 属性 | `proptest 1.11` (10K+ 用例) | 每 PR | VT 解析器、Grid、PTY 编码 |
| L3 集成 | `torvox-integration-tests` | 每 PR | 跨 crate 交互、会话生命周期 |
| L4 模糊 | `cargo-fuzz` (3 目标, 1B+ 迭代/夜) | 每夜 | 零崩溃 |

## 确定性回放测试

记录 PTY 输出 → postcard 序列化 → 回放 → 断言 Grid 状态。
用于回归验证和跨平台一致性检查。

## 具体测试要求

- **每个公共函数**: 必须有单元测试
- **VT 解析器**: proptest (10K+ 用例) + fuzz (每夜 1B+ 迭代)
- **Grid/DirtyMask**: proptest (不变量: mark 后 is_dirty 为 true, clear 后 any_dirty 为 false)
- **序列化**: 每个可序列化类型须有 serde roundtrip 测试
- **PTY**: 非阻塞读写、resize、kill_on_drop (Linux 单元测试, Android 集成测试)
- **boltffi 桥接**: Kotlin 调用 Rust 函数，返回值正确 (端到端测试)

---

# 第十部分: 架构关键点

完整架构见 `docs/ARCHITECTURE.md`。以下是智能体必须记住的关键点:

## Crate 依赖方向 (严格单向)

```
torvox-core (no_std, 零依赖)
      ↑
torvox-terminal (nix, vte, crossbeam)
      ↑
torvox-renderer (wgpu, cosmic-text, swash, etagere)
      ↑
torvox-gui-android (boltffi, 依赖上述所有)
```

**依赖只能从下往上**。torvox-core 不能知道 torvox-terminal 的存在。

## 线程模型

| 线程 | 职责 | 注意 |
|------|------|------|
| PTY 读取线程 | `read()` → crossbeam SPSC → VT 解析器 | 阻塞 I/O，独立线程 |
| VT 解析器线程 | 消费 SPSC，更新 Grid | Arc<Mutex<Grid>> |
| 渲染线程 | DirtyMask → 图集查找 → 实例缓冲区 → wgpu submit | **单线程** (wgpu 设备在自己线程) |
| wgpu 内部线程 | 1-2 个 Vulkan 内部线程 | wgpu 管理 |
| Android 主线程 | Compose UI, 事件分发 | 不做重计算 |

空闲时总线程数: 4-5

## 数据流

```
PTY write → kernel → read() → raw bytes → crossbeam SPSC
  → VT Parser → Grid + DirtyMask (Vec<u64>分区) → notify
  → RenderThread → glyph atlas lookup → instance buffer
  → wgpu submit → SurfaceView
```

## 关键不变量

- **torvox-core 需要 `alloc`** (no_std + alloc)。Vec/String 通过 `extern crate alloc` 支持，`no_std` 环境需启用 `alloc` feature。
- **torvox-terminal 拥有所有 PTY I/O** 在专用线程中。fork 是唯一 unsafe。
- **torvox-renderer 是单线程** (wgpu 设备在自己线程上)。
- **FFI 边界传递结构化事件**，不是原始字节 (boltffi #[data]/#[error])。
- **DirtyMask** `Vec<u64>` 分区，每 u64 覆盖 64 行，支持任意行数。不再有行数限制。
- **热路径用 crossbeam**，不用 tokio。crossbeam 零分配、无锁、有界反压。

## 渲染管线

```
fontdb → cosmic-text 0.19 (整形) → swash 0.2.7 (光栅化, 内部用 skrifa 做 scaling)
  → guillotiere 0.7 (shelf packing 图集) → 实例化四边形 → 单次 draw call
```

- swash 0.2.x: scaling 完全由内部 skrifa 处理，**不需要单独依赖 skrifa crate**
- SurfaceView (不是 AHardwareBuffer): 零拷贝，wgpu 原生支持

---

# 第十一部分: AI 工作流

## 会话开始流程

1. **阅读本文件** — 完整阅读，不要略读
2. **阅读 ROADMAP.md 当前阶段** — 确认当前要做什么
3. **阅读相关 ADR** — 如果修改 PTY 读 ADR 004，修改渲染读 ADR 003，以此类推
4. **检查当前代码状态** — 确认文档描述与实际代码一致

## 实现流程

1. **规划**: 列出本次会话要完成的具体步骤，包含验证标准
2. **类型先行**: 先定义类型，再实现行为
3. **小步提交**: 每个逻辑步骤提交，不积累 10+ 文件变更
4. **验证**: 每步 `cargo clippy -- -D warnings && cargo nextest -p <affected-crate>`
5. **同步**: 修改 Rust 类型 → 检查 bridge.rs → 重新生成 Kotlin 绑定
6. **更新**: 完成后更新 AGENTS.md 和相关文档

## 修改 Rust 类型时检查清单

```
[ ] 类型定义在哪个 crate? (core → terminal → renderer → gui-android)
[ ] 该类型是否通过 boltffi 导出? (检查 bridge.rs)
[ ] bridge.rs 的桥接类型是否需要同步更新?
[ ] boltffi Kotlin 绑定是否需要重新生成?
[ ] 此类型变更是否影响 serde 序列化格式? (破坏性变更?)
[ ] 相关单元测试是否需要更新?
```

## 添加新函数时检查清单

```
[ ] 公共还是内部? 默认 pub(crate)，仅必要时 pub
[ ] 是否需要单元测试? 每个公共函数必须
[ ] 是否需要 proptest? VT 解析器和 Grid 必须
[ ] 错误类型用 thiserror，不用 anyhow (库 crate)
[ ] no_std 兼容? (如果在 torvox-core 中)
[ ] unsafe? (如果在 torvox-core 中，不允许)
```

---

# 第十二部分: 会话结束检查清单

每次工作会话后，逐项检查:

## Rust 质量

1. [ ] `cargo nextest --workspace` 通过
2. [ ] `cargo clippy -- -D warnings` 通过
3. [ ] `cargo fmt --check` 通过
4. [ ] `cargo geiger` 检查: 无新的 unsafe 引入 (torvox-core 零 unsafe)
5. [ ] 测试覆盖率: 新公共函数都有单元测试?

## Android 质量 (如有 Android 变更)

6. [ ] `cd android && ./gradlew lint` 通过
7. [ ] `cd android && ./gradlew test` 通过
8. [ ] `./scripts/build-android-libs.nu` 成功
9. [ ] boltffi 绑定已重新生成 (如 bridge.rs 有变更)

## 文档同步

10. [ ] AGENTS.md 更新了新约定/工具/状态变化
11. [ ] 重大决策创建了 ADR
12. [ ] 添加了新 crate? → 更新 ARCHITECTURE.md 和 Cargo.toml workspace
13. [ ] 添加了新 Android 模块? → 更新 settings.gradle.kts
14. [ ] 修改了依赖版本? → 确认 ARCHITECTURE.md 版本锁定表同步

## 序列化兼容性

15. [ ] 修改了 serde 序列化的类型? → 评估是否破坏已保存状态兼容性

---

# 第十三部分: 已知问题与待办

以下问题已知但尚未修复。不要在新代码中重复这些错误:

| # | 问题 | 状态 | 计划 |
|---|------|------|------|
| 1 | `torvox-bench/benches/` 不存在 | 待建 | 性能基准无法运行 |
| 2 | CI 使用 `@main` 引用 | 待修 | 应固定到 SHA 或 tag |

---

# 附录: 术语表

| 术语 | 含义 |
|------|------|
| SDD | Specification-Driven Development — 规范驱动开发 |
| VT | Video Terminal — 视频终端 (VT100/220/320/ECMA-48 标准) |
| PTY | Pseudo-Terminal — 伪终端 |
| W^X | Write XOR Execute — Android 安全策略，限制同时可写可执行 |
| SGR | Select Graphic Rendition — VT 终端文本属性控制序列 |
| CSI | Control Sequence Introducer — VT 控制序列引导 |
| OSC | Operating System Command — VT 操作系统命令序列 |
| DirtyMask | Vec<u64> 分区位标志，每 u64 覆盖 64 行，支持任意行数 |
| SPA | Single Point of Authority — 单一权威源 (如 ARCHITECTURE.md 是版本的 SPA) |
| PIE | Position-Independent Executable — 位置无关可执行文件 (Android 要求) |
