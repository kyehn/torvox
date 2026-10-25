# Proposal

## Why

真机 logcat（2026-10-25 采集）里渲染帧出现周期性的 ~53–56ms 长帧：
`SLOW_RENDER_INNER phase1=0.001ms receive=0.003ms lock=0.000ms render=54.210ms`，
而同一批日志里 `SLOW_RENDER_PHASE`（GPU 段）与 `SLOW_CELL_BUILD`（单元构建段）
**一次都没有出现**——说明耗时既不在 GPU 提交/呈现，也不在单元构建，而在两者之间。

那段区间里唯一会阻塞的事情是渲染线程向 VT 线程发起的 **kitty 放置查询**
（`take_kitty_placements_on`）。根因在 VT 线程的命令循环：查询走独立通道，
只在**命令批尾**与**空闲节拍到点**（`VT_IDLE_TICK_MS` = 50ms）时排空。
无命令在途时 VT 线程阻塞在 `recv_timeout(50ms)` 里，查询必须等满一个节拍才被应答。
渲染线程刚收到单元数据帧（VT 线程刚推完帧、正要进入下一次等待）就发出查询，
于是这一帧必然多等 ~50ms。

代码里已有同一现象的记录：「滚动每一步都去 VT 线程同步查一次放置，会让渲染线程每行等一次
完整查询（真机 yazi 预览下实测每行约 60ms，滚动掉到 ~16fps）」——但只做了「滚动时不查」
的规避，没有消除应答延迟本身。

现有自动化判据也看不见这个问题：查询用例只断言「在 `QUERY_TIMEOUT_MS`（500ms）内返回」，
而节拍（50ms）比它小一个数量级——用例全绿也掩盖了「每次查询被稳定串行化到 50ms」的事实。

## What Changes

- VT 线程**同时**等待命令通道与查询通道：任一就绪即醒，查询到达即应答，
  不再等空闲节拍。
- 空闲节拍只保留它真正的职责：排空通道里残留的查询 + 在状态变化后推送单元数据。
- `VT_IDLE_TICK_MS` 的语义从「查询排空的最坏延迟」改回「空闲等待下一条命令的时长」。
- 新增回归用例钉住「空闲 VT 线程的连续查询不得被节拍串行化」。

## Capabilities

### New Capabilities

- `vt-query-latency`：VT 查询的应答延迟上界（不得被空闲节拍串行化）。

### Modified Capabilities

无。

## Impact

- `native/src/terminal/ghostty_terminal/internal.rs`：VT 线程等待逻辑（命令 + 查询双通道
  `flume::Selector`）、`drain_queries` 接受本轮已取出的查询。
- `native/src/terminal/ghostty_terminal/types.rs`：`VT_IDLE_TICK_MS` 的语义注释。
- `native/src/terminal/ghostty_terminal/tests.rs`：新增查询延迟回归用例；
  订正「查询延迟上界即节拍」的旧注释。
