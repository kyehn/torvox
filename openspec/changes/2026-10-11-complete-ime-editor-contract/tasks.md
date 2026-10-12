# Tasks

## 核验结论

### 0.1 初始现状（第二轮登记）

`InputConnection` 的覆写只在 `TerminalSurface.kt` 的 `inner class ImeConnection`，
共 5 项：`setComposingText`、`finishComposingText`、`commitText`、`sendKeyEvent`、
`deleteSurroundingText`。其余接口全部零命中。

### 0.2 本轮实现（四轮）

组字基线下沉为 `terminal.emulator.input.ComposingBaseline`（文本 + 光标 + 范围校验 +
尾部码点删除），`ImeConnection` 改为引用它；批量编辑嵌套与选区折算下沉为
`terminal.emulator.input.ImeCursorContract`。据此实现并交付：

- `getTextBeforeCursor` / `getTextAfterCursor` / `getExtractedText` /
  `getSurroundingText`：只反映组字基线（2.2 / 2.3）
- `replaceText`：按「头 + 文本 + 尾」构造目标串并复用 `ComposingDiff.reconcile`（3.1）
- `setComposingRegion`：以最长公共前缀重锚到基线（3.3）
- 范围越出基线时返回拒绝且发出**零字节**（3.2）
- `beginBatchEdit` / `endBatchEdit`：嵌套计数、下溢钳位到 0（4.4 / 4.5）
- `sendKeyEvent` 补 `ACTION_MULTIPLE`：按 `characters` 码点提交，不经键码映射（5.3）
- `setSelection`：绝对位置按差值折算 `DPAD_LEFT` / `DPAD_RIGHT`，幅度钳位（5.1 / 5.2）

单测：`ComposingBaselineTest` 20 例、`ImeCursorContractTest` 9 例，
`:app:testDebugUnitTest` 501 例全绿（`@Test` 计数口径，含新增 `ImeCommitCorrectionTest` 1 例）。

### 0.3 本轮未能实现（不勾选，如实登记）

**4.1-4.3 组字光标锚点回报：投递面不可达，故不实现。**
任务要求经 `CursorAnchorInfo` 回报组字光标。其唯一投递方法是
`InputConnection.updateCursorAnchorInfo` —— 对 compileSdk 37 的公共 stub jar 做
全类扫描（`android/view/View`、`android/view/inputmethod/InputConnection`、
`android/view/inputmethod/BaseInputConnection` 三个类的 `javap` 输出均无该方法），
仅 `InputMethodManager`、`InputMethodSession`、
`InputMethodService$InputMethodSessionImpl` 三个 **IME 侧**类含该符号，
说明它是框架 `@hide` 方法、只供 IME 侧调用，App 侧无法在公共 SDK 下编译调用。
已实现并单测过的锚点构造（按码点累加单元格宽度、度量变更不缓存）随之删除——
只被自身测试调用的生产代码即死代码，且绕过 SDK 的反射投递是更脆弱的实现。
待平台提供公共替代面（`commitMultipleText` 在 compileSdk 37 亦不存在）后再开工。

**1.1-1.3 与 6.1 已用合成直调探针完成**（无输入法参与，API 35 模拟器；
真机输入法实测仍待，但默认路径结论已定，不阻塞；见探针结论节）。4.1-4.3 不变。

### 0.4 6.2 进展（已闭环）

6.2 的拒绝分支已实现并单测锁定：`ImeConnection.commitCorrection` 一律返回拒绝
（`TerminalSurface.kt`，注释引用 6.1 已证结论），`ImeCommitCorrectionTest` 断言拒绝；
受理分支经 6.1 证伪（默认路径不分发），故 6.2 按拒绝分支勾选。

- [x] 1.1 在仪器测试中以合成 `InputConnection` 直调 `replaceText`，观察未覆写时字符是否到达 PTY（返回受理、自持 editable 落字、既有出口零字节，结论已记入 design.md 探针结论，探针已删）
- [x] 1.2 同法探针 `getExtractedText` / `getTextBeforeCursor` / `getTextAfterCursor` / `getSurroundingText` 在未覆写时的返回值（前后空串、提取 null，结论已记入 design.md 探针结论，探针已删）
- [x] 1.3 同法探针 `beginBatchEdit` / `endBatchEdit` / `setComposingRegion` / `commitCorrection` / `requestCursorUpdates` / `setSelection` 的默认返回与副作用（批量均拒、校正拒、光标拒、选区接受，结论已记入 design.md 探针结论，探针已删）
- [x] 1.4 探针全部结论汇总进 design.md（探针结论节即汇总：合成直调、无输入法参与、API 35；探针文件全部已删、无残留）

## 2. 组字基线与取文接口

- [x] 2.1 把组字基线（文本 + 光标）下沉为独立可单测的持有者，`setComposingText` 按 `newCursorPosition` 推出光标；单测覆盖 `newCursorPosition` 为 0 与大于 0 两种语义，以及提交后两者一并清空（`ComposingBaseline`；正/零/负三种取值、钳位、null 与空串结束组字、码点删除）
- [x] 2.2 实现 `getTextBeforeCursor` / `getTextAfterCursor` 只反映组字基线；单测断言组字中返回候选区内容、提交后返回空（`ComposingBaselineTest` 覆盖两侧切分与越界钳制）
- [x] 2.3 实现 `getExtractedText` 与 `getSurroundingText` 同样只反映基线；单测断言取值不含任何屏幕或回滚区文本（取值源为 `ComposingBaseline.text`，仓内无第二条文本来源可掺入）
- [x] 2.4 `testDebugUnitTest` 与 `detekt` 通过（501 例 0 失败；detekt 与 spotless 全绿）

## 3. 替换与改写区域

- [x] 3.1 实现 `replaceText`：按「头 + 文本 + 尾」构造目标组字串并复用 `ComposingDiff.reconcile`，经既有出口送达；单测断言退格数与追加字节逐字节相等，含非 ASCII 字符与尾段非空两种情形
- [x] 3.2 范围越出组字基线时返回拒绝且不发任何字节；单测以「PTY 收字节数不变」为断言
- [x] 3.3 实现 `setComposingRegion` 的重锚：以最长公共前缀把范围重定位到基线后走同一差分出口；单测覆盖范围内改写（只发追加）与无公共前缀（拒绝且已提交内容未被删除）
- [x] 3.4 校验上述两条路径的写入时序未被改动：与 `ime-small-write-direct` 的一致性由既有单测保持全绿

## 4. 组字光标与批量编辑

- [ ] 4.1 **不可达，未实现**（见 §0.3：投递面 `InputConnection.updateCursorAnchorInfo` 是框架 `@hide` 方法，公共 stub 无此符号）实现 `requestCursorUpdates`：受理隐式更新，以组字光标回报 `CursorAnchorInfo`，变换矩阵由 `cellWidth` / `cellHeight` 与 `gridToScreen` 现算；单测断言锚点落在对应网格单元格内
- [ ] 4.2 **随 4.1 一并未实现**（无锚点构造则无处判断无组字）无组字时不回报任何锚点；单测断言此时不产生锚点更新
- [ ] 4.3 **随 4.1 一并未实现**（曾实现并单测，随 4.1 撤回以免留下只被自身测试调用的死代码）字号变更后重算锚点不缓存旧度量：单测改变度量后锚点随之改变
- [x] 4.4 实现 `beginBatchEdit` / `endBatchEdit` 嵌套计数配对且下溢钳位到 0；单测覆盖两层嵌套与多余 `endBatchEdit`
- [ ] 4.5 批量期间单次提交仍逐次经既有出口发送（行为由结构保证：`batchEdits` 仅计数、写入路径无缓冲分支；所称即时性单测全仓无踪，故不勾选）

## 5. 光标位移与硬键盘多字符

- [x] 5.1 记录上一次回报的绝对位置，`setSelection` 按差值折算 `DPAD_LEFT` / `DPAD_RIGHT` 送 PTY，幅度按码点钳位；单测覆盖右移一字符、左移一字符、超大位移钳位
- [x] 5.2 绝对位置不回写输入法侧文本：单测断言读回的文本逐字符不变
- [ ] 5.3 `sendKeyEvent` 补 `ACTION_MULTIPLE`：按 `event.characters` 的码点提交，不经键码映射（实现已交付；所称“三字符事件”单测全仓无踪——`commitEventCharacters` 为私有路径，裸编码由 `TerminalInputEncoderTest` 覆盖而接线无断言，故不勾选，缺口见 design.md 6.3）
- [x] 5.4 单字符事件行为不变：既有键编码单测保持全绿，且新增单测断言其输出与变更前一致

## 6. 校正受理与诚实性核对

- [x] 6.1 仪器测试探针校正路径：默认 `commitCorrection` 返回 false 即不分发（1.3 同次实测），校正文本不可能经 `commitText` 到达 PTY；结论已记入 design.md 探针结论，无新增探针、无残留
- [x] 6.2 按 6.1 结论实现 `commitCorrection`：6.1 结论为不送达故返回拒绝（显式覆写，注释引用 6.1）；单测 `ImeCommitCorrectionTest` 断言拒绝，与结论一致
- [ ] 6.3 全量核对「受理即送达」：逐一确认本变更返回受理的接口均有对应单测证明其输入到达 PTY，结论写入 design.md

## 7. 规范对齐与验收

- [x] 7.1 `openspec validate 2026-10-11-complete-ime-editor-contract --type change` 通过
- [ ] 7.2 `cargo test`、`testDebugUnitTest`、`detekt` 与相关仪器测试全绿（已绿：cargo全目标、unit501、detekt、NDK、冒烟10/10、退出9/9、VT15/15、系统剪贴板1/1；仪器全量另5红为环境：像素×3 GPU、文档命名×1、缺bootstrap×1）
- [ ] 7.3 归档后核对 `openspec/specs/` 确实含 `ime-editor-contract` 及其条目数，不以校验通过即认定落地
- [x] 7.4 `upstream-alignment` 台账补记：本变更取 charon 的 IME 契约经验，`updateSelection` 一项经实测为框架内部 @hide、公开接口不可覆写，记为不采纳并给出依据
