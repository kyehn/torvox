# Design

## Context

见 proposal.md 的动机。此处只记录实现约束：

- 上游权威口径取自 termux-app `v0.119.0-beta.3` 的
  `TermuxPropertyConstants`（默认值、`TERMUX_DEFAULT_FALSE_BOOLEAN_BEHAVIOUR_PROPERTIES_LIST`）
  与 `TermuxSharedProperties`（取值映射、越界回退）。
- 本仓的解析器是 `TermuxPropertiesParser`（纯函数，Kotlin 单测可直接驱动），
  读取时机与缓存由 `TermuxPropertiesRepository` 负责。
- 启动弹键盘的执行点是 `TerminalSurface.requestKeyboardOnStart()`，由
  `TerminalScreen` 在 `hide-soft-keyboard-on-startup` 为假时调用。

## Goals / Non-Goals

**Goals:**

- 默认值与上游 termux 逐键一致，且默认行为满足 `DESIGN.md` 的「启动后自动弹出输入法」。
- 20 个键各有一组可执行的测试证据：默认值、有效取值、非法取值回退并留痕。

**Non-Goals:**

- 不改动 `termux.properties` 的读取时机、文件容错口径与语法子集。
- 不为 `volume-keys` 的虚拟 FN 映射补功能（本仓没有 FN 映射表）。
- 不改动键栏字形表、宏解析与键名别名表（已逐项对齐上游）。

## Decisions

1. **默认取 `false`（上游同值），而不是保留本仓的 `true`。**
   上游把 `hide-soft-keyboard-on-startup` 列在「默认 false」的布尔集合里，`false`
   意味着启动时不隐藏输入法——而终端视图在 termux 里恒为文本编辑器（`onCheckIsTextEditor`
   恒真），故冷启动必然弹键盘。本仓取 `true` 是该键还是空转键时的遗留（那时键只被解析、
   不改变行为），键变成真开关后默认值没有跟着上游一起复核。
   备选方案是保留 `true` 并改 `DESIGN.md`——`docs/specification/` 是受保护文件且明确
   要求「启动后自动弹出输入法」，故不采用。

2. **不引入「启动策略」之外的新状态。**
   `requestKeyboardOnStart()` 把「本视图可接收文本输入」置真后不再撤销：平台此后在焦点
   变化时仍会自动弹键盘，这与上游「终端恒为文本编辑器」一致，故不为「用户手动收起后
   不再弹回」另设标志位——收起由平台自身的可见性状态决定。

3. **逐键测试写成一个专门的测试类，而不是继续堆进解析器测试。**
   解析器测试关心的是「写错会怎样」，逐键测试关心的是「每个键的默认值与效果」，
   两者混在一起会让失败原因难辨。新测试类逐键断言具体取值，不做「非空/非零」式断言。

## Risks / Trade-offs

- 默认弹键盘 → 冷启动后终端可视区变矮 → 与上游与 `DESIGN.md` 一致，用户可轻点终端收起。
- 终端重新获得焦点时平台可能再次自动弹键盘 → 上游同款行为；`soft-keyboard-toggle-behaviour`
  的 `enable/disable` 仍可彻底禁用，两条路径互不干扰。
