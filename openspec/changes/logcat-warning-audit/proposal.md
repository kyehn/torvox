# Proposal

## Why

用户提供的两份运行日志共 753 条 `W/`/`E/`。应用自身的错误只有一类：等待线程把 shell 的
正常退出码 254 误判成 `execve` 失败。收尾路径还暴露了真实缺口：`request_exit` 没有按
`pty-lifecycle` 规范短路，重复拆卸会再发一组信号，且子进程已回收时仍按旧 PID 发信号，
存在 PID 复用误伤风险。其余条目分散在平台、GPU 驱动、SELinux 与外部包，需要分类留档，
避免下次把外部噪声当成应用回归。

## What Changes

- 等待线程只把 `100 + errno`（errno 1..=133）解释为 `execve` 失败；普通非零退出码不再产生
  错误日志。
- `request_exit` 以 `exited` 原子交换短路：第二次拆卸不再发信号；子进程已被回收或 `getpgid`
  返回 `ESRCH` 时直接返回。
- 进程组收尾日志按真实原因分级：子进程与宿主同组、子进程已退出记 debug；取不到组号才记 warn。
- 新增聚合审计文档，记录两份日志的分类、计数、归因与处置；**不保留任何原始日志行、
  时间戳、PID、设备标识、下载链接或可还原设备的细节**。
- 慢帧新增 `SLOW_RENDER_INNER` 与 `SLOW_CELL_BUILD` 两级诊断，只在慢帧时输出；
  在拿到复现证据前不盲改 present mode 或帧延迟参数。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `pty-lifecycle`: 明确 `execve` 退出码解码的 errno 上界，并补全「已回收子进程不再发信号」
  的拆卸安全要求。

## Impact

- `native/src/terminal/session.rs`：退出码解码、`request_exit` 短路、进程组日志分支。
- `native/src/android/ffi.rs`、`native/src/render/pass.rs`：慢帧阶段计时。
- `openspec/changes/logcat-warning-audit/`：聚合审计与变更记录。
- 不改变 PTY 生成、resize、信号顺序与渲染行为；不改公开 FFI。
