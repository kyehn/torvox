# Proposal

## Why

2026-10-22 的真机 logcat（3816 条应用日志、13214 行）里，应用自身的 `E/` 只有一条：把 shell 正常执行的
`exit 254` 误判成 `execve errno=154`。同一份日志还暴露了收尾路径的真实缺口：`request_exit`
没有按 `pty-lifecycle` 规范短路，重复拆卸会再发一组信号，且子进程已回收时仍按旧 PID 发信号，
存在 PID 复用误伤风险。其余 W/E 条目分散在平台、GPU 驱动、SELinux 与外部包，需要逐条留档，
避免下次把设备噪声当成应用回归。

## What Changes

- 等待线程只把 `100 + errno`（errno 1..=133）解释为 `execve` 失败；`exit 254`、`exit 255`
  等普通退出码不再产生 `E/native::terminal::session`。
- `request_exit` 以 `exited` 原子交换短路：第二次拆卸不再发信号；子进程已被回收或 `getpgid`
  返回 `ESRCH` 时直接返回，杜绝按可能复用的 PID 误杀。
- 进程组收尾日志按真实原因分级：子进程与本进程同组、子进程已退出记 debug；取不到组号才记 warn。
- 新增 logcat 审计文档，逐条记录本次 215 条 W/E 的分类、证据与处置（应用修复 / 平台 / 设备 / 外部包）。
- 慢帧新增 `SLOW_RENDER_PHASE` 原生分段打点（acquire／encode／submit／present），只在总时长 ≥30ms 时输出；
  在拿到真机复现前不盲改 present mode 或帧延迟参数。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `pty-lifecycle`: 明确 `execve` 退出码解码的 errno 上界，并补全「已回收子进程不再发信号」的拆卸安全要求。

## Impact

- `native/src/terminal/session.rs`：退出码解码、`request_exit` 短路、进程组日志分支。
- `openspec/changes/logcat-warning-audit/`：审计文档与变更记录。
- 不改变 PTY 生成、resize、信号顺序与渲染行为；不改公开 FFI。
