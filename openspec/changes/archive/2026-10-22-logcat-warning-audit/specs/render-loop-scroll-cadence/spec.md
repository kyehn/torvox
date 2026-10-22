# render-loop-scroll-cadence Delta

## ADDED Requirements

### Requirement: 空白默认背景单元不发射实例

整屏重绘时，实例构建 MUST 省略空白、背景等于主题清除色、无光标、无下划线／删除线／
上划线的网格单元；这些格的背景 MUST 由全量帧的 `Clear` 或部分帧的脏带清除铺好。

有显式背景色、反显、搜索高亮或装饰线的空白格 MUST 继续发射背景四边形；光标所在格
MUST 继续发射光标几何。省略 MUST NOT 改变任一场景的可见像素。

#### Scenario: 空白网格只发射字形

- **WHEN** 一行为 1 个 `A` 加 79 个默认背景空白格，且光标隐藏
- **THEN** 实例数为 1，而不是 80
      （用例 `blank_default_cells_do_not_emit_instances`）

#### Scenario: 有背景或装饰时不省略

- **WHEN** 空白格带非默认背景、光标、下划线或删除线
- **THEN** 该格仍发射背景四边形
      （用例 `blank_cell_quad_skipped_only_when_clear_color_and_undecorated`）
