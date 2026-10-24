# Tasks

## 1. 原生侧：可恢复渲染失败的分级

- [x] 1.1 新增 `GpuError::SurfaceUnavailable` 变体与 `GpuError::is_recoverable()`
      （判据为错误类型），验证：`gpu_error_recoverability` 两个用例通过
- [x] 1.2 渲染暂停、surface 尚未挂载、取帧失败三条无纹理路径改用该变体，
      验证：生产路径无按 `GpuError::Surface` 分支的控制流，改动只影响日志分级
- [x] 1.3 JNI 层 `log_frame_failure()` 按可恢复性分级：可恢复记 debug，真异常记 error，
      验证：`cargo clippy --all-targets --all-features -- -D warnings` 通过

## 2. 原生侧：未捕获 GPU 错误按来源分级

- [x] 2.1 `gpu_error_is_surface_lost()`：仅当校验错误同时含上游固定文案
      `Surface::configure` 与「不支持该适配器的队列族」时判为窗口被收回，
      验证：`gpu_error_classification` 三个用例通过（命中、不命中、非校验错误不命中）
- [x] 2.2 `log_gpu_error()` 按判据分级：窗口被收回记 debug，其余记 error，
      验证：同上用例 + 全量 `cargo test --lib` 通过

## 3. 宿主侧：瞬时渲染错误按连续次数分级

- [x] 3.1 首次瞬时渲染错误记 info，连续出现才记 warn，原有限频节奏不变，
      验证：`TransientRenderErrorSeverityTest` 两个用例通过（首次非 warn、连续仍 warn）
- [x] 3.2 `:app:testDebugUnitTest` 全绿，`:app:detekt`、`:app:spotlessCheck` 通过

## 4. 退出码上报用例的稳定性

- [x] 4.1 定位偶发失败根因：退出标志由读线程（PTY EOF）与等待线程（拿到退出码）
      两条路径置位且无序，用例只读一次会读到中间的「码未到手」，
      验证：全量跑实测命中一次（`exec_failure_session_exits_with_errno_encoding`）
- [x] 4.2 抽出 `wait_for_reported_exit()` 助手并让两个用例统一轮询到「码可上报」，
      与生产路径（逐帧轮询）同口径，验证：该用例连跑 30 次全绿

## 5. 规范与留档

- [x] 5.1 规范增量落 `specs/logcat-hygiene/spec.md`：正常退化路径补上「平台收走原生窗口」，
      新增「未捕获 GPU 错误必须按来源分级」，
      验证：`openspec validate --strict` 通过
- [x] 5.2 留档只写结论与判据，MUST NOT 写原始日志行、时间戳、PID、设备标识，
      验证：逐条核对本 change 的全部文档

## 6. 集成验证

- [x] 6.1 Rust 全量，验证：`cargo fmt --all`、`cargo clippy --all-targets --all-features -- -D warnings`、
      `cargo test --lib` 全绿
- [x] 6.2 Kotlin JVM 单测，验证：`:app:testDebugUnitTest` 全绿

## Workflow follow-up

- `openspec validate 2026-10-24-logcat-hygiene-surface-reclaim --strict` 通过后，
  执行 `openspec archive 2026-10-24-logcat-hygiene-surface-reclaim`，
  把 `logcat-hygiene` 的增量同步进主 spec。
- 归档后执行 `openspec validate --all --strict`，确认全量规范一致。
- 归档结果连同本 change 的全部文档作为一个小步提交并推送到 `origin/main`。
