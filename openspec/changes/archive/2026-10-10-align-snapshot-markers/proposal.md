## Why

第三轮独立审查发现四处问题，均属「判定标记与颜色不自洽」或「失败处理破坏聚合语义」：

- `GridSnapshot::fallback` 用 `CellSnapshot::default()` 填充，颜色为 `[0,0,0,0]` 而两个
  标记为 `false`，与 `faccfcc7c` 在其余构造点确立的不变量相反。
- `build_snapshot` 的背景色取自 `cell.bg_color()`，标记却取自 `style.bg_color`；两者
  并非同源表达式，在带内容标记的单元上可以不一致。
- `index_styled` 用 `assert!` 报告重复坐标，panic 会中断语料循环，使其余语料的诊断与
  「应写入内容」全部丢失，与运行器刻意实现的聚合语义相悖。
- 规格仍描述已删除的畸形 WGSL 用例，并错误地把非空断言归给配对检查用例。

## What Changes

- 回退网格显式标记为默认色，与颜色自洽。
- `build_snapshot` 的两个标记改由取色所用的同一表达式导出。
- 重复坐标改为差异项，随其余语料一并报告。
- 语料写入后以 `flush_with_timeout` 确认刷新，未确认即判定失败，避免查询回退被误报成
  内容不符；序列化只在失败路径执行。
- 规格删除失效条目并订正非空断言与调色板表述的归属。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `terminal-state-regression`：重复坐标 MUST 作为差异项报告；语料刷新 MUST 得到确认。

## Impact

- `native/src/terminal/ghostty_terminal/types.rs`、`internal.rs`
- `native/src/terminal/snapshot_test.rs`
- `openspec/specs/render-stability/spec.md`、`terminal-state-regression/spec.md`
