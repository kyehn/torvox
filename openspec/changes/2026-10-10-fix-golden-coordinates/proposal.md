## Why

对 `2026-10-09-add-vt-golden-corpus` 的独立审查发现一处真实正确性缺陷：`collect_styled`
接收的是单行切片（长度等于列数），却用 `index / cols` 求行号，因此行号恒为 0。
后果是第 1 行之后的所有样式单元都被记到第 0 行的坐标上：

- 已提交金标中 `sgr_bold_italic_color` 有 9 处、`shell_prompt_first_line` 有 10 处
  `(row, col)` 坐标重复，即两个不同单元被记在同一坐标；
- 样式内容在第 1、2 行之间同列移动时，`styled` 完全无差异，行级定位失效；
- 金标由缺陷代码的输出直接粘贴生成，`serde_round_trip` 又走同一条路径，因此套件全绿
  掩盖了缺陷。

同时审查指出两处不合规：

- `malformed_wgsl_reports_naga_diagnostic` 断言的是 naga 第三方行为而非本项目行为
  （`TESTING.md` 「只测试本项目功能」「不保留无意义或低价值测试」），且它只走
  `parse_str`、从未构造 `Validator`，规格中「使校验路径本身可验证」的理由不成立。
- `corpus_matches_expectation` 在语料目录为空时循环体不执行而断言通过，单独按名字
  过滤运行会得到假阳性的绿灯。

## What Changes

- `collect_styled` 改为接收行号并用 `enumerate()` 求列号，样式坐标回到真实网格坐标。
- 新增用例锁定「同一列、不同行」各有一条样式单元，防止该缺陷以任何形式回归。
- 重新生成受影响的两份金标，并逐项确认差异仅为 `row` 值与重复坐标消失。
- 着色器负例改为「可解析但校验失败」的 WGSL，使断言落在本项目的校验步骤上；删除原先
  只断言第三方解析器行为的用例。
- 语料运行器内置非空断言，`corpus_pairs_are_complete` 只保留配对检查，两者不再可能
  各自给出矛盾的结论。
- 样式差异改为按 `(row, col)` 建索引比对，避免插入单个单元导致其后所有条目错位。
- 移除未被引用的派生实现；语料循环中的 `expect` 改为聚合失败，不中断其余语料报告。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `terminal-state-regression`：样式坐标 MUST 为真实网格坐标；语料运行器 MUST 自身拒绝空语料。

## Impact

- `native/src/terminal/snapshot_test.rs`
- `native/src/render/pipeline.rs`
- `native/src/terminal/testdata/sgr_bold_italic_color.json`、`shell_prompt_first_line.json`