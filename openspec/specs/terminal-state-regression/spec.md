# terminal-state-regression Specification

## Purpose

终端网格整体状态的测试覆盖方式。`docs/specification/TESTING.md` 明确不进行回归测试，故不设
金标语料与快照比对装置；终端状态行为由 `ghostty_terminal::tests` 中针对具体行为的断言覆盖，
其中回滚区旧行顺序、输入回显与光标、显示宽度、字重与颜色等均逐项直接断言网格输出。

实现细节：

- 网格状态经 `dump_grid`（`Query::DumpGrid` → `build_dumped_grid`）与 `take_snapshot`
  （`build_snapshot`）两条路径采集，两者均以 `CellSnapshot` 承载逐格 codepoint 与样式。
- `getTerminalText` 只消费单元文本，不读取样式；`CellSnapshot` 不跨 FFI 传往 Kotlin。
- 回滚区样式保留由 `ghostty_terminal::tests::scrollback_retains_explicit_cell_style` 覆盖；
  选区与搜索只消费文本行。
- `docs/specification/TESTING.md` 的「不进行回归测试」为本能力不引入金标语料与期望文件的依据。

## Requirements

### Requirement: 并发不变量测试与单元测试进程隔离

并发不变量用例（shuttle 穷举线程交错）MUST 运行在独立的测试目标 `native/tests/concurrency.rs`
中，MUST NOT 与 `#[cfg(test)]` 单元测试同进程。

shuttle 在安装全局 panic 钩子时会先访问自身的调度状态，在非 shuttle 线程上触发二次 panic，
从而吞掉同进程内其余测试的真实断言信息，与「不得隐藏错误」冲突。进程隔离 MUST 保证任一
单元测试失败时输出其真实断言内容。

#### Scenario: 单元测试失败显示真实断言

- **WHEN** 任意单元测试断言失败
- **THEN** 失败输出包含该测试的断言内容与实际值，不含第三方库的调度状态错误

#### Scenario: 并发用例保持穷举能力

- **WHEN** 运行并发不变量用例
- **THEN** 两个用例均在独立测试目标中运行并通过，穷举交错的能力保持不变
