# 任务

## 一、缺陷核定

- [x] 确认 `read_terminal_atomic_flag` 的实现取的是**阻塞**锁：
      `native/src/android/ffi/state.rs` 的实现为 `entry.session.lock()`，
      而其文档写「会话未知或锁被占用时返回 `false`」——`parking_lot::Mutex::lock`
      只会等到锁释放，从不「因锁被占用而返回」。
- [x] 确认 `mouseTrackingActive`（`android/ffi/input.rs`）经
      `read_terminal_flag_jboolean` → `read_terminal_atomic_flag` 取会话锁，
      与其文档「无锁读取 VT 线程每帧维护的镜像……走查询通道会阻塞 UI 线程」相悖。
- [x] 确认 `getAltScreenState`（`android/ffi/state.rs`）同样取会话锁，
      与其文档「可在 Android 输入路径的每次触摸滚动事件中调用而不阻塞 UI 线程」相悖。
- [x] 确认会话锁的持有时间**无上界**：PTY 主端为 O_NONBLOCK，
      `Pty::write_all` 在 `WouldBlock` 上 poll 到子进程消费，每有部分写入即重置期限，
      上限 `pty::WRITE_DRAIN_TIMEOUT`（5s）。
- [x] 确认镜像的刷新点：`terminal/ghostty_terminal/internal/rendering.rs` 的
      `build_cell_data` 每帧经 `ScreenMirrors` 写入 `alt_screen` 与 `mouse_tracking`
      两个 `Arc<AtomicBool>`；备用屏镜像**已**由 `SessionEntry` 持有并被
      `android/ffi/render/diagnostics.rs` 使用，鼠标上报镜像**未**暴露给注册表条目。
- [x] 确认既有正确范式：`diagnostics.rs` 明确记录「`new_output` 与备用屏标志都从
      注册表的共享原子量读取，**不取会话锁**：滚动期间 PTY 洪流会把会话锁按住数十
      毫秒，渲染线程若在此排队，Kotlin 侧实测每帧 55-60ms」——输入路径应同此口径。

## 二、实现

- [x] `terminal/session/output.rs`：新增 `Session::mouse_tracking_active_flag()`，
      克隆 `GhosttyTerminal` 内镜像的 `Arc`（不复制值，与 `alt_screen_active_flag` 同款）。
- [x] `android/ffi.rs`：`SessionEntry` 新增 `mouse_tracking_active: Arc<AtomicBool>`
      字段，构造时与 `new_output` / `alt_screen_active` 同批取自会话。
- [x] `android/ffi/state.rs`：`read_terminal_atomic_flag` 与
      `read_terminal_flag_jboolean` 的读取闭包由 `&Session` 改为 `&SessionEntry`，
      实现改为「注册表查找 → 读共享原子量」，并订正「锁被占用时返回 false」的失效表述。
- [x] `android/ffi/state.rs`：`getAltScreenState` 改读 `entry.alt_screen_active`。
- [x] `android/ffi/input.rs`：`mouseTrackingActive` 改读 `entry.mouse_tracking_active`。

## 三、回归钉

- [x] `android/ffi/session_entry_flag_tests.rs`：
      - `entry_shares_session_flags_for_lock_free_sampling` 扩展为三标志指针相等断言
        （备用屏、鼠标上报、PTY 摄入），确保注册表条目持有的是**同一份** `Arc`，
        而非另建一个副本——副本会与 VT 线程的写入脱钩。
      - 新增 `input_path_state_queries_do_not_take_the_session_lock`：
        登记会话进注册表 → **占住会话锁** → 走导出所用的读取路径
        （`read_terminal_flag_jboolean`）读两个标志。
        取锁的实现会在此阻塞到锁被释放（测试挂起），无锁实现立即返回。
        `try_lock` 先断言锁确已被占住，保证「前置条件」成立。
- [x] 反向验证：把读取路径临时改回「取会话锁」，确认新用例**挂起**（非静默通过），
      随后恢复实现并复跑，用例转绿。

## 四、验证

- [x] `cargo test --lib android::ffi`：53 项全通过。
- [x] `cargo test --lib session_entry_flag`：2 项全通过。
- [x] `cargo clippy --all-targets -- -D warnings`：0 告警。
- [x] `cargo test --lib`：785 项中 781 通过；余 4 项为宿主字体缺失（Maple Mono 已装、
      Noto Sans CJK VF 待装），与本次改动无关。
