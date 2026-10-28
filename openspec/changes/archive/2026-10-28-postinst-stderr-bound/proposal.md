# Proposal

## Why

postinst 的 stderr 由守护线程读到 EOF 后整体追加进 `StringBuilder`，只在最终上报时取
前 400 字符。也就是说：**诊断只用 400 字符，内存却按子进程的全部输出增长**。一个崩溃
后循环刷错误的脚本（或把自身重定向成死循环的维护脚本）就能把整个应用进程读成 OOM，
而这正是「引导失败」最需要看到日志的时刻。

同一处还有并发缺陷：`StringBuilder` 不是线程安全的，而主线程在 `join` 超时（孙进程
仍持有管道，这是本文件其他注释里记录过的真实形态）之后照样读它，等于无同步的跨线程
访问。

## What Changes

- `drainQuietly` 改为**有界读取**：仍读到 EOF（提前停会让子进程卡在写满的管道上），
  但只保留前 64 KiB。
- stderr / stdout 的交接改用 `AtomicReference`，替掉 `StringBuilder` 与
  `synchronized` 组合：`join` 超时后读取不再有数据竞争。
- 新增单测：4 MiB 输入必须被读完（EOF），返回值不得超过上限。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `bootstrap-install`：postinst 的 stderr 排空补上「读到 EOF、保留量有界」的要求。

## Impact

- `android/app/src/main/java/terminal/emulator/installer/SecondStageRunner.kt`
- `android/app/src/test/java/terminal/emulator/installer/SecondStageRunnerTest.kt`
- 行为：诊断摘录仍取前 400 字符（远小于上限），上报内容不变
