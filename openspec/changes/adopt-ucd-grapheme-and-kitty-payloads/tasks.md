# 任务

## 环境

- [x] `flake.nix` 的 `packages` 新增 `curl`（已获用户明确授权）
- [x] `shellHook` 按固定版本 18.0.0 拉取 `/tmp/unicode-ucd/GraphemeBreakTest.txt`
- [x] `shellHook` 从 `libghostty-vt-sys` 构建所用的 ghostty 版本拉取 2 份 kitty 图像载荷
- [x] 核对拉取内容与上游仓库逐字节一致

## 测试可靠性

- [x] 新增 `native/tests/concurrency.rs` 承载两个 shuttle 用例
- [x] 删除 `native/src/prop_tests.rs` 与 `lib.rs` 中的 `#[cfg(test)]` 声明，更新 crate 文档注释
- [x] 验证单元测试失败时输出真实断言：故意改错一处断言值，确认输出含该断言与实际值

## 字素簇不变式

- [x] `vt_grapheme_cluster.rs`：解析 UCD 记法（`÷` 断点 / `×` 不断点）得到期望簇
- [x] 断言守恒：占列码位恰好出现一次
- [x] 断言保序：读回序列是输入的子序列
- [x] 断言列布局：列区间自第 0 列首尾相接
- [x] 排除含控制码位的用例并说明理由
- [x] 语义抖动自检：丢输入码位、反转读回序列、改推进量各一次，确认对应不变式失败

## 宽度核对扩展

- [x] 新增 `RECENT_SCRIPT_RANGES`（7 段 320 码位）与区段划分断言
- [x] 新增 `UPSTREAM_WIDTH_DIVERGENCES` 精确集合（8 个间距标记）
- [x] 断言同时覆盖「未登记的偏离」与「已登记但不再成立」两个方向
- [x] 语义抖动自检：加一个假登记项、删一个真登记项各一次，确认对应失败

## Kitty 图像载荷

- [x] 读上游 20×15 未压缩载荷，断言逐像素 RGB→RGBA 相等
- [x] 读上游 128×96 zlib 载荷，断言解压尺寸、像素值与不同像素数
- [x] 传输序列按 4096 字节分块并断言确实走了多块路径
- [x] 语义抖动自检：改一处期望像素值与不同像素数各一次，确认失败

## 文档

- [x] 订正归档 `design.md` 的调研表：xterm 无 `test/` 目录、kitty 596、wezterm 111、补 contour
- [x] 补全归档 `design.md` 的 esctest2 否决理由（零数据文件 / GPL-2.0 / 非无头）
- [x] 订正归档 `design.md` 的宽度区段码位数（180 374 而非 180 915，零偏离而非 3 个）
- [x] 勾选归档 change 中实际已完成但未勾的两项任务
- [x] 新建本 change 的 proposal / design / specs

## 验证

- [x] `cargo fmt --check` 与 `cargo clippy --all-targets -- -D warnings` 无告警
- [x] `cargo test --workspace --no-fail-fast` 全量通过
- [x] `markdownlint-cli2` 无问题
- [ ] 运行 `scripts/check-rust.nu` 全量门禁
- [ ] 更新 `openspec/specs/` 下的正式 spec
- [ ] 归档本 change
