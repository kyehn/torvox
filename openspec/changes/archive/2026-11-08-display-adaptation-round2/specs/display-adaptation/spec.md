## MODIFIED Requirements

### Requirement: 配置变化必须就地生效

显示密度、字体缩放、窗口尺寸与输入设备配置的变化 MUST 就地生效，MUST NOT 依赖
Activity 重建后重新下发。已有会话的字体光栅系数、单元格度量与网格 MUST 在变化后与
当前配置一致；会话、PTY 与终端状态 MUST NOT 因这些变化重建。

系统显示大小改变时，`densityDpi` 的比较 MUST 走 `applySettings()` 的同一串行闸
重推渲染契约；已有会话的文字光栅系数与网格 MUST 与新密度一致，会话与 PTY 状态
MUST NOT 重建。配置回调 MUST 记录并比较 `densityDpi`，与夜间模式、字体缩放共用
一次下发。

#### Scenario: 修改系统显示大小

- **WHEN** 应用运行中系统显示大小改变
- **THEN** 终端文字按新密度光栅，网格与字号一致，无需重启会话或改动字号设置

#### Scenario: 接入硬件键盘

- **WHEN** 用户接入或拔出硬件键盘
- **THEN** Activity MUST NOT 因此重建，会话数量、终端状态与输入状态保持不变

#### Scenario: 字体缩放与密度同时变化

- **WHEN** 系统字体缩放改变，随后显示大小也改变
- **THEN** 两次变化各自生效，后一次不会把前一次的光栅系数覆盖回旧值

#### Scenario: 显示大小改变

- **WHEN** 系统显示大小改变且应用未重建
- **THEN** 终端文字按新密度光栅，网格与字号一致，无需重启会话

### Requirement: 任意合法窗口尺寸必须能启动会话

首次建会话 MUST 以当前窗口尺寸为准，MUST NOT 以整块显示尺寸推导最小 Surface 或
像素下界。只有非正尺寸与无法容纳一个单元格的几何可以被拒绝；被拒绝时 MUST 在
窗口尺寸变为合法后自动重试，不要求用户重启应用。合法小窗口（分屏、自由窗口、
折叠半屏）MUST 能建出至少一行一列的会话。

#### Scenario: 分屏冷启动

- **WHEN** 应用在分屏、自由窗口或折叠半屏中以小于半屏的尺寸冷启动
- **THEN** 会话建出，网格至少一行一列，界面不停留在空白

#### Scenario: 小窗口放大

- **WHEN** 小窗口中已建出的会话随后被放大
- **THEN** 网格按新尺寸重算，PTY 行列同步更新，不出现内容错位

### Requirement: 帧回调链必须随活跃度启停

帧回调链 MUST 在没有会话、应用不可见或连续空闲超过阈值且无待消费动画时停止，
MUST 在输出到达、输入写入、搜索或选区变化、恢复前台、会话切换时重启。
MUST NOT 以无条件自投维持链；逐帧动画若存在，MUST 由显式的活跃租约持有者申请帧，
不得依赖链常驻。

帧回调链 MUST 提供与启动成对的停止路径；链常驻的理由 MUST 有代码中真实存在的
字段支撑，无依据的常驻注释 MUST 删除。

#### Scenario: 空闲期零回调

- **WHEN** 终端连续空闲超过阈值且没有待处理动画
- **THEN** 从下一个显示周期起不再投递帧回调，主线程不被逐帧唤醒

#### Scenario: 输出到达即恢复

- **WHEN** 空闲期收到 PTY 输出或用户输入
- **THEN** 帧回调链重启，变化在下一个显示帧上屏，不等待空闲闭锁到期

#### Scenario: 关闭全部会话

- **WHEN** 用户关闭最后一个会话
- **THEN** 帧回调链停止，进程不再按显示帧唤醒主线程

## ADDED Requirements

### Requirement: 配置变更声明补齐硬件与形态限定符

`configChanges` MUST 包含 `density`、`smallestScreenSize`、`keyboard` 与
`navigation`，与 termux-app、ghostty-android-terminal 的完整声明一致；
已有的 orientation、screenSize、screenLayout、keyboardHidden、uiMode、
fontScale MUST 保留。

#### Scenario: 接入硬件键盘

- **WHEN** 用户接入或拔出硬件键盘
- **THEN** Activity MUST NOT 因此重建，会话数量与终端状态保持不变
