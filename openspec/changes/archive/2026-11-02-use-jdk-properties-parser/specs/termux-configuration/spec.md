## MODIFIED Requirements

### Requirement: properties 语法

解析器 MUST 按 Java Properties 语法解析：键与值以 `=`、`:` 或空白分隔；`#` 与 `!`
起行注释；行尾续行符 `\` 续行；值内支持 `\n` `\t` `\r` `\f` `\uXXXX` 转义。未知键
MUST 被忽略，MUST NOT 使整份文件解析失败。

单个键写坏 MUST NOT 作废整份文件：非法转义（如 `\u12`）MUST 只降级该值，其余键
MUST 照常生效。

数值键越界 MUST 回退各自的默认值并记录错误，MUST NOT 钳到区间边界——
上游 `getDefaultIfNotInRange` 的口径是「没配好就用默认」。

#### Scenario: 未知键

- **WHEN** 文件里出现本应用不认识的键
- **THEN** 该行被忽略，其余键照常生效

#### Scenario: 上游有但本应用不支持的键

- **WHEN** 文件里出现 `shortcut.rename-session`（本应用没有重命名 UI）
- **THEN** 该键不生效，且日志里明确报告不支持；其余键照常生效

#### Scenario: 数值越界

- **WHEN** `terminal-transcript-rows` 写成 99999（有效区间 100–50000）
- **THEN** 回滚行数取默认 2000，日志里有一条越界说明

#### Scenario: 空白分隔的键值

- **WHEN** `termux.properties` 写 `terminal-transcript-rows 100`
- **THEN** 该键取 100

#### Scenario: 注释行不吞掉下一行

- **WHEN** `termux.properties` 里有一行以 `#` 开头且以 `\` 结尾，下一行是有效配置
- **THEN** 注释行整行忽略，下一行照常生效

#### Scenario: 非法转义只降级该值

- **WHEN** `termux.properties` 里 `default-working-directory` 的值含非法转义 `\u12`，
      其余键合法
- **THEN** 该值按字面量处理，其余键照常生效，整份文件不被作废

#### Scenario: 非 ASCII 值不被破坏

- **WHEN** `termux.properties` 里 `default-working-directory = /sdcard/我的目录`
- **THEN** 该值按原文生效，不出现乱码
