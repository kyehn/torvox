# 29 减码杠杆与 libghostty 家族结论收敛

取样口径：上游为 ghostty `main@main`、rio `main@main`；本仓为 `main` 快照。行数/提交数为取样时实测，仓库在演进，非恒定值。上游仓库取自 <https://github.com/XYenon/termux-app-ghostty> 与 <https://github.com/termux/termux-app> tag `v0.119.0-beta.3`，以及 <https://github.com/ghostty-org/ghostty>、<https://github.com/raphamorim/rio>。

## 一、血缘与性能结论

- XYenon 是 vanilla 的最小增量 fork：删 `TerminalRenderer.java`（Canvas 逐格），新增 ghostty JNI（`ghostty_engine.cpp` 2298、`ghostty_renderer.cpp` 2325）+ `ghostty-android.patch`（709）+ 其他原生（`termux.c`/头/CMake），Java 净增 4105 行；作者增量约 1 万行，凭空继承 vanilla 约 4.5 万行 Java。原生栈 C++/Zig/Vulkan，minSdk29、compileSdk30、4 ABI、version `0.200.0-beta.1`、`TERM=xterm-ghostty`。
- 本仓（terminal）为全新重写、零复用 termux Java：生产 Rust 约 2.48 万 + Kotlin 约 1.87 万 ≈ 4.35 万行；自 5 月起累计约 1082 次提交。Rust/wgpu + libghostty-rs，minSdk33、compileSdk37、2 ABI、version `0.1.0`、`TERM=xterm-256color`。
- 性能：两者均为原生 GPU（Vulkan），都数倍于 Canvas 逐格。端到端延迟本仓结构略优——PTY+解析+渲染全在 Rust、只跨 JNI 传轻量事件、FFI 往返更少；手写原生 Vulkan 与 FreeType+HarfBuzz 整形 XYenon 略优。**无真机榜单，不排定序**；本仓是唯一自带帧时序/像素验收者。

## 二、减码：现实与杠杆

本仓几乎无死代码可白捡：`allow(dead_code)`/`todo!`/`unimplemented!` 计 0，仅 1 处测试替身 `unreachable!`，风格由 detekt/ktfmt/clippy 把着。想砍到 XYenon 量级不可能（它白拿 4.5 万行），安全重构上限约 5–10%。

分级（按 收益×安全）：

- **L1** 合并 JNI 接口为批量查询。ffi.rs 约 4.0k、61 个 `extern` 函数，其中约 18 个 getter、约 12 个标量查询（标题/回滚长/滚动行/单元格宽高/光标/网格/alt-screen/会话数/search 上限/光栅范围）。承 DESIGN「部分接口支持批量查询以保证性能」。预估 −600~−1000，顺带降 FFI 往返。
- **L2** Rust/Kotlin 状态单源化，删两侧镜像。
- **L3** 内联一次性包装（约 10 个小 helper 共约 660 行中的单次使用者；`KeyModifiers`/`ComposingBaseline` 等有领域语义者保留）。
- **L4** 简单设置项声明式化；字体对话框/主题预览/Shell 入口框等定制项保持手写。
- **L6** 复核 `internal.rs`（约 2.9k）有无重复 libghostty-vt 已暴露能力。
- 字体子系统约 5.5k 为最大单块；向 cosmic-text/swash 收敛需先 spike，风险自评。

红线（不得触碰）：不删/减测试、不降覆盖、不弱化断言、不 skip；不删 DESIGN 功能。每步验证门：`cargo ndk` 重建 + `cargo test`（lavapipe）+ `testDebugUnitTest` + 定向 `connectedDebugAndroidTest`（font/selection/search/session）+ detekt/clippy 通过；按 STYLE 只跑针对性用例。想「大幅度」（≥25%）只能改 DESIGN 收窄规格（砍 kitty / OSC 集 / CJK 硬对齐 / 自研 bootstrap 之一），属产品决策，需单独立项。

## 三、libghostty 家族与本仓位置

libghostty 家族实测两层均已存在：

- **libghostty-vt**：`src/build/GhosttyLibVt.zig` → `src/lib_vt.zig`；头 `include/ghostty/vt*.h`；无头、跨平台（含 Android）、C ABI。本仓经 `libghostty-rs` 使用，`native/Cargo.toml` 跟踪其 master 修订、开启 `log`/`png`、无本地补丁。
- **完整 libghostty**：`src/build/GhosttyLib.zig` → `src/main_c.zig`；头 `include/ghostty.h`（`ghostty_app_t/config_t/surface_t/inspector_t`、剪贴板、检查器），胖静态库 + pkg-config；为移植「应用运行时」。核验 `ghostty_platform_e` 仅 `INVALID/MACOS/IOS`，**当前仅 Apple 平台，对本仓（Android）无收益**。

vt 面已很大，本仓已消费 key 编码（`libghostty_vt::key`）、search、selection、render 等；仍可贴近 `render/color_scheme/size_report/modes/screen`。据 [docs/libghostty.md](../libghostty.md)：libghostty-rs 公开项 718，本仓引用 201，未用 517（整模块未接线如 `snapshot`/`selection::gesture`/`osc`/`sgr`/`focus`/`build_info`/`alloc`）。

## 四、libghostty-vt 对比 librio

librio 真实存在（rio workspace 成员），自述「Embeddable terminal core ... exposed as a C ABI」，`crate-type=["rlib","staticlib"]`、`publish=false`（须从 rio 仓库自建，不上 crates.io）；依赖 `rio-vt`（自带 VT，非 alacritty_terminal）、`rio-fonts`（nerd 符号）、`rio-graphics`；含 PTY（`teletypewriter`/`corcovado`）与 kitty 图形（`image_rs`）；有 `librio-wasm`。同族 `sugaloaf`/`libsugaloaf`（wgpu 字体/字形/渲染引擎）、`rio-backend`/`grid`/`window`/`notifier`。

libghostty-vt 对本仓的优势：分发（有版本、可 pinned、已构建 C 产物 vs librio 源码自建）；维护规模（Ghostty 62k★ + `test/fuzz-libghostty` parser/stream/osc 模糊测试 + 正式发版 vs Rio 更小）；无头 C ABI 且已在 Android 双仓（XYenon、本仓）验证；DESIGN 单一来源锁定。librio 的可取点：`sugaloaf`/`libsugaloaf` 是最贴近本仓 render/font glue 的外渲引擎（Rust + wgpu）。

「比 libghostty 更完整的库」分轴看：成熟度上 `alacritty_terminal`（更久经沙场）与 `wezterm-terminal`（自带字体引擎 + mux，功能更广）可 rival/exceed；整机含 GUI 上完整 libghostty 最全但 Apple-only。**对本仓确切约束（Android + Rust 宿主 + wgpu + 无头 + C ABI + kitty 图形 + 单一来源），无「更完整又能直接换」者**；能借来减码的不是换 VT 核，而是（a）多接 libghostty-vt 已导出面、（b）评估 sugaloaf 承接 render/font。

## 五、可参考点（openspec 落点）

- **P0**：L1 JNI 批量合并（DESIGN 背书、降风险、提性能）。docs/libghostty.md 的 517 未用项中挑 true-positive 就近接入（`size_report`/`color_scheme`/`modes`/`screen`）。已在途变更不重复立项。
- **P1**：librio 的 render-state pull API + staticlib 打包思路作为设计参照（思路，非照搬代码）；L2/L3/L4/L6。
- **P2**：sugaloaf/libsugaloaf 承接 render/font 的有界 spike（须 lavapipe 基准 + 复现 DESIGN 字体保证才动）；font 向 cosmic-text/swash 收敛 spike。

## 六、禁止项勿抄

- 不 paste/vendor wezterm/rio/alacritty 源码：违 BUILD.md（可自动获取的文件不得 vendor、不得作代码片段/文档段落引入）与 STYLE（不引入许可证文件、必须最小实现、不留死代码）。净省≈0、维护负担大。
- 不为此换 VT 核（`alacritty_terminal`/`wezterm-terminal`/`rio-vt`）：违 DESIGN 单一来源；且非贴合本仓无头 C ABI 模型。
- 不等完整 libghostty 上 Android：当前 Apple-only。
- 不减测试/覆盖/断言，不删 DESIGN 功能；不用 `git checkout`/`restore` 动会话外改动文件（本轮仅落本文档）。

## 七、核验记录

- 本仓：提交数、生产 Rust/Kotlin LOC、font 模块 LOC、ffi `extern` 函数数、死代码计数，均为取样时实测（脚本见执行命令，结果随仓库演进）。
- 上游：curl 核验 ghostty `include/ghostty.h` 的 `ghostty_platform_e` 仅 `INVALID/MACOS/IOS`；`librio/Cargo.toml` 为 C ABI 且 `publish=false`；`src/build` 下 `GhosttyLib.zig` 与 `GhosttyLibVt.zig` 双目标并存。
- XYenon 侧为 `/tmp` 冻结克隆相对 vanilla 的 `diff` 实测（新增/删除/改动文件数与关键文件行数）。
