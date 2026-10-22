# Tasks

## 1. 日志审计留档

- [x] 1.1 解析两份日志的 W/E，确认总数为 753（第一份 215、第二份 538），按标签聚合计数
- [x] 1.2 在 `logcat-audit.md` 记录每类条目的归因与处置；原始行、时间戳、PID、设备标识与
      下载链接不入库
- [x] 1.3 删除逐条原始日志附录；仓库历史中也不保留原始日志文件

## 2. execve 退出码解码

- [x] 2.1 新增 `execve_errno_from_exit_code`，只接受 1..=133 的 errno；等待线程改用该函数；
      子进程侧 errno 封顶由 155 改为 133
- [x] 2.2 新增 `execve_errno_only_decodes_real_errno_range` 单测，覆盖 102/113/233/254/255/100；
      `cargo test terminal::session::tests::execve_errno_only_decodes_real_errno_range` 通过

## 3. 收尾安全与日志分级

- [x] 3.1 `request_exit` 以 `exited` 原子交换短路，第二次调用不再发信号；`getpgid` 为
      `ESRCH` 时直接返回
- [x] 3.2 进程组日志按同组/已退出/取不到组号分级，正常关闭不再出现「进程组不可用」warn
- [x] 3.3 复核 `exit_signal_target_declines_group_kill_when_unsafe` 与
      `request_exit_kills_child_while_session_still_alive` 仍通过

## 4. 集成验证

- [x] 4.1 在 `native/` 执行 `cargo fmt --check` 通过
- [x] 4.2 在 `native/` 执行 `nix develop --command cargo test` 通过
- [x] 4.3 在 `native/` 执行 `nix develop --command cargo clippy --all-targets -- -D warnings`
      通过；仅保留构建脚本既有 linker 告警
- [x] 4.4 运行 `openspec validate logcat-warning-audit --strict`，确认
      proposal/specs/design/tasks 一致

## 5. 慢帧取证

- [x] 5.1 在 `render_frame_with_plan` 新增 `SLOW_RENDER_PHASE`；`attach_surface` 补记实际
      format/present/latency；`SLOW_FRAME` 补记 `scrollRemainderPx`；单元测试与 debug APK
      构建通过
- [x] 5.2 新增 `SLOW_RENDER_INNER` 与 `SLOW_CELL_BUILD`，拆分前置准备、会话接收、渲染锁、
      实例构建与 GPU 调用
- [ ] 5.3 用新日志确认慢帧主因并修复；在拿到分项前不修改 present mode 或帧延迟参数

## 6. 隐私与历史清理

- [x] 6.1 跟踪文件中不含账号名、代码托管链接、设备型号/名称或原始日志
- [x] 6.2 用 git-filter-repo 重写全部引用并强制推送 `main`，清除历史中的原始日志、
      账号名与设备标识；作者统一为项目维护者
- [x] 6.3 删除远端旧功能分支与旧版本标签，`git ls-remote` 只剩 `refs/heads/main`
- [x] 6.4 复核：`git log --all -S` 对账号名、设备标识、原始日志文件名与原始日志行均无命中；
      `openspec validate --all --strict` 与 markdownlint 通过
- [ ] 6.5 若需清除 GitHub 侧缓存/PR 引用中的旧对象，联系平台支持触发 GC（Git 层已无引用）

## Workflow follow-up

- 评审通过后归档变更，并确认 `openspec/specs/pty-lifecycle/spec.md` 已吸收本次 delta。
- 慢帧修复需要一次滚动复现日志；只保留聚合结果。
