# 修掉 OSC 52 削减路径的锁误用与宽字符光标几何的失实描述

## Why

深度通读 native/ 时发现三处确凿缺陷。

**其一，洪泛削减路径骑在注册表写锁上做无上界阻塞写。**
`handle_clipboard_reads` 的削减分支只查表，却取 `wlock_session_registry()`；随后
`answer_clipboard_read` 经 `PtyMaster::write_with_timeout` 等 `pty_write_order` 互斥量，
而该锁可能被用户输入写入按住、持有时长**无上界**。持注册表**写**锁等它，会把
`pollEvent`、`switchSession`、`destroySession` 连同其余会话的取句柄一并冻在锁外——
本文件余下部分与 `pump_sessions` 一律在注册表锁外做会话工作，此处是唯一例外。

同一分支还有一层自伤：`unanswered_clipboard_reads` 锁 `REQUEST_REGISTRY` 并遍历全表，
对**每条** pending 请求各调一次，而封顶判断在它**之后**。一帧可含数千条读请求，于是
数千次全表扫描——洪泛削减本是为挡住病态远端，不能自己先成为瓶颈。

**其二，`build_cursor` 的宽字符归位会下溢。**
`col - u32::from(at_wide_tail)` 在网格与光标失配（`at_wide_tail` 为真而 `col == 0`）时，
debug 构建 panic、release 绕成 `u32::MAX`，后者把光标画到视口外且再无记录能匹配它。

**其三，宽字符光标几何的注释与实现对不上。**
`append_row_instances` 的注释称「Bar 标记按 `cell_span` 缩放正是让光标覆盖两列的手段」，
但两处实现（空单元格与文本格）都只取 `cell_width * BAR_CURSOR_WIDTH_FRACTION`，
未乘 `cell_span`；同作用域的 Block 与 Underline 则都乘了。三处里两处乘、一处不乘，
而规范 `cursor-rendering` 的「宽字符光标几何」只写了「MUST 覆盖整个字形宽度」，
没交代竖线该不该随列跨度变粗——规范本身在此有歧义。

## What Changes

- `native/src/android/ffi/render/clipboard.rs`：削减分支改用 `session_handle`（读锁），
  取到 `Arc` 后 guard 立即释放，写 PTY 发生在锁外。另加帧内 `shed_counts` 缓存——
  只缓存**已超限**的会话：削减分支不注册槽位，故其计数在本帧内不变；未超限的走正常
  分支会注册槽位使计数 +1，缓存会失准，故逐条重算（每会话至多
  `MAX_UNANSWERED_CLIPBOARD_READS_PER_SESSION` 条走这条路径）。
- `native/src/android/ffi/render/overlay.rs`：`build_cursor` 的归位改饱和减法，
  注释写明兜底语义是「宁可不画」。
- `native/src/render/cell_builder.rs`：注释改为如实描述——三种样式里只有方块与下划线
  把 `cell_span` 乘进宽度，竖线不乘。
- `openspec/specs/cursor-rendering/spec.md`：补齐竖线宽度的规范（MUST NOT 乘列跨度，
  横向覆盖由 `quad_origin` 锚在字形左边缘达成），消除上述歧义；归位改述为饱和减法。
- `native/src/render/context/renderer.rs`：描述 `mark_surface_invalidated` 的文档块误挂在
  `can_reuse_attached_surface` 上（其中 intra-doc 链接亦指向错函数），被描述者本身反而
  无文档——拆回各自头上。
- `native/src/render/font/pipeline.rs`：重建缓存时用字面量 `128` 而非
  `ASCII_UPPER_BOUND`，常量一改即静默失配，改用常量。

## Impact

- 受影响能力：`cursor-rendering`、剪贴板读取（OSC 52）、`render-stability`。
- 行为变化：洪泛削减不再冻住其余会话操作；宽字符尾列光标在失配数据下不再被画到视口外。
- 无渲染视觉变化：竖线宽度维持原值（注释与规范原本即与实际不符，改的是描述而非行为）。
