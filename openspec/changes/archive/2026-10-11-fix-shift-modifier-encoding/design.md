# Design

## Context

见 proposal.md - Why。本仓的按键→转义序列编码全部集中在
`TerminalInputEncoder.kt`，它是硬件键、输入法与可配置键栏三条路径的唯一编码来源
（见 `modifier-bar-sticky-encoding`）。现状有两份平行表：
`escapeSequenceForKeyCode`（无修饰）与 `csiSequenceWithModifier`（带修饰），
F1–F12、方向、Home/End、Insert/Delete、PgUp/PgDn 各写两遍共约 40 行；
`modifierParam` 只认 Ctrl/Alt。

约束：`docs/specification/STYLE.md` 要求代码重复率低；本仓无新增依赖余量
（DESIGN：多使用库，尽量避免自定义实现）；`PROHIBITED.md` 不涉及修饰键。

## Goals / Non-Goals

**Goals:**

- Shift 进入修饰键位算式，四条输入路径行为一致。
- CSI 功能键编码单一来源，消掉两份平行表。
- 每个修饰分支的期望值都能在四个权威实现里找到字面依据。

**Non-Goals:**

- 不引入 kitty 键盘协议（CSI u）上报。
- 不改 Rust 侧 `text_utils::encode_modifiers`：它只服务可打印字符，
  Shift 已由平台并入字符，加 Shift 位是死代码。
- 不改 Ctrl+Backspace / Alt+Backspace（zed/ghostty 为 `0x08` / `ESC DEL`，
  本仓现发 `0x7F`）：与本次 Shift 主题无关，另行提案。
- 不改修饰键栏的按键集合（键栏无 Shift 键）。

## Decisions

**1. Shift 作为显式参数而非从掩码反推。**
`encodeKeyEvent` 已经以 `ctrlActive`/`altActive` 两个布尔显式收修饰态，
Shift 沿用同一形状（`shiftActive: Boolean = false`），默认值让键栏调用点无需改动。
替代方案：传整个 `KeyModifiers` 掩码让编码器自己拆——会把"哪些位属于键盘修饰"
的分散知识收进一处，但键栏的粘滞态本就不是同一个来源（`fromStickyStates` 不含 Shift），
两种来源混在一个参数里反而更容易出错。

**2. `modifierParam(ctrl, alt, shift) = 1 + 2·alt + 4·ctrl + 1·shift`。**
与 xterm PC-Style Function Keys 的 modifier 参数、zed `modifier_code()`
（`shift→1, alt→2, ctrl→4, +1`）逐位一致。现状漏 Shift 位会让 Shift+Ctrl 与
单独 Ctrl 不可区分。

**3. CSI 功能键收敛为一张表 `CSI_KEYS: Map<Int, CsiKey>`，`CsiKey(plain, intro, final)`。**
`plain` 是无修饰序列（`CSI A` / `SS3 P` / `CSI 15~`），带修饰序列由
`intro`/`final` 现场拼 `CSI <intro>;<mod><final>`。
两张平行表的风险已经现实化：`csiSequenceWithModifier` 有 `KEYCODE_DEL -> CSI 3;mod~`
而 `escapeSequenceForKeyCode` 对 `KEYCODE_DEL` 给 `0x7F`——这种不对称只能靠读代码发现，
合成一张表后由 `plain` 字段直接承载，无第二条路径。
替代方案：保留两函数但让无修饰函数调用带修饰函数（`modifier=1` 时退化）——
SS3 与 DECCKM 分支会在两处都出现，反而更绕。

**4. Tab / Enter 的修饰分支用显式 `when` 分流，不进 CSI 表。**
Tab 的 Shift/Alt 分支不是 `CSI intro;mod final` 形态（分别是 `CSI Z` 与 `ESC HT`），
Enter 的裸/Alt 分支是单字节而非 CSI。硬塞进统一表会引入"这条为什么特殊"的特例分支，
不如在 `when` 里按参考实现逐条写清并在注释给出来源。

**5. Ctrl+Tab 采用 `CSI 27;mod;9~` 而非保留 `CSI 9;mod~`。**
ghostty `function_keys.zig` 与 xterm `ctlseqs.txt` 都规定 `CSI 27;mod;9~`；
`CSI 9;mod~` 无任何实现使用（`CSI 9;mod u` 是 xterm 的 `formatOtherKeys` 变体，
末字节是 `u` 不是 `~`）。取 `CSI 27` 形态也与本仓既有 Ctrl+Enter（`CSI 27;5;13~`）
同源，减少同一文件里的两种约定。

## Risks / Trade-offs

- **旧行为被替换（Alt+Tab / Ctrl+Tab / Shift+Enter）** → 三条旧值均无参考实现背书，
  新值有字面依据；既有三条断言同步订正并在测试注释写明出处。
- **修饰键位 1 与"无修饰"歧义** → 算式里"无修饰"= 1，
  因此 `modifier > 1` 才是带修饰的门槛；这一条写成命名常量
  `UNMODIFIED_MODIFIER_PARAM = 1` 避免字面量散落。
- **Shift 在输入法提交路径无效** → 输入法只交出 `commitText`，
  拿不到 Shift；`encodeCommittedText` 不改（其注释已说明修饰态与提交文本无关）。
  这与 Tab/Enter 的硬件路径不是同一手势，不构成分叉。

## Migration Plan

纯 Kotlin 侧改动，无数据迁移。回滚即 revert 单个 commit。

## Open Questions

无。
