# shell-env Specification

## MODIFIED Requirements

### Requirement: ENV 指向 mksh rc

`build_env` MUST 注入 `ENV`，其值为调用方经 `ShellEnv.mkshrc_path` 传入的 mksh rc 绝对路径（`DESIGN.md` Shell 节：`/data/data/com.termux/.mkshrc`，应用私有根目录，在 `files/` 用户数据树之外）；mksh 交互 shell 经由 `ENV` 加载该文件获得短提示符。`mkshrc_path` 为 `None` 时 MUST NOT 注入 `ENV`。

#### Scenario: mksh 加载短提示符

- **WHEN** 子进程以 mksh 交互方式启动且 `mkshrc_path` 指向的应用私有 rc 文件存在
- **THEN** 提示符为 `PS1='$ '` 定义的短提示符，不出现长提示符横滚与左缘裁剪

#### Scenario: 路径不指向 HOME

- **WHEN** 构建子进程环境变量
- **THEN** `ENV` 的值等于传入的 `mkshrc_path`，不为 `{home}/.mkshrc`，不落在 `$HOME` 或用户数据目录内

#### Scenario: 未传入路径时不注入 ENV

- **WHEN** `ShellEnv.mkshrc_path` 为 `None`（如测试或调用方未提供）
- **THEN** 构建结果中不出现 `ENV` 键

#### Scenario: bash 忽略 ENV

- **WHEN** 子进程以 bash 启动
- **THEN** `ENV` 被忽略，bash 启动行为与注入前一致
