## 1. 规范

- [x] `openspec/specs/vt-query-latency/spec.md`：补「命令批必须有时长与条数上界」
- [x] `openspec/specs/render-stability/spec.md`：补「持续输出时帧不得停发」

## 2. 实现

- [x] `native/src/terminal/ghostty_terminal/internal/mod.rs`：`COMMAND_BATCH_BUDGET`、
      `MAX_COMMANDS_PER_BATCH` 两道上界
- [x] `native/src/terminal/ghostty_terminal/internal/run.rs`：批尾判据放在取下一条命令
      之前（断批不丢命令）
- [x] `native/src/terminal/ghostty_terminal/internal/run.rs`：撤掉「上一帧未被取走就跳过
      构建」的优化——它让最新状态滞留，消费者先取到旧帧
- [x] 批大小计数（仅测试使用）：`BatchStats` + `take_batch_stats`

## 3. 验证

- [x] `native/src/terminal/ghostty_terminal/tests/batch_latency.rs`：积压被切成有界的若干批
- [x] `native/src/terminal/ghostty_terminal/tests/batch_latency.rs`：持续输出下查询仍被
      及时应答（拿到真实网格行数而非超时回退值）
- [x] `native/src/terminal/ghostty_terminal/tests/batch_latency.rs`：断批不丢命令（突发写入
      的编号序列连续，`flush` 应答不丢）
- [x] `cargo test --lib`：769 项全过
