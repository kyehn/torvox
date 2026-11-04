## MODIFIED Requirements

### Requirement: 不解析禁止的配置文件

应用 MUST NOT 读取或解析 `termux.float.properties`。文件存在 MUST NOT 改变窗口行为或
任何其他运行结果。`colors.properties` 不在此列：它按 `colors-configuration` 能力作为
终端配色的部分覆盖读取。

#### Scenario: 存在 colors.properties

- **WHEN** 用户文件目录中存在 `colors.properties`
- **THEN** 文件内容按 `colors-configuration` 读取为终端配色的部分覆盖，不改变其它行为

#### Scenario: 存在 termux.float.properties

- **WHEN** 用户文件目录中存在 `termux.float.properties`
- **THEN** 文件内容不被解析，应用不出现浮动窗口行为
