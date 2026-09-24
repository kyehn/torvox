# modifier-bar-gesture-cancel Specification

## Purpose

触摸被系统取消（全面屏手势认领、窗口取消）时修饰键栏绝不触发按键，且键栏不阻挡系统底部手势。

## Requirements

### Requirement: 触摸取消不触发按键

修饰键栏按键收到 `ACTION_CANCEL` 时 MUST 吞掉本次手势，不触发任何按键输出、粘滞切换或长按副动作。

#### Scenario: 按键上起滑被系统手势认领

- **WHEN** 在修饰键栏按键上按下后触摸被取消（如底部上滑、侧缘返回被系统认领后向应用下发 `ACTION_CANCEL`）
- **THEN** 按键不触发，修饰状态与终端输入均不变

#### Scenario: 正常点按仍触发

- **WHEN** 在按键上按下后原位抬手（无位移、无取消）
- **THEN** 按键正常触发一次

#### Scenario: 超出 touchSlop 上滑不触发

- **WHEN** 在按键上按下后位移超出 touchSlop 再抬手
- **THEN** 按键不触发（既有滑出取消保持）

### Requirement: 键栏位于系统底部手势区之上

修饰键栏底边 MUST 位于导航栏手势区之上，系统“底部上滑”手势必须落在键栏之外正常完成。

#### Scenario: 底部边缘上滑回桌面

- **WHEN** 从屏幕底部边缘起向上滑动
- **THEN** 系统手势正常完成（回到桌面），起滑点不落在任何修饰键按键上
