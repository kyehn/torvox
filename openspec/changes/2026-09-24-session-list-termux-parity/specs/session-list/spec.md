# session-list Specification

## REMOVED Requirements

### Requirement: 工作目录跟踪经 OSC 7/9/1337

termux 抽屉不显示目录；第二行改为终端标题后该管线无消费者（死代码），`DESIGN.md` 侧边面板节同步删除该声明。

## MODIFIED Requirements

### Requirement: 会话项内容

每一项 MUST 包含会话序号（从 1 开始、随列表改变按位置递增）与该会话终端标题（斜体第二行，来自 OSC 0/2），未设置标题时 MUST NOT 显示第二行；MUST 保留行内关闭按钮，点击条目 MUST 切换会话并关闭面板，MUST NOT 实现重命名。

#### Scenario: 关闭会话后重新编号

- **WHEN** 关闭序号 1 的会话
- **THEN** 其余会话按位置从 1 重新递增，不显示过时编号

#### Scenario: 未设置标题时仅显示序号

- **WHEN** 会话从未通过 OSC 0/2 设置标题
- **THEN** 该条目仅显示会话序号，不显示第二行

#### Scenario: 设置标题后抽屉显示

- **WHEN** 会话设置终端标题后打开会话列表
- **THEN** 该条目第二行斜体显示该标题

### Requirement: 抽屉打开时元数据刷新

会话列表打开时 MUST 强制刷新各会话终端标题（不受节流限制）；状态变化触发的刷新 MUST 按 2 秒节流合并，非强制刷新距上次刷新不足 2 秒时 MUST 跳过。

#### Scenario: 节流内重复刷新合并

- **WHEN** 距上次非强制刷新不足 2 秒再次触发
- **THEN** 本次刷新被跳过

#### Scenario: 强制刷新不受节流限制

- **WHEN** 抽屉打开触发强制刷新
- **THEN** 无论距上次刷新多久均立即执行
