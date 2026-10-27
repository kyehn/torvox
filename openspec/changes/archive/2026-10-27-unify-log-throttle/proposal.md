# Proposal

## Why

原生侧此前有**五套**限频闸门：渲染侧的 `SlowLogThrottle`（进程内单调时钟），终端侧
单元帧丢弃告警、超长 OSC 52 选择器告警、剪贴板读请求洪泛告警三处私有实现（均为
墙上时钟），以及事件队列溢出告警的 `Mutex<Option<Instant>>` 自判逻辑。多套口径带来
两个问题：同一份限频逻辑写了好几遍，改一处漏一处；更要紧的是三处墙上时钟判定
间隔——时钟被校准时向前跳会让两次告警的真实间隔远小于 500ms，直接违反
`logcat-hygiene` 对「两次输出之间的间隔 MUST 不小于 500ms」的要求；向后跳则会让
告警在时钟追上之前长期沉默。

## What Changes

- 限频闸门收敛为**一处**实现：移到 crate 根（`crate::slow_log`），渲染侧、终端侧与
  事件队列共用；渲染侧原有的四类慢帧诊断不受影响。
- 限频闸门的间隔可按调用点配置：默认 500ms（渲染侧逐帧诊断），单元帧丢弃/洪泛/
  超长选择器告警用 5000ms，事件队列溢出用 1000ms。
- 三处墙上时钟闸门改用该闸门：判定基准由墙上时钟换成进程内单调时钟，间隔语义与
  渲染侧一致。
- 事件队列溢出告警并入同一闸门，热路径上少一次加锁。
- 限频闸门新增「清空窗口起点」入口（仅测试构建）：静态闸门被进程内其它用例推进
  过时，用例需要回到「首条必定可见」的初始态。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `logcat-hygiene`：新增「限频闸门必须使用单调时钟」要求（间隔判定不得受墙上时钟
  校准影响），并明确同一类诊断只能有一处限频判定实现。

## Impact

- `native/src/slow_log.rs`（由 `native/src/render/slow_log.rs` 迁出）：限频闸门改为
  间隔可配置，新增测试用清空入口。
- `native/src/lib.rs`、`native/src/render/mod.rs`：模块声明调整。
- `native/src/render/pass/{frame,acquire}.rs`、`native/src/render/context/{gpu,mod,renderer}.rs`、
  `native/src/android/ffi.rs`：引用路径改为 `crate::slow_log`。
- `native/src/terminal/ghostty_terminal/internal/mod.rs`：单元帧丢弃告警改用共用闸门。
- `native/src/terminal/ghostty_terminal/internal/tests.rs`：限频回归用例改为驱动新闸门。
- `native/src/terminal/output_processor.rs`：超长选择器告警改用共用闸门。
- `native/src/android/ffi.rs`：剪贴板读请求洪泛告警改用共用闸门。
- `native/src/event.rs`：队列溢出告警改持共用闸门。
