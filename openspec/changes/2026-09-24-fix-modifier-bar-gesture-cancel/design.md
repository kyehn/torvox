# Design

## Context

见 proposal.md - Why。触摸管线事实（Compose UI 1.12.1 反汇编 + 红灯实证）：

- `MotionEventAdapter.convertToPointerInputEvent` 对 action 3/4（`ACTION_CANCEL`/`ACTION_OUTSIDE`）**直接返回 null**；`AndroidComposeView.sendMotionEvent` 随即走 `PointerInputEventProcessor.processCancel()`，取消事件**从不携带原始 MotionEvent**。
- `SuspendingPointerInputModifierNodeImpl.onCancelPointerInput` 用单参构造合成 `PointerEvent(changes)`（`motionEvent` 恒为 null、指针全部释放、previousPressed 为真），三趟 pass 派发给挂起的 `awaitPointerEvent()`。
- 真实 DOWN/MOVE/UP 均经 adapter 转换，`PointerEvent.motionEvent` 非空；Compose 取消与抬手在 handler 内同为 `pressed=false`，此即根因。
- 既有防线：touchSlop 滑出取消、抬手确认触发、滑出后排空至抬手的 drain 循环。

## Goals / Non-Goals

**Goals:**
- 三分支（点按、自动重复、长按副动作）对取消一视同仁：吞掉、不触发、不排空等待。
- 不改变正常点按/长按/重复节奏，不吞下一次手势。

**Non-Goals:**
- 不改键栏几何与系统手势区划分（既有测试守住）。
- 不改 foundation/框架层与其他控件的取消语义。

## Decisions

1. **取消识别**：手势处理器内本地函数——“本手势见过原始 MotionEvent，且当前事件无原始 MotionEvent 且全部释放”判为取消。
   - 备选 A `event.type == Cancel`：`PointerEventType` 无 Cancel 类型，不可用。
   - 备选 B `motionEvent?.actionMasked == ACTION_CANCEL`：首版实现，红灯证明无效（适配器丢弃取消，合成事件 motionEvent 恒 null）。
   - 备选 C 仅“null + 全释放”：全合成注入（若测试框架不带原始事件）的 UP 同形会被误判，故加“见过原始事件”前提；`ACTION_CANCEL` 非空分支保留作防御。
2. **触发前拦截**：取消即置 `gestureValid`/`tapValid`/`repeatValid` 为假并跳出，先于任何 fire 与 `maybeFireLongPress()`。
3. **排空循环改写**：由“等下一个事件再判释放”改为“`currentEvent` 仍有指针按下才排空”——取消时已全释放直接跳过，否则会挂起等待并吞掉下一手势的 DOWN。
4. **测试注入**：View 派发原始 `MotionEvent`（`performTouchInput` 无法表达取消），并配 `DOWN`+`UP` 对照防假绿。

## Risks / Trade-offs

- [测试框架合成注入不带原始事件] → “见过原始事件”前提使全合成点按不受影响；对照测试钉住点按仍触发。
- [长按副动作已触发后被取消] → 已发生部分保留（与 View `onLongClick` 语义一致），仅吞未触发部分。
- [依赖 `currentEvent` 判排空] → 取消事件会写入 node 的 `currentEvent`；红绿测试与设备矩阵覆盖。

## Migration Plan

单提交直接发版，无数据/接口迁移；回滚 = revert 修复提交。

## Open Questions

无。
