## ADDED Requirements

### Requirement: 显示密度变化必须重推渲染契约

系统显示大小改变时，`densityDpi` 的比较 MUST 走 `applySettings()` 的
同一串行闸重推渲染契约；已有会话的文字光栅系数与网格 MUST 与新密度
一致，会话与 PTY 状态 MUST NOT 重建。配置回调 MUST 记录并比较
`densityDpi`，与夜间模式、字体缩放共用一次下发。

#### Scenario: 显示大小改变

- **WHEN** 系统显示大小改变且应用未重建
- **THEN** 终端文字按新密度光栅，网格与字号一致，无需重启会话

### Requirement: 帧回调链必须可停止

帧回调链 MUST 提供与启动成对的停止路径；停止条件为无会话、应用不可见
或连续空闲超过阈值且无待消费动画。链常驻的理由 MUST 有代码中真实
存在的字段支撑，无依据的常驻注释 MUST 删除。

#### Scenario: 关闭全部会话

- **WHEN** 用户关闭最后一个会话
- **THEN** 帧回调链停止，进程不再按显示帧唤醒主线程

### Requirement: 启动尺寸以下界仅拒绝非正尺寸

首次建会话 MUST 只拒绝非正尺寸与退化几何，MUST NOT 用整块显示尺寸
推导像素下界。合法小窗口（分屏、自由窗口、折叠半屏）MUST 能建出
至少一行一列的会话。

#### Scenario: 分屏冷启动

- **WHEN** 应用在分屏中以小于半屏的尺寸冷启动
- **THEN** 会话建出，网格至少一行一列，界面不停留在空白

### Requirement: 配置变更声明补齐硬件与形态限定符

`configChanges` MUST 包含 `density`、`smallestScreenSize`、`keyboard` 与
`navigation`，与 termux-app、ghostty-android-terminal 的完整声明一致；
已有的 orientation、screenSize、screenLayout、keyboardHidden、uiMode、
fontScale MUST 保留。

#### Scenario: 接入硬件键盘

- **WHEN** 用户接入或拔出硬件键盘
- **THEN** Activity MUST NOT 因此重建，会话数量与终端状态保持不变
