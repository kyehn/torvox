# 修正 CJK 单元宽度的测试口径

## Why

字体层用例 `han_is_wide_and_non_emoji` 断言「汉字位图宽度 > ASCII 位图宽度 × 1.5」，
并把它当成「汉字是双格宽」的证据。两者都不是事实：

- `GlyphInfo.width` 是**光栅化位图的墨迹像素宽**（`render/font/mod.rs` 的 `GlyphInfo`，
  取自 `swash::zeno::Placement`），不是终端**单元宽度**。单元宽度取自上游 VT 的
  宽度表，经 `CellData.width`（1 = 常规，2 = 宽）传到渲染层。
- 位图墨迹宽度随字体与字形而变：同一个汉字在 Noto Sans CJK 与在 Maple Mono NF CN
  下墨迹宽度不同，ASCII 的 `A` 也随主字体而变。该比值在开发 shell 里实测 13/9
  （=1.44 < 1.5），换一套宿主字体就可能翻面——这是一条随宿主漂移的用例，违反
  TESTING「不得存在不稳定的测试，测试必须可复现」。

该断言还与既有要求相抵触：`cjk-rendering` 的「CJK 字形按位图物理像素 1:1 取样」
规定 CJK 字形按自身位图宽度绘制，故位图宽度恰是**绘制宽度**；把它当作单元宽度
判据，等于把「画出来多宽」当成「占几列」。

同源的 `cjk_width_is_double_ascii` 更空：它按同一条错误比值算出 `cell_span`，
再断言 `cell_span >= 1`——恒真，不构成任何行为断言（TESTING「每个测试必须断言
具体行为与结果」「不保留无意义或低价值测试」）。

真正需要被钉住的两件事分别落在两层：

- **字体层**：汉字在主字体不覆盖时，必须由回退面解析出真实字形（非 `.notdef`），
  否则屏幕上就是豆腐块。
- **终端层**：宽字符占两列，其后的单元列号须跳过这两列。该契约已由
  `cell_iterator_cjk_double_width` 钉住，只是被放在 `mouse_osc.rs`（鼠标/OSC 用例
  文件）里，与内容无关。

## What Changes

- 删除 `render/font/tests/glyphs.rs` 的 `han_is_wide_and_non_emoji` 与
  `cjk_width_is_double_ascii`。
- 新增 `render/font/tests/fallback.rs` 的 `han_resolves_via_fallback_face_to_a_real_glyph`：
  主字体取不覆盖汉字的 Liberation Mono，断言汉字由**非主字体**解析到**非零字形号**，
  并光栅化出非空位图。`symbol_glyph_resolves_via_database_scan` 与它共用
  `assert_resolved_via_fallback_face` 助手。
- 把 `cell_iterator_cjk_double_width` 移到 `tests/grid_text.rs`（网格文本与列映射），
  并补一句说明：宽度取自上游 VT，本仓只按该宽度推进列号。
- `cjk-rendering` 增加要求「单元宽度取自上游，字形位图宽度不得充当宽度判据」。

## Capabilities

### Modified Capabilities

- `cjk-rendering`：单元宽度与位图宽度的职责边界。

## Impact

- 原生测试：`render/font/tests/{glyphs,fallback}.rs`、
  `terminal/ghostty_terminal/tests/{grid_text,mouse_osc}.rs`。
- 生产代码与对外行为不变；不改受保护文件。
