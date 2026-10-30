# cursor-rendering Specification

## Purpose

光标几何。锚点为 termux 的 `TerminalRenderer.java`（`docs/specification/DESIGN.md` 要求
光标「高度、宽度等均参考 Termux」）：其光标矩形为
`canvas.drawRect(left, y - cursorHeight, right, y, mTextPaint)`，其中
`cursorHeight = mFontLineSpacingAndAscent - mFontAscent`、`mFontLineSpacingAndAscent =
mFontLineSpacing + mFontAscent`（`mFontAscent` 为负）⇒ termux 的光标矩形即整格。本仓沿用
「以整格作光标盒」的口径，但整格 = 本仓单元盒（`ascent + descent`），**不计**字体
line gap（理由见 `native/src/render/font/rasterization.rs` 的 `cell_metrics` 注释），
故光标高度不等于 termux 的 `mFontLineSpacing`，两者只在「整格」这一点上一致。

实现：`native/src/render/cell_builder.rs` 的 `append_row_instances`——光标盒就是该函数的
默认 `quad_origin` / `quad_size`，Block 分支只把背景换成光标色；Bar/Underline 在单元盒内
收缩为细标记（空单元格收缩那唯一的四边形，文本格在字形之后追加一个标记四边形），
同样不读字形位图。

## Requirements

### Requirement: 方块光标按整格几何

方块光标 MUST 是整格矩形：宽 = 单元宽 × 字形列跨度，高 = 单元高，顶边 = 单元顶边。
MUST NOT 跟随格内字形位图定尺寸——位图高度随字符变化，同一光标会随内容变高变矮、
越过行界；空单元格取 `'M'` 位图、文本单元格取该字形位图时两者还不相等。
宽度 MUST NOT 用字形 advance 推算（advance 与单元宽不等会留下缝隙）。

#### Scenario: 光标在空单元格与文本单元格上形状一致

- **WHEN** 光标从空白格移到该行任一字符上
- **THEN** 光标盒的顶边与高度不变，恒等于整格

#### Scenario: 光标在降部字形上不越出本行

- **WHEN** 光标停在 `g`、`j`、`中` 等位图高于或低于参考字形的字符上
- **THEN** 光标盒仍限制在本行整格内，不侵入上一行或下一行

### Requirement: 光标标记锚在单元盒

竖线（Bar）与下划线（Underline）光标 MUST 锚在单元盒，MUST NOT 锚在格内字形位图：
竖线的高 = 单元高、顶边 = 单元顶边；下划线的底边 = 单元底边、宽 = 单元宽 × 列跨度。
位图高度随字符变化，空单元格还得另取参考字形，锚位图会让同一光标在空白格与文本格之间
变高变矮。

竖线的宽 MUST NOT 乘列跨度：它是固定宽度的细条（`BAR_CURSOR_WIDTH_FRACTION`），乘了
列跨度会在宽字符上变成两倍粗的短横而不再是竖线；其横向覆盖由 `quad_origin` 锚在字形
左边缘达成——宽字符只有头列一条记录，该原点即字形原点。下划线无此顾虑，故按列跨度取宽。

#### Scenario: 竖线光标在空单元格与文本格上等高

- **WHEN** 同一个竖线光标依次停在空白格、`M` 与降部字形 `g` 上
- **THEN** 三处的顶边与高度相同，恒为整格

#### Scenario: 下划线光标贴单元底边

- **WHEN** 光标停在任一单元格
- **THEN** 下划线横条的下沿与单元底边重合，不侵入下一行

### Requirement: 宽字符光标几何

宽字符（`CellData::width == 2`）上的光标 MUST 覆盖整个字形宽度（两列），
MUST NOT 只覆盖首列。尾列（spacer）在单元层面不产出独立记录，但光标可以停在该尾列上：
渲染锚点 MUST 折算到该字形的头列记录，否则该帧匹配不到任何记录、不生成光标四边形。

实现：`build_cursor` 以 `col.saturating_sub(u32::from(at_wide_tail))` 归位；
`getCursorViewportPacked` 等可观测查询仍报 VT 的真实列。用饱和减法而非裸减：
网格与光标失配时可出现 `at_wide_tail` 为真而 `col == 0` 的陈旧数据，裸减在 debug
构建里 panic、在 release 里绕成 `u32::MAX`——后者把光标画到视口外，且再无任何记录
能匹配它。饱和减法的兜底语义是「宁可不画」。

#### Scenario: 光标停在宽字符上

- **WHEN** VT 光标落在某个宽字符的首列
- **THEN** 光标盒横跨该宽字符的两列，右边缘与字形右边缘对齐

#### Scenario: 光标停在宽字符的尾列

- **WHEN** VT 光标落在某个宽字符的第二格（`at_wide_tail`）
- **THEN** 渲染锚点等值于该字形头列上的光标，该帧仍生成光标四边形

### Requirement: 默认光标样式由配置提供，DECSCUSR 优先

`terminal-cursor-style`（`block` / `bar` / `underline`）MUST 只作为**初始**样式下发给 VT，
MUST NOT 在每次重绘或每次改配置时无条件覆盖当前样式。应用通过 `DECSCUSR`
（`CSI Ps SP q`）设置的样式 MUST 保持有效，直到应用自己再次改变它或被重置。

实现：Kotlin 侧把样式折算成 `setCursorStyle` 的原生枚举值（`block = 0`、`bar = 1`、
`underline = 2`）随 `RenderContract` 下发，JNI 转调 `Terminal::set_default_cursor_style`；
该 API 的语义是「默认值」，已由应用覆盖时不生效。

#### Scenario: 应用设置的竖线光标不被配置刷掉

- **WHEN** `terminal-cursor-style = block`，终端内运行 `printf '\033[5 q'` 切到竖线闪烁光标
- **THEN** 光标保持竖线闪烁；直到应用发 `\033[0 q` 才回到配置的方块

#### Scenario: 配置文件写错样式名不改变现状

- **WHEN** `terminal-cursor-style = blocky`
- **THEN** 该键被忽略、回落到默认值，其他键仍照常生效
