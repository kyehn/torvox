# 收尾措辞与实现对齐

## Why

审阅 `pty-lifecycle` 时发现两处规范文字与实现不符，实现是对的，错的是文字：

1. **收尾对象**：规范写「先向 subtree_leader 发送挂断信号」，但实现是
   `nix::unistd::getpgid(Some(pid))` 取 shell 所在进程组再 `kill(-pgid)`
   （`native/src/terminal/session.rs` 的 `request_exit`）。子进程经 `setsid`
   自成会话首进程，仓库内根本没有 `subtree_leader` 概念
   （`grep -rn subtree` 在 `native/src` 与 Kotlin 源码零命中）。
   以不存在的概念写规范，会让后来者去找一个没有的东西。
2. **宽限数字**：规范 Scenario 写「未在 200ms 内退出」，但实现
   `TRAILING_EXIT_GRACE = 50ms`（`session.rs:23`），SIGHUP→SIGKILL 间隔为
   一个宽限期即 50ms。数字错了，实现是对的（台账早已按 50ms 上界否证过
   1.1s 阻塞嫌疑）。

## What Changes

- `pty-lifecycle` 的收尾 Requirement：收尾对象改为「shell 所在进程组
  （`getpgid(child)`）」，场景文字去掉 `subtree_leader`。
- 同一条的宽限 Scenario：200ms 改为 50ms，与 `TRAILING_EXIT_GRACE` 一致。

## Capabilities

### Modified Capabilities

- `pty-lifecycle`：收尾对象措辞与宽限数字与实现对齐。

## Non-goals

- 不改收尾实现（`request_exit` 的组杀优先、自杀 guard、有界升级均不动）。
- 不改宽限期数值本身（50ms 不动，只改规范文字）。
- 不碰 `docs/specification/`。
