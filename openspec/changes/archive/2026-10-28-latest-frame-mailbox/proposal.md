# Proposal

## Why

VT 线程交给渲染线程的单元帧走有界队列（容量 4），队列满时丢弃的是**新**帧：
最新的画面反而丢失，终端停在旧视口。突发输出时（每次写入单独 flush 即触发一次
推帧）4 帧之后的所有帧全部被丢，消费者取到的是若干帧之前的内容——单元帧通道
满时丢最新帧，是 `search_select_does_not_scroll_the_viewport` 偶发失败（单跑通过、
并行跑失败）的根因。

帧被丢弃还有第二个后果：渲染暂停期（输入法弹出、进入设置）刻意不消费通道，
帧留在队列里把队列填满，此后每次推送都失败——「IME 弹出时输入不可见」的主因；
且每次丢弃都记一条告警，实测设备上每秒约 40 条，持续到恢复为止。

## What Changes

- 单元帧交付改为单槽「最新帧」邮箱：生产者写入即覆盖旧帧，消费者取到的必是最新画面。
- 移除帧丢弃告警与其限频闸门（不再存在丢弃）。
- 顺带降低内存占用：一份 24×80 网格快照约 75KiB，原先最多滞留 4 份。

## Capabilities

### New Capabilities

- 无

### Modified Capabilities

- `render-stability`：补上「帧交付保最新帧」的要求，并把暂停期的取帧口径写清。

## Impact

- `native/src/terminal/ghostty_terminal/**`：帧交付由有界队列改为最新帧邮箱；
  `receive_cell_data` 的语义改为「取走最新一帧」（对消费者等价：旧实现本就只
  返回最新一帧，其余帧一并丢弃）。
- `native/src/android/ffi/render/frame.rs`：暂停期不取帧的注释口径同步。
- 不改 `docs/specification/`、`docs/STATUS.md` 等保护文件。
- 不引入新依赖（`parking_lot` 已在用）。
