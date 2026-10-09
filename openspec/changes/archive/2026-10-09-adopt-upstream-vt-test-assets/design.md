## Context

本项目的终端状态由 `libghostty-vt` 提供（`DESIGN.md`：以 Ghostty 作为终端状态的单一来源，
不重复实现 Ghostty 已有功能），渲染在 Rust 侧由 `wgpu` 完成，测试体系由 `docs/specification/TESTING.md`
规定：只测试本项目功能、无不稳定测试、不跳过、不隐藏错误、进入 `cargo test` 等既有体系。

`AGENTS.md` 将 `flake.nix` 列入禁止修改清单，因此「把上游工具加进开发环境」这一路径整体关闭：
任何需要新的宿主工具或需要 clone 上游源码才能运行的测试都无法落地。这直接决定了对各上游资产的取舍。

约束叠加后的可采纳形态只有一种：**以数据形式纳入、由既有 `cargo test` 运行、不引入第二套终端引擎**。

## 调研范围与结论

调研 kitty、rio、wezterm、alacritty、ghostty、xterm、foot、vttest、esctest2、contour 十个项目。
表内数字为逐仓库克隆后实测所得（`git clone --depth 1` 后按内容统计命令核对）。

| 项目 | 主要测试资产 | 结论 |
| --- | --- | --- |
| ghostty | `src/terminal` 下 3 239 个 Zig 内联用例；`test/fuzz-libghostty/corpus/` 4 001 份原始字节；`src/terminal/kitty/testdata/` 5 份图像载荷；`test/esctest/` | 采纳手写种子语料与图像载荷 |
| alacritty | `alacritty_terminal/tests/ref/` 45 份录音 + 网格期望 | 采纳 31 份录音 |
| rio | `rio-vt` 566 个用例，全部 crate 内 `#[cfg(test)]` | 不可达，否决 |
| wezterm | `term` 58 个、`termwiz` 53 个用例，全部 crate 内 `#[cfg(test)]`；`test-data/` 21 份零引用的人工目检素材 | 不可达且引入第二套 VT，否决 |
| kitty | `kitty_tests/` 596 个用例，期望值为内联 Python 断言；数据文件仅 `GraphemeBreakTest.json` 带期望值 | GPLv3，改取 Unicode 官方 UCD 数据 |
| foot | 仅 `tests/test-config.c`（56 045 字节，161 个内联用例，零文件读取） | 无 VT 资产，否决 |
| xterm | **无 `test/` 目录**；自检为 `charclass.c` `ptydata.c` `wcwidth.c` 内的 `-DTEST_DRIVER` 块；`vttests/` 61 个脚本为人眼观察 | 无机器可比对数据，否决 |
| contour | `src` 下 3 471 个 `TEST_CASE`，其中 `vtbackend` 1 670 个；143 份 golden dump **只有期望屏没有输入字节** | 期望值不可用，否决 |
| vttest | 交互式全屏 TUI；唯一数据文件 `tech.set` 无任何代码读取 | 不可无头驱动，否决 |
| esctest2 | 79 个测试模块 559 用例，期望值全内联，零数据文件；pty 反转向下驱动外部终端进程并用 DECRQCRA 核对 | 见下 |

### 否决 esctest2 的理由

esctest2 是覆盖最广的 VT 一致性套件，且 ghostty 已用 `test/esctest/` 证明它可以无头驱动
libghostty-vt。不可采纳有三重独立原因，任一都足以否决：

1. 零数据文件：esctest2 仓库除 `LICENSE` 与 `README.txt` 外全是 `.py`，559 个期望值内联在
   用例方法体里，没有可复用的数据；且许可为 GPL-2.0（无「or later」）。
2. 上游 pinned rev 的 FFI 未导出其必需能力：esctest2 靠 DECRQCRA 校验和核对屏幕，而
   `libghostty-rs` pinned rev（`8953a74`）的 `TerminalOption` 枚举逐项枚举核实后上限为
   `34 = MODE`，既无 ghostty master 才有的 `xt_checksum_report`（44）也无
   `xt_checksum_extension`（45）。没有该校验和，esctest2 无法核对任何屏幕状态。
3. 非无头：esctest2 经 `tty.setraw(stdin)` 直接驱动 `/dev/tty`，每个用例都要求 pty 另一端
   是**已在运行的终端进程**，并依赖 X11（`xwininfo -window-id`），`cargo test` 两者皆无。
4. 文档化的失败面：libghostty-vt 未实现 DECSTR，esctest 每个用例前依赖它复位；终端不响应
   XTWINOPS/DECCOLM 缩放，相关用例必然失败。ghostty 自己的 CI 对该步骤 `continue-on-error`，
   即以「不失败」的方式运行；这与 `TESTING.md`「不存在跳过，不得隐藏错误」冲突。

结论：即使将来允许引入 Python 宿主工具并放宽许可，缺 44/45 两项 FFI 能力这一条仍足以否决。

### 否决 rio 与 wezterm 的共同理由

两者的 VT 测试都写成 crate 内 `#[cfg(test)] mod test;`，发布物不含这些模块，外部无法引用其
测试辅助。同时二者各自带一套 VT 实现（`vtparse` + `wezterm-escape-parser`、rio 的 `Crosswords`），
引入即违反「不重复实现 Ghostty 已有功能」，并带来约 30 个传递依赖。

wezterm 的 `test-data/`（21 份、499KB）另有独立否决理由：逐份核对后无任何 `.rs` 引用，
是供人工目检的素材，且其断言值内联在 `termwiz` 的 Rust 测试体里，数据文件不含期望值。

### 否决 contour 的理由

contour 是本轮新增调研项目，golden dump 数量可观（143 份、Apache-2.0），但每份 dump 只序列化
**期望**终端屏幕，不含产生该屏幕的 VT 输入字节。没有输入就无法在本项目里重放，也无法与
`libghostty-vt` 的实际输出比对；其 `TEST_CASE` 本身全部是 C++ 源码内的 `CHECK` 断言。

## Decisions

### 种子语料取 `-initial` 而非 `-cmin`

`corpus/parser-cmin`（616 份）与 `stream-cmin`（3 271 份）共 26MB，是 AFL++ minimizer 生成的
中间产物，其内容等价性依赖 minimizer 实现，可读性为零。三组 `-initial` 共 94 份、实际内容
5.2KB，是上游手写并按解析特性命名（如 `13-csi-sgr-256`、`26-osc-color`、`46-line-drawing`），
作为输入集的信噪比高得多。`du` 显示的 384KB 是 96 个 4KB 块的块开销，不是内容体积。

### 去掉种子首字节

三个 fuzz 目标各自用输入首字节选择目标内部路径：`fuzz_stream.zig` 取 `input[0]` 的奇偶在
`nextSlice` 与 `next` 之间切换，`fuzz_osc.zig` 取 `input[0] % 3` 选择 BEL/ST/无终止符。
这些选择器是给上游 harness 的，不是给终端的。导入时去除，否则 `stream-initial` 里的 `\x00`
等字节会改变终端输入语义，且「与上游同一份字节」的意义丢失。

### 分块写入不变性是本项目自己的属性，不是上游属性

`GhosttyTerminal::vt_write` 经有界命令通道（容量 1024）以 `try_send` 投递，满则丢弃
（`public_api.rs:151`：VT 线程卡住时不得无限期阻塞调用方）。因此「任意切块的最终状态一致」
只有在每块都被确认排空后才成立。该约束本身就是本项目必须保证的性质：PTY 读取按块到达，
IME 提交按小写入到达，两种路径都走同一通道。

实验确认：94 个种子在「整块」与「逐字节 + 逐块 flush」下网格文本、光标、尺寸、回滚区全部一致。
未逐块确认的逐字节写入会因通道丢弃而差异，曾据此一度误判为上游解析缺陷——这也是该测试必须
自带确认步骤的原因。

### 语音料只断言屏幕文本，不断言样式与回滚

45 份录音全部跑通后的比对结果是 31 份完全一致、14 份不一致。不一致的原因逐份可解释且全部
属于引擎差异，而非本项目缺陷：

- 列宽切换：`vttest_*` 发送 DECCOLM，本引擎不缩放（`dump_grid` 列数停在 80）。
- 选择性擦除：`selective_erasure` `erase_in_line` 依赖 BCE 行为差异。
- DEC 特殊图形：alacritty 把 `_` 映射为空白，本引擎不是。
- 制表符填充：alacritty 的单元存 `\t`，本引擎存空格。
- 回滚上限：alacritty `history_size=0` 时丢弃滚出内容，本引擎按 page 粒度保留历史，
  即使上限为 0 也会残留（FFI 文档明确「实际可用行数几乎总比配置值高」）。

`grid.json` 的解析是本次唯一的技术坑：`raw.inner` 是**最新行在前**（bottom-up），可见屏恒为
末尾 `screen_lines` 行，`display_offset` 是视图回滚量。按自然序解析会得到 0/45 一致，
按 bottom-up 解析得到 31/45。31 份一致的样本纳入语料，14 份差异在资产说明中逐份记录原因。

跨引擎语料因此只断言屏幕文本、行序、光标与尺寸。断言样式需要两套调色板一致（alacritty 用
默认 256 色板，本项目用 `catppuccin_mocha_palette()`），断言回滚行数需要两套修剪策略一致，
两者都不是终端正确性而是解算与修剪策略差异。

### 运行器按期望文件创建终端

原运行器以 `CORPUS_ROWS`/`CORPUS_COLS`/`CORPUS_SCROLLBACK` 三个常量创建终端，语料必须适配
6×20。录音的期望屏幕是 3×10 到 96×174 不等的真实尺寸，适配固定尺寸会改变语义。改为期望文件
自带 `rows`/`cols`/`scrollback`，运行器按声明创建。这同时消除了一处硬编码。

### 宽度分类用 `unicode-width` 而非自造表

`unicode-width` 已在 `Cargo.lock`（`cosmic-text` 的传递依赖），声明为开发依赖不增加编译单元。
自造期望表等于把权威数据抄一份进仓库，与「低硬编码」相反。比对区段取已分配且上下界稳定的
CJK 表意、扩展、音节、兼容、彝文等区段，共 180 374 个码位，全部与权威分类一致。

测量方法：每行写「候选字符 + `#`」，`#` 所在列即占用列数；末行不能再写 `\r\n`，否则末行滚动
使该行读数为 0（曾据此产生每批一个假偏离）。

> 后续修正：本节原记「180 915 个码位、实测仅 3 个偏离」，两处数字均有误。3 个偏离实为
> 笔误——按上段方法实测 CJK 区段零偏离；180 915 是把未分配码位一并计入所致，逐段求和
> 应为 180 374。

## Consequences

- 语料从 10 份增至 10 + 94 + 31 = 135 份，新增断言覆盖解析器边界构造、真实应用输出与
  18 万码位的宽度分类。
- `snapshot_test.rs` 的 `TestSnapshot` 增加 `scrollback` 字段，`SNAPSHOT_VERSION` 递增，
  存量期望文件需重生成；重生成仍走「测试失败并打印应写入内容」的人工流程，不引入重写开关。
- 新增两份资产的许可与来源说明，明确 MIT / Apache-2.0 与其在仓库中的形态（数据，非代码）。
- 已知未修的生产缺陷（`render/font/shaping.rs` 的 CJK span 区间表缺扩展 B/C/D/E/F/G、
  康熙部首、兼容补充）不在本次改动内：修改渲染行为需要真机验证，记录为后续项。
- 后续若 `flake.nix` 允许修改，esctest2 仍是最优先候选，需同时等待上游 FFI 导出
  `xt_checksum_report`。
