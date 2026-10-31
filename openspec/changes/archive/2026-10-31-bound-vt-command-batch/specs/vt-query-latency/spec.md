## ADDED Requirements

### Requirement: 命令批必须有时长与条数上界

VT 线程的命令批 MUST 同时受条数上界与时长上界约束。批尾是**唯一**的查询应答点，
批若无界（只等命令通道排空），持续输出下通道恒非空：批永远不结束，查询无人应答，
逐个等满 `QUERY_TIMEOUT_MS` 后回退到断连值。

断批 MUST NOT 丢命令：判据 MUST 在取下一条命令之前生效，已出队的命令 MUST 留在通道里
等下一批处理。

实现：`native/src/terminal/ghostty_terminal/internal/mod.rs` 声明
`MAX_COMMANDS_PER_BATCH`（条数界）与 `COMMAND_BATCH_BUDGET`（时长界）；批处理循环在
处理完一条命令后先判上界，再取下一条命令。

#### Scenario: 持续输出下积压被切成有界的若干批

- **WHEN** 命令通道里积压了远超一条批容量的命令，且通道持续有数据
- **THEN** 每一批的条数 MUST 不超过条数上界，批 MUST 在时长上界内结束；
    剩余命令 MUST 留在通道里等下一批，MUST NOT 被丢弃

#### Scenario: 持续输出下查询仍被及时应答

- **WHEN** 终端持续输出，同时发起网格查询
- **THEN** 查询 MUST 在若干批之内被应答，MUST NOT 等到通道排空或超时
