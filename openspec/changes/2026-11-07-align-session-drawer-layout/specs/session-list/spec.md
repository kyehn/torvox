## ADDED Requirements

### Requirement: 抽屉布局与规范一致

会话抽屉 MUST 与 `docs/specification/DESIGN.md` 侧边面板节一致：「新建会话」入口 MUST
位于会话列表底部，MUST NOT 放在标题栏；动作区 MUST 排成两行两列，MUST 按「重置终端
文本搜索 / 显示、隐藏输入法 设置」排列；面板宽度 MUST NOT 设下限，MUST NOT 超出可用
宽度。

#### Scenario: 新建会话入口位于列表底部

- **WHEN** 用户打开会话抽屉
- **THEN** 「新建会话」按钮 MUST 位于会话列表下方，MUST NOT 出现在标题栏

#### Scenario: 动作按规范顺序排列

- **WHEN** 用户打开会话抽屉
- **THEN** 第一行 MUST 是「重置终端」与「文本搜索」，第二行 MUST 是「显示、隐藏输入法」与「设置」

#### Scenario: 窄屏抽屉不溢出

- **WHEN** 可用宽度小于 240dp 的设备打开会话抽屉
- **THEN** 面板宽度 MUST 不超过可用宽度，四个动作 MUST 直接可见且不溢出
