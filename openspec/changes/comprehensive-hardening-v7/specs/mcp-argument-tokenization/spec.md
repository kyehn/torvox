## Purpose

消除 MCP `run_command` 中手写分词的逃逸风险，使用零维护的成熟 crate 实现 POSIX 兼容的 argv 拆分，支撑自动化测试的确定性。

## ADDED Requirements

### Requirement: 安全分词

系统 SHALL 使用 `shell-words` crate（或等价 `shlex`）将 `command` 字符串拆分为 `argv`，禁止直接 `sh -c`，确保 `;`/`|`/`&&`/`>` 等元字符不被解释，且引号与转义按 POSIX 语义处理。

#### Scenario: 带引号命令

- **WHEN** MCP 调用 `run_command("echo \"hello world\" --flag")`
- **THEN** 拆分结果为 `["echo", "hello world", "--flag"]`，而非 `["echo", "\"hello", "world\""]`

#### Scenario: 恶意注入被中和

- **WHEN** 参数含 `"; rm -rf /"`
- **THEN** 该字符串作为单个 argv 元素传递，不触发 shell 解释

#### Scenario: 单引号与转义

- **WHEN** 输入为 `echo 'a b' c\ d`
- **THEN** 结果为 `["echo", "a b", "c d"]`

### Requirement: 错误反馈

系统 SHALL 在分词失败（如未闭合引号）时返回明确错误 `InvalidArgument`，而非静默截断。

#### Scenario: 未闭合引号

- **WHEN** 输入为 `echo "unclosed`
- **THEN** 返回错误，MCP 工具报告 `status: error` 且消息含“unclosed quote”
