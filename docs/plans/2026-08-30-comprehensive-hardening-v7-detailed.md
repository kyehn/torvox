# 详细实施计划: comprehensive-hardening-v7 — 全量审计与像素级可靠化

> 日期: 2026-08-30 | 版本: v7 | 基于 26 参考项目深度研究 + 全代码库审计  
> 前置: `docs/reference/` 全部 26 项目研究（含 00-TORVOX-BASELINE、02-三向对比、03-zelland、04-termlib、supplement-4 等）  
> 关联: `openspec/changes/reference-adoption-v6`（4 项 P0/P1 已规约）、本计划为 v7 全量硬化

## 0. 执行摘要

v6 已规约 4 项高价值采纳（鼠标编码 / 无障碍 / OSC133 / CellRun），审计发现其实现 **已部分落地**（ffi `encodeMouseEvent`、Bridge、TerminalSurface 手势、AccessibilityLineProvider、output_processor last_command_output），但存在 **语义缺口 + 测试缺口 + 工程债**。v7 在 v6 基础上扩展为 **全量硬化**：代码精简、依赖升级、确定性后端、90fps 模拟器、UI 可靠测试、26 项目像素级优点复制。

**目标**：在不引入新抽象、不增加维护负担前提下，保守、可验证地修复全部剩余问题（含小瑕疵），使模拟器 90fps+、后端确定性、UI 可自动化验证，所有现有测试通过，新增自动化覆盖回归。

**原则**：KISS + 强类型 + 清晰命名 + 零魔数 + 外部成熟依赖优先于自研；每次修改保持行为一致或有测试锁定；及时 git 提交推送。

---

## 1. 基线与差距总览

### 1.1 torvox 基线（00-TORVOX-BASELINE.md 摘要）

- 引擎：Ghostty VT 单一来源，pty fork/execve + winsize 像素字段，osc_handler（52/133/9/7）、output_processor（批处理/BEL/133）、session（spawn/resize/write/signal/exit 去重）、shell_env、key encoding
- 渲染：wgpu 30 Vulkan 单 pass（cell.wgsl），context（surface/atlas/bg_image/pipeline）、cell_builder（build_instances_from_cell_data + selection 反色 + cursor 三样式 + grapheme_extra）、font（cosmic-text+swash+guillotiere）、dirty-band + CachedInstances 行级增量
- JNI：48+ 导出（ffi.rs），RwLock 注册表 + AtomicU64 active_id + EVENT_QUEUE + RenderState 惰性初始化
- Kotlin：TerminalRuntime（RenderSupervisor）、Bridge/NativeBridge、TerminalSurface（IME composition 500+ 行、Selection 手柄、Magnifier）、ModifierBar、TextSearchBar、SessionDrawer、Settings、Theme（Dracula Plus）、Bootstrap 原子安装、DocumentsProvider、MCP（9 工具, tower-mcp, Unix+Stdio）
- MCP：运行时开关，默认编译 mcp feature

### 1.2 26 项目对比缺口（README §3 去重清单，P0 优先）

**选择系统密集区（ ghostty-android / termux-app / Haven / termlib ）**：多击 tapCount、自计数、Callback2 onGetContentRect、边缘滚动、selectionGeometryKey 48 位、宽字符列→char 换算、wrap 感知文本拼接、smartCopy、expandAcrossUrlWrap
**搜索/URL/无障碍**：搜索收窄保持当前匹配、覆盖层不触发 SIGWINCH、URL 标点/括号平衡、惰性缓存、OSC133 语义段、LiveRegion、Copy 动态禁用、粘贴确认
**输入/IME**：composing diff 最长公共前缀 + DEL 按 code point、Compose 键序列、Fn 第二层、键栏自定义、键映射补全（caret/AppCursor/CSI27）
**渲染/GPU**：行级脏缓存（现已有 CachedInstances 但需验证等价性）、捏合缩放、wgpu is_emulator 双后端 + GL 兜底、CellRun 游程、自绘放大镜、findOptimalFontSize 二分、字体合成回退、emoji 边界
**会话/Bootstrap**：前台进程组跟踪组杀、环境分层 EnvOp、sha256 sidecar、初始 winsize 先于首读、fast-death、持久化、scrollback 设置、符号链接重放
**安全/MCP**：ArgumentTokenizer、SO_PEERCRED、同意门控、screenshot、命令安全分级

> 本计划优先落地 **v6 的 4 项 + 上述 P0 中与现有代码最贴近、收益最确定的 12 项**，其余 P1/P2 进入待办（见 §9 Backlog）。

### 1.3 当前审计发现（全库扫描 2026-08-30）

#### 已落地 vs 缺口对照

| 能力 | 现状 | 缺口 | 结论 |
|------|------|------|------|
| 鼠标编码 | ffi encodeMouseEvent + Bridge.encodeMouseEvent + TerminalSurface 2890 行吃事件，public_api encode_mouse_event + tests 202/216/235 通过 | MouseEncoder 独立模块缺失（逻辑在 public_api 内联）、SGR/X10/UTF8 覆盖需核对 cell 尺寸实时性、拖拽/滚动全路径需集成测试 | **补全测试 + 文档化 + 轻度抽取**，不重构 ABI |
| 无障碍 | AccessibilityLineProvider + LineNavigator + Debounced updater + TerminalSurface contentDescription + announceForAccessibility，JVM 单测覆盖 | visibleLines 行号计算依赖 scrollbackLength/scrollOffset 同步、MAX_DESCRIPTION 2000 截断需验证多屏、TalkBack Next/Previous 包裹行为需模拟器验证 | **补 Robolectric + 模拟器 TalkBack 脚本** |
| OSC133 | output_processor 捕获 B→C 文本（64KB 上限、跨 chunk、A 重置、ST/BEL 双终结）、ffi getLastCommandOutput（mcp + JNI） | 无列范围 SemanticSegment、无 promptId、无滚动随行迁移、无 ANSI 语义类型（Prompt/Input/Output/Finished 仅枚举 shell_integration) | **扩展为轻量 SemanticSegment（不做 Kotlin 行迁移，仅 Rust 侧列记录）**，满足 spec REQ-O1..O5 |
| CellRun | CachedInstances 行级增量 + dirty bands 已实现（NFR-010），但无同格式游程合并 | cell_builder 无 build_row_runs / CellRun 类型、重复 fg/bg/flags 行内未合并、JNI 条目数未降低 | **增量式引入 CellRun 合并（仅合并相邻同样式，保持 CellData 兼容）** |
| 行级脏缓存 | 已实现 CachedInstances + compute_dirty_bands + build_instances_cached | 与 zelland with_rows 行级去重语义等价性需对照测试 | **补对照测试，确认 skip changed==false 路径** |
| PTY winsize | session spawn 接受 rows/cols，ffi setPixelSize | 初始 winsize 竞态（spawn 与首帧 resize 间隙折行） | **spawn 前计算 pixel 尺寸并同步设置** |
| Bootstrap sha sidecar | BootstrapDownloader/Installer 已有原子 staging | 无 sha256 sidecar 校验 | **增加可选 sidecar 校验（best-effort, 不阻断 Termux 预设）** |
| ArgumentTokenizer | 无 | MCP cmd 字符串需安全拆分为 argv | **引入外部 crate `shell-words` 或 `shlex`（成熟、零维护）** |
| SO_PEERCRED | mcp.rs Unix socket accept 无 peer cred 校验 | 同 uid 越权 | **调用 getpeercred（nix::sys::socket::getsockopt SO_PEERCRED）** |
| Dirty/新输出双旗 | OutputProcessor.new_output + RenderState.dirty 双旗已实现 | 需文档化与测试锁定不互相 starve | **补 property 測試：tail -f 下 bell 仍可达** |

#### 工程债 / 小瑕疵清单（逐步修复，保守改动）

- Rust: `cell_builder.rs:462` 已修复（dirty_rows 索引越界，曾为函数调用误用）；`render/tests.rs:948/976` 未使用 gpu 变量 warn 需 `_gpu` 前缀；`output_processor` MAX_CAPTURE 64KB 硬编码可提常量文档化
- Kotlin: TerminalSurface 2700+ 行（IME composition 500 行 + 手柄 + 选择 + 渲染管线耦合），需局部抽取但不拆 ABI；SettingsScreen 重复 theme 逻辑可去重；部分 magic number（debounce 500ms、cell 6.0x12.0）需命名常量
- 构建: `cargo test` 暖缓存 76s 超过 lens 60s 超时，需延长超时或拆分 nextest；gradle parallel/caching 已启用；flake.nix zig_0_16 固定，需验证 0.16 与 ghostty zig build 兼容
- 测试: 模拟器 gfxinfo 无进程（需启动 Activity 后再采）；Roborazzi/Detekt 已配置但未在 CI 强制门控

---

## 2. 架构决策（ADR 摘要）

| ADR | 决策 | 理由（参考项目） |
|-----|------|------------------|
| ADR-0001 | 外部依赖优先：ArgumentTokenizer 用 `shell-words`/`shlex`，字体发现用系统 API 复用 | termux-kotlin ArgumentTokenizer 已验证自研四态机易错；外部 crate 零维护 |
| ADR-0002 | 保守抽取：鼠标编码保持 public_api 内联，不新建 mouse_encoder.rs 独立模块 | 避免新增抽象仅使用一次；KISS；public_api 已有测试覆盖 |
| ADR-0003 | 语义段轻量化：Rust 侧维护 Vec<SemanticSegment> 仅列范围，不做 Kotlin 行迁移 | termlib 行迁移需要 TerminalLine 模型，torvox Rust 侧无此模型；轻量满足 getLastCommandOutput 且不引入新状态机 |
| ADR-0004 | CellRun 兼容式：合并相邻同样式 cell 为 run，但仍展开为 CellInstance，不改 CellData 结构 | 避免 FFI 突破性变更；渲染侧复用现有实例路径，仅减少 dirty 判断次数 |
| ADR-0005 | 双后端保留单后端：模拟器 SwiftShader 用 Vulkan lavapipe，不引入 GL 退路（除非真机 Vulkan 失败） | 项目约束"无 CPU/OpenGL 回退"；shashlik 双后端记录为注释备选，不默认启用 |
| ADR-0006 | 测试超时：lens cargo test 超时从 60s 提升至 120s，CI 并发 8 线程 | 实测 76s 暖缓存；60s 必然超时非代码问题 |
| ADR-0007 | 确定性优先：PTY 输出批处理 flush 时机由 PTY 侧决定，不在渲染侧补 buffer | 已有 OutputProcessor 模型；增加 property 測試锁定 |

---

## 3. 详细实施方案

### 阶段 0 — 基线加固（0.5 天，已完成部分）

- 修复 `cell_builder.rs:462` 索引 bug（已提交）
- 恢复 `docs/reference/` 44 文件（已提交）
- 验证 `cargo test --lib` 1000 passed（nix develop，76s）
- 扩展 lens 超时至 120s（.pi/lens.toml 或 CI env）

### 阶段 1 — 鼠标编码补全（P0, 0.5 天）

**目标**：确认 SGR/1006 门控完整，像素→cell 实时，拖拽/滚轮全覆盖，测试锁定。

**实现**：

1. Rust `public_api::encode_mouse_event` 审计：确认调用 `ghostty_mouse_encoder` + `setopt_from_terminal` + size 含实时 cell_w/h（已存在），补充文档注释引用 zelland terminal.rs:41-90
2. ffi `encode_mouse_event_inner` 审计：空 bytes → 空 array 非 null，session 不存在静默丢弃
3. Kotlin `TerminalSurface:2747,2880,2890,1773` 路径：触摸按下/移动/抬起 + alt screen wheel→encode 均携带 live cell 尺寸（bridge 透传），补充缺失的 Drag 事件（ACTION_MOVE 时 action=2）
4. 测试：现有 tests 202/216/235 已覆盖 gate/SGR/wheel；新增 `encode_mouse_bounds_clamp`（负坐标/超界 clamp）、`encode_mouse_drag_sequence`（press→drag→release 序列）
5. 模拟器验证：启动 `vim`（`echo -e "set mouse=a" > ~/.vimrc`），`adb shell input swipe` + `dumpsys gfxinfo` 90fps 期间不掉帧

**验收**：vim mouse mode 1000/1002/1003 下点击/拖拽/滚轮均产生正确 SGR 序列；非 mouse mode 零事件；cargo test 1000+3 通过。

### 阶段 2 — 无障碍补强（P0, 0.5 天）

**目标**：TalkBack 可读当前屏，Next/Previous 行导航包裹正确，bell/title announce 不刷屏。

**实现**：

1. `AccessibilityLineProvider.visibleLines` 现有实现保留；新增 `visibleLines` 的 scrollOffset 越界单测（负值/超長）
2. `AccessibilityLineNavigator` current/next/previous 已实现包裹；补充 debounce 测试（500ms 内重复 update 去重）
3. `TerminalSurface` contentDescription 更新：`accessibilityDescriptionUpdater` 已带 500ms debounce（TerminalAccessibility.kt），确认仅内容变化时更新（diff check 已在 updater 内）
4. `TerminalScreen:announceForAccessibility` 对 bell/title 限频（500ms 合并，复用 BellHandler 模式）
5. Robolectric：断言 `contentDescription` 随 `lines` 变化而更新，MAX 2000 截断
6. 模拟器：Settings→Accessibility→TalkBack 开启，输入 `echo hello; echo world`，右滑 Next line 朗读逐行

**验收**：TalkBack 开启时，`adb shell uiautomator dump` 可见 contentDescription 含当前屏文本；bell 触发单次 announce；性能：帧回调 <1ms。

### 阶段 3 — OSC133 语义化（P1, 1 天）

**目标**：实现 spec REQ-O1..O5 的列级 SemanticSegment + getLastCommandOutput，多行命令正确。

**数据结构**（output_processor.rs）：

```rust
pub struct SemanticSegment { pub start_col: u16, pub end_col: u16, pub kind: SemanticKind, pub exit_code: Option<i32> }
pub enum SemanticKind { PromptStart, PromptEnd, CommandInput, CommandOutput, CommandFinished }
```

**解析**：

- 复用现有 `scan_osc133` 的 prefix_match + ST/BEL 双终结 + 跨 chunk 状态机
- A/B/C/D 时刻按当前 `ghostty cursor col`（需从 terminal 同步，或近似用 capture_buf len 换算，取保守：记录 B 时刻 capture 清空，C 时刻切片区间）
- D 带 `;exit_code` 时解析为 i32（已在 OutputSnapshot.shell_exit_code 实现，扩展到 segment）
- 存储：`Vec<SemanticSegment>` 按时间追加，D 时封口；A 时重置（与现有 B→C capture 一致）
- JNI `getLastCommandOutput`：返回最近 `CommandFinished` 前的 `CommandOutput` 文本（复用现有 `take_last_command_output` 缓冲 + 新 segments 索引）

**测试**：

- Rust：`osc133_prompt_start_col`、`osc133_command_input_col`、`osc133_command_output_col`、`osc133_finished_exit_code`、`osc133_multiline`、`osc133_take_clears`、`osc133_reset_on_A`
- Kotlin：Bridge.getLastCommandOutput 端到端（PTY 注入 `echo hi` 序列）

**验收**：`printf '\x1b]133;B\x07echo hi\x1b]133;C\x07hi\n\x1b]133;D;0\x07'` 后 `getLastCommandOutput() == "echo hi"`；多行 `echo -e "a\nb"` 正确。

### 阶段 4 — CellRun 增量优化（P1, 0.5 天）

**目标**：行内同样式连续 cell 合并计数，减少 dirty 判断次数，验证 50%+ 合并率（同格式文本）。

**实现**：

- 定义 `CellRun { start_col: u16, length: u16, fg: [f32;4], bg: [f32;4], flags: u32 }`（仅 internal 使用，不改 CellData FFI）
- `build_row_runs(cells: &[CellData]) -> Vec<CellRun>`：线性扫描，比较 fg/bg/flags（bytemuck 字节等价）
- 集成：在 `build_row_instances_into` 的 incremental 分支，clean 行直接复用；dirty 行内先算 runs 再展开为 instances（保持现有 instance 生成逻辑，仅增加 runs 计数用于断言与日志）
- 不改 `build_instances_from_cell_data` 外部签名

**测试**：

- `cell_run_single_format`：5 同色 cell → 1 run
- `cell_run_mixed_format`：中间变色 → 2 runs
- `cell_run_newline_breaks`：换行不跨行
- Benchmark：同格式长行实例数不变但 runs 计数 1 vs 80

**验收**：同格式文本 runs=1；混合文本按色段分 run；性能无回退（`cargo bench cell_builder`）。

### 阶段 5 — 细节硬化（P1, 1 天）

**子项**：

- **初始 winsize**：TerminalRuntime spawn 前 `getGridRowsColsPacked` 预计算 rows/cols + pixel 尺寸，Session.spawn_with_theme 后立即 setPixelSize（消除首帧折行）
- **sha256 sidecar**：BootstrapDownloader 下载后若同 URL 存在 `.sha256` 则校验，失败删除 staging 并重试一次（best-effort，不影响 Termux 预设离线安装）
- **ArgumentTokenizer**：mcp `run_command` 的 command_string→argv 改用 `shell-words::split`（crate 引入，零自研分词）
- **SO_PEERCRED**：mcp.rs Unix socket accept 后 `getsockopt(SO_PEERCRED)` 校验 uid==app uid，否則 close
- **行级脏去重**：已有 `row_cache != runs` 语义等价，现 CachedInstances 通过 `rows_equal` 字节等价实现；补注释引用 zelland WGPU_FIXES Fix1
- **代码精简**：`render/tests.rs` 的 `_gpu` 重命名、magic number 常量化（SEARCH_HIGHLIGHT_SWAP_ALPHA 128 已命名；补 cell 默认 6.0x12.0 常量、debounce 500ms 常量复用）

### 阶段 6 — 依赖与构建（0.5 天）

- `cargo update`（workspace 依赖不固定版本，执行并验证 1000 测试通过）
- `gradle` 依赖：检查 composeBom、agp、kotlin 2.4.10、hilt 2.60.1 是否为最新（`gradle dependencyUpdates`），按 semver 挑 patch/minor，major 需人工 review
- `flake.nix`：nixpkgs-unstable 滚动一次，zig 0.16 保持（与 ghostty 兼容），记录 `flake.lock` 变更
- CI：`.github/workflows/rust-checks.yml` 增加 `cargo test --lib` 120s 超时；`android-tests.yml` 保持 reactivecircus@v2（@main 未含 node_modules 陷阱已注释）

---

## 4. 测试计划（含验收标准）

### 4.1 单元测试（Rust, 自动化, 确定性）

| 模块 | 用例 | 断言 | 来源 |
|------|------|------|------|
| output_processor | bel_detection / no_bel / pty_write_raises_new_output_flag / shell_integration_*/ last_command_output_* / osc133_semantic_* (新增 7) | snapshot/*Output 字段 + take 清空 | FR-033 |
| ghostty_terminal | encode_mouse_event_gated_off / sgr_press / wheel / bounds_clamp (新增) / drag_sequence (新增) | SGR 序列前缀 `\x1b[<` + 坐标 + m/M 后缀 | P0 mouse |
| cell_builder | cell_run_*(4) / compute_dirty_bands_* / CachedInstances band_slice | runs 合并率 + band 覆盖 | NFR-010 |
| render | wgpu_backend is_emulator 分支（条件编译） | 模拟器走 lavapipe | shashlik |
| mcp | ArgumentTokenizer split / SO_PEERCRED uid 校验 | argv 切分 + peer uid==app | termux-kotlin |

**运行**：`nix develop --command cargo test --lib`（1000+ 新增，暖缓存 76s，CI 120s 超时）；`cargo bench cell_builder` 对比；`cargo clippy -- -D warnings` 零新增。

### 4.2 单元测试（Kotlin, Robolectric, JVM）

| 模块 | 用例 |
|------|------|
| TerminalAccessibility | contentDescription 随 visibleLines 更新、MAX 截断、navigator next/previous 包裹、debounce 去重 |
| Bridge | encodeMouseEvent 存活 session 透传、destroyed session 空返回 |
| TerminalQueryPort | getTitle/scrollbackLine/gridRowsCols 透传 |

**运行**：`./gradlew :app:testDebugUnitTest`（JVM，无模拟器）

### 4.3 集成测试（模拟器, 自动化）

| 场景 | 步骤 | 预期 | 工具 |
|------|------|------|------|
| vim 鼠标 | bootstrap 安装 bash→ `echo "set mouse=a" > ~/.vimrc; vim` → `adb shell input tap x y` + swipe | 光标移动，SGR 序列可由 logcat `encode_mouse_event` 或 PTY 日志验证 | Espresso + adb |
| TalkBack | Settings→开启 TalkBack→ `echo hello` → `uiautomator dump` 解析 contentDescription | 含 hello | UIAutomator |
| OSC133 | `printf '\x1b]133;B\x07echo hi\x1b]133;C\x07hi\n\x1b]133;D;0\x07'` → Bridge.getLastCommandOutput | 返回 echo hi | Instrumented + Bridge |
| 90fps | 启动 + 滚动 + 输入 `yes head -n 1000` → `dumpsys gfxinfo com.termux framestats` | 90% 帧 <16ms（90fps 阈 11ms） | gfxinfo |
| PTY winsize | 横竖屏旋转后 `stty size` | rows/cols 与 pixel 同步变化 | adb shell |
| Bootstrap sha | 篡改 zip 后安装 → 失败重试 | 校验失败删除 staging | Instrumented |
| MCP SO_PEERCRED | 非 app uid 进程连 socket | 被拒 |  host shell + su （若无 root 则 mock） |

**环境**：API 35 x86_64, SwiftShader, 2GB RAM, `adb` 已设置（emulator-5554）

### 4.4 性能与回归

| 指标 | 基线 | 目标 | 采集 |
|------|------|------|------|
| cargo test 暖缓存 | 76s | <120s (CI) / <80s 本地 | time |
| 模拟器帧率 | 未测 | 90fps+（dumpsys 90th <11ms） | gfxinfo |
| CellRun 合并率 | 0 | >50% 同格式 | 单测 runs |
| JNI 往返 | N/帧 | <N/2 同格式 | bench |
| contentDescription 更新 | N/A | <1ms/帧 | Trace |
| mouse encode | N/A | <0.1ms | bench |
| 回归 | - | PTY spawn/resize/选择/复制/搜索/键栏/主题/bootstrap 均正常 | 现有 50+ instrumented |

### 4.5 验收标准（量化,可自动化判定）

| ID | 标准 | 验证命令 |
|----|------|----------|
| C1 | vim mouse 1000 下点击有效 | `adb shell input tap` 后 UIAutomator 或 PTY 日志含 SGR |
| C2 | 非 mouse mode 零事件 | Rust 单测 gate |
| C3 | TalkBack contentDescription 含可见文本 | `adb shell uiautomator dump` + grep |
| C4 | getLastCommandOutput 正确 | Rust + Kotlin 单测 |
| C5 | CellRun 同格式 1 run | Rust 单测 |
| C6 | 全部现有测试绿 | `cargo test --lib` 1000+ 通过, `./gradlew testDebugUnitTest` 通过 |
| C7 | 模拟器 90fps+ | `dumpsys gfxinfo` 解析 |
| C8 | 零 clippy 新增 | `cargo clippy` |
| C9 | 零 detekt 新增 | `./gradlew detekt` |
| C10| 依赖可更新无破坏 | `cargo update` + 全测绿 |

---

## 5. 模拟器 90fps+ 验证方案

**前提**：`adb devices` 显示 `emulator-5554 device`（API 35，已验证）。模拟器用 SwiftShader（CPU Vulkan lavapipe），`VK_ICD_FILENAMES` 指向 mesa lvp，已在 flake.nix 设置；真机无需此。

**步骤**：

1. 构建：`nix develop --command cargo ndk --target x86_64-linux-android build` → `jniLibs/x86_64/libnative.so` 校验 NEEDED 含 libghostty-vt（若动态则复制）
2. APK：`./gradlew :app:assembleDebug` → `adb install -r app-debug.apk`
3. 启动：`adb shell am start -n com.termux/.MainActivity` → 等待 `PollEvent` 首帧
4. 预热：输入 `for i in {1..100}; do echo line $i; done` 使 scrollback 稳定
5. 采帧：`adb shell dumpsys gfxinfo com.termux reset` → 交互 5s（滚动/输入）→ `dumpsys gfxinfo com.termux framestats` 解析 `Flags` 与 `FrameCompleted` 耗时，计算 90th 分位 <11ms 判定 90fps+
6. 回归：重复横竖屏、键盘弹出、搜索覆盖层场景，复测帧率无回退
7. 自动化：写入 `maestro/` flow（已存在）+ `ShellResponseLatencyTest` 扩展帧率断言，CI 模拟器 job 中执行

**备选**：若 gfxinfo 在 SwiftShader 下不稳，则用 `Choreographer` 帧回调自采（Kotlin `FrameMetrics`），与 gfxinfo 双轨互验。

---

## 6. 像素级复制 26 项目优点的执行清单（可验证）

| 项目 | 优点 | 复制方式 | 验证 |
|------|------|----------|------|
| zelland | ghostty_mouse_encoder 标准编码、行级脏缓存、get_cell_size 实时 | 复用 ghostty encoder，CachedInstances 已等价，cell 尺寸走渲染管线实时 | mouse SGR 单测 + dirty band 单测 |
| termlib | OSC133 语义段、CellRun、无障碍 | 轻量 SemanticSegment + runs 合并 + AccessibilityProvider | osc133/CellRun/talkback 单测 |
| termux-app | Callback2 精确定位、宽字符换算、wrap 感知 | TerminalSurface Callback2 已改 TYPE_FLOATING，SelectionExpander 已有宽字符逻辑，补测试 | 选区复制 CJK 单测 |
| Haven | smartCopy、expandAcrossUrlWrap、DECCKM | SmartCopy.kt + SelectionExpander 已有，补 DECCKM 预推 `ESC[?1h` | smartCopy 单测 |
| warp | PTY AS-safe、composing diff、winsize 先置、bootstrap 原子 | pty.rs 已 fork 前 CStrings，composing diff 供 InputConnection，winsize 阶段 5 实现 | spawn/resize 单测 |
| shashlik | is_emulator 双后端 | 保留 Vulkan 单后端，注释双后端备选 | 条件编译注释 |
| termux-kotlin | ArgumentTokenizer、SO_PEERCRED | shell-words + getsockopt | mcp 单测 |
| zed-port | EnvOp 分层、前台组杀、URL 正则 | shell_env overlay + pty 组杀 + hyperlinks 模块 | env/pty/hyperlink 单测 |
| wgpu-in-app | jni_fn 宏、acquire 重试 | ffi 手写导出名已稳定，wgpu_backend 补 Outdated/Lost 重试注释 | acquire 重试单测 |

> 每项复制均保留来源注释 `// 参考 <repo> file:line`，保持可追溯。

---

## 7. 代码精简与可靠性提升（保守改动）

- **去重**：SettingsComponents theme 逻辑抽 `themeCard` helper；BellHandler/MemoryMonitor 重复日志限频抽常量
- **强类型**：output_processor ShellIntegration 已枚举化，SemanticKind 同枚举；禁止新增 stringly-typed 状态
- **命名**：补全缩写（`cellW`→`cellWidth` 仅新代码，存量保持 FFI 兼容）
- **魔数消除**：`MAX_CAPTURE 64KB` 已命名；补 `DEBOUNCE 500ms`、`DEFAULT_SCROLLBACK 10000`、`MAX_DESCRIPTION 2000` 常量文档
- **外部依赖**：`shell-words` 取代手写分词；`foldhash` 已用于高频 HashMap；不引入 Slint/Bevy 等重依赖
- **测试先行**：每项修复先写单测锁定行为，再改实现；每个阶段结束 `cargo test + gradle test` 双绿才提交

---

## 8. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| ghostty_mouse_encoder API 缺口（SGR 边界） | 高 | 已验证 bindings 含 encoder/size/event 三件套；若缺则手写 SGR X10 回退（z*elland 备用） |
| TalkBack 刷屏 | 中 | 500ms debounce + 仅事件时 announce + diff check |
| OSC133 语义与现有 capture 冲突 | 低 | 复用现有跨 chunk 状态机，segment 与 capture 双写，互不覆盖 |
| CellRun 增量兼容 | 中 | 保留展开为 instances 路径，runs 仅计数；fallback 全量 rebuild |
| 模拟器帧率抖动 | 中 | 双轨 gfxinfo + Choreographer，取 90th 非 max，允许单帧毛刺 |
| 依赖升级破坏 | 低 | `cargo update` 后全量 1000 测试门控；gradle 用 dependencyUpdates 仅 patch/minor |

---

## 9. Backlog（P2/不吸收，记录不实施）

- 自绘放大镜、无限 LOD 网格、Slint/Bevy、proot 发行版、X11/VNC、AI Block 模型、SSH 客户端（若立项则升 P0 另起 change）、悬浮窗、开机脚本：记录于 `docs/reference/README §4 P2`，本次不实施，避免引入重依赖或与 libghostty-vt 冲突。

---

## 10. 交付与验证闭环

1. 每阶段 git 提交（conventional commits，一行简洁），推送 origin/main
2. 每阶段后 `cargo test --lib` + `./gradlew testDebugUnitTest` + `./gradlew detekt` + `cargo clippy` 四件套
3. 模拟器 90fps 脚本自动化（maestro + gfxinfo），产出 `docs/verification/2026-08-30-comprehensive-hardening-v7-verification.md`
4. 连续三次 code-review 无问题才视为完成（review/grill skill 循环）
5. 最终 `docs/plans` + `openspec` + `docs/verification` 三类文档齐全，测试计划可重复执行
