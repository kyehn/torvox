# Proposal

## Why

输入路径上的两个状态查询——备用屏激活（`getAltScreenState`）与鼠标上报开关
（`mouseTrackingActive`）——的**文档承诺**与**实现**不一致：

1. `android/ffi/state.rs` 的 `read_terminal_atomic_flag` 文档写「会话未知或锁被占用时
   返回 `false`」，但实现取的是**阻塞**的 `entry.session.lock()`。`parking_lot::Mutex::lock`
   会一直等到锁被释放，从不「因锁被占用而返回」。
2. `android/ffi/input.rs` 的 `mouseTrackingActive` 文档写「无锁读取 VT 线程每帧维护的
   镜像——触摸拖动的每个事件都要判一次，走查询通道会阻塞 UI 线程」，而它经
   `read_terminal_flag_jboolean` → `read_terminal_atomic_flag`，恰恰取了会话锁。
3. `android/ffi/state.rs` 的 `getAltScreenState` 文档写「可在 Android 输入路径的每次
   触摸滚动事件中调用而不阻塞 UI 线程」，实现同样取了会话锁。

会话锁的持有时间**无上界**：PTY 主端是非阻塞的，`write_all` 在 `WouldBlock` 上 poll
到子进程消费，且每有部分写入就重置期限，上限为 `WRITE_DRAIN_TIMEOUT`（5s）。
一次「粘贴一大段文本 + 子进程暂时不读 stdin」就能把会话锁按住数秒。此时：

- 触摸滚动的每个 MOVE 事件都调一次 `isAltScreenActive()`；
- 选区拖动的每个事件都调一次 `isMouseTrackingActive()`。

两者都要等会话锁释放，于是**手势事件在 UI 线程上排队等锁**——表现正是
「滚动卡顿」与「拖手柄无响应」。这不是理论风险：仓库内已有同类问题的修复记录
（`render/diagnostics.rs` 的 `new_output` / `alt_screen_active` 就是为「渲染线程若在此
排队，Kotlin 侧实测每帧 55-60ms」而改成读注册表共享原子量的），但**导出给输入路径用的
那两个读点**没有跟上。

VT 线程每帧刷新这两个镜像（`rendering.rs` 的 `ScreenMirrors`），值本来就放在
`SessionEntry` 持有的共享 `Arc<AtomicBool>` 上——备用屏的镜像**已经存在**且已被
`diagnostics.rs` 使用，只是导出层没走它；鼠标上报的镜像**根本没暴露给注册表条目**，
于是只能绕道会话锁去问 `GhosttyTerminal`。

## What Changes

- `Session::mouse_tracking_active_flag()`：新增与 `alt_screen_active_flag()` 同款的
  共享原子量暴露（克隆 `GhosttyTerminal` 内的镜像 `Arc`，不复制值）。
- `SessionEntry`：新增 `mouse_tracking_active: Arc<AtomicBool>` 字段，与既有的
  `new_output` / `alt_screen_active` 同批持有。
- `read_terminal_atomic_flag` / `read_terminal_flag_jboolean`：读取闭包从
  `&Session` 改为 `&SessionEntry`，实现从「取会话锁再问终端」改为「读注册表条目上的
  共享原子量」，并订正文档中关于「锁被占用时返回 false」的失效表述。
- `getAltScreenState` / `mouseTrackingActive`：改读各自镜像的 `Acquire` 载入。

## Capabilities

### New Capabilities

无。本次不引入任何新的能力面。

### Modified Capabilities

- `vt-query-latency`：新增一条 Requirement，把「状态查询不得取会话锁」这条既有实践
  写成规范——它此前只体现在渲染路径的诊断读点上，输入路径的两个导出未纳入。

## 不做什么

- 不改 VT 线程的镜像刷新时机：`ScreenMirrors` 每帧刷新本就是正确口径。
- 不改 `GhosttyTerminal::alt_screen_active_atomic` / `mouse_tracking_active_atomic`：
  它们仍是终端层的权威读口，只是输入路径不再经由它们取会话锁。
- 不动 `docs/specification/`（禁改区）。
- 不改渲染路径的 `diagnostics.rs`：它已经走注册表共享原子量。

## Impact

- Affected specs: `vt-query-latency`（一条 ADDED Requirement）。
- Affected code:
  - `native/src/terminal/session/output.rs`（新增镜像暴露）
  - `native/src/android/ffi.rs`（`SessionEntry` 新增字段）
  - `native/src/android/ffi/state.rs`（读取路径与文档）
  - `native/src/android/ffi/input.rs`（`mouseTrackingActive` 改读镜像）
  - `native/src/android/ffi/session_entry_flag_tests.rs`（回归钉）
