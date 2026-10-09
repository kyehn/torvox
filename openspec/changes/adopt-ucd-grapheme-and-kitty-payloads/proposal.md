## Why

第二轮调研（克隆 10 个上游仓库逐项核实）在既有采纳之外，发现三处真实缺口与一处体系缺陷：

1. **字素簇维度完全无权威覆盖。** `libghostty-vt` 的分簇是终端排版的核心行为，而本项目
   改动前只有 `combining_mark_reaches_grapheme_channel` 一个用例断言 `e` + U+0301 的合并。
   kitty、rio、wezterm 三家都测这个维度，但期望值或内联、或在 GPL 数据里。权威数据是
   Unicode 官方 `GraphemeBreakTest.txt`（Unicode 条款，非 GPL），可绕开许可问题直取。
2. **kitty 图像解码未经真实载荷检验。** 现有 4 个用例全是 1×1 手写像素，`image-rgb` 的
   真实尺寸、zlib 压缩、跨块分块传输三条路径零覆盖。上游有 5 份现成载荷（MIT）。
3. **宽度核对只覆盖中日韩区段。** 较新文种（Balinese/Sundanese/Kawi/Nag Mundari）的
   间距标记宽度与 `unicode-width` 不一致，此前无任何断言会暴露。
4. **shuttle 的全局 panic 钩子掩盖其余测试的失败信息**，与 `TESTING.md`「不得隐藏错误」冲突。

同时实测出引擎在单元层面并不遵循 UAX #29（见 design.md），故不能直接断言簇边界分组。

## What Changes

- 新增 `native/src/terminal/vt_grapheme_cluster.rs`：解析 UCD 18.0.0 的 853 条用例，
  剔除 223 条含控制码位的行后对 630 条逐条断言三条排布不变式（守恒、保序、列布局）。
- 扩展 `native/src/terminal/vt_width_classification.rs`：新增 7 段共 320 个较新文种码位，
  并对 8 个间距标记的宽度分歧做**精确集合断言**（非跳过名单）。
- `native/src/terminal/ghostty_terminal/tests.rs`：新增两个用例消费上游 20×15 未压缩与
  128×96 zlib 载荷，断言逐像素相等、解压后尺寸与像素值。
- `native/tests/concurrency.rs`：把两个 shuttle 用例从 `#[cfg(test)]` 单元测试移入独立
  测试目标，用进程隔离消除全局 panic 钩子的影响。
- `flake.nix`：`packages` 新增 `curl`；shellHook 拉取 UCD 语料（固定 18.0.0）与 2 份
  kitty 图像载荷（取自 `libghostty-vt-sys` 构建时所用的 ghostty 版本）。
- 订正归档 change `design.md` 中与事实不符的调研结论。

不新增 crate features、不新增测试体系、不引入第二套终端引擎，全部进入现有 `cargo test`。

## Capabilities

### New Capabilities

- `upstream-vt-test-assets`：字素簇排布三条不变式、上游真实图像载荷解码、上游宽度分歧
  精确集合断言三项核对，以及并发测试的进程隔离约束。

### Modified Capabilities

- `terminal-state-regression`：并发不变量用例由 `#[cfg(test)]` 移至独立测试目标，
  并记录引擎在 UAX #29 分簇上的既知偏离。

## Impact

- `native/src/terminal/vt_grapheme_cluster.rs`：新增。
- `native/src/terminal/vt_width_classification.rs`：新增 `RECENT_SCRIPT_RANGES` 与
  `UPSTREAM_WIDTH_DIVERGENCES`，比对区段由 180 374 增至 180 694 个码位。
- `native/src/terminal/ghostty_terminal/tests.rs`：新增两个载荷用例与两个辅助函数。
- `native/tests/concurrency.rs`：新增；`native/src/prop_tests.rs` 删除。
- `native/src/lib.rs`：删除 `#[cfg(test)] mod prop_tests` 声明。
- `flake.nix`：`packages` 新增 `curl`；`shellHook` 新增两处 `curl` 下载。
- 不修改生产代码路径，不引入新的运行时依赖。