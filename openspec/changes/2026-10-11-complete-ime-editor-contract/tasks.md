# Tasks

## 1. 平台默认行为探针

- [ ] 1.1 在仪器测试中以合成 `InputConnection` 直调 `replaceText`，观察未覆写时字符是否到达 PTY；记录结论与调用序列，删除探针代码，仅把结论写入本变更的 design.md
- [ ] 1.2 同法探针 `getExtractedText` / `getTextBeforeCursor` / `getTextAfterCursor` / `getSurroundingText` 在未覆写时的返回值（应为空 editable），确认「输入法读不到自己刚写入的内容」在本仓成立；记录后删除探针
- [ ] 1.3 同法探针 `beginBatchEdit` / `endBatchEdit` / `setComposingRegion` / `commitCorrection` / `requestCursorUpdates` / `setSelection` 的默认返回与副作用；据此确认哪些需覆写、哪些保持默认；记录后删除探针
- [ ] 1.4 探针全部结论汇总进 design.md 并逐项标注输入法标识与系统版本；仓库中 MUST NOT 残留任何常红探针

## 2. 组字基线与取文接口

- [ ] 2.1 把组字基线（文本 + 光标）下沉为独立可单测的持有者，`setComposingText` 按 `newCursorPosition` 推出光标；单测覆盖 `newCursorPosition` 为 0 与大于 0 两种语义，以及提交后两者一并清空
- [ ] 2.2 实现 `getTextBeforeCursor` / `getTextAfterCursor` 只反映组字基线；单测断言组字中返回候选区内容、提交后返回空
- [ ] 2.3 实现 `getExtractedText` 与 `getSurroundingText` 同样只反映基线；单测断言取值不含任何屏幕或回滚区文本
- [ ] 2.4 `testDebugUnitTest` 与 `detekt` 通过

## 3. 替换与改写区域

- [ ] 3.1 实现 `replaceText`：按「头 + 文本 + 尾」构造目标组字串并复用 `ComposingDiff.reconcile`，经既有出口送达；单测断言退格数与追加字节逐字节相等，含非 ASCII 字符与尾段非空两种情形
- [ ] 3.2 范围越出组字基线时返回拒绝且不发任何字节；单测以「PTY 收字节数不变」为断言
- [ ] 3.3 实现 `setComposingRegion` 的重锚：以最长公共前缀把范围重定位到基线后走同一差分出口；单测覆盖范围内改写（只发追加）与无公共前缀（拒绝且已提交内容未被删除）
- [ ] 3.4 校验上述两条路径的写入时序未被改动：与 `ime-small-write-direct` 的一致性由既有单测保持全绿

## 4. 组字光标与批量编辑

- [ ] 4.1 实现 `requestCursorUpdates`：受理隐式更新，以组字光标回报 `CursorAnchorInfo`，变换矩阵由 `cellWidth` / `cellHeight` 与 `gridToScreen` 现算；单测断言锚点落在对应网格单元格内
- [ ] 4.2 无组字时不回报任何锚点；单测断言此时不产生锚点更新
- [ ] 4.3 字号变更后重算锚点不缓存旧度量：单测改变度量后锚点随之改变
- [ ] 4.4 实现 `beginBatchEdit` / `endBatchEdit` 嵌套计数配对且下溢钳位到 0；单测覆盖两层嵌套与多余 `endBatchEdit`
- [ ] 4.5 批量期间单次提交仍逐次经既有出口发送：单测断言批量进行中的提交即时产出字节，未滞留

## 5. 光标位移与硬键盘多字符

- [ ] 5.1 记录上一次回报的绝对位置，`setSelection` 按差值折算 `DPAD_LEFT` / `DPAD_RIGHT` 送 PTY，幅度按码点钳位；单测覆盖右移一字符、左移一字符、超大位移钳位
- [ ] 5.2 绝对位置不回写输入法侧文本：单测断言读回的文本逐字符不变
- [ ] 5.3 `sendKeyEvent` 补 `ACTION_MULTIPLE`：按 `event.characters` 的码点提交，不经键码映射；单测断言含非 ASCII 的三字符事件按序产出编码字节
- [ ] 5.4 单字符事件行为不变：既有键编码单测保持全绿，且新增单测断言其输出与变更前一致

## 6. 校正受理与诚实性核对

- [ ] 6.1 仪器测试探针校正路径：输入法以 `CorrectionInfo` 提交校正，观察文本是否经 `commitText` 到达 PTY；记录结论后删除探针
- [ ] 6.2 按 6.1 结论实现 `commitCorrection`：证明成立则受理，未证明则返回拒绝；单测断言返回值与结论一致（两种结论各一个断言，结论写进注释）
- [ ] 6.3 全量核对「受理即送达」：逐一确认本变更返回受理的接口均有对应单测证明其输入到达 PTY，结论写入 design.md

## 7. 规范对齐与验收

- [ ] 7.1 `openspec validate 2026-10-11-complete-ime-editor-contract --type change` 通过
- [ ] 7.2 `cargo test`、`testDebugUnitTest`、`detekt` 与相关仪器测试全绿
- [ ] 7.3 归档后核对 `openspec/specs/` 确实含 `ime-editor-contract` 及其条目数，不以校验通过即认定落地
- [ ] 7.4 `upstream-alignment` 台账补记：本变更取 charon 的 IME 契约经验，`updateSelection` 一项经实测为框架内部 @hide、公开接口不可覆写，记为不采纳并给出依据
