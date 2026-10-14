# terminal-state-regression Specification

## Purpose

终端网格整体状态的测试覆盖方式。本能力不设金标语料与快照比对装置——判据是
`docs/specification/TESTING.md` 的「只测试本项目功能」「被测试部分应当曾经发生过错误
或该测试可以证明最终功能正确」与「不保留无意义或低价值测试」：语料回放考的是上游
`libghostty-vt` 的 VT 语义，属 `TESTING.md` 明禁的「不测试依赖行为」，且
`upstream-alignment`「上游 VT 语义不由本仓测试覆盖」已登记该门槛。终端状态行为改由
`ghostty_terminal::tests` 中针对具体行为的断言覆盖，其中回滚区旧行顺序、输入回显与
光标、显示宽度、字重与颜色四类均逐项直接断言网格输出（逐格读 `CellSnapshot` 或
`CellData`，不经中间文本层）。

实现细节：

- 网格状态经 `dump_grid`（`Query::DumpGrid` → `build_dumped_grid`）与 `take_snapshot`
  （`build_snapshot`）两条路径采集，两者均以 `CellSnapshot` 承载逐格 codepoint 与样式。
  渲染热路径另有一条 `build_cell_data`（`Vec<CellData>`），行级增量下未变行整段复制
  上一帧缓存——增量与全量重建的输出内容本应相同，故其正确性不能由内容断言判别，
  须比对两者输出并另设可观测计数，口径见 `grid-extraction-incremental` 的行缓存失效与
  脏行来源条。
- `getTerminalText` 只消费单元文本，不读取样式；`CellSnapshot` 不跨 FFI 传往 Kotlin。
- 回滚区样式保留由 `ghostty_terminal::tests::scrollback_retains_explicit_cell_style` 覆盖；
  选区与搜索只消费文本行。
- 四类断言的落点：回滚顺序 `scrollback_keeps_order_and_newest_line_stays_visible`（经
  `read_line_text` 逐格读网格）、回显与光标 `cursor_matches_last_printed_row_after_echo_print`
  （直接读 CellData 行列）、显示宽度 `cell_iterator_cjk_double_width`（断 `width` 字段）、
  字重与颜色 `sgr_style_attributes_reach_snapshot_and_cell_data`（断 `flags` 与三色通道）。

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

### Requirement: 网格状态断言直接针对网格输出

终端网格状态 MUST 由直接读取网格的断言覆盖：回滚区旧行顺序、输入回显与光标跟随、
简体中文显示宽度、字重与颜色文本，各 MUST 有断言其网格输出的用例，MUST NOT 仅断言
某一层的中间量或派生值。

期望值来源为本仓实现自身输出的网格快照装置 MUST NOT 引入：它把本仓的输出当成
本仓的正确答案，无法在实现出错时失败。期望值来自标准文本、上游协议文档或真机实测的
语料不在此限，按 `upstream-alignment` 的采纳门槛单独判定。

本能力的规范依据 MUST 以 `docs/specification/TESTING.md` 与 `upstream-alignment` 的实际
条款为准，MUST NOT 引用该文件中不存在的表述。

#### Scenario: 外部基准经运行期取数

- **WHEN** 网格断言的期望值取自外部真相来源（Unicode UCD 宽度表、上游模糊语料、kitty 键盘协议规范）
- **THEN** 该数据 MUST 于测试运行期现取、仓内不留副本，取数失败即判红，MUST NOT 跳过

#### Scenario: 回滚区旧行按顺序断言

- **WHEN** 输出超出网格高度后回滚区被逐行读取
- **THEN** 断言逐行比对回滚区文本与顺序，新行出现在视口底部

#### Scenario: 字重与颜色断言网格呈现

- **WHEN** 终端以不同字重与前景色显示文本
- **THEN** 断言检查对应网格单元的样式与颜色，不止检查单元文本非空

#### Scenario: 不引入自验证快照装置

- **WHEN** 评估是否引入网格快照语料
- **THEN** 期望值由本仓实现自身产出的不引入，理由记为自验证；外部真相来源的语料
      单独按 `upstream-alignment` 的门槛评估

#### Scenario: 规范引用不得超出被引用文件

- **WHEN** 本能力引用 `docs/specification/TESTING.md` 作为某项取舍的依据
- **THEN** 被引用的表述在该文件中确实存在，否则改引 `upstream-alignment` 的对应条款

### Requirement: 重置终端同步归位 OSC 52 扫描器

RIS 全重置 MUST 同时丢弃 OSC 52 读取扫描器的暂存状态。扫描器是 VT 之外的第二份解析
状态，不随 RIS 重置就会停在未闭合的读取请求上：重置后的前若干字节被当成仍在读取的
选择器名吞掉，用户看到「重置终端」后头一段输出凭空消失，并伴随一条与本次重置毫无
关系的「选择器名过长」告警。

#### Scenario: 重置后首个字节即按地面态透传

- **WHEN** 扫描器停在未闭合的 OSC 52 读取请求上，此时执行 RIS 重置
- **THEN** 重置后的每个字节都原样透传给 VT 解析器，不被当作选择器名暂存
