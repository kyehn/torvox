## ADDED Requirements

### Requirement: 外部 Intent 的数据只走结构化通道

外部应用可投递的启动 Intent MUST 只经结构化通道被消费：布尔、长整型与受限枚举取值
各自对应一个既有动作。外部字符串 MUST NOT 被键入 PTY、MUST NOT 拼进命令行、
MUST NOT 当作工作目录或环境变量取值。取值不合法时 MUST 拒绝，MUST NOT 回落成
宽松默认；每次外部输入的处置修复 MUST 留一个可离线运行的回归用例。

#### Scenario: 外部字符串不得键入 PTY

- **WHEN** 第三方应用投递带任意字符串 extra 的启动 Intent
- **THEN** 应用 MUST NOT 把该字符串写入 PTY、命令行、工作目录或环境变量
- **AND** 只接受布尔、长整型与枚举取值；未知枚举值 MUST 拒绝

#### Scenario: 引导包地址只认官方预设

- **WHEN** 启动 Intent 携带的不是本变种官方预设的引导包地址
- **THEN** MUST 拒绝该请求，MUST NOT 下载或解压该地址的归档

#### Scenario: 未知日志等级取值不占告警档

- **WHEN** 启动 Intent 带来未知的日志等级名
- **THEN** MUST 拒绝该取值（不套用任何等级），MUST 记为 debug，MUST NOT 记为 warn
      或 error
- **AND** MUST NOT 把该取值原样写进日志

#### Scenario: 外部输入的修复留永久回归测试

- **WHEN** 修复一处外部输入的处置缺陷
- **THEN** MUST 留一个不依赖网络与真机的回归用例
