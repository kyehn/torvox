# Proposal

## Why

会话正常收尾时，应用仍会打出 E 级日志：

```text
E/native::terminal::session: wait thread: child exited with code 129 (possible execve errno=29; ...)
```

根因是「execve 失败」的判定靠退出码反推：子进程把 execve 的 errno 编码成
`100 + errno`，等待线程见退出码落在 101..=233 便判「execve 失败」。而 shell 被
SIGHUP 杀掉时退出码是 129（shell 的 128+信号惯例），与「errno=29」的编码撞车——
正常收尾被误报成异常。

日志分级是排障时区分「应用缺陷」与「环境噪声」的第一道闸门；正常的会话收尾占据
E 级，等于把闸门自己弄坏：真正的 execve 失败反而淹没在这类噪声里。

## What Changes

- execve 失败改由子进程经管道回报**确切 errno**：fork 前建 CLOEXEC 管道，execve
  成功时写端随 exec 关闭（父进程读到 EOF 即「exec 成功」），失败时子进程写入
  errno 后退出。
- 等待线程删掉退出码反推分支，只在管道给出 errno 时记 error。
- 子进程退出码仍按 `100 + errno` 编码（供 UI 显示），但不再承担判定职责。

## Capabilities

### Modified Capabilities

- `logcat-hygiene`: 补「execve 失败的判定必须依据确切 errno，正常收尾不得被误报」。

## Impact

- `native/src/terminal/pty.rs`：管道创建、子进程写回 errno、读端读取。
- `native/src/terminal/session.rs`：等待线程改为按管道判定，删掉退出码反推分支。
- `openspec/specs/logcat-hygiene/spec.md`。
