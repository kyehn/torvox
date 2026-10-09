## Context

`DESIGN.md` 规定以 Ghostty 为终端状态的单一来源、不重复实现其功能、上游跟踪 git master
rev 且无本地补丁。这三条同时决定了本轮所有取舍：引擎的既有行为不可修改，因此测试只能
核对不能修正；资产只能以数据形式引入并由既有 `cargo test` 运行。

`docs/specification/TESTING.md` 要求：只测试本项目功能、无不稳定测试、不检查环境
（缺失即自然失败而非跳过）、每个测试断言具体行为、新测试进入既有体系。

第一轮（`adopt-upstream-vt-test-assets`）已采纳 ghostty 种子语料与 alacritty 录音。本轮
在其之上补齐余下维度，并修正第一轮调研结论中的事实错误。

## 调研复核

对 10 个上游仓库重新克隆核实，修正如下事实错误（第一轮结论写错，已在归档 `design.md`
就地订正）：

| 项目 | 第一轮记载 | 实测 |
| --- | --- | --- |
| xterm | 「`test/` 三个 C 自检驱动」 | **根本没有 `test/` 目录**；自检是 `charclass.c` `ptydata.c` `wcwidth.c` 内的 `-DTEST_DRIVER` 块，且恒 `return 0`/`EXIT_SUCCESS`，永不失败 |
| kitty | 577 个用例 | 596 个 `def test_` 方法 |
| wezterm | `term`/`termwiz` 119 个用例 | `term` 58 + `termwiz` 53 = 111 |
| contour | 未调研 | 3 471 个 `TEST_CASE`；143 份 golden dump **只有期望屏没有输入字节**，不可重放 |
| foot | 「仅 `tests/test-config.c`」 | 正确，且补充：161 个内联用例、56 045 字节、零文件读取 |

第一轮「esctest2 因 FFI 缺 `xt_checksum_report` 而否决」的推理经复核成立：pinned
`libghostty-rs`（`8953a74`）的 `TerminalOption` 枚举逐项枚举核实后上限为 `34 = MODE`，
无 `44`/`45`。但第一轮把「缺该能力」当作唯一理由，实际还有三条更硬的：零数据文件、
GPL-2.0、需外部终端进程与 X11。已在归档 `design.md` 补全。

另外核实：esctest2 **不是** curses 程序（`grep -rn 'curses' esctest/` 仅命中注释与
`import tty`）；`ThomasDickey/vttest` 仓库不存在（404），须用 invisible-island.net 的
tarball；`ThomasDickey/xterm-snapshots` 是发布快照仓库。

## Decisions

### 字素簇断言三条不变式，而非 UAX #29 边界分组

用一次性探针（`receive_cell_data` 生产渲染通道）对 630 条可排布用例实测，得到三类偏离：

| 现象 | 条数 | 例 |
| --- | --- | --- |
| 零宽码位被并入前一单元，即使 UAX #29 判定此处断开 | 42 | `÷ 1F1E6 ÷ 1160 ÷` → 单元 `1F1E6+1160` |
| 簇内首个零宽码位被丢弃 | 79 | `÷ 094D+0903 ÷` → 单元 `0903` |
| Balinese/Sundanese spacing mark 的单元划分与权威分类不同 | 4 | `1B17 × 1B44 × 1B13` |

这三类都是引擎在**单元层面**的既有行为。要修正需 fork `libghostty-rs`（Cargo 对 git 依赖
无本地 diff 机制，只能 `[patch]` 到自有 fork），与 `DESIGN.md`「无本地补丁」「不重复实现
Ghostty 已有功能」直接冲突，且每次升 ghostty rev 都要 rebase。用户已确认不 fork。

故测试断言与分组无关、对全部 630 条精确成立的三条不变式：

1. **守恒**：权威表中占列的码位在网格里恰好出现一次（不丢不重）。
2. **保序**：读回的码位序列是输入序列的子序列（不乱序、不凭空产生）。
3. **列布局**：单元列区间自第 0 列起首尾相接，不留缝不重叠。

抖动自检：丢弃输入首码位 → 426 处守恒失败；反转读回序列 → 400 处保序失败；把推进量
改为 `width + 1` → 列布局失败。三条断言均确认会失败，非恒真。

### 宽度分歧用精确集合断言，而非排除名单

同样用探针实测较新文种区段，初测得 54 个分歧、其中 46 个为 `Mn` 非间距标记，据此写下
「uucode 表陈旧导致 45 个标记被误判占列」。**该结论是探针自身的 bug**：`engine_widths`
用 `.filter(|cell| cell.codepoint == '#').count()` 统计 `#` 出现次数，任何含 `#` 的行都返回 1，
于是把每个零宽码位都误报成占 1 列。改用 `.position(...)` 取列号后重新实测，真实分歧
只有 8 个：

- 7 个 `Mc` 间距标记：U+1B35 U+1B3B U+1B3D U+1B43 U+1B44 U+1BAA U+11F41
- 1 个 `Lo`：U+11F02（KAWI SIGN REPHA，虽登记为 `Lo` 但同样占位）

`East_Asian_Width` 均为 `N`，`unicode-width` 按不可见处理，引擎按「间距标记需要横向空间」
判 1 列——属约定差异而非缺陷。这些码位全部落在 `RECENT_SCRIPT_RANGES` 内，故逐码位登记，
不做范围通配。

断言方式是「精确集合」而非「跳过名单」：集合外的码位必须与权威分类一致，集合内的码位必须
恰好偏离引擎宽度 1。新出现的分歧报「未登记的偏离」，已被修复的分歧报「已登记但不再成立」。
抖动自检：往集合里加一个假码位 → 报「已登记但引擎给出 0 列」；从集合删掉一个真码位 →
报「未登记的偏离 1 / 180694」。两个方向都确认会失败。

同时订正第一轮 `design.md` 的两处数字：「180 915 个码位」实为 180 374（多计未分配码位），
「实测仅 3 个偏离」实为零偏离。

### Kitty 传输命令的 `s`/`v` 是像素宽高

写载荷用例时先按协议文档直觉把 `s=1` 当「显示」、`v=1` 当「虚拟放置」，结果所有大于 1×1
的载荷一律不入库。逐项读上游 `graphics_command.zig` 后发现，在 **`Transmission`** 结构里
`s` 是 width、`v` 是 height（像素），而「显示/虚拟放置」属于另一个结构（`Display`）。
`LoadingImage.complete()` 要求 `width * height * bpp` 与载荷长度严格相等，不等即
`error.InvalidData`，这解释了为何只有 1×1 能过。

另有两处踩坑记入注释：`o` 键的 zlib 取值是字母 `z`（`'z' => .zlib_deflate`）而非数字；
`w`/`h` 键在传输命令里不被解析（它们属于 `Display`），设了也不生效。

### 并发用例移入独立测试目标

`shuttle-engine` 的 `init_panic_hook` 用 `Once` 安装全局 panic 钩子，钩子内部先调
`ExecutionState::failing_task()`，该调用在非 shuttle 线程上会自己 panic，而
`original_hook(panic_info)` 在其后——于是同一进程内**其余全部测试**的失败信息都被
`Tried to get ExecutionState, but got the following error: NotSet` 顶掉。本次调研中
亲眼见到 `conformance_screen_matches_reference_engine` 的真实失败原因被掩盖。

可选修法有三种：guard 卸钩子（第二个用例失败时 schedule 不落盘，无法重放）、换掉 shuttle
（丢掉交错探索能力）、移到独立 `[[test]]` 目标。选第三种：进程隔离，钩子作用域限于本进程，
571 个单元测试恢复真实报错，shuttle 保留全部能力。

### UCD 版本固定 18.0.0

UCD 每年发版。若 shellHook 跟随 `latest`，新版发布会让测试在无人审阅时变红，且难以定位
是哪条用例变化。固定版本号使测试可重复，升版由显式提交完成并审阅新增用例。

### 图像载荷取自构建时的 ghostty 版本

载荷 URL 固定在 `22d13172cde98a0a4dda05d3d6a3fcb0dd8ed018`——正是 `libghostty-vt-sys`
的 `build.rs` 硬编码的 `GHOSTTY_COMMIT`，即本项目实际编译链接的那个 ghostty 版本。
取 `main` 会让载荷与引擎错配。

## Consequences

- 语料从 10 + 94 + 27 = 131 份增至再加 630 条 UCD 用例与 320 个新宽度码位。
- 宽度核对码位数由 180 374 增至 180 694；新增 8 个码位的精确分歧登记。
- `native/src/prop_tests.rs` 删除，文件头注释（提到它）随 `lib.rs` 更新。
- `flake.nix` 属 AGENTS.md 禁止修改清单，本次已获用户明确授权（只加 `curl` 与两处下载，
  不改任何既有条目）。
- UCD 与图像载荷落在 `/tmp`，不入库：`/tmp` 是 shellHook 的既有惯例（alacritty-theme
  已如此），且符合「避免 vendor」；代价是缺失时测试失败而非跳过——这正是 TESTING.md 要求。
- 已知未修的引擎偏离（42 条 GCB 边界、79 条簇首零宽码位丢弃）记录在
  `openspec/specs/upstream-vt-test-assets/spec.md`，作为上游 rev 升级时需复查的清单。
