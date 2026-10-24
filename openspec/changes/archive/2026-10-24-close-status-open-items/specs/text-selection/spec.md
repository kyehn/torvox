## MODIFIED Requirements

### Requirement: 控制柄拖动

拖动控制柄 MUST 实时更新选区，宽字符单元格 MUST 吸附不被拆分，该吸附 MUST 由网格单元宽度事实给出而非由行文本字符位置推算；一端越过对侧端点 MUST 交换两端归属；拖动期间选择菜单 MUST 隐藏，抬手 MUST 按新选区几何重显菜单。

实现：`SelectionState.applyHandleDrag` 返回五元组（含翻转后的被拖柄归属），越界判据为严格不等（落点恰为锚点时不翻转）；快速路径 `dragMove` 与慢速路径 `dragSelection` 均回传新归属，调用方据此更新 `handleDragState`，连续回拖不再按旧柄解释。

#### Scenario: 宽字符吸附

- **WHEN** 手柄停在双宽字符的后半格
- **THEN** 手柄吸附至该字符起始列

#### Scenario: 行内多个宽字符均吸附

- **WHEN** 行内前一个字符也是双宽字符且手柄停在后一个宽字符的后半格
- **THEN** 手柄吸附至后一个字符的起始列而非更左的列

#### Scenario: 交叉交换归属

- **WHEN** 起点手柄越过终点手柄
- **THEN** 两端归属交换且选区保持非空

#### Scenario: 拖动隐藏菜单

- **WHEN** 抓住控制柄开始移动
- **THEN** 选择菜单不可见；抬手后菜单按新选区重显

## ADDED Requirements

### Requirement: 手柄交叉时的触点折算

触点到网格目标的折算 MUST 以**抓柄时**锁定的参考点为基准（首端柄 = 首格左缘，末端柄 = 末格右缘），该参考点 MUST 在整次拖动内保持不变：归属翻转只换被拖柄扮演的角色，MUST NOT 换参考点——参考点随归属翻转会让目标格在翻转瞬间跳一格，被拖的边界反向移动、交叉被立刻撤销。手柄 MUST 画在选区边界上（首端柄在首格左缘、末端柄在末格右缘），MUST NOT 与边界错位。

实现：触点折算由纯函数 `dragTargetColumn`/`dragTargetRow` 承担，抓柄侧在 `latchDragAnchor` 时锁定并贯穿整次手势，边缘滚动路径与快速路径共用同一套列折算。

#### Scenario: 交叉后被拖边界停在手指所在格

- **WHEN** 拖动末端手柄越过首端手柄并继续移动
- **THEN** 被拖的边界停在手指所在格，MUST NOT 反向跳回一格；未拖动的一侧 MUST 钉在交叉前的位置

#### Scenario: 手柄与选区边界对齐

- **WHEN** 手柄拖动结束（含交叉翻转之后）
- **THEN** 首端手柄画在首格左缘、末端手柄画在末格右缘，MUST NOT 与选区边界错位

### Requirement: 抬手帧以最终触点重算

抬手那一帧 MUST 先按最终触点重算选区再提交（最后一个 MOVE 允许落后于手指，不得据此提交）；`ACTION_CANCEL` 不带可靠坐标，MUST NOT 驱动该帧。

实现：抬手帧由 `HandleOverlayLayout.routeStreamEnd` 在 `ACTION_UP` 时以事件坐标驱动一帧 `driveHandleDragMove` 再调用 `finishHandleDrag`。

#### Scenario: 抬手落在手指处

- **WHEN** 快速拖动控制柄后抬手，最后一个 MOVE 事件落后于抬手坐标一两格
- **THEN** 选区以抬手坐标落格，菜单按该落格后的选区重显
