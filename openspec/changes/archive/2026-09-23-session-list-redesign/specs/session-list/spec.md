# session-list Specification

## ADDED Requirements

### Requirement: 工作目录跟踪经 OSC 7/9/1337

终端 MUST 跟踪上游解析的 OSC 7、OSC 9(ConEmu CurrentDir `9;9`)、OSC 1337 CurrentDir 序列并经会话查询暴露工作目录;应用声明的 mksh 启动文件 MUST 在源时与每次成功 `cd` 后经 OSC 7(`file://` + `$PWD`,空主机)上报,失败 `cd` 不上报,可见提示符保持 `PS1='$ '`。

#### Scenario: cd 后抽屉显示目录

- **WHEN** 会话中执行 `cd /data/local/tmp` 并打开会话列表
- **THEN** 该项显示缩写后的 `/data/local/tmp`

#### Scenario: 打开抽屉即反映最新目录

- **WHEN** 工作目录变化后重新打开会话列表
- **THEN** 显示最新目录,刷新不依赖其他状态变化触发

#### Scenario: mksh 启动文件发射

- **WHEN** 交互式 mksh 经 `ENV` 加载启动文件后执行 `cd`
- **THEN** 源时发射一次、每次成功 `cd` 发射一次,失败 `cd` 不发射,提示符仍为 `$` 加尾随空格的短提示符

### Requirement: 会话项内容

每一项 MUST 仅包含会话序号(从 1 开始、随列表改变按位置递增)、缩写后的目录路径、关闭按钮,MUST NOT 携带其他展示字段。

#### Scenario: 关闭会话后重新编号

- **WHEN** 关闭序号 1 的会话
- **THEN** 其余会话按位置从 1 重新递增,不显示过时编号

#### Scenario: 未上报目录时副标题为空

- **WHEN** 会话从未上报工作目录
- **THEN** 副标题为空,不回退任何标题文本

### Requirement: 抽屉打开时元数据刷新

会话列表打开时 MUST 强制刷新各会话工作目录(不受节流限制);运行状态变化触发的刷新 MUST 按 2 秒节流合并,非强制刷新距上次刷新不足 2 秒时 MUST 跳过。

#### Scenario: 节流内重复刷新合并

- **WHEN** 距上次非强制刷新不足 2 秒再次触发
- **THEN** 本次刷新被跳过

#### Scenario: 强制刷新不受节流限制

- **WHEN** 抽屉打开或会话关闭触发强制刷新
- **THEN** 无论距上次刷新多久均立即执行
