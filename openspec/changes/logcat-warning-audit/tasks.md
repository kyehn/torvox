# Tasks

## 1. 日志审计留档

- [x] 1.1 逐条解析 logcat 的 W/E，确认总数为 215（W 184、E 31），生成 `logcat-audit-appendix.md`；用 `grep -c` 复核附录表格行数为 215
- [x] 1.2 在 `logcat-audit.md` 记录每类日志的证据、归因与处置，并核对分类条数之和为 215

## 2. execve 退出码解码

- [x] 2.1 新增 `execve_errno_from_exit_code`，只接受 1..=133 的 errno；等待线程改用该函数；子进程侧 errno 封顶由 155 改为 133
- [x] 2.2 新增 `execve_errno_only_decodes_real_errno_range` 单测，覆盖 102/113/233/254/255/100；`cargo test terminal::session::tests::execve_errno_only_decodes_real_errno_range` 通过

## 3. 收尾安全与日志分级

- [x] 3.1 `request_exit` 以 `exited` 原子交换短路，第二次调用不再发信号；`getpgid` 为 `ESRCH` 时直接返回
- [x] 3.2 进程组日志按同组/已退出/取不到组号分级，正常关闭不再出现「进程组不可用」warn
- [x] 3.3 复核 `exit_signal_target_declines_group_kill_when_unsafe` 与 `request_exit_kills_child_while_session_still_alive` 仍通过

## 4. 集成验证

- [x] 4.1 在 `native/` 执行 `cargo fmt --check` 通过
- [x] 4.2 在 `native/` 执行 `nix develop --command cargo test` 通过（653 项单元测试 + 2 项并发集成测试）
- [x] 4.3 在 `native/` 执行 `nix develop --command cargo clippy --all-targets -- -D warnings` 通过；仅保留构建脚本既有 gold linker 告警
- [x] 4.4 运行 `openspec validate logcat-warning-audit --strict`，确认 proposal/specs/design/tasks 一致

## 5. 慢帧后续取证

- [x] 5.1 在 `render_frame_with_plan` 新增 `SLOW_RENDER_PHASE`，只在总耗时 ≥30ms 时输出 acquire/encode/submit/present 四段；`attach_surface` 日志补记实际 format/present/latency，`SLOW_FRAME` 补记 `scrollRemainderPx`；`cargo test render::tests` 87 项通过，debug APK 构建通过
- [ ] 5.2 按 `logcat-audit.md` 的「复现与取证」在 Mali 真机抓取含 `SLOW_RENDER_PHASE` 的新 logcat；在拿到分项前不修改 present mode 或帧延迟参数

## Workflow follow-up

- 评审通过后归档变更，并确认 `openspec/specs/pty-lifecycle/spec.md` 已吸收本次 delta。
- 慢帧取证需要用户提供新日志或允许添加临时打点；该条不阻塞本次日志误报修复的归档。
