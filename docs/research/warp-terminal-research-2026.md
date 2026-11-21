# Warp 终端对比研究报告

## 摘要

本报告对比 `warpdotdev/warp`（Warp，一个以终端为起点的智能体开发环境）与本仓的形态、架构与取舍，
并按本仓的采纳门槛给出判定。

结论先行：

- **Warp 与本仓是两条不同的技术路线**。Warp 是桌面级、自研终端模型（基于 `vte` 状态机 + alacritty 血统的
  网格与块模型）+ 自研 UI 框架 `warpui`（macOS 主走 Metal，wgpu 为实验性渲染器）+ 大量云/AI/Drive 能力；
  本仓是 Android、`libghostty-vt` 单一 VT 来源、wgpu/Vulkan 渲染、本地 Shell。两者在 VT 核、渲染栈、目标平台
  三条主线上都不重合。
- **可借鉴的只在工程方法与局部技术**：秘密脱敏（`secret_redaction` + `ObfuscateSecrets`）、命令块的输入/输出分块
  语义、`ref_tests` 的 harness 形态、`sum_tree` 文本缓冲、`warpui` 的 damage 跟踪与字形缓存思路。这些都属「仅参照」，
  不引入代码。
- **Warp 的测试资产不采纳**：`app/src/terminal/ref_tests` 是 alacritty harness 的移植，期望值由实现自身产出
  （门槛 1 成立）；VT 语义本身由上游 `vte` 与 alacritty 血统承担，本仓 VT 语义不由自研测试覆盖。
- **Warp 的产品能力面撞本仓明禁或未声明**：AI/代理与 MCP（`PROHIBITED.md` 明禁 MCP，AI 助手未声明）、
  云同步与会话持久化（明禁「会话数据持久化 / 恢复」）、环境变量编辑器（明禁）、远程与控制面（未声明）、
  Vi 模式/编辑器/Drive/补全/启动项（未声明）。这些一律不立项。
- **Warp 的渲染栈与本仓不兼容**：Warp 主走 Metal（macOS），wgpu 只是实验性；本仓是 Vulkan 唯一渲染路径、
  无回退。移植渲染代码没有对象。
- **许可证不是判定依据**：Warp 的 `warpui` 为 MIT，其余为 AGPL v3。按本仓口径（不检查依赖许可证、不引入许可证
  文件），许可证不作为采纳与否的依据，只作事实登记。

## 研究方法与坐标

原则：只用一手资料。对外部项目直接克隆仓库并阅读源码、构建脚本与测试数据，不引用项目自述以外的二手描述；
对本仓结论回到本仓源码与 `docs/specification/` 规范核对。

| 对象 | 研究坐标 | 形态 |
| --- | --- | --- |
| `warpdotdev/warp` | master 快照（无发布标签，提交日期 2026-11-20） | Rust 工作区（`app` + 82 个 crate），桌面 GUI + TUI，macOS/Linux/Windows/Web |
| 本仓 | 工作树 | Android 终端模拟器（Kotlin + Compose + Rust + wgpu/Vulkan + `libghostty-vt`） |

本仓约定文档不写 git 提交号，故坐标用快照标识。

一手资料的落点：

- 工作区清单：根 `Cargo.toml`（`members = ["crates/*", "app"]`，`resolver = "2"`，`publish = false`，许可证
  `AGPL-3.0-only`，`warpui` 单独 MIT）。
- 终端模拟层：`crates/warp_terminal/`（46.5k 行 Rust），含 `model/`（`ansi`、`grid`、`blockgrid`、`kitty`、
  `iterm_image`、`secrets`、`selection`、`session`）、`local_tty/`、`shell/`、`bootstrap.rs`、`writeable_pty/`。
- UI 框架：`crates/warpui/` + `crates/warpui_core/`（Metal 主渲染器，wgpu 实验性）。
- 测试资产：`app/src/terminal/ref_tests/`（40 组 golden 数据 + harness），`crates/warp_terminal/src/model/**/*_tests.rs`。

## Warp 的形态与架构

### 工作区与 crate 分层

Warp 是一个单 Cargo 工作区，`app` 为 GUI 入口，`crates/` 下 82 个 crate 按职责拆分。按行数排序的前几项：
`warp_tui`（94.9k）、`warpui_core`（91.6k）、`editor`（67.1k）、`ai`（52.3k）、`warp_terminal`（46.5k）、
`warpui`（34.4k）、`integration`（22.1k）、`warp_completer`（15.8k）、`computer_use`（15.5k）、`onboarding`（12.7k）、
`mcp`（5.7k）、`remote_server`（9.8k）、`persistence`（?）、`settings`（4.5k）、`warp_core`（8.8k）、
`warp_util`（5.6k）。

依赖方向大致为：`app` → `warp_terminal` + `warpui` + `ai` + `cloud_*` + `editor` + `settings` + …；
`warp_terminal` → `warp_core` + `warpui_core` + `warp_util` + `vte` + `command` + `secret_redaction`。

与本仓的差异：本仓是「Rust 终端/渲染 + Kotlin UI」的两层，Warp 是「Rust 全栈」的单层（UI 也在 Rust 里），
并把 AI/云/Drive/编辑器/补全/远程等能力都放进同一个工作区。

### 渲染与 UI 框架

- `warpui`/`warpui_core` 是 Warp 自研的 UI 框架，基于 `winit` 窗口与自定义场景图（`scene.rs`），文本布局
  （`text_layout.rs`）、字形缓存、图片缓存（`image_cache.rs`）、无障碍（`accessibility.rs`）、剪贴板、通知、
  缩放、模态、动作系统（`action.rs`/`keymap.rs`）。
- 渲染后端：macOS 主走 Metal（`objc2-metal`、`CAMetalLayer`，`warpui/Cargo.toml` 的 `enable-metal-frame-capture`
  特性）；非 macOS 走 wgpu（`experimental-wgpu-renderer` 特性，`dep:wgpu`、`dep:bytemuck`），Linux/Windows/FreeBSD
  走 `wgpu` + `winit`；Web 走 wasm（`serve-wasm`）。
- 本仓与之对比：本仓渲染固定 Vulkan（`wgpu` 的 `Vulkan` 后端，无 CPU/OpenGL 回退），目标 Android；Warp 的主渲染
  路径是 Metal，Vulkan/wgpu 是实验性。两条栈在 GPU API、surface 管理、字形图集策略上都不重合，移植渲染代码没有对象。

### 终端模拟层

- `warp_terminal` 是 Warp 的终端模型，**基于 `vte` crate（Warp 自己 fork 的 `warpdotdev/vte`，pinned rev）做状态机，
  网格与块模型是 Warp 自研**（文件头注释明确「adapted from the alacritty_terminal crate under the Apache license」）。
- 模块：`ansi/`（控制序列参数与处理器）、`grid/`（`GridStorage`、`GridHandler`、`cell`、`row`、`storage`、
  `resize`、`filtering`、`displayed_output`、`secrets`、`hyperlink_registry`、`selection_cursor`、`grapheme_cursor`、
  `flat_storage`）、`blockgrid.rs`（命令块的网格）、`block_id.rs`/`block_index.rs`（块索引）、`kitty.rs`（Kitty 图形）、
  `iterm_image.rs`（iTerm 图像）、`escape_sequences.rs`、`selection.rs`、`mouse.rs`、`mode.rs`（`TermMode` 位标志）、
  `session.rs`、`completions.rs`、`find.rs`、`image_map.rs`、`char_or_str.rs`、`indexing.rs`。
- 支持的模式（`Mode` 枚举）：CursorKeys、DECCOLM、Insert、Origin、LineWrap、BlinkingCursor、LineFeedNewLine、
  ShowCursor、ReportMouseClicks/CellMouseMotion/AllMouseMotion、ReportFocusInOut、Utf8Mouse、SgrMouse、AlternateScroll、
  UrgencyHints、SwapScreen、BracketedPaste、SyncOutput（?2026）。另支持 Kitty 图形与 iTerm 图像；未见 sixel。
- 本仓与之对比：本仓 VT 语义完全由 `libghostty-vt` 提供（单一来源，DESIGN 明确禁止自研 VT 与换核）；Warp 是
  「自研终端模型 + `vte` 解析器」，等于第二套 VT 核。两者在 VT 解析、网格存储、宽字符/组合字符、回滚、搜索、
  选区上各自实现，无法直接对齐；Warp 的 grid 与本仓上游 Ghostty 的 grid 在语义上是两套真相。

### 输入与编辑

- `crates/vim/`：Vi 模式编辑器（在终端内编辑）。
- `crates/editor/`（67k 行）：Warp 内置代码编辑器，基于 `sum_tree`（rope 类文本缓冲）与 tree-sitter 语法树。
- `crates/input_classifier/`：输入分类（自然语言 vs 命令）。
- 本仓：输入走 Android 输入法 + 修饰键栏，无 Vi 模式、无内置编辑器、无输入分类。这些在 `DESIGN.md` 未声明，
  不立项。

### 命令块与输出

- `blockgrid.rs`、`block_id.rs`、`block_index.rs`、`displayed_output.rs`、`filtering.rs`、`secrets.rs`：Warp 把终端输出
  组织成「命令块」（prompt + 命令 + 输出），支持按块过滤、按块复制、输出脱敏。
- `secrets.rs` + `secret_redaction` crate：在终端输出里识别并脱敏秘密（API key、token 等），`ObfuscateSecrets`
  枚举控制视觉遮蔽/删除线/始终显示；复制时可 respect 脱敏。
- `crates/command/`、`crates/command-corrections/`：命令解析与纠错建议。
- 本仓：无命令块概念、无输出脱敏、无命令纠错。`DESIGN.md` 未声明，不立项。秘密脱敏本身是值得登记的候选能力，
  但未声明前不实现。

### AI 与代理

- `crates/ai/`（52k 行）、`crates/ai_types/`、`app/src/ai/`、`app/src/ai_assistant/`、`app/src/coding_entrypoints/`、
  `app/src/code_review/`、`crates/computer_use/`、`crates/computer_use_mcp/`、`crates/mcp/`（5.7k）、`app/src/agents/`。
- 功能：内置编码代理（Agent Mode）、MCP 客户端/服务器、computer use、代码审查、自然语言命令生成、上下文芯片。
- 本仓：`PROHIBITED.md` 明禁 Model Context Protocol；`upstream-alignment`「能力裁定补齐行」把「AI 助手集成与
  ACP/crash 分诊」判为「未声明，不立项」。Warp 的 AI/代理能力整体撞明禁或未声明，不立项。

### 云、同步与持久化

- `crates/cloud_objects/`、`cloud_object_client/`、`cloud_object_models/`、`cloud_object_persistence/`、
  `crates/persistence/`、`crates/firebase/`、`crates/graphql/`、`app/src/auth/`、`app/src/billing/`、`app/src/persistence/`、
  `app/src/drive/`、`app/src/notebooks/`、`app/src/env_vars/`、`app/src/launch_configs/`。
- 功能：Warp Drive（跨设备同步的命令、环境、笔记本、启动配置）、账号、计费、云对象。
- 本仓：`PROHIBITED.md` 明禁「会话数据持久化 / 恢复」与「自定义环境变量 / 环境变量编辑功能」；
  `upstream-alignment` 把「会话恢复/持久化、配置保险库、daemon 常驻会话」判为 PROHIBITED。Warp 的云同步与 Drive
  整体撞明禁，不立项。

### 远程与控制

- `crates/remote_server/`、`crates/local_control/`、`crates/ipc/`：本地控制 socket、远程服务器、进程间通信。
- `crates/session-sharing-protocol/`：会话共享协议。
- 本仓：`upstream-alignment`「能力裁定补齐行」把「远程会话（设备配对、隧道、移动端控制）」判为「未声明，不立项」；
  DBus 远程控制 likewise。Warp 的远程与控制面与本仓裁定一致，不立项。

### 搜索、补全与 Drive

- `crates/warp_search_core/`、`app/src/search/`、`crates/warp_completer/`、`crates/fuzzy_match/`、`crates/lsp/`、
  `crates/repo_metadata/`、`crates/natural_language_detection/`。
- 本仓：全文搜索已走上游 `search::Search`（`upstream-alignment` 已登记采用）；命令补全、LSP、仓库元数据在
  `DESIGN.md` 未声明，不立项。

### 平台与构建

- 平台：macOS（主，Metal 渲染器）、Linux/FreeBSD、Windows、Web（wasm，`serve-wasm`）；Android/iOS 无。
- 构建：Cargo 工作区 + `winit` + Metal/WGSL 着色器（`wgslfmt`）+ C/C++/ObjC（clang-format）+ `build.rs`；使用
  `flake.nix` + `rust-toolchain.toml`（1.94.0）+ `deny.toml`（cargo-deny）+ `about.hbs`/`about.toml`（许可证清单）。
- 与本仓对比：本仓目标 Android（minSdk 31、targetSdk 28、arm64-v8a/x86_64），Rust 1.99+/edition 2024，Vulkan 唯一；
  Warp 是桌面多平台，两者在平台矩阵与构建形态上无重合。Warp 的 `flake.nix` 与本仓相同（Nix 开发环境）是少数共同点。

### 测试与质量

- 测试：`cargo nextest run --no-fail-fast --workspace`，13.7k 个 `#[test]`；`warp_terminal` 有 30 个测试文件，
  含 `ref_tests`（alacritty harness 移植，40 组 golden 数据）与 `escape_sequences_tests`（断言 Kitty/Ghostty 在
  `DISAMBIGUATE_ESCAPE` 下发出的字节）。
- 质量工具：clippy `-D warnings`、rustfmt、clang-format（C/C++/ObjC）、wgslfmt、cargo-deny（`deny.toml`）、
  `./script/presubmit`。
- 本仓：`cargo test` + `cargo clippy --deny warnings` + `semgrep` + `markdownlint-cli2` + `openspec validate` +
  detekt/spotless + Android 仪器测试。两者在 Rust 侧门禁形态相近（clippy deny warnings、fmt），差异在 Warp 多了
  clang-format 与 wgslfmt（本仓无 C/C++/WGSL 之外的着色器）与 cargo-deny。

### 许可证与治理

- `warpui`/`warpui_core` 为 MIT（`LICENSE-MIT`），其余为 AGPL v3（`LICENSE-AGPL`）；`about.toml` 生成第三方许可证清单。
- 贡献：`CONTRIBUTING.md`、`CODE_OF_CONDUCT.md`、`SECURITY.md`、GitHub Actions 驱动的 Factory 代理。
- 本仓：不设许可证、不检查依赖许可证、不放许可证文件（`STYLE.md` 明确）；治理面为 `openspec/` + `docs/specification/`。

## 与本仓逐项对照

### 能力对照表

| 能力 | Warp 的实测做法 | 本仓现状 | 差距 | 阻塞层 |
| --- | --- | --- | --- | --- |
| VT 核 | 自研终端模型（alacritty 血统 + `vte` 状态机），`crates/warp_terminal/src/model/`（`ansi`/`grid`/`blockgrid`/`kitty`/`iterm_image`） | `libghostty-vt` 单一来源（DESIGN 明禁自研 VT 与换核） | Warp 是第二套 VT 核 | DESIGN（单一来源） |
| 渲染 | `warpui` 自研场景图，macOS Metal 主路径，wgpu 实验性，`winit` 窗口 | wgpu/Vulkan 唯一，无回退，Android Surface | 渲染栈不重合 | 平台/渲染架构 |
| 平台 | macOS/Linux/Windows/FreeBSD/Web（wasm） | Android（arm64-v8a/x86_64） | 平台矩阵无重合 | BUILD.md |
| 命令块 | `blockgrid`/`block_id`/`block_index`/`displayed_output`/`filtering` | 无块概念，会话按序号列表 | 产品面未声明 | DESIGN（未声明） |
| 秘密脱敏 | `secrets.rs` + `secret_redaction` crate，复制时 respect 脱敏 | 无 | 可作为候选能力 | DESIGN（未声明） |
| AI/代理 | `ai`/`ai_assistant`/`computer_use`/`mcp`/`agents` | `PROHIBITED.md` 明禁 MCP，无 AI 面 | 整面撞明禁/未声明 | PROHIBITED + 未声明 |
| 云/同步/Drive | `cloud_objects`/`persistence`/`firebase`/`graphql`/`drive`/`notebooks`/`env_vars` | 明禁会话持久化、环境变量编辑 | 整面撞明禁 | PROHIBITED |
| 远程/控制 | `remote_server`/`local_control`/`ipc`/`session-sharing-protocol` | 未声明（远程会话行） | 整面未声明 | DESIGN（未声明） |
| Vi 模式/编辑器 | `vim` crate、`editor` crate（67k 行，sum_tree） | 无 | 产品面未声明 | DESIGN（未声明） |
| 补全/LSP/仓库元数据 | `warp_completer`/`lsp`/`repo_metadata`/`fuzzy_match` | 无 | 产品面未声明 | DESIGN（未声明） |
| 搜索 | `warp_search_core` | 上游 `search::Search`（已登记采用） | 已有等价物 | 无差距 |
| Kitty 图形 | `kitty.rs`（KittyAction/StorageError） | 上游 kitty graphics（已登记采用） | 已有等价物 | 无差距 |
| iTerm 图像 | `iterm_image.rs` | 上游 iTerm inline images | 已有等价物 | 无差距 |
| 选区/滚动/颜色 | 自研 grid 选区与滚动 | 上游 selection/search/scrollback | 已有等价物 | 无差距 |
| Wake lock/前台服务 | `prevent_sleep` | `TerminalForegroundService`（DESIGN 已声明） | 已有等价物 | 无差距 |
| 通知 | `notification.rs` | OSC 9/777 桌面通知（DESIGN 已声明） | 已有等价物 | 无差距 |
| 测试门禁 | clippy deny warnings、rustfmt、clang-format、wgslfmt、cargo-deny | clippy deny warnings、semgrep、markdownlint、openspec validate、detekt/spotless | Rust 侧门禁形态相近 | 无差距 |
| Nix 开发环境 | `flake.nix` + `rust-toolchain.toml` | `flake.nix` + `rust-toolchain.toml` | 共同点 | 无 |
| 许可证 | AGPL v3（主体）+ MIT（warpui） | 不设许可证、不检查依赖许可证 | 治理面不同 | STYLE.md |

### 测试资产对照

| 资产 | Warp 做法 | 本仓口径 |
| --- | --- | --- |
| `app/src/terminal/ref_tests`（40 组 golden） | alacritty harness 移植，输入 `.recording` + 期望 `grid.json` | 门槛 1 成立（期望值是实现自身产出），不采纳 |
| `escape_sequences_tests` | 断言 Kitty/Ghostty 在 `DISAMBIGUATE_ESCAPE` 下的字节 | 本仓键位编码走上游 `key::Encoder`，不自写字节断言 |
| `warp_terminal` 单元测试（13.7k 个 `#[test]`） | 自研 grid/ansi/selection 的单元测试 | 本仓只测接缝与契约，VT 语义归上游 |
| cargo-deny | 依赖许可证与安全审计 | 本仓不检查依赖许可证（STYLE.md） |

## 可采纳性判定

### 测试资产

按 `upstream-alignment` 的三条门槛：期望值来自外部真相（门槛 1）、不引入无程序消费者的二进制资产（门槛 2）、
净增覆盖或净减代码（门槛 3）：

- `ref_tests` golden：门槛 1 成立（期望 grid 由实现自身产出），不采纳。harness 形态（输入记录 + 期望网格 + 单例
  测试宏）仅作参照。
- `escape_sequences_tests`：期望值是 Kitty/Ghostty 的字节输出，本仓键位编码走上游 `key::Encoder`，断言上游产出
  的字节属「不测依赖行为」，不采纳。
- `warp_terminal` 单元测试：断言对象是 Warp 自研 grid/ansi/selection，本仓不存在该组件，门槛 3 不满足，不采纳。

### 功能面

按 `upstream-alignment`「能力裁定补齐行」与 `PROHIBITED.md` 的口径：

| 功能面 | Warp 出处 | 判定 |
| --- | --- | --- |
| AI/代理与 MCP | `crates/ai`、`crates/mcp`、`crates/computer_use`、`app/src/agents` | PROHIBITED（MCP 明禁）+ 未声明（AI 助手） |
| 云同步与 Drive | `crates/cloud_objects`、`app/src/drive`、`app/src/notebooks` | PROHIBITED（会话持久化） |
| 环境变量编辑 | `app/src/env_vars` | PROHIBITED（自定义环境变量） |
| 远程/控制与会话共享 | `crates/remote_server`、`crates/local_control`、`crates/ipc`、`session-sharing-protocol` | 未声明，不立项 |
| Vi 模式与内置编辑器 | `crates/vim`、`crates/editor` | 未声明，不立项 |
| 命令块 | `crates/warp_terminal/src/model/blockgrid.rs` 等 | 未声明，不立项 |
| 命令补全/LSP/仓库元数据 | `crates/warp_completer`、`crates/lsp`、`crates/repo_metadata` | 未声明，不立项 |
| 秘密脱敏 | `crates/warp_terminal/src/model/secrets.rs`、`secret_redaction` | 未声明，不立项（但登记为候选能力） |
| 自研 VT 核 | `crates/warp_terminal` 整体 | 不采纳（违单一来源） |
| 自研 UI 框架与 Metal 渲染 | `crates/warpui`、`crates/warpui_core` | 不采纳（渲染栈不重合） |
| 多平台（macOS/Linux/Windows/Web） | `app/src/platform/` | 不适用（本仓仅 Android） |

### 参照点

以下做法仅作参照，不引入代码：

- `secret_redaction` 的秘密识别与脱敏分级（`ObfuscateSecrets` 的视觉遮蔽/删除线/始终显示三态）——本仓若日后声明
  「输出脱敏」能力，可按此形态设计。
- `displayed_output.rs` 的「显示输出」分层与块级过滤——与本仓的脏跟踪/跳过干净快照思路同向，可作性能对照。
- `sum_tree` 的 rope 类文本缓冲——仅在本仓若日后声明内置编辑器时有参考价值。
- `ref_tests` 的 harness 形态（输入记录 + 期望网格 + 单例测试宏）——仅作测试组织对照，期望值本身不可采纳。
- Cargo 工作区 + `deny.toml` + clang-format/wgslfmt 的多语言门禁——本仓已用 semgrep/markdownlint/openspec validate
  覆盖等价面，不新增体系。

## 事实、推论与未知

### 事实

- Warp 是 Rust 全栈桌面终端：UI 框架 `warpui`（macOS Metal 主路径，wgpu 实验性），终端模型 `warp_terminal`
  （alacritty 血统 + `vte` 状态机），支持 macOS/Linux/Windows/FreeBSD/Web。
- Warp 的 VT 层是自研终端模型，不是 `libghostty-vt`；`crates/warp_terminal/src/model/ansi_handler.rs` 文件头注明
  「adapted from the alacritty_terminal crate under the Apache license」。
- Warp 使用 `vte` crate（Warp fork，pinned rev）做状态机，网格/块/选区/图像由 Warp 自研。
- Warp 支持 Kitty 图形与 iTerm 内联图像、Kitty 键盘协议（`DISAMBIGUATE_ESCAPE`）、同步输出（?2026）、
  bracketed paste、鼠标报告、焦点报告；未见 sixel。
- Warp 有秘密脱敏（`secrets.rs` + `secret_redaction` crate）、命令块（`blockgrid`）、AI/代理（`ai`/`mcp`/
  `computer_use`）、云同步与 Drive（`cloud_objects`/`drive`/`notebooks`/`env_vars`）、远程/控制（`remote_server`/
  `local_control`/`ipc`/`session-sharing-protocol`）、Vi 模式与内置编辑器（`vim`/`editor`）、命令补全/LSP/
  仓库元数据、搜索（`warp_search_core`）。
- Warp 的测试资产：`ref_tests`（alacritty harness 移植，40 组 golden）+ `escape_sequences_tests` +
  13.7k 个 `#[test]`。
- Warp 的许可证：`warpui`/`warpui_core` 为 MIT，其余 AGPL v3；使用 `flake.nix` + `rust-toolchain.toml`（1.94.0）+
  `deny.toml`。
- 本仓 VT 语义由 `libghostty-vt` 单一来源提供；`PROHIBITED.md` 明禁 MCP、会话持久化、环境变量编辑、自定义主题等；
  `upstream-alignment` 已把 AI 助手、远程会话、WebSocket/网络终端、会话恢复/持久化、分屏、插件等判为未声明/PROHIBITED。
- 本仓渲染固定 Vulkan、目标 Android、无回退；Warp 主渲染 Metal、wgpu 实验性。

### 推论

- Warp 与本仓在 VT 核、渲染栈、目标平台三条主线上都不重合，因此「移植 Warp 代码」不是可行路径；
  本仓若要吸收 Warp 的能力，必须先在 `DESIGN.md` 声明对应功能，再按本仓分层自行实现或取用上游。
- Warp 的秘密脱敏是本仓尚未声明但技术上可落地的能力（上游 Ghostty 不提供，需自研或取第三方库），登记为候选。
- Warp 的命令块模型与本仓的「会话按序号列表」形态差异大，若立项需要先裁定块语义在本仓的落点。
- Warp 的 `ref_tests` 是 alacritty 血统的自验证，与本仓「不测依赖行为」口径一致，不采纳。
- Warp 的 Nix 开发环境与本仓相同，是少数可直接对照的工程实践点。

### 未知

- Warp 的 `vte` fork 相对上游 `vte` 的改动范围与稳定性（影响其 VT 语义与本仓的可比性）。
- Warp 的秘密脱敏正则与误报率、对 CJK/多字节输入的处理。
- Warp 的块模型与 shell 集成（OSC 133）的耦合程度——本仓上游已解析 OSC 133 但本仓不消费。
- Warp 的 AI 能力对终端核心的侵入程度（是否污染 `warp_terminal` 的公共 API）。
- Warp 的性能数据与本仓 Android 设备的可比性（Warp 是桌面，本仓是移动）。

## 不做清单

本报告不主张实现以下任何一项：

- 移植 Warp 的 VT 核、网格、选区、滚动、搜索、图像实现（违单一来源）。
- 移植 Warp 的渲染栈或 UI 框架（渲染栈不重合）。
- AI/代理、MCP、computer use、代码审查、自然语言命令生成。
- 云同步、Warp Drive、账号、计费、笔记本、环境变量编辑、启动配置。
- 远程/控制面、会话共享、本地控制 socket、进程间通信。
- Vi 模式、内置编辑器、命令补全、LSP、仓库元数据、模糊搜索。
- 命令块、输出脱敏、命令纠错（未声明；秘密脱敏登记为候选但未立项）。
- 多平台支持（macOS/Linux/Windows/Web）——本仓仅 Android。
- 修改 `docs/specification/`、`Cargo.toml`、`build.gradle.kts`、`settings.gradle.kts`。

## 复现方式

- 外部证据：`warpdotdev/warp` 仓库的根 `Cargo.toml`、`crates/warp_terminal/Cargo.toml` 与 `src/`（`model/`、
  `local_tty/`、`shell/`）、`crates/warpui/` 与 `crates/warpui_core/`、`crates/secret_redaction/`、
  `app/src/terminal/ref_tests/`、`app/src/platform/`、`rust-toolchain.toml`、`flake.nix`、`deny.toml`、
  `LICENSE-AGPL`、`LICENSE-MIT`、`AGENTS.md`。
- 本仓证据：`docs/specification/{STYLE,DESIGN,PROHIBITED}.md`、`openspec/specs/upstream-alignment/spec.md`
  （能力裁定补齐行、上游 API 取用判定、Android 侧终端项目复查台账）、`native/src/terminal/` 接缝、
  `native/src/render/`、`openspec/specs/pty-output-backpressure/spec.md`。
