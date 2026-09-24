# Design

## Context

文本选择横跨 Kotlin 手势层（`TerminalSurface`/`TerminalViewModel`）、JNI 桥与 native（libghostty-vt）。选择状态已由终端持有（`terminal.set_selection` → `ffi::TerminalOption::SELECTION`，即 `GHOSTTY_TERMINAL_OPT_SELECTION`），坐标为绝对网格行（0 = 回滚顶部），`install_selection_impl`/`selection_text_impl` 已实现坐标 ↔ `Point::History/Viewport` ↔ `GridRef` 映射。菜单与控制柄走独立系统窗口 PopupWindow——系统 ActionMode TYPE_FLOATING 在 API-35 模拟器 SurfaceView 上实测不渲染（`TerminalSurface` KDoc 实证），此架构不可回退。动机见 proposal.md - Why。

## Goals / Non-Goals

**Goals:**

- 词/行/全选与长按选词统一走上游 `select_word`/`select_line`/`select_all` + `OPT_SELECTION` 安装，Kotlin 只消费返回的选区界限。
- 菜单在任何时刻不遮挡选区，拖动即隐藏、抬手/全选后按新几何重显。
- 删除全部死代码与未声明行为，代码量只减不增。

**Non-Goals:**

- 不引入 `Selection::adjust`（拖动热路径按坐标直推，交叉翻转与宽字符吸附已按 termlib/termux 参考在 Kotlin 实现；每次移动做 gref 快照往返违反性能优先）。
- 不引入 `select_word_between`（无双击拖词交互，ghostty-android 亦未使用）。
- 不改触摸架构、不自绘放大镜、不加 ◀/▶ 菜单项（PROHIBITED）。
- 不改多语言与菜单文案（已为简中资源）。

## Decisions

1. **选择语义查询返回界限**：native 新增三个 Query（`SelectWordAt`/`SelectLineAt`/`SelectAll`），在 VT 线程派生快照 → `to_ordered` 取序 → gref 反解为绝对坐标 → `set_selection(OPT_SELECTION)` 安装 → 经既有 query 通道回传 `Option<(start, end)>`。Kotlin 长按/多击据此更新 `SelectionState`（控制柄、菜单几何不变量）。备选：Kotlin 保留自实现——被 STYLE「最大外部依赖、最小自实现」与 DESIGN L172 否决。
2. **全选语义**：采用上游 `select_all`（"all selectable terminal content"）；cargo 测试钉住界限不含尾部空行/空列，若上游语义不符则如实报告并最小修正，不回退视口矩形。
3. **菜单锚定纯函数**：`menuAnchor(selectionRect, viewport, handleHeight) -> (x, y)?`——上方优先、贴顶翻下、贴右钳制、两侧与选区相交则返回 null（隐藏）。菜单在选择几何变化处（抬手、全选动作）以隐藏→重显完成重锚；单测覆盖全部分支，替代已死的 `computeMenuPosition` 测试。
4. **菜单样式**：PopupWindow 架构不变，`buildMenuBar` 改解析 `colorSurface`/`colorOnSurface` 主题属性替代 `0xEE2B2B2B` 硬编码（DESIGN L38 Material 3、L171 系统样式）。
5. **边缘滚动**：删 30ms 定时自循环，改为触点移动事件内 `py` 越界即 ±1 行（termux/ghostty-android 同款、无定时器）；手指静止无移动事件自然不滚。
6. **删除清单**（依据 STYLE L63-65、DESIGN「有且只有」）：ModifierBar `SelectionActions` 死路径与 `barMode`；`computeMenuPosition`/`TerminalScreenMenuTest`；`expandAndSetSelection` 链、`SelectionExpander`、`SelectionMode`/mode/rectangle 全通道（native `Selection::new` 固定线性）；`selectionBackground` 只写不读通道；方向键移锚；SmartCopy 后处理与 `SmartCopyTest`；重复重显函数与过时注释。
7. **保留项的文档依据**：粘贴门控（ghostty-android `clipboardHasText` 参考）、空白长按单格反色（DESIGN「被长按文本单元格反色」）、交叉翻转（termlib 参考）、多击映射（ghostty-android tapCount）、300ms 防误关（termux 同款）。
8. **OSC 8 回退**：`resolveOpenLinkUri(text, hyperlinkUri)` 纯函数——URL 形态优先，否则用 `hyperlinkAt(selection.start)`，供菜单动作与单测共用。

## Risks / Trade-offs

- [上游 `select_all`/`select_word` 界限与既有仪器测试期望不完全一致] → 落地前先跑 cargo 与 MultiTap 仪器测试，按上游语义最小调整期望值并记录差异
- [gref 反解依赖 `GridRef` 点访问器] → 复用 `install_selection_impl` 的 Point 空间规则；实现时若访问器缺失，用 `selection()` 回读或在上游封装内取点，不引入坐标猜测
- [删 SmartCopy 后复制内容变化影响既有断言] → 审计未见 cucumber/仪器依赖边框剥离；落地时以复制断言全绿为准
- [PopupWindow 菜单在拖动中重显抖动] → 隐藏/重显仅发生在抓柄与抬手两个离散点，与 300ms 防误关配合

## Migration Plan

按 tasks 分组推进，每组独立提交推送且门禁全绿；删除先行（减面）、语义接入居中、菜单修复在后；任一组失败可单独回退。

## Open Questions

（无——所有影响 specs、方案与任务拆分的问题均已在 Decisions 中闭合。）
