# modifier-bar-gesture-cancel Specification

## Purpose

修饰键栏位于系统底部手势区，全面上滑与侧缘返回会被系统手势认领，并向应用补发
`ACTION_CANCEL`。按键若不吞掉取消手势，就会在用户意图返回桌面时误发按键序列。
`ModifierBar` 经 `MotionEventAdapter` 丢弃 `ACTION_CANCEL` 并合成 `processCancel`，
使被认领的手势彻底无输出；键栏底边同时须留在导航手势区之外不被起滑点命中。

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

修饰键栏底边 MUST 位于**系统手势区**之上，系统“底部上滑”手势必须落在键栏之外
正常完成。

系统手势区 MUST 取平台手势 inset（`systemGestures` 与 `mandatorySystemGestures` 的
并集，即 Compose 的 `safeGestures`），MUST NOT 只按导航栏 inset 判定：全面屏手势
导航下导航栏只占很窄一条，只避导航栏时键栏底边仍可能落在手势区里。

底部留白 MUST 取导航栏 inset 与系统手势区的**较大者**，并 MUST 加在同时承载终端
与键栏的同一层上：终端 Surface 随之变矮，网格按新高度重算，键栏与网格的预留
口径保持一致。

#### Scenario: 底部边缘上滑回桌面

- **WHEN** 从屏幕底部边缘起向上滑动
- **THEN** 系统手势正常完成（回到桌面），起滑点不落在任何修饰键按键上

#### Scenario: 全面屏手势导航下的底部上滑

- **WHEN** 设备使用全面屏手势导航（导航栏 inset 窄于系统手势区）且用户从底部上滑
- **THEN** 上滑被系统正常认领，修饰键栏不出现按下反馈，按键不触发

### Requirement: 按下反馈不得早于系统手势取消窗口

按键的按下反馈（缩放、底色）MUST NOT 在 `ACTION_DOWN` 立即启动。系统底部上滑先
派发 `ACTION_DOWN`、再把同一手势认领走并补发 `ACTION_CANCEL`；反馈若在取消之前
已经画出一帧，用户看到的是「上滑触发了按键动画」，尽管没有任何按键输出。

反馈的启动 MUST 延迟一段覆盖该取消窗口的时间；取消或滑出 `touchSlop` 时 MUST
回到未按下状态且**不产生动画位移**——复位 MUST 是瞬时的，MUST NOT 以插值反向
播放一遍（先缩后弹同样是位移）。抬手复位 MUST NOT 延迟，反馈仍须短促。

#### Scenario: 底部上滑取消

- **WHEN** 用户在按键上按下后，触摸被系统底部上滑认领并收到 `ACTION_CANCEL`
- **THEN** 按下反馈从未启动，屏幕上看不到缩放或底色变化

#### Scenario: 取消后的复位不得有位移

- **WHEN** 按下反馈已经启动后手势被取消
- **THEN** 按键立即回到未按下外观，过程中不出现反向插值的缩放或底色变化

#### Scenario: 正常按住

- **WHEN** 用户按住按键超过延迟窗口且未滑出 `touchSlop`
- **THEN** 按下反馈正常出现，松手后立即复位

