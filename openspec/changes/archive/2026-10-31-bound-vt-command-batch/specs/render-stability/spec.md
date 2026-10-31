## ADDED Requirements

### Requirement: 持续输出时帧不得停发

VT 线程 MUST 在持续输出下持续发布帧：命令批有界（见 `vt-query-latency`），批尾即帧
发布点，批按上界反复切分使帧不被无限期推迟。剩余输出 MUST 可见。

帧发布 MUST NOT 因「上一帧尚未被渲染线程取走」而跳过构建：帧是全量快照，后到的帧覆盖
先到的帧，跳过构建会让最新状态滞留到下一批，消费者先取到的仍是旧帧，与「帧交付必须
保最新帧」相悖。抽取成本 MUST 由「每批至多构建一帧」约束，而不是由丢弃最新帧约束。

实现：`native/src/terminal/ghostty_terminal/internal/run.rs` 的批处理循环受
`MAX_COMMANDS_PER_BATCH` 与 `COMMAND_BATCH_BUDGET` 约束；`publish_cell_data` 不检查
邮箱是否已有未被取走的帧。

#### Scenario: 输出较多时剩余内容可见

- **WHEN** 终端持续输出，输出量超过一帧能显示的容量
- **THEN** 屏幕 MUST 持续推进到最新内容，MUST NOT 停在某一帧不再更新

#### Scenario: 上一帧未被取走时最新帧仍会送达

- **WHEN** 渲染线程迟迟没有取走上一帧，终端又产生了新内容
- **THEN** 渲染线程下一次取到的 MUST 是最新一帧，MUST NOT 是若干帧之前的画面
