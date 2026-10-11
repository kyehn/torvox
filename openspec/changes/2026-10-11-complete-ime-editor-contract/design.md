# Design

## Context

动机见 `proposal.md`。本设计只交代实现取舍。

现状要点：`InputConnection` 匿名类已维护 `composingBuffer`（唯一输入法文本镜像），
写入出口为 `encodeAndSend` / `sendBackspaces` 两个函数，差分由 `ComposingDiff.reconcile`
统一完成——「最长公共前缀 + 其后重打」。新写入受两个既有约束保护：抑制窗口内一律返回
false 让输入法重发（N1-24），同一事务的退格与追加 MUST 走同一出口（N1-27）。

组字光标当前被丢弃：`setComposingText(text, newCursorPosition)` 收到了
`newCursorPosition` 却未使用。网格度量（`cellWidth` / `cellHeight` 与 `gridToScreen`）
已存在，选区命中测试正在使用，可直接复用。

平台默认实现的源码在本机不可得（SDK 无 `sources/`），故每个接口的默认行为 MUST 先
探针实测，MUST NOT 按文档推测。

## Goals / Non-Goals

**Goals:**

- 新增编辑通道全部归约到既有差分与既有出口，不引入第二套写入路径。
- 组字光标成为一等状态，可回报输入法。
- 任何「受理」都有探针背书。

**Non-Goals:**

- 不建立终端输入行的完整镜像（见 proposal Non-goals）。
- 不改既有组字/提交的时序与抑制窗口语义。
- 不为 API 33 加版本分支（`replaceText` 在该版本运行时不调用，覆写本身无副作用）。

## Decisions

### D1 所有编辑操作归约为「算出新组字串，复用 `ComposingDiff.reconcile`」

范围替换与改写区域都先算出一个目标组字串，再交给既有差分：

```text
replaceText(start, end, text)  →  base[0, start) + text + base[end, len)
setComposingRegion(start, end) →  base[0, start) + <光标定位标记> + base[end, len)
```

差分取最长公共前缀，故公共部分零字节、无需手工算退格数。

**备选与否决理由**：覆写 `getEditable` 并维护 `Editable` 镜像——否决，它制造终端与
输入法两份真相，且输入法直接改 editable 正是本变更要消灭的静默丢失路径；各接口各写
一套差分——否决，与 `ComposingDiff` 重复且将来必然分叉。

### D2 替换范围按「头 + 文本 + 尾」精确还原，MUST NOT 近似为尾替换

`replaceText` 的范围若不在组字尾部，朴素的「退到 start 再追加」会丢掉 `[end, len)`
尾段。取尾段一并重发后语义精确，多出的字节量至多为一次组字长度。

**备选与否决理由**：只接受 `end == len` 的尾替换——否决，那会让 Android 14 输入法
的部分替换路径被拒，退化成新的能力缺失。

### D3 组字光标由本仓跟踪，不读 `BaseInputConnection` 的 editable

新增 `composingCursor`，由 `newCursorPosition` 语义推出（`> 0` 时为
`len + newCursorPosition`，否则为 `len`），随组字基线一并清空。

**备选与否决理由**：读 editable 的 `selectionStart`——否决，本仓已覆写
`setComposingText` / `commitText`，那两条写入路径不再同步该 editable，取到的是陈旧值。

### D4 方向键折算只取相对位移

记录上一次向输入法回报的绝对位置，`setSelection` 到达时算差值，映射为等量
`DPAD_LEFT` / `DPAD_RIGHT`（钳位到单次合理幅度）。绝对位置不回写。

**备选与否决理由**：绝对位置直接换算网格列——否决，输入法的绝对位置与终端输入行
无对应关系，会把「回到行首」之类的编辑意图变成大量方向键。

### D5 光标锚点坐标复用选区命中测试的换算

`CursorAnchorInfo` 的变换矩阵由 `cellWidth` / `cellHeight` 与 `gridToScreen` 现算，
与选区命中测试共用同一套换算函数，MUST NOT 另写一份坐标公式。度量每次现取，MUST NOT
缓存——字号变更与滚动会让缓存失准。

### D6 批量编辑只计数，不改变写入时序

`beginBatchEdit` / `endBatchEdit` 以嵌套计数配对（下溢钳位到 0），写入路径完全不变：
组字与提交仍逐次经既有出口发送。

**备选与否决理由**：批量期间缓冲到 `endBatchEdit` 再发——否决，那会改动
`ime-small-write-direct` 已锁定的发送时序，把一个契约变更混入本次变更。

### D7 `commitCorrection` 先拒绝，待探针证明后再受理

校正文本是否经 `commitText` 到达 PTY 未经验证。探针证明之前返回拒绝——输入法停止调用
比「以为生效而实际丢失」安全。

### D8 平台新增接口 MUST 先探针后覆写

`replaceText` 等平台新增方法的默认实现不可见。实现顺序 MUST 为「写探针 → 观察默认行为
→ 覆写并经既有出口」，MUST NOT 照文档直接实现。

## Risks / Trade-offs

- **[输入法把绝对 `setSelection` 当语义用]** → 只按差值响应可能多发或少发方向键。
  缓解：仅在差值非零时响应，幅度按码点钳位；以仪器测试覆盖主流输入法的空格滑动路径。
- **[批量编辑嵌套数与输入法实际不符]** → 计数漂移。缓解：计数只参与受理判定，不参与
  写入时序；下溢钳位到 0，故漂移不影响任何既有行为。
- **[光标锚点坐标在滚动或改字号后失准]** → 缓解：坐标每帧现取，不缓存；无组字时不回报。
- **[探针结论依赖具体输入法版本]** → 结论 MUST 记录输入法标识与系统版本，探针本身
  MUST 删除，结论写进本变更文档，避免常红探针。
- **[覆写方法增多使匿名类膨胀]** → 组字基线的光标、取文、替换逻辑 MUST 下沉到
  独立文件并单测，匿名类只做转发，保持可读。

## Migration Plan

无数据迁移，无新增依赖。回滚 = 还原单个文件的覆写；镜像类状态仅存活于输入法连接
生命周期内，无残留。
