# session-list Specification

## MODIFIED Requirements

### Requirement: 抽屉布局与规范一致

会话抽屉 MUST 与 `docs/specification/DESIGN.md` 侧边面板节一致：「新建会话」入口 MUST
位于会话列表底部，MUST NOT 放在标题栏；动作区 MUST 排成两行两列，MUST 按「重置终端
文本搜索 / 显示/隐藏输入法 设置」排列。

#### Scenario: 新建会话入口位于列表底部

- **WHEN** 用户打开会话抽屉
- **THEN** 「新建会话」按钮 MUST 位于会话列表下方，MUST NOT 出现在标题栏

#### Scenario: 动作按规范顺序排列

- **WHEN** 用户打开会话抽屉
- **THEN** 第一行 MUST 是「重置终端」与「文本搜索」，第二行 MUST 是「显示/隐藏输入法」与「设置」

#### Scenario: 窄屏抽屉不溢出

- **WHEN** 可用宽度小于 240dp 的设备打开会话抽屉
- **THEN** 面板宽度 MUST 不超过可用宽度，四个动作 MUST 直接可见且不溢出

## ADDED Requirements

### Requirement: 抽屉面板宽度上限

抽屉面板宽度 MUST 取抽屉可用宽度的 75%，MUST NOT 超过 280dp，MUST NOT 设下限。

#### Scenario: 宽屏抽屉收窄到上限

- **WHEN** 可用宽度的 75% 超过 280dp 的设备打开会话抽屉
- **THEN** 面板宽度 MUST 不超过 280dp，终端 MUST 保留可辨认的一条

### Requirement: 动作区不挤占会话列表

动作区高度 MUST 紧凑：两行动作合计 MUST NOT 超过抽屉高度的三分之一；会话列表区
（标题下沿到「新建会话」上沿）MUST 不少于抽屉高度的五分之二。

#### Scenario: 抽屉变矮时列表仍留出空间

- **WHEN** 输入法弹起使抽屉只剩半屏高度
- **THEN** 动作区合计 MUST NOT 超过抽屉高度的三分之一，会话列表区 MUST 不少于五分之二
