# Proposal

## Why

待回写的 VT 应答（DECRPM、DSR、DA）队列有显式上限 256 条。子进程不读 stdin
时队列净增长，超限即丢最旧——这是有意的背压，不会丢字节（应答本就是请求-响应
语义）。

但丢弃时的告警是逐帧记的 `error`：子进程不读 stdin 且持续查询设备状态时，
溢出逐帧命中，logcat 几秒内就被 error 级洪水刷满。日志分级是排障时区分
「应用缺陷」与「环境噪声」的第一道闸门，被自己的告警淹没即等于取消这道闸门。

同一类的剪贴板队列溢出告警早已按窗口汇总（一个窗口一行、行内带累计条数），
VT 应答队列这条是漏网的一条：两者都是「对端不消费导致的有界队列丢最旧」，
判据相同却分成两种口径。

## What Changes

- VT 应答队列溢出的告警改走带计数的限频闸门：一个窗口一行，行内带窗口内
  累计丢弃的条数，窗口内的计数延后交出而不丢弃。
- 队列裁剪与计数记入收敛到一处，使该口径可被用例直接断言。
- 补单测：首条必可见且带条数；窗口内再次溢出不再输出；窗口内条数攒着下次
  交出。
- `logcat-hygiene` 增一条场景：VT 应答队列溢出的告警按窗口输出。

## Capabilities

### New Capabilities

无（不新增能力）。

### Modified Capabilities

- `logcat-hygiene`：给「高频诊断日志必须限频」增一条 VT 应答队列溢出的场景。

## Impact

- `native/src/terminal/session/output.rs`：溢出告警改走限频闸门。
- `native/src/terminal/session/mod.rs`：会话新增溢出告警的限频闸门字段。
- `native/src/terminal/session/spawn.rs`：初始化该字段。
- `native/src/terminal/session/tests/write_backpressure.rs`：新增回归用例。
- `openspec/specs/logcat-hygiene/spec.md`：归档时同步该条场景。
