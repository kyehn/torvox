## Why

独立审查指出：判定「单元未设置样式」原先通过比较解算后的颜色与 `DumpedGrid` 导出的默认色
实现，这是代理判据而非真实语义。后果有两处：

- 任何「默认色恰好等于自身某个 ANSI 调色板项」的主题，会把显式 `SGR 37`/`SGR 40` 的单元
  误判为无样式，从期望文件中静默丢弃。`默认前景 = palette white` 是常见主题取法。
- `resolve_style_color` 对 `StyleColor::Palette(idx)` 在取不到调色板时 `unwrap_or(default)`，
  显式的 `SGR 38;5;N` 同样会被记成默认色。

判据所需的原始信息本就在手边：`apply_style_to_snapshot` 接收 `&style`，其中
`StyleColor::None` 即「未设置」，与调色板取值无关，也不需要浮点比较。

## What Changes

- `CellSnapshot` 增加 `foreground_is_default` / `background_is_default`，在样式套用处按
  `StyleColor::None` 直接判定。
- 快照侧改用该判定，删除浮点相等比较。
- 随之删除 `DumpedGrid` 的 `default_foreground` / `default_background`：它们只服务于该
  代理判据，保留即冗余。字段数净增减为零，判据由启发式变为精确。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `terminal-state-regression`：未设置样式的判定 MUST 基于原始样式而非解算后颜色的数值比较。

## Impact

- `native/src/terminal/ghostty_terminal/types.rs`、`internal.rs`、`public_api.rs`、`tests.rs`
- `native/src/terminal/snapshot_test.rs`
- 已提交金标内容不变：当前调色板下两种判据结论一致，本次只消除潜在陷阱。
