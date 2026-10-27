# Tasks

## 1. 限频闸门收敛与迁移

- [x] 1.1 把 `render/slow_log.rs` 迁到 crate 根 `slow_log.rs`，模块声明在 `lib.rs`
      与 `render/mod.rs` 之间对调（渲染侧引用路径统一改为 `crate::slow_log`）。
      验证：`cargo build` 通过，`grep -rn 'render::slow_log' native/src` 无命中。
- [x] 1.2 闸门间隔改为构造时给定：`new()` 取 500ms 默认，`with_interval(ms)` 指定；
      补测试钉住「自定义间隔生效」与「时钟回跳不放行」。
      验证：`cargo test --lib slow_log` 全绿。
- [x] 1.3 闸门补测试用的「清空窗口起点」入口（仅 `cfg(test)`），供静态闸门复位。
      验证：`cargo clippy --all-targets -- --deny warnings` 无告警。

## 2. 单元帧丢弃告警改用共用闸门

- [x] 2.1 终端侧 `warn_drop_throttled` 删掉墙上时钟与私有原子，改用
      `crate::slow_log::SlowLogThrottle`（窗口 5000ms），并回报本次是否真的输出。
      验证：`cargo build` 通过。
- [x] 2.2 限频回归用例改为驱动新闸门：开头复位闸门，连续 50 次调用只放行一次。
      验证：`cargo test --lib cell_frame_drop_warning_is_throttled` 通过。

## 3. 其余墙上时钟闸门并入共用实现

- [x] 3.1 `output_processor.rs` 的超长选择器告警改用 `SlowLogThrottle`
      （窗口 5000ms），删掉私有原子与墙上时钟。
- [x] 3.2 `android/ffi.rs` 的剪贴板读请求洪泛告警改用 `SlowLogThrottle`
      （窗口 5000ms），删掉私有原子与墙上时钟。
- [x] 3.3 `event.rs` 的队列溢出告警改持 `SlowLogThrottle`（窗口 1000ms），
      删掉 `Mutex<Option<Instant>>` 自判逻辑。
      验证：`grep -rn 'SystemTime' native/src` 无命中。

## 4. 全量验证

- [x] 4.1 `cargo fmt --all -- --check`、`cargo clippy --all-targets -- --deny warnings`、
      `cargo test --no-fail-fast` 全绿。
      验证：三条命令退出码均为 0。
- [ ] 4.2 `openspec validate 2026-10-27-unify-log-throttle --strict` 通过。
      验证：命令退出码为 0。

## Workflow follow-up

- 归档本变更（`openspec archive 2026-10-27-unify-log-throttle -y`），把
  `logcat-hygiene` 的新增要求并入 `openspec/specs/logcat-hygiene/spec.md`。
