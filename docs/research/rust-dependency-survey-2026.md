# Rust 依赖与代码资产扫描：awesome-rust 与 Warp

## 摘要

结论先行：两个来源**都没有可直接取用的依赖或代码**。唯一有信息量的产出是 cosmic-text／fontdb
阻断的现状更新，以及一次可直接执行的小幅依赖升级。

- **awesome-rust** 是人工维护的索引，不是依赖真值表。用它反查本项目的直接依赖：`cosmic-text`、
  `swash`、`fontdb`、`unicode-script`、`etagere`、`bytemuck`、`pollster`、`roxmltree`、
  `thiserror`、`flume` 在该 README 里**一次都没有出现**；出现的只有 `wgpu`、`nix-rust/nix`、
  `jeromefroe/lru-rs` 三项，而这三项本项目已在用。索引对本项目的覆盖率接近零，与既有生态扫描
  「跨 25 仓扫描 `wgpu` 零命中、完整 GPU 终端渲染路径仅本仓」同口径。
- 从索引筛出的候选依赖逐一否决，理由全部落在既有约束上（见第 4 节表）：`rustix` 缺 `fork()`；
  `thread-priority`／`inotify-rs`／`encoding_rs`／`tabwriter`／`textwrap`／`crossbeam`／
  `moka`／`cargo-dinghy` 无落点；`insta`／`serial_test`／`test-log` 与本仓测试口径或受保护的
  `native/Cargo.toml` 冲突；`cargo-nextest`／`cargo-mutants`／`cargo-tarpaulin` 属门禁改动，
  落在 `scripts/` 与 `.github/` 禁改区，只登记不实施。
- **`warpdotdev/warp` 是 issues-only 仓库**：README 自述 "This is an issues-only repo for Warp
  where you can submit issues, bugs and feature requests."，仓库根没有 `Cargo.toml`、没有
  `package.json`、没有工作流目录，只有 README 与许可文件。**无源码、无依赖清单、无测试资产**。
- Warp 组织下 36 个仓库全部过筛。唯一接触本项目技术栈的是 **`warpdotdev/cosmic-text`**，但它是
  **上游镜像而非分叉**：tip 提交由上游 cosmic-text 维护者署名，`Cargo.toml` 声明
  `version = "0.15.0"`，最后同步于 2025-11-06；本项目已用 `0.19.0`（crates.io 最新，
  2026-04-22），该镜像**落后四个次版本**，零取用价值。它唯一的作用是**交叉验证**：Warp 与本项目
  用的是同一套文本整形栈（cosmic-text + swash + fontdb + harfrust）。
- **唯一可执行的依赖结论**：本项目 `native/Cargo.toml` 记录的阻断「cosmic-text 0.19.0 要求
  `fontdb ^0.23`，故 0.24 不可用」**仍然成立**。上游 master 已于 2026-09-20 把 `fontdb` 抬到
  0.24，但**未发布**——crates.io 上 cosmic-text 最新仍是 0.19.0。故维持 `fontdb = "0.23"`，
  解锁条件是「cosmic-text 发布 > 0.19.0」，不是「fontdb 发布 0.24」（后者早已就绪）。
- 附带实测：25 项直接依赖 = **23 项 crates.io 直接依赖已是最新** + `lru`（0.19.0 → 0.19.1，
  补丁版，已按例行维护升级）+ `libghostty-vt`（git 钉 rev，无 crates.io 版本可比）。
  `fontdb` 0.23.0 是 `^0.23` 约束内的最新，故它与 `lru` 不构成同一类「落后」。

## 研究方法与坐标

原则与 `rex-protocol-support-2026.md` 一致：只用一手资料；索引只用作候选来源，任何候选都必须回到
crate 元数据、上游提交流或本仓源码核实。

| 对象 | 研究坐标 | 一手核验手段 |
| --- | --- | --- |
| `rust-unofficial/awesome-rust` | `main` 分支 README（2740 行） | 全文按依赖名反查 + 逐节人工判读 |
| `warpdotdev/warp` | `main` 分支 README 与仓库根 | 逐个探测 `Cargo.toml`、`package.json`、工作流目录的存在性 |
| `warpdotdev/*`（36 仓） | 组织仓库列表 + 各仓 README 首段 | 逐仓拉取 README 定性 |
| `pop-os/cosmic-text` | `main` 提交流 + crates.io 版本接口 + 版本依赖表 | 提交标题与日期、依赖约束解析 |
| `warpdotdev/cosmic-text` | `main` 提交流 + `Cargo.toml` + tip 补丁头 | 版本比对与作者署名 |
| 本项目 | `native/Cargo.toml`、`native/Cargo.lock`、`native/src/terminal/pty.rs`、`native/src/render/pass/acquire.rs`、`RenderSupervisor.kt`、`DocumentWatcher.kt` | 直接读源码 |

本仓约定文档不写提交号，故一律用标签、日期或「crates.io 最新版本」作坐标。

## awesome-rust 的覆盖率

对 2740 行 README 按名字做不区分大小写的命中计数：

| 本项目直接依赖 | 索引内命中 | 索引内对应条目 |
| --- | --- | --- |
| `wgpu` | 7 | `gfx-rs/wgpu`（Graphics） |
| `nix` | 24（多数是 NixOS 的 "nix"） | `nix-rust/nix`（Platform specific → Unix-like） |
| `lru` | 2 | `jeromefroe/lru-rs`（Data structures） |
| `cosmic-text` | 0 | 无 |
| `swash` / `fontdb` / `skrifa` / `harfrust` | 0 | 无 |
| `unicode-script` / `unicode-width` / `unicode-segmentation` | 0 | 无 |
| `etagere` / `bytemuck` / `pollster` / `roxmltree` / `thiserror` / `flume` | 0 | 无 |
| `rustix`（未用，候选） | 2 | `bytecodealliance/rustix`（Platform specific → Unix-like） |

索引里有条目的三项本项目都在用；本项目实际依赖的字体、图集、序列化与通道层在索引里**完全缺席**。
原因在分类口径：该索引面向通用应用与库，不覆盖「GPU 终端渲染 + 字体整形 + JNI 边界」这一层。

这一缺席不能当作「这些库不值得用」的证据——索引的收录标准是社区知名度，不是技术适配度。反过来也
成立：索引里出现不等于可用，每个候选都必须回到本项目约束上判定。

## 候选依赖的逐项判定

| 候选 | 索引分类 | 判定 | 依据 |
| --- | --- | --- | --- |
| `rustix` | Platform specific → Unix-like | 不采纳 | `rustix::process` 没有 `fork()`：模块索引里 "fork" 零命中，`fn.fork` 页面不存在。本项目的 PTY 路径要求 `fork()` 之后紧接 `execve()`，且子进程内禁止分配与记日志（`native/src/terminal/pty.rs` 的 fork/exec 段）。该形态只有 `nix` + `libc` 能表达，换 `rustix` 等于重写整条 spawn 路径并丢掉其成立理由 |
| `thread-priority` | Platform specific → Cross-platform | 不采纳（无落点） | 渲染线程的调度优先级已由 Kotlin 侧 `Process.setThreadPriority(Process.THREAD_PRIORITY_DISPLAY)` 设定（`RenderSupervisor.kt` 的渲染线程启动段）。原生侧线程只有 PTY 读线程、VT 线程与 gpu-acquire 工作线程，其延迟由通道容量、闭锁与在途记账决定，不是调度类问题 |
| `inotify-rs` | Platform specific → Linux | 不采纳（无落点） | 目录监视在 Kotlin 侧用 `android.os.FileObserver`（`DocumentWatcher.kt`，上限 64 个目录 + 淘汰 + 合并窗口）；原生侧没有消费者 |
| `encoding_rs` | Encoding → Character Encoding | 不采纳（无落点） | 终端字节一律 UTF-8，解码与不完整序列归上游 `libghostty-vt`；本项目侧不存在第二个解码器 |
| `BurntSushi/tabwriter` | Text processing | 不采纳（无落点） | 制表位是 VT 语义（上游职责）；本项目的网格是定宽单元格，不做弹性列对齐 |
| `textwrap` | Text processing | 不采纳（无落点） | 不做折行：换行由上游按列数决定，渲染只按单元格出图 |
| `crossbeam` | Concurrency | 不采纳 | `flume` 已覆盖命令、查询、输出、裁剪结果回送四条通道的 MPMC 需求，不存在需要第二种通道实现的形状 |
| `moka` | Caching | 不采纳 | 缓存侧已用 `lru`（计算结果与图集淘汰），无并发缓存需求 |
| `insta` | Testing | 不采纳 | 快照测试把期望值写成仓库内文件；`TESTING.md` 与既有裁定要求期望值是显式断言、外部数据不入库。此外它是 dev-dependency，改动落在受保护的 `native/Cargo.toml` |
| `serial_test` | Testing | 不采纳 | 并发敏感的用例已用 `shuttle` 与显式夹具（`PtyWriteLockHold` 一类）表达顺序，没有靠全局串行化掩盖竞争 |
| `test-log` | Testing | 不采纳 | 测试不初始化日志；生产日志经 `log` 门面进 JNI sink，测试不需要第二条通路 |
| `cargo-dinghy` | Testing（工具） | 不采纳（无落点） | 设备侧验证走 Gradle 仪器测试（`connectedDebugAndroidTest` 三个任务），不是 cargo 目标 |
| `cargo-nextest` / `cargo-mutants` / `cargo-tarpaulin` | Testing（工具） | 只登记，不实施 | 三者都是门禁改动：`scripts/check.nu` 与 `.github/` 均属禁改区，须维护者同意方可实施 |

## Warp 的仓库形态

### `warpdotdev/warp`

README 首段的原文是 "This is an issues-only repo for Warp where you can submit issues, bugs and
feature requests."；同一 README 写明开源计划为「先开源 Rust UI 框架，再开源部分客户端代码」，
并注明服务端保持闭源。

仓库根探测：`Cargo.toml` 不存在、`package.json` 不存在、`.github/workflows` 不存在；只有 README
与许可文件。

判定：**不采纳（无源码可评估）**。它与既有台账中「`git ls-files` 仅 README、源码私有」的同类行
同口径。其 issue 讨论区可作产品面参照，但产品面（智能体、云执行、会话共享、AI 集成）在本项目里是
`PROHIBITED.md` 明禁或 `DESIGN.md` 未声明，不构成可采纳内容；其渲染与输入实现不在仓库内，无从
核验，故 MUST NOT 以「Warp 这样做」作为任何实现依据。

### `warpdotdev/*` 全组织过筛（36 仓）

| 仓库 | 形态 | 判定 |
| --- | --- | --- |
| `cosmic-text` | 上游镜像：`version = "0.15.0"`，tip 由上游维护者署名，最后同步 2025-11-06 | 不采纳：落后本项目在用的 0.19.0 四个次版本 |
| `rust-email_address` | 已发布 crate `email_address`（0.2.10） | 不采纳：邮件地址解析，无落点 |
| `global-hotkey` | 已发布 crate `global-hotkey`（0.10.0） | 不采纳：桌面全局热键，Android 无对应面 |
| `sentry-contrib-native` | 已发布 crate（0.3.1，2021 年后无更新） | 不采纳：外部崩溃上报服务；本仓日志口径是 logcat |
| `harbor` | 智能体评测框架（Terminal-Bench 血统） | 不采纳：智能体评测，属明禁方向 |
| `portable-tmux` | 预编译 tmux 二进制（glibc 2.17） | 不采纳：与原生 PTY 实现无交集 |
| `warp-for-benchmarks` / `warp-stanford` | 落地页／品牌页仓库 | 不采纳：无代码资产 |
| `workflows` / `docs` / `homebrew-warp` | 命令模板、文档、分发配方 | 不采纳：无代码资产 |
| `session-sharing-protocol` / `warp-proto-apis` | 产品协议 | 不采纳：会话共享撞会话持久化明禁；协议面未声明 |
| `oz-agent-action` / `oz-agent-worker` / `oz-dev-environments` / `oz-for-oss` / `oz-sdk-python` / `oz-sdk-typescript` / `oz-workspace` | 云智能体编排与 SDK | 不采纳：AI 集成与外部服务，明禁方向 |
| `claude-code-warp` / `codex-warp` / `opencode-warp` / `common-skills` / `do-things` / `warp-factory-examples` | 外部智能体与技能集成 | 不采纳：明禁方向 |
| `generate-changelog` / `repo-sync` / `packages` / `people` / `projects` / `repositories` / `sponsoring` / `tab_counts` / `.github` | 运维、脚手架、无 README | 不采纳：无终端相关代码 |

### 全组织唯一有价值的事实

`warpdotdev/cosmic-text` 的存在说明 **Warp 与本案用的是同一套文本栈**：cosmic-text 的整形交
harfrust、光栅交 swash、字体库交 fontdb。这是一条独立的交叉验证——本项目在 `native/Cargo.toml`
里选的那条路不是孤例。但它不带来任何代码：镜像落后四个次版本，且上游才是真值源。

## cosmic-text 与 fontdb：阻断现状

事实链（全部为读取当日的实测值）：

1. 本项目 `native/Cargo.toml` 声明 `fontdb = "0.23"`，并记录阻断：cosmic-text 0.19.0 要求
   `fontdb ^0.23`，而本项目需要 `fontdb::ID` 与 `cosmic_text::fontdb::ID` 是同一版本，故 0.24
   不可用。该约束已在本机 vendored 源码上核实：`cosmic-text-0.19.0/Cargo.toml` 的
   `[dependencies.fontdb]` 段写 `version = "0.23"`（即 `^0.23`，0.24 越界）。
2. crates.io：`cosmic-text` 最新稳定版为 **0.19.0**（2026-04-22），其依赖约束为
   `fontdb ^0.23`、`skrifa ^0.40.0`、`harfrust ^0.5.0`、`swash ^0.2.6`；`fontdb` 最新稳定版为
   **0.24.0**（2026-07-29）。
3. 上游 `pop-os/cosmic-text` 的 `main` 提交流显示 **2026-09-20 有 `chore: bump fontdb to 0.24`**，
   其后还有一批 2026-10-27 的改动：非二次方复杂度的字体回退、ASCII 快路径下 `default-attrs`
   尾部的取舍、按语言子标签匹配汉字统一区、非左对齐 LTR 文本的 `Buffer::hit`、装饰与整形解耦、
   `LayoutGlyph` 增加以像素计的 ascender／descender。
4. 上述改动**均未发布**：crates.io 上没有高于 0.19.0 的版本。

结论：**阻断仍然成立，维持 `fontdb = "0.23"`**。解锁判据是「cosmic-text 发布 > 0.19.0」，而不是
「fontdb 发布 0.24」——后者早在 2026-07-29 就已就绪，卡的是 cosmic-text 的发版节奏。不得为此改走
git 依赖：那既动受保护的 `native/Cargo.toml`，又违反上游必须钉版本、可复核的口径。

上游那批 2026-10-27 的改动里，与本项目直接相关的是**字体回退的非二次方复杂度**与**汉字统一区按
语言子标签匹配**两条：前者对应 CJK 回退在长行上的耗时，后者对应中日韩字形选择。二者都属上游职责，
本项目只经 `cosmic-text` 消费，不自行实现；等发布后按例行升级评估，届时须以 `native/benches/cjk_resolve`
的实测值复验。

## 依赖版本实测

25 项直接依赖逐一与 crates.io 最新稳定版比对（读取当日）。判据取 `cargo update --dry-run`
的实测输出：该命令只列出「会改动」的条目，未被列出的直接依赖即为其约束内的最新版本。

| 结果 | 依赖 |
| --- | --- |
| 已是最新（23 项） | `base64`、`bytemuck`、`cosmic-text`、`etagere`、`flume`、`foldhash`、`fontdb`（0.23.0，`^0.23` 约束内最新）、`jni`、`libc`、`log`、`nix`、`parking_lot`、`pollster`、`raw-window-handle`、`roxmltree`、`serde`、`serde_json`、`swash`、`thiserror`、`toml`、`unicode-script`、`wgpu`、`wgpu-types` |
| 落后且可例行升级（1 项） | `lru` 0.19.0 → 0.19.1（补丁版） |
| git 钉版（1 项，无 crates.io 版本可比） | `libghostty-vt`（git 依赖，钉版坐标以 `native/Cargo.toml` 为准，不在本文复写提交号） |

`lru` 的升级只动 `native/Cargo.lock`，不触碰受保护的 `native/Cargo.toml`，故按例行维护处理。
0.19.1 与 0.19.0 的源码差异实测仅两处：清单 `edition` 改 2024，以及 `compile_fail`
doctest 从测试模块末尾移到其前（使其真正被执行）与两处测试代码整理；生产 API 无变化。
`fontdb` 不动：0.24.0 越出 `native/Cargo.toml` 的 `fontdb = "0.23"` 约束，且
`cosmic-text 0.19.0` 的 vendored 清单亦声明 `version = "0.23"`，改它会破坏
`fontdb::ID` 的类型同一性。

同一命令另测出三项**传递**依赖落后最新：`cc` 1.11.2 → 1.11.3、`walkdir` 2.5.0 → 2.5.1
（`lru` 之外）。二者是构建期管道件，本轮不升级：它们不在直接依赖面内，升级收益为零
而构建面风险非零。

## 不采纳清单（合并）

| 项 | 类型 | 一句话理由 |
| --- | --- | --- |
| `warpdotdev/warp` | 仓库 | issues-only，无源码、无依赖清单、无测试资产 |
| `warpdotdev/cosmic-text` | 仓库 | 上游镜像，落后四个次版本 |
| `warpdotdev/*` 其余 34 仓 | 仓库 | 产品内容、智能体编排、分发或脚手架，无终端引擎代码 |
| `rustix` | crate | `rustix::process` 无 `fork()`，无法承载 fork/execve 路径 |
| `thread-priority`、`inotify-rs`、`encoding_rs`、`tabwriter`、`textwrap`、`crossbeam`、`moka`、`cargo-dinghy` | crate | 无落点（各自职责已由既有实现或平台能力覆盖） |
| `insta`、`serial_test`、`test-log` | crate | 与测试口径冲突，或落在受保护依赖文件 |
| `cargo-nextest`、`cargo-mutants`、`cargo-tarpaulin` | 工具 | 门禁改动，落在禁改区，只登记不实施 |

## 复核方式

- **索引覆盖率**：取 `rust-unofficial/awesome-rust` 的 `main` 分支 README，对每个依赖名做不区分
  大小写的计数；判定以「命中条目是否为本项目在用的同一 crate」为准，而不是以计数为准。
- **候选可采性**：每条候选必须落到 (a) crate 文档里是否存在所需能力、(b) 本仓是否存在消费方、
  (c) 改动是否落在受保护文件——三条全过才进入下一步。
- **cosmic-text 阻断**：crates.io 版本接口取 `max_stable_version` 与版本依赖表；上游提交流取
  `pop-os/cosmic-text` 的 `main` 分支 Atom 源，看是否有 `fontdb` 相关提交以及该提交之后是否发布了
  版本；本仓侧取 `~/.cargo/registry/src/` 下 vendored 的 `cosmic-text-0.19.0/Cargo.toml` 核对
  `fontdb` 约束原文。
- **依赖版本落后面**：`cargo update --dry-run` 的实测输出（只列会改动的条目）；`lru` 两版源码
  逐行 diff（vendored `src/lib.rs` 与清单）。
- **Warp 形态**：探测仓库根的 `Cargo.toml`、`package.json`、工作流目录；组织级过筛取组织仓库列表
  并对每个仓库拉 README 首段。
- **本仓侧**：`native/Cargo.toml` 的依赖段、`native/Cargo.lock` 的锁定版本、
  `native/src/terminal/pty.rs` 的 fork/exec 段。
